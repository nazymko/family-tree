//! Applying edit commands to an open session, and undo/redo over their change sets.
//!
//! Every applied command is one `ChangeSet` (one change-log group). Undo reverts it fact by fact
//! (see `revert.rs`) as a new group labelled "Undo: <label>"; redo reverts that again as a group
//! labelled "Redo: <label>". With nothing in between both are byte for byte the old state; when an
//! agent has changed the same fact since, they stop. The stacks hold the owner's edits only, live
//! in the session, hold at most 200 groups, and are lost when the session closes.
//!
//! An agent's changes never enter the stacks. They are undone from the log (`undo_group`,
//! `undo_batch`), which works from what the log stores and so also after a restart.

use crate::command::{DeleteImpact, EditCommand, EditResult, HistoryEntry, UndoReport, UndoState};
use crate::dto::DayOrder as WireDayOrder;
use crate::error::{ApiError, ErrorCode};
use crate::log::history_op;
use crate::ops::{apply_to_tree_tracked, plan_with};
use crate::revert::{settle_kept, undo_on_tree};
use crate::session::Session;
use gt_core::date::DayOrder;
use gt_core::model::Citation;
use gt_store::{Actor, Change, ChangeSet, LogFilter, LogGroup, LogMeta, RecordKind};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// How many groups each stack keeps.
pub const MAX_UNDO: usize = 200;

