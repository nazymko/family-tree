//! Place housekeeping by an agent: a new place (the one place an agent may make; every other place
//! comes with an event or media that names it), and the merge of duplicate places. Both are one
//! change set under the agent's batch, so the owner undoes them with it.

use super::{AgentCtx, AgentWrite, TEXT_PIECE_NOTE_MAX, people_reached, refused};
use crate::error::{ApiError, ErrorCode};
use crate::ops::place_merge::{PlaceMergeCounts, find_by_name, plan_add_place, plan_merge};
use crate::session::Session;
use gt_store::{Actor, PlaceRecordAdjust};
use uuid::Uuid;

/// The longest name a place gets.
const PLACE_NAME_MAX: usize = 200;
/// The most places one merge takes.
pub const PLACE_MERGE_MAX: usize = 50;

/// What a merge did, in counts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaceMergeOutcome {
    pub counts: PlaceMergeCounts,
    /// Places removed.
    pub removed: usize,
    /// Place records (church-book records about other people) that now belong to the kept place.
    pub place_records: u32,
}

impl Session {
    /// Makes a place with a name, a kind, a parent, optionally a point and a note, in one change
    /// set. A place of the same name (any of its names, ignoring case) under the same parent is
    /// never made twice: that one is answered as [`AgentWrite::AlreadyThere`]. A point needs its
    /// `source`, which is kept with the note in a research note on the place.
    #[allow(clippy::too_many_arguments)]
    pub fn add_place_as_agent(
        &mut self,
        name: &str,
        kind: Option<&str>,
        parent: Option<Uuid>,
        point: Option<(f64, f64)>,
        source: Option<&str>,
        note: Option<&str>,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        let name = name.trim();
        if name.is_empty() || name.chars().count() > PLACE_NAME_MAX {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                format!("name must be the place's name, up to {PLACE_NAME_MAX} characters."),
            ));
        }
        if let Some(p) = parent
            && !self.tree.places.contains_key(&p)
        {
            return Err(ApiError::not_found("place", p));
        }
        if let Some((lat, lon)) = point
            && (!lat.is_finite()
                || !lon.is_finite()
                || !(-90.0..=90.0).contains(&lat)
                || !(-180.0..=180.0).contains(&lon))
        {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "lat must be from -90 to 90 and lon from -180 to 180.",
            ));
        }
        let source = source.map(str::trim).filter(|s| !s.is_empty());
        if point.is_some() && source.is_none() {
            return Err(refused(
                "Give the source of the coordinates, such as GeoNames 702550 or the Wikipedia \
                 article of the village.",
            ));
        }
        let note = note.map(str::trim).filter(|n| !n.is_empty());
        if note.is_some_and(|n| n.len() > TEXT_PIECE_NOTE_MAX) {
            return Err(refused(
                "The note holds up to 64 KiB. Put a longer text on a source.",
            ));
        }
        if let Some(existing) = find_by_name(&self.tree, name, parent) {
            return Ok((AgentWrite::AlreadyThere { record: existing }, Vec::new()));
        }
        // Millionths of a degree, as the owner's Map stores them.
        let point = point.map(|(la, lo)| ((la * 1e6).round() / 1e6, (lo * 1e6).round() / 1e6));
        let mut text = String::new();
        if let (Some((la, lo)), Some(src)) = (point, source) {
            text.push_str(&format!(
                "Coordinates {la:.6}, {lo:.6} (latitude, longitude). Source: {src}"
            ));
        } else if let Some(src) = source {
            text.push_str(&format!("Source: {src}"));
        }
        if let Some(n) = note {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(n);
        }
        let (_, now) = (self.clock.0)();
        let (cs, id) = plan_add_place(
            &self.tree,
            name,
            kind,
            parent,
            point,
            (!text.is_empty()).then_some(text.as_str()),
            now,
            self.day_order(),
        )?;
        let actor = Actor::Agent {
            id: ctx.id,
            name: ctx.name.clone(),
        };
        self.commit(&cs, actor, Some(batch))?;
        let result = self.result(&cs, cs.label.clone(), Some(id));
        Ok((AgentWrite::Done(result), Vec::new()))
    }

    /// Merges duplicate places into `keep`, in one change set (and so one undo step): see
    /// `plan_merge`. The place records follow in the same transaction, and come back with an undo.
    /// This is the one place an agent changes what the owner entered: the merge changes events and
    /// media of others to name the kept place, and removes the merged ones. Refused when `keep` is
    /// among `merge`, when a merged place is an ancestor of `keep` (its places would move under
    /// themselves), or when a place is unknown.
    pub fn merge_places_as_agent(
        &mut self,
        keep: Uuid,
        merge: &[Uuid],
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(PlaceMergeOutcome, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        if !self.tree.places.contains_key(&keep) {
            return Err(ApiError::not_found("place", keep));
        }
        let mut ids: Vec<Uuid> = Vec::with_capacity(merge.len());
        for id in merge {
            if !ids.contains(id) {
                ids.push(*id);
            }
        }
        if ids.is_empty() || ids.len() > PLACE_MERGE_MAX {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                format!("merge_ids must list 1 to {PLACE_MERGE_MAX} places."),
            ));
        }
        if ids.contains(&keep) {
            return Err(refused(
                "keep_id is among merge_ids: a place cannot be merged into itself.",
            ));
        }
        for id in &ids {
            if !self.tree.places.contains_key(id) {
                return Err(ApiError::not_found("place", *id));
            }
        }
        // A merged place above the kept one: the kept place's chain would run through a place that
        // is gone. A chain is short; the limit only guards against a parent cycle.
        let mut up = self.tree.places.get(&keep).and_then(|p| p.parent);
        for _ in 0..64 {
            let Some(at) = up else { break };
            if ids.contains(&at) {
                return Err(refused(
                    "A place in merge_ids is an ancestor of keep_id. Keep the ancestor, or \
                     merge only places that are not above the one you keep.",
                ));
            }
            up = self.tree.places.get(&at).and_then(|p| p.parent);
        }
        let (_, now) = (self.clock.0)();
        let (cs, counts) = plan_merge(&self.tree, keep, &ids, now, self.day_order())?;
        let mut place_records = 0;
        for id in &ids {
            place_records += self.project.count_place_records(&[*id])?;
        }
        let adjust = PlaceRecordAdjust {
            moves: ids.iter().map(|id| (*id, keep)).collect(),
            person_moves: Vec::new(),
            restore_group: None,
        };
        let changed: Vec<Uuid> = cs.changes.iter().map(|c| c.id).collect();
        let mut reached = people_reached(&self.tree, &changed);
        let actor = Actor::Agent {
            id: ctx.id,
            name: ctx.name.clone(),
        };
        self.commit_adjusting(&cs, actor, Some(batch), &adjust)?;
        for id in people_reached(&self.tree, &changed) {
            if !reached.contains(&id) {
                reached.push(id);
            }
        }
        Ok((
            PlaceMergeOutcome {
                counts,
                removed: ids.len(),
                place_records,
            },
            reached,
        ))
    }
}
