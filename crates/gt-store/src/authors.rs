//! Who made each change: the author and batch stored on every change-log row, the people each
//! change group touched (with a per-person "reviewed" mark), agent batches, and the queries
//! behind the change log view.

use crate::StoreError;
use crate::codec::{corrupt, parse_uuid};
use crate::edit::{Change, ChangeOp, Record, RecordKind};
use crate::project::Project;
use gt_core::Uuid;
use rusqlite::{OptionalExtension, Row, params};
use std::collections::HashSet;

/// Who made a change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Actor {
    User,
    Agent {
        id: Uuid,
        name: String,
    },
    /// A relative's edit taken in by merging their copy of the tree; `name` is whose copy.
    Copy {
        name: String,
    },
}

/// Author data stored with one change set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogMeta {
    pub actor: Actor,
    pub batch: Option<Uuid>,
    /// The people the change is about (shown in the log, reviewed one by one).
    pub people: Vec<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogPerson {
    pub id: Uuid,
    /// When the user reviewed the change for this person (Unix ms).
    pub reviewed: Option<i64>,
}

/// One change group (one user action or one agent call) as the change log shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct LogGroup {
    pub group: Uuid,
    pub ts: i64,
    pub label: String,
    /// `None` for rows written before authors were tracked.
    pub actor: Option<Actor>,
    pub batch: Option<Uuid>,
    /// The group this one undoes.
    pub reverts: Option<Uuid>,
    /// Some later group undid this one.
    pub reverted: bool,
    pub people: Vec<LogPerson>,
    pub changes: Vec<Change>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogFilter {
    pub actor_id: Option<Uuid>,
    /// `user` or `agent`.
    pub actor_kind: Option<String>,
    pub batch: Option<Uuid>,
    pub person: Option<Uuid>,
    /// Agent groups with a person the user has not reviewed yet, and not undone.
    pub unreviewed_only: bool,
    /// Leave out groups a later group undid.
    pub standing_only: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewScope {
    Person(Uuid),
    Batch(Uuid),
    Group(Uuid),
}

/// A named run of agent edits, such as one imported file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Batch {
    pub id: Uuid,
    pub title: String,
    pub source_file: Option<String>,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub started: i64,
    pub ended: Option<i64>,
    pub report: Option<String>,
}

/// The `change_log` columns that name the author, as written by `apply_changes`.
pub(crate) fn actor_columns(a: &Actor) -> (&'static str, Option<String>, Option<&str>) {
    match a {
        Actor::User => ("user", None, None),
        Actor::Agent { id, name } => ("agent", Some(id.to_string()), Some(name)),
        Actor::Copy { name } => ("copy", None, Some(name)),
    }
}

fn read_actor(
    kind: Option<String>,
    id: Option<String>,
    name: Option<String>,
) -> Result<Option<Actor>, StoreError> {
    Ok(match kind.as_deref() {
        None => None,
        Some("user") => Some(Actor::User),
        Some("agent") => Some(Actor::Agent {
            id: parse_uuid(&id.ok_or_else(|| corrupt("agent row without actor_id"))?)?,
            name: name.unwrap_or_default(),
        }),
        Some("copy") => Some(Actor::Copy {
            name: name.unwrap_or_default(),
        }),
        Some(other) => return Err(corrupt(format!("bad change_log actor_kind {other:?}"))),
    })
}

pub(crate) fn decode_record(json: Option<String>) -> Result<Option<Record>, StoreError> {
    json.map(|j| serde_json::from_str(&j).map_err(|e| corrupt(e.to_string())))
        .transpose()
}

fn read_change(r: &Row) -> Result<Change, StoreError> {
    let kind = RecordKind::parse(&r.get::<_, String>(0)?)?;
    Ok(Change {
        kind,
        id: parse_uuid(&r.get::<_, String>(1)?)?,
        before: decode_record(r.get(2)?)?,
        after: decode_record(r.get(3)?)?,
    })
}

const CHANGE_COLS: &str = "target_type, target_id, before_json, after_json";