const AGENT_CHANGED: &str = "An agent changed this since. Undo it from the change log first.";

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
    /// The log records `actor` and `batch`, and the people `cs` is about.
    pub(crate) fn commit(
        &mut self,
        cs: &ChangeSet,
        actor: Actor,
        batch: Option<Uuid>,
    ) -> Result<(), ApiError> {
        let people = self.people_of(cs);
        apply_to_tree_tracked(&mut self.tree, cs, &mut self.edit.removed_at);
        // One pass over the tree gives both maps; the flags that differ are stored with the edit.
        let (effective, computed) = self.living_maps();
        let changed: Vec<(Uuid, bool)> = effective
            .iter()
            .filter(|(id, v)| self.living.get(id) != Some(v))
            .map(|(id, v)| (*id, *v))
            .collect();
        if let Err(e) = self.project.apply_changes(
            cs,
            &changed,
            &LogMeta {
                actor,
                batch,
                people,
            },
        ) {
            apply_to_tree_tracked(&mut self.tree, &cs.inverse(), &mut self.edit.removed_at);
            return Err(e.into());
        }
        // Links, living flags, the person index and everything else derived from the tree
        // (and the revision counter) follow the change set.
        self.refresh_after(cs, effective, computed, &changed);
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

    fn plan_command(&mut self, cmd: &EditCommand) -> Result<(ChangeSet, Option<Uuid>), ApiError> {
        let (_, now) = (self.clock.0)();
        if let EditCommand::ImportDraft {
            persons,
            families,
            attach_picture,
        } = cmd
        {
            self.plan_import_draft(persons, families, *attach_picture, now)
        } else {
            plan_with(
                &self.tree,
                cmd,
                now,
                self.day_order(),
                self.edit.last_citation.as_ref(),
            )
        }
    }

    /// Plans, stores and applies one command of the owner; it becomes the newest undo step and
    /// clears redo.
    pub fn apply(&mut self, cmd: EditCommand) -> Result<EditResult, ApiError> {
        let (cs, focus) = self.plan_command(&cmd)?;
        if cs.changes.is_empty() {
            return Ok(self.result(&cs, cs.label.clone(), focus));
        }
        self.commit(&cs, Actor::User, None)?;
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

    /// Plans and stores a command under another author, leaving the undo stacks alone. No checks
    /// on what the author may do: those stand in front of this.
    #[doc(hidden)]
    pub fn commit_command_as(
        &mut self,
        cmd: EditCommand,
        actor: Actor,
        batch: Option<Uuid>,
    ) -> Result<EditResult, ApiError> {
        let (cs, focus) = self.plan_command(&cmd)?;
        if !cs.changes.is_empty() {
            self.commit(&cs, actor, batch)?;
        }
        Ok(self.result(&cs, cs.label.clone(), focus))
    }

    /// The writes that take `changes` (in the order to undo them) back on the tree as it stands,
    /// or the error that stops Ctrl+Z when an agent has changed the same fact or now depends on it.
    fn ctrl_z_writes(&self, changes: &[&Change]) -> Result<Vec<Change>, ApiError> {
        let (_, now) = (self.clock.0)();
        let kept = settle_kept(&self.tree, changes);
        let done = undo_on_tree(&self.tree, changes, &kept, now, false);
        if !kept.is_empty() || !done.skipped.is_empty() {
            return Err(ApiError::new(ErrorCode::Conflict, AGENT_CHANGED));
        }
        Ok(done.writes)
    }

    /// Reverts the newest applied group. `None` when there is nothing to undo.
    pub fn undo(&mut self) -> Result<Option<EditResult>, ApiError> {
        let Some(done) = self.edit.undo.last().cloned() else {
            return Ok(None);
        };
        let (_, now) = (self.clock.0)();
        let newest_first: Vec<&Change> = done.changes.iter().rev().collect();
        let inv = ChangeSet {
            group: gt_core::ids::new_id(),
            reverts: Some(done.group),
            label: format!("Undo: {}", done.label),
            ts: now,
            changes: self.ctrl_z_writes(&newest_first)?,
        };
        if !inv.changes.is_empty() {
            self.commit(&inv, Actor::User, None)?;
        }
        self.edit.undo.pop();
        self.edit.redo.push(done);
        Ok(Some(self.result(&inv, inv.label.clone(), None)))
    }

    /// Applies the newest undone group again: reverts its inverse.
    pub fn redo(&mut self) -> Result<Option<EditResult>, ApiError> {
        let Some(undone) = self.edit.redo.last().cloned() else {
            return Ok(None);
        };
        let (_, now) = (self.clock.0)();
        let inverse = undone.inverse();
        let in_order: Vec<&Change> = inverse.changes.iter().rev().collect();
        let logged = ChangeSet {
            group: gt_core::ids::new_id(),
            reverts: None,
            label: format!("Redo: {}", undone.label),
            ts: now,
            changes: self.ctrl_z_writes(&in_order)?,
        };
        if !logged.changes.is_empty() {
            self.commit(&logged, Actor::User, None)?;
        }
        self.edit.redo.pop();
        self.edit.undo.push(ChangeSet {
            group: logged.group,
            reverts: None,
            label: undone.label.clone(),
            ts: now,
            changes: undone.changes,
        });
        if self.edit.undo.len() > MAX_UNDO {
            self.edit.undo.remove(0);
        }
        Ok(Some(self.result(&logged, logged.label.clone(), None)))
    }

    /// Undoes one agent's change from the log, as a new group by the owner. A fact someone has
    /// changed since stays and is reported. A group already undone is left alone. The Ctrl+Z
    /// stacks are not touched.
    pub fn undo_group(&mut self, group: Uuid) -> Result<UndoReport, ApiError> {
        let g = self
            .project
            .log_group(group)?
            .ok_or_else(|| ApiError::not_found("change", group))?;
        Self::refuse_unless_agent(&g)?;
        if g.reverted {
            return Ok(UndoReport::default());
        }
        let newest_first: Vec<&Change> = g.changes.iter().rev().collect();
        let kept = settle_kept(&self.tree, &newest_first);
        self.undo_logged(&g, &kept)
    }

    /// Undoes every change of a batch that still stands, newest first. What has to stay is worked
    /// out over all of them, so a person kept by one group keeps the links another group made.
    pub fn undo_batch(&mut self, batch: Uuid) -> Result<UndoReport, ApiError> {
        if self.project.batch_get(batch)?.is_none() {
            return Err(ApiError::not_found("batch", batch));
        }
        let filter = LogFilter {
            batch: Some(batch),
            ..Default::default()
        };
        let (groups, _) = self.project.log_page(&filter, 0, usize::MAX)?;
        let standing: Vec<&LogGroup> = groups.iter().filter(|g| !g.reverted).collect();
        for g in &standing {
            Self::refuse_unless_agent(g)?;
        }
        let newest_first: Vec<&Change> = standing
            .iter()
            .flat_map(|g| g.changes.iter().rev())
            .collect();
        let kept = settle_kept(&self.tree, &newest_first);
        let mut report = UndoReport::default();
        for g in standing {
            let one = self.undo_logged(g, &kept)?;
            report.undone += one.undone;
            report.skipped.extend(one.skipped);
        }
        Ok(report)
    }

    fn undo_logged(&mut self, g: &LogGroup, kept: &HashSet<Uuid>) -> Result<UndoReport, ApiError> {
        let (_, now) = (self.clock.0)();
        let newest_first: Vec<&Change> = g.changes.iter().rev().collect();
        let done = undo_on_tree(&self.tree, &newest_first, kept, now, true);
        let cs = ChangeSet {
            group: gt_core::ids::new_id(),
            reverts: Some(g.group),
            label: format!("Undo: {}", g.label),
            ts: now,
            changes: done.writes,
        };
        if !cs.changes.is_empty() {
            self.commit(&cs, Actor::User, None)?;
        }
        Ok(UndoReport {
            undone: done.undone,
            skipped: done
                .skipped
                .into_iter()
                .map(|ch| self.skipped_fact(ch))
                .collect(),
        })
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
                op: history_op(h.op),
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
    /// Applies a change set planned outside `apply` (media attach, relink) as a new undo step
    /// (clears redo).
    pub(crate) fn apply_planned(&mut self, cs: ChangeSet) -> Result<EditResult, ApiError> {
        self.commit(&cs, Actor::User, None)?;
        self.edit.undo.push(cs.clone());
        if self.edit.undo.len() > MAX_UNDO {
            self.edit.undo.remove(0);
        }
        self.edit.redo.clear();
        Ok(self.result(&cs, cs.label.clone(), None))
    }

    /// As `apply_planned`, without building the result.
    pub(crate) fn commit_group(&mut self, cs: ChangeSet) -> Result<(), ApiError> {
        self.apply_planned(cs).map(|_| ())
    }
}
