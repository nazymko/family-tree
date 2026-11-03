//! Applying edit commands to an open session, and undo/redo over their change sets.
//!
//! Every applied command is one `ChangeSet` (one change-log group). Undo applies the inverse
//! as a new group labelled "Undo: <label>"; redo applies the original changes again as a
//! group labelled "Redo: <label>". The stacks live in the session, hold at most 200 groups,
//! and are lost when the session closes.

use crate::command::{DeleteImpact, EditCommand, EditResult, HistoryEntry, HistoryOp, UndoState};
use crate::dto::DayOrder as WireDayOrder;
use crate::error::ApiError;
use crate::ops::{apply_to_tree_tracked, plan_with};
use crate::session::Session;
use gt_core::date::DayOrder;
use gt_core::living::{LivingRule, compute_living};
use gt_core::model::Citation;
use gt_store::{ChangeOp, ChangeSet, RecordKind};
use std::collections::HashMap;
use uuid::Uuid;

/// How many groups each stack keeps.
pub const MAX_UNDO: usize = 200;

#[derive(Default)]
pub struct EditState {
    undo: Vec<ChangeSet>,
    redo: Vec<ChangeSet>,
    /// The citation most recently created or cloned; survives undo.
    last_citation: Option<Citation>,
    /// Where removed records sat in the tree order, so that undoing a delete puts them back.
    removed_at: HashMap<Uuid, usize>,
}

fn core_order(o: WireDayOrder) -> DayOrder {
    match o {
        WireDayOrder::Dmy => DayOrder::Dmy,
        WireDayOrder::Mdy => DayOrder::Mdy,
    }
}

impl Session {
    pub(crate) fn day_order(&self) -> DayOrder {
        core_order(self.settings.day_order)
    }

    /// Writes `cs` to the tree and the database; on a database error the tree is put back.
    /// Living flags are recomputed for the whole tree and the changed ones are stored with it.
    fn commit(&mut self, cs: &ChangeSet) -> Result<(), ApiError> {
        apply_to_tree_tracked(&mut self.tree, cs, &mut self.edit.removed_at);
        let (today, _) = (self.clock.0)();
        let rule = LivingRule {
            max_age_years: self.settings.living_max_age,
        };
        let fresh = compute_living(&self.tree, &rule, today);
        let changed: Vec<(Uuid, bool)> = fresh
            .iter()
            .filter(|(id, v)| self.living.get(id) != Some(v))
            .map(|(id, v)| (*id, *v))
            .collect();
        if let Err(e) = self.project.apply_changes(cs, &changed) {
            apply_to_tree_tracked(&mut self.tree, &cs.inverse(), &mut self.edit.removed_at);
            return Err(e.into());
        }
        // Links, living flags, the person index and everything else derived from the tree
        // (and the revision counter) are refreshed in one place.
        self.rebuild();
        Ok(())
    }

    fn result(&self, cs: &ChangeSet, label: String, focus: Option<Uuid>) -> EditResult {
        let mut touched: Vec<Uuid> = Vec::with_capacity(cs.changes.len());
        for ch in &cs.changes {
            if !touched.contains(&ch.id) {
                touched.push(ch.id);
            }
        }
        EditResult {
            label,
            focus,
            touched,
            undo: self.undo_state(),
        }
    }

    /// Plans, stores and applies one command; it becomes the newest undo step and clears redo.
    pub fn apply(&mut self, cmd: EditCommand) -> Result<EditResult, ApiError> {
        let (_, now) = (self.clock.0)();
        let (cs, focus) = plan_with(
            &self.tree,
            &cmd,
            now,
            self.day_order(),
            self.edit.last_citation.as_ref(),
        )?;
        if cs.changes.is_empty() {
            return Ok(self.result(&cs, cs.label.clone(), focus));
        }
        self.commit(&cs)?;
        if matches!(cmd, EditCommand::AddCitation { .. }) {
            self.edit.last_citation = cs
                .changes
                .iter()
                .rev()
                .find(|c| c.kind == RecordKind::Citation && c.before.is_none())
                .and_then(|c| match &c.after {
                    Some(gt_store::Record::Citation(x)) => Some(x.clone()),
                    _ => None,
                });
        }
        self.edit.undo.push(cs.clone());
        if self.edit.undo.len() > MAX_UNDO {
            self.edit.undo.remove(0);
        }
        self.edit.redo.clear();
        Ok(self.result(&cs, cs.label.clone(), focus))
    }

