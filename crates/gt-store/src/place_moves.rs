//! Place records that follow a place merge. The records table is outside the change log, so undoing a
//! merge from the log cannot bring them back by itself: the merge remembers where each record came
//! from in `place_record_move` (made on first use, so the schema version stays as it is), in the same
//! transaction as the merge's change set.

use crate::StoreError;
use crate::project::Project;
use gt_core::Uuid;
use rusqlite::params;

/// What happens to the place records inside the transaction of a change set.
#[derive(Clone, Debug, Default)]
pub struct PlaceRecordAdjust {
    /// `(from, to)`: every record of place `from` now belongs to place `to`.
    pub moves: Vec<(Uuid, Uuid)>,
    /// The group of an earlier merge whose moves this change set undoes.
    pub restore_group: Option<Uuid>,
}

pub(crate) fn adjust_place_records(
    tx: &rusqlite::Transaction<'_>,
    group: Uuid,
    adjust: &PlaceRecordAdjust,
) -> Result<(), StoreError> {
    if adjust.moves.is_empty() && adjust.restore_group.is_none() {
        return Ok(());
    }
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS place_record_move (\
            group_id TEXT NOT NULL, record_id INTEGER NOT NULL, from_place TEXT NOT NULL); \
         CREATE INDEX IF NOT EXISTS idx_place_record_move_group ON place_record_move(group_id);",
    )?;
    let group_text = group.to_string();
    for (from, to) in &adjust.moves {
        tx.execute(
            "INSERT INTO place_record_move(group_id, record_id, from_place) \
             SELECT ?1, id, place_id FROM place_record WHERE place_id = ?2",
            params![group_text, from.to_string()],
        )?;
        tx.execute(
            "UPDATE place_record SET place_id = ?2 WHERE place_id = ?1",
            params![from.to_string(), to.to_string()],
        )?;
    }
    if let Some(earlier) = adjust.restore_group {
        let earlier = earlier.to_string();
        tx.execute(
            "UPDATE place_record SET place_id = (SELECT m.from_place FROM place_record_move m \
             WHERE m.group_id = ?1 AND m.record_id = place_record.id) \
             WHERE id IN (SELECT record_id FROM place_record_move WHERE group_id = ?1)",
            params![earlier],
        )?;
        tx.execute(
            "DELETE FROM place_record_move WHERE group_id = ?1",
            params![earlier],
        )?;
    }
    Ok(())
}

impl Project {
    /// Whether the merge in change group `group` moved place records that its undo puts back.
    pub fn place_records_moved_by(&self, group: Uuid) -> Result<bool, StoreError> {
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='place_record_move')",
            [],
            |r| r.get(0),
        )?;
        if !exists {
            return Ok(false);
        }
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM place_record_move WHERE group_id = ?1)",
            params![group.to_string()],
            |r| r.get(0),
        )?)
    }
}