/// Filter shared by the page query and its count. `g` is a group summary row (`group_id`,
/// `first` = id of its first row), `f` that first row. Parameters: 1 actor id, 2 actor kind,
/// 3 batch, 4 person, 5 unreviewed only, 6 standing only. A group written before schema 3 has no `change_person`
/// rows; it reaches only the person record it changed itself (its other people were not kept).
const LOG_FROM: &str = "FROM (SELECT group_id, MIN(id) AS first, MAX(id) AS last FROM change_log \
     WHERE group_id IS NOT NULL GROUP BY group_id) g \
     JOIN change_log f ON f.id = g.first \
     WHERE (?1 IS NULL OR f.actor_id = ?1) \
       AND (?2 IS NULL OR f.actor_kind = ?2) \
       AND (?3 IS NULL OR f.batch_id = ?3) \
       AND (?4 IS NULL OR EXISTS (SELECT 1 FROM change_person cp \
            WHERE cp.group_id = g.group_id AND cp.person_id = ?4) \
            OR (NOT EXISTS (SELECT 1 FROM change_person cp WHERE cp.group_id = g.group_id) \
                AND EXISTS (SELECT 1 FROM change_log t WHERE t.group_id = g.group_id \
                    AND t.target_id = ?4))) \
       AND (?5 = 0 OR (f.actor_kind = 'agent' \
            AND EXISTS (SELECT 1 FROM change_person cp WHERE cp.group_id = g.group_id \
                AND cp.reviewed_ts IS NULL AND cp.person_id = COALESCE(?4, cp.person_id)) \
            AND g.group_id NOT IN (SELECT reverts_group FROM change_log WHERE reverts_group IS NOT NULL))) \
       AND (?6 = 0 OR g.group_id NOT IN (SELECT reverts_group FROM change_log WHERE reverts_group IS NOT NULL))";