    /// Reverts the newest applied group. `None` when there is nothing to undo.
    pub fn undo(&mut self) -> Result<Option<EditResult>, ApiError> {
        let Some(done) = self.edit.undo.pop() else {
            return Ok(None);
        };
        let (_, now) = (self.clock.0)();
        let mut inv = done.inverse();
        inv.label = format!("Undo: {}", done.label);
        inv.ts = now;
        if let Err(e) = self.commit(&inv) {
            self.edit.undo.push(done);
            return Err(e);
        }
        self.edit.redo.push(done);
        Ok(Some(self.result(&inv, inv.label.clone(), None)))
    }

    /// Applies the newest undone group again.
    pub fn redo(&mut self) -> Result<Option<EditResult>, ApiError> {
        let Some(undone) = self.edit.redo.pop() else {
            return Ok(None);
        };
        let (_, now) = (self.clock.0)();
        let again = ChangeSet {
            group: gt_core::ids::new_id(),
            reverts: None,
            label: undone.label.clone(),
            ts: now,
            changes: undone.changes.clone(),
        };
        let mut logged = again.clone();
        logged.label = format!("Redo: {}", undone.label);
        if let Err(e) = self.commit(&logged) {
            self.edit.redo.push(undone);
            return Err(e);
        }
        self.edit.undo.push(again);
        if self.edit.undo.len() > MAX_UNDO {
            self.edit.undo.remove(0);
        }
        Ok(Some(self.result(&logged, logged.label.clone(), None)))
    }

    pub fn undo_state(&self) -> UndoState {
        UndoState {
            can_undo: !self.edit.undo.is_empty(),
            can_redo: !self.edit.redo.is_empty(),
            undo_label: self.edit.undo.last().map(|c| c.label.clone()),
            redo_label: self.edit.redo.last().map(|c| c.label.clone()),
        }
    }

    /// The citation `{reuseLast: true}` would copy.
    pub fn reuse_citation(&self) -> Option<&Citation> {
        self.edit.last_citation.as_ref()
    }

    /// Change-log entries of one record, newest first.
    pub fn history(&self, record: Uuid, limit: usize) -> Result<Vec<HistoryEntry>, ApiError> {
        Ok(self
            .project
            .history(record, limit)?
            .into_iter()
            .map(|h| HistoryEntry {
                ts: h.ts,
                label: h.label,
                op: match h.op {
                    ChangeOp::Create => HistoryOp::Create,
                    ChangeOp::Update => HistoryOp::Update,
                    ChangeOp::Delete => HistoryOp::Delete,
                },
            })
            .collect())
    }

    /// What `deletePerson` would delete besides the person, for the confirmation text.
    pub fn delete_impact(&self, id: Uuid) -> Result<DeleteImpact, ApiError> {
        let (_, now) = (self.clock.0)();
        let (cs, _) = plan_with(
            &self.tree,
            &EditCommand::DeletePerson { id },
            now,
            self.day_order(),
            None,
        )?;
        let gone = |k: RecordKind| {
            let n = cs
                .changes
                .iter()
                .filter(|c| c.kind == k && c.after.is_none())
                .count();
            u32::try_from(n).unwrap_or(u32::MAX)
        };
        Ok(DeleteImpact {
            families: gone(RecordKind::Family),
            events: gone(RecordKind::Event),
        })
    }
}

impl Session {
    /// Stores and applies an already planned change set as the newest undo step (clears redo).
    pub(crate) fn commit_group(&mut self, cs: ChangeSet) -> Result<(), ApiError> {
        self.commit(&cs)?;
        self.edit.undo.push(cs);
        if self.edit.undo.len() > MAX_UNDO {
            self.edit.undo.remove(0);
        }
        self.edit.redo.clear();
        Ok(())
    }
}
