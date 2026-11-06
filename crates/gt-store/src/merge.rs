//! Storage for merging a relative's copy of the tree: reading a change log as raw rows, the
//! ledger of groups already taken from a copy, and the one transaction that applies a merge.

use crate::StoreError;
use crate::authors::{Batch, LogMeta};
use crate::codec::{corrupt, parse_uuid};
use crate::edit::{Affected, ChangeOp, ChangeSet, RecordKind, apply_set, finish_apply};
use crate::project::Project;
use gt_core::Uuid;
use rusqlite::{Row, params};
use std::collections::HashSet;

/// One `change_log` row as stored. The JSON stays text: whether it still decodes with today's
/// model is one of the checks a merge makes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogRow {
    pub id: i64,
    pub group: Option<Uuid>,
    pub ts: i64,
    pub label: String,
    pub kind: RecordKind,
    pub target: Uuid,
    pub op: ChangeOp,
    pub before_json: Option<String>,
    pub after_json: Option<String>,
}

/// A group taken from a copy: written to the ledger in the same transaction as the merge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergedGroup {
    pub source_group: Uuid,
    /// The owner's group written for it; `None` when nothing needed writing.
    pub merge_group: Option<Uuid>,
    pub source_name: String,
    /// When the relative made the group (their clock; shown, never used to decide).
    pub source_ts: Option<i64>,
}

const ROW_COLS: &str =
    "id, group_id, ts, label, target_type, target_id, op, before_json, after_json";

fn read_row(r: &Row) -> Result<LogRow, StoreError> {
    let group: Option<String> = r.get(1)?;
    let label: Option<String> = r.get(3)?;
    Ok(LogRow {
        id: r.get(0)?,
        group: group.as_deref().map(parse_uuid).transpose()?,
        ts: r.get(2)?,
        label: label.unwrap_or_default(),
        kind: RecordKind::parse(&r.get::<_, String>(4)?)?,
        target: parse_uuid(&r.get::<_, String>(5)?)?,
        op: ChangeOp::parse(&r.get::<_, String>(6)?)?,
        before_json: r.get(7)?,
        after_json: r.get(8)?,
    })
}

impl Project {
    /// Every change group in the log, in the order it was first written.
    pub fn log_groups_in_order(&self) -> Result<Vec<Uuid>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT group_id FROM change_log WHERE group_id IS NOT NULL \
             GROUP BY group_id ORDER BY MIN(id)",
        )?;
        let ids: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        ids.iter().map(|s| parse_uuid(s)).collect()
    }

    /// The groups already taken from some copy (the ledger).
    pub fn merged_source_groups(&self) -> Result<HashSet<Uuid>, StoreError> {
        let mut st = self
            .conn
            .prepare_cached("SELECT source_group FROM merged_group")?;
        let ids: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        ids.iter().map(|s| parse_uuid(s)).collect()
    }

    /// A stamp of the log: it changes whenever anything is written through `apply_changes`.
    pub fn log_stamp(&self) -> Result<i64, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT COALESCE(MAX(id), 0) FROM change_log", [], |r| {
                r.get(0)
            })?)
    }

    /// The rows of one group, in the order they were written.
    pub fn group_rows(&self, group: Uuid) -> Result<Vec<LogRow>, StoreError> {
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT {ROW_COLS} FROM change_log WHERE group_id = ?1 ORDER BY id"
        ))?;
        let mut rows = st.query([group.to_string()])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push(read_row(r)?);
        }
        Ok(out)
    }

    /// Every logged row of one record, oldest first.
    pub fn record_rows(&self, target: Uuid) -> Result<Vec<LogRow>, StoreError> {
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT {ROW_COLS} FROM change_log WHERE target_id = ?1 ORDER BY id"
        ))?;
        let mut rows = st.query([target.to_string()])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push(read_row(r)?);
        }
        Ok(out)
    }

    /// Whether the log has any row for this record.
    pub fn record_is_logged(&self, target: Uuid) -> Result<bool, StoreError> {
        Ok(self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM change_log WHERE target_id = ?1)",
            [target.to_string()],
            |r| r.get(0),
        )?)
    }

    /// The ledger rows written by one merge.
    pub fn merged_groups_of(&self, batch: Uuid) -> Result<Vec<MergedGroup>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT source_group, merge_group, source_name, source_ts FROM merged_group \
             WHERE batch_id = ?1 ORDER BY rowid",
        )?;
        let mut rows = st.query([batch.to_string()])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let merge: Option<String> = r.get(1)?;
            out.push(MergedGroup {
                source_group: parse_uuid(&r.get::<_, String>(0)?)?,
                merge_group: merge.as_deref().map(parse_uuid).transpose()?,
                source_name: r.get(2)?,
                source_ts: r.get(3)?,
            });
        }
        Ok(out)
    }

    /// Applies a whole merge in ONE transaction: the batch row, every change set (with its
    /// author), the living flags and the ledger. Any error keeps nothing, so a crash leaves the
    /// tree as it was or fully merged. The caller makes the backup first.
    pub fn apply_merge(
        &mut self,
        batch: &Batch,
        sets: &[(ChangeSet, LogMeta)],
        living: &[(Uuid, bool)],
        ledger: &[MergedGroup],
        merged_ts: i64,
    ) -> Result<(), StoreError> {
        self.conn.set_prepared_statement_cache_capacity(64);
        let tx = self.conn.transaction()?;
        tx.pragma_update(None, "defer_foreign_keys", true)?;
        tx.execute(
            "INSERT INTO agent_batch(id, title, source_file, actor_id, actor_name, started_ts, \
             ended_ts, report) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                batch.id.to_string(),
                batch.title,
                batch.source_file,
                batch.actor_id.to_string(),
                batch.actor_name,
                batch.started,
                batch.ended,
                batch.report
            ],
        )?;
        let mut affected = Affected::default();
        for (cs, meta) in sets {
            if meta.batch != Some(batch.id) {
                return Err(corrupt("a merge change set is not in the merge batch"));
            }
            apply_set(&tx, cs, meta, &mut affected)?;
        }
        finish_apply(&tx, affected, living)?;
        for g in ledger {
            tx.prepare_cached(
                "INSERT INTO merged_group(source_group, merge_group, batch_id, source_name, \
                 source_ts, merged_ts) VALUES (?1,?2,?3,?4,?5,?6)",
            )?
            .execute(params![
                g.source_group.to_string(),
                g.merge_group.map(|u| u.to_string()),
                batch.id.to_string(),
                g.source_name,
                g.source_ts,
                merged_ts
            ])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Takes a merge out of the ledger once it is undone in full, so the same copy can be
    /// looked at again.
    pub fn forget_merge(&mut self, batch: Uuid) -> Result<(), StoreError> {
        self.conn.execute(
            "DELETE FROM merged_group WHERE batch_id = ?1",
            [batch.to_string()],
        )?;
        Ok(())
    }
}