impl Project {
    /// The changes of one group, in the order they were applied.
    pub fn group_changes(&self, group: Uuid) -> Result<Vec<Change>, StoreError> {
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT {CHANGE_COLS} FROM change_log WHERE group_id = ?1 ORDER BY id"
        ))?;
        let mut rows = st.query([group.to_string()])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push(read_change(r)?);
        }
        Ok(out)
    }

    /// Every logged change to one record, oldest first.
    pub fn rows_of(&self, target: Uuid) -> Result<Vec<Change>, StoreError> {
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT {CHANGE_COLS} FROM change_log WHERE target_id = ?1 ORDER BY id"
        ))?;
        let mut rows = st.query([target.to_string()])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push(read_change(r)?);
        }
        Ok(out)
    }

    /// Who wrote each logged change to a record, oldest first. `None` is the user or a row
    /// from before authors were tracked.
    pub fn last_writers(&self, target: Uuid) -> Result<Vec<(Option<Uuid>, ChangeOp)>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT actor_id, op FROM change_log WHERE target_id = ?1 ORDER BY id",
        )?;
        let mut rows = st.query([target.to_string()])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let who = r
                .get::<_, Option<String>>(0)?
                .as_deref()
                .map(parse_uuid)
                .transpose()?;
            out.push((who, ChangeOp::parse(&r.get::<_, String>(1)?)?));
        }
        Ok(out)
    }

    /// The batch the first logged change to a record was written in, when it was a creation.
    pub fn created_in_batch(&self, target: Uuid) -> Result<Option<Uuid>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT op, batch_id FROM change_log WHERE target_id = ?1 ORDER BY id LIMIT 1",
        )?;
        let mut rows = st.query([target.to_string()])?;
        let Some(r) = rows.next()? else {
            return Ok(None);
        };
        if ChangeOp::parse(&r.get::<_, String>(0)?)? != ChangeOp::Create {
            return Ok(None);
        }
        r.get::<_, Option<String>>(1)?
            .as_deref()
            .map(parse_uuid)
            .transpose()
    }

    /// One page of the change log, newest group first, and the number of groups matching
    /// the filter. Rows without a group (written by schema 1) are not groups and are left out.
    pub fn log_page(
        &self,
        f: &LogFilter,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<LogGroup>, u32), StoreError> {
        let args = |extra: &[i64]| -> Vec<rusqlite::types::Value> {
            let mut v: Vec<rusqlite::types::Value> = vec![
                f.actor_id.map(|u| u.to_string()).into(),
                f.actor_kind.clone().into(),
                f.batch.map(|u| u.to_string()).into(),
                f.person.map(|u| u.to_string()).into(),
                i64::from(f.unreviewed_only).into(),
                i64::from(f.standing_only).into(),
            ];
            v.extend(extra.iter().map(|n| rusqlite::types::Value::Integer(*n)));
            v
        };
        let total: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) {LOG_FROM}"),
            rusqlite::params_from_iter(args(&[])),
            |r| r.get(0),
        )?;
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT g.group_id {LOG_FROM} ORDER BY g.last DESC LIMIT ?7 OFFSET ?8"
        ))?;
        let off = i64::try_from(offset).unwrap_or(i64::MAX);
        let lim = i64::try_from(limit).unwrap_or(i64::MAX);
        let ids: Vec<String> = st
            .query_map(rusqlite::params_from_iter(args(&[lim, off])), |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(g) = self.log_group(parse_uuid(&id)?)? {
                out.push(g);
            }
        }
        Ok((out, u32::try_from(total).unwrap_or(u32::MAX)))
    }

    /// Whether the log holds groups from before authors were tracked (no author on them).
    pub fn has_untracked_groups(&self) -> Result<bool, StoreError> {
        Ok(self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM change_log WHERE group_id IS NOT NULL AND actor_kind IS NULL)",
            [],
            |r| r.get(0),
        )?)
    }

    /// One group with its author, people and changes; `None` when no such group exists.
    pub fn log_group(&self, group: Uuid) -> Result<Option<LogGroup>, StoreError> {
        let gs = group.to_string();
        // A group has exactly one author, so its first row speaks for all of them.
        type Head = (
            i64,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        );
        let head: Option<Head> = self
            .conn
            .prepare_cached(
                "SELECT ts, label, actor_kind, actor_id, actor_name, batch_id, reverts_group \
                 FROM change_log WHERE group_id = ?1 ORDER BY id LIMIT 1",
            )?
            .query_row([&gs], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            })
            .optional()?;
        let Some((ts, label, kind, aid, aname, batch, reverts)) = head else {
            return Ok(None);
        };
        let reverted: bool = self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM change_log WHERE reverts_group = ?1)",
            [&gs],
            |r| r.get(0),
        )?;
        let mut people = Vec::new();
        {
            let mut st = self.conn.prepare_cached(
                "SELECT person_id, reviewed_ts FROM change_person WHERE group_id = ?1 ORDER BY rowid",
            )?;
            let mut rows = st.query([&gs])?;
            while let Some(r) = rows.next()? {
                people.push(LogPerson {
                    id: parse_uuid(&r.get::<_, String>(0)?)?,
                    reviewed: r.get(1)?,
                });
            }
        }
        Ok(Some(LogGroup {
            group,
            ts,
            label: label.unwrap_or_default(),
            actor: read_actor(kind, aid, aname)?,
            batch: batch.as_deref().map(parse_uuid).transpose()?,
            reverts: reverts.as_deref().map(parse_uuid).transpose()?,
            reverted,
            people,
            changes: self.group_changes(group)?,
        }))
    }

    /// Marks changes as reviewed at `ts`; marks that are already set keep their first time.
    pub fn mark_reviewed(&mut self, scope: ReviewScope, ts: i64) -> Result<(), StoreError> {
        let (sql, arg) = match scope {
            ReviewScope::Person(p) => (
                "UPDATE change_person SET reviewed_ts = ?1 WHERE person_id = ?2 AND reviewed_ts IS NULL",
                p,
            ),
            ReviewScope::Batch(b) => (
                "UPDATE change_person SET reviewed_ts = ?1 WHERE reviewed_ts IS NULL AND group_id IN \
                 (SELECT group_id FROM change_log WHERE batch_id = ?2)",
                b,
            ),
            ReviewScope::Group(g) => (
                "UPDATE change_person SET reviewed_ts = ?1 WHERE group_id = ?2 AND reviewed_ts IS NULL",
                g,
            ),
        };
        self.conn.execute(sql, params![ts, arg.to_string()])?;
        Ok(())
    }

    /// People in agent groups that were not undone and that the user has not reviewed.
    pub fn unreviewed_people(&self) -> Result<HashSet<Uuid>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT DISTINCT cp.person_id FROM change_person cp \
             WHERE cp.reviewed_ts IS NULL \
               AND EXISTS (SELECT 1 FROM change_log c WHERE c.group_id = cp.group_id \
                           AND c.actor_kind = 'agent') \
               AND cp.group_id NOT IN (SELECT reverts_group FROM change_log WHERE reverts_group IS NOT NULL)",
        )?;
        let ids: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        ids.iter().map(|s| parse_uuid(s)).collect()
    }

    pub fn batch_create(&mut self, b: &Batch) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO agent_batch(id, title, source_file, actor_id, actor_name, started_ts, \
             ended_ts, report) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                b.id.to_string(),
                b.title,
                b.source_file,
                b.actor_id.to_string(),
                b.actor_name,
                b.started,
                b.ended,
                b.report
            ],
        )?;
        Ok(())
    }

    pub fn batch_close(&mut self, id: Uuid, report: &str, ts: i64) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE agent_batch SET ended_ts = ?2, report = ?3 WHERE id = ?1",
            params![id.to_string(), ts, report],
        )?;
        Ok(())
    }

    pub fn batch_get(&self, id: Uuid) -> Result<Option<Batch>, StoreError> {
        let mut st = self
            .conn
            .prepare_cached(&format!("{BATCH_SELECT} WHERE id = ?1"))?;
        let mut rows = st.query([id.to_string()])?;
        rows.next()?.map(read_batch).transpose()
    }

    /// How much a batch holds: `(change groups, people, people still unreviewed)`. Groups that
    /// were undone do not count as unreviewed.
    pub fn batch_counts(&self, id: Uuid) -> Result<(u32, u32, u32), StoreError> {
        let id = id.to_string();
        let count = |sql: &str| -> Result<u32, StoreError> {
            let n: i64 = self
                .conn
                .prepare_cached(sql)?
                .query_row([&id], |r| r.get(0))?;
            Ok(u32::try_from(n).unwrap_or(u32::MAX))
        };
        Ok((
            count("SELECT COUNT(DISTINCT group_id) FROM change_log WHERE batch_id = ?1")?,
            count(
                "SELECT COUNT(DISTINCT person_id) FROM change_person WHERE group_id IN \
                 (SELECT group_id FROM change_log WHERE batch_id = ?1)",
            )?,
            count(
                "SELECT COUNT(DISTINCT cp.person_id) FROM change_person cp \
                 WHERE cp.reviewed_ts IS NULL \
                   AND cp.group_id IN (SELECT group_id FROM change_log WHERE batch_id = ?1) \
                   AND cp.group_id NOT IN (SELECT reverts_group FROM change_log WHERE reverts_group IS NOT NULL)",
            )?,
        ))
    }

    /// How many of a batch's change groups have not been undone.
    pub fn batch_standing(&self, id: Uuid) -> Result<u32, StoreError> {
        let n: i64 = self
            .conn
            .prepare_cached(
                "SELECT COUNT(DISTINCT g.group_id) FROM change_log g WHERE g.batch_id = ?1 \
                 AND g.group_id NOT IN (SELECT reverts_group FROM change_log WHERE reverts_group IS NOT NULL)",
            )?
            .query_row([id.to_string()], |r| r.get(0))?;
        Ok(u32::try_from(n).unwrap_or(u32::MAX))
    }

    /// Every agent in the log, with the name it last wrote under, sorted by name. Log rows
    /// outlive connections, so this is not the list of connections.
    pub fn log_agents(&self) -> Result<Vec<(Uuid, String)>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT actor_id, actor_name, MAX(id) FROM change_log \
             WHERE actor_kind = 'agent' AND actor_id IS NOT NULL \
             GROUP BY actor_id ORDER BY actor_name COLLATE NOCASE, actor_id",
        )?;
        let mut rows = st.query([])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let name: Option<String> = r.get(1)?;
            out.push((
                parse_uuid(&r.get::<_, String>(0)?)?,
                name.unwrap_or_default(),
            ));
        }
        Ok(out)
    }

    /// All batches, newest first.
    pub fn batch_list(&self) -> Result<Vec<Batch>, StoreError> {
        let mut st = self.conn.prepare_cached(&format!(
            "{BATCH_SELECT} ORDER BY started_ts DESC, rowid DESC"
        ))?;
        let mut rows = st.query([])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push(read_batch(r)?);
        }
        Ok(out)
    }
}

const BATCH_SELECT: &str = "SELECT id, title, source_file, actor_id, actor_name, started_ts, \
     ended_ts, report FROM agent_batch";

fn read_batch(r: &Row) -> Result<Batch, StoreError> {
    Ok(Batch {
        id: parse_uuid(&r.get::<_, String>(0)?)?,
        title: r.get(1)?,
        source_file: r.get(2)?,
        actor_id: parse_uuid(&r.get::<_, String>(3)?)?,
        actor_name: r.get(4)?,
        started: r.get(5)?,
        ended: r.get(6)?,
        report: r.get(7)?,
    })
}
