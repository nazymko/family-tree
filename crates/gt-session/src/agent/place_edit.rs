//! Housekeeping an agent needs after its own work: delete an empty place, move a place under
//! another parent, edit a place's names, kind and point, and take back an event it made itself.
//! Each is one change set under the agent's batch, so the owner undoes it with the batch.

use super::{
    AgentCtx, AgentWrite, CORRECTION_REASON_MAX, TEXT_PIECE_NOTE_MAX, Written, people_reached,
    point_is_own, record_is_own, refused,
};
use crate::error::{ApiError, ErrorCode};
use crate::ops::places::full_text;
use crate::ops::{Planner, new_common};
use crate::session::Session;
use gt_core::model::{
    Citation, Event, Family, Note, NoteKind, ParticipantRef, Person, Place, PlaceName, Role,
};
use gt_store::{Actor, ChangeSet};
use uuid::Uuid;

const PLACE_NAME_MAX: usize = 200;
/// The most alternative names one call adds or removes.
pub const PLACE_NAMES_MAX: usize = 20;

/// What `update_place` is asked to change. Every field left out stays as it is.
#[derive(Clone, Debug, Default)]
pub struct PlaceEdit {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub add_alt_names: Vec<String>,
    pub remove_alt_names: Vec<String>,
    pub point: Option<(f64, f64)>,
    pub source: Option<String>,
}

fn key(s: &str) -> String {
    gt_store::fold_search(s.trim())
}

fn research_note(text: String, now: i64) -> Note {
    Note {
        common: new_common(now),
        kind: NoteKind::Research,
        text,
        lang: None,
        shared: false,
        story: None,
    }
}

fn clean_name(name: &str) -> Result<String, ApiError> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > PLACE_NAME_MAX {
        return Err(ApiError::new(
            ErrorCode::Invalid,
            format!("A place name is 1 to {PLACE_NAME_MAX} characters."),
        ));
    }
    Ok(n.to_string())
}

impl Session {
    fn commit_as_agent(
        &mut self,
        cs: &ChangeSet,
        ctx: &AgentCtx,
        batch: Uuid,
        focus: Option<Uuid>,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        let changed: Vec<Uuid> = cs.changes.iter().map(|c| c.id).collect();
        let mut reached = people_reached(&self.tree, &changed);
        let actor = Actor::Agent {
            id: ctx.id,
            name: ctx.name.clone(),
        };
        self.commit(cs, actor, Some(batch))?;
        for id in people_reached(&self.tree, &changed) {
            if !reached.contains(&id) {
                reached.push(id);
            }
        }
        let result = self.result(cs, cs.label.clone(), focus);
        Ok((AgentWrite::Done(result), reached))
    }

    /// Deletes a place that nothing refers to. Refused, with the counts, when an event, a place
    /// record from a book, a picture, a citation or a place under it still refers to the place.
    /// Its own notes go with it, and an undo brings it back.
    pub fn delete_place_as_agent(
        &mut self,
        place: Uuid,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        let Some(p) = self.tree.places.get(&place).cloned() else {
            return Err(ApiError::not_found("place", place));
        };
        let events = self
            .tree
            .events
            .values()
            .filter(|e| e.place == Some(place))
            .count();
        let media = self
            .tree
            .media
            .values()
            .filter(|m| m.place == Some(place))
            .count()
            + p.common.media.len();
        let children = self
            .tree
            .places
            .values()
            .filter(|c| c.parent == Some(place))
            .count();
        let records = self.project.count_place_records(&[place])?;
        let citations = p.common.citations.len();
        if events + media + children + citations > 0 || records > 0 {
            return Err(refused(format!(
                "This place is not empty, so it stays: {events} events, {records} place records, \
                 {media} pictures, {citations} citations and {children} places under it refer to \
                 it. Merge it into the right place with merge_places, or leave it and list it in \
                 your report."
            )));
        }
        let (_, now) = (self.clock.0)();
        let mut pl = Planner::new(&self.tree, now, self.day_order());
        pl.abandon(&[], &p.common.notes, false);
        pl.remove::<Place>(place);
        let cs = pl.finish("Delete place".into());
        self.commit_as_agent(&cs, ctx, batch, None)
    }

    /// Puts a place under another parent (or at the top). Refused when the new parent is the
    /// place itself or lies under it.
    pub fn move_place_as_agent(
        &mut self,
        place: Uuid,
        new_parent: Option<Uuid>,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        let Some(p) = self.tree.places.get(&place).cloned() else {
            return Err(ApiError::not_found("place", place));
        };
        if let Some(np) = new_parent {
            if !self.tree.places.contains_key(&np) {
                return Err(ApiError::not_found("place", np));
            }
            // The new parent's chain upwards may not run through this place (a chain is short;
            // the limit guards against a cycle already stored).
            let mut at = Some(np);
            for _ in 0..64 {
                let Some(id) = at else { break };
                if id == place {
                    return Err(refused(
                        "A place cannot go under itself or under a place below it. Pick a parent \
                         that is not inside this place.",
                    ));
                }
                at = self.tree.places.get(&id).and_then(|x| x.parent);
            }
        }
        if p.parent == new_parent {
            return Ok((AgentWrite::AlreadyThere { record: place }, Vec::new()));
        }
        let (_, now) = (self.clock.0)();
        let mut pl = Planner::new(&self.tree, now, self.day_order());
        let mut moved = p;
        moved.parent = new_parent;
        moved.common.changed = Some(now);
        pl.put(moved);
        let cs = pl.finish("Move place".into());
        self.commit_as_agent(&cs, ctx, batch, Some(place))
    }

    /// Edits a place's names, kind and point. `name` renames the primary name (the first); the
    /// alternative names are added and removed by `add_alt_names` and `remove_alt_names`; the
    /// primary name is never removed. A point needs its `source` (kept in a research note) and may
    /// replace only a point the agent itself set.
    pub fn update_place_as_agent(
        &mut self,
        place: Uuid,
        edit: &PlaceEdit,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        let Some(old) = self.tree.places.get(&place).cloned() else {
            return Err(ApiError::not_found("place", place));
        };
        if edit.add_alt_names.len() > PLACE_NAMES_MAX
            || edit.remove_alt_names.len() > PLACE_NAMES_MAX
        {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                format!("Add or remove at most {PLACE_NAMES_MAX} names in one call."),
            ));
        }
        let mut p = old.clone();
        let primary = p.names.first().map(|n| key(&n.name)).unwrap_or_default();
        let renamed = edit.name.as_deref().is_some_and(|n| key(n) != primary);
        // Removals first, so that a name can be removed and a new one added in the same call.
        for gone in &edit.remove_alt_names {
            let k = key(gone);
            if k.is_empty() {
                continue;
            }
            if k == primary && !renamed {
                return Err(refused(
                    "The primary name cannot be removed. Rename the place with name, then remove \
                     the old one.",
                ));
            }
            let before = p.names.len();
            let mut i = 0;
            p.names.retain(|n| {
                let drop = i > 0 && key(&n.name) == k;
                i += 1;
                !drop
            });
            if p.names.len() == before && k != primary {
                return Err(refused(format!(
                    "The place has no alternative name \"{}\". Its names are: {}.",
                    gone.trim(),
                    old.names
                        .iter()
                        .map(|n| n.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }
        }
        if let Some(n) = &edit.name {
            let n = clean_name(n)?;
            let k = key(&n);
            // The same name as an alternative one makes that one the primary.
            let mut i = 0;
            p.names.retain(|x| {
                let dup = i > 0 && key(&x.name) == k;
                i += 1;
                !dup
            });
            match p.names.first_mut() {
                Some(first) => first.name = n,
                None => p.names.push(PlaceName {
                    name: n,
                    lang: None,
                    date: None,
                }),
            }
        }
        for n in &edit.add_alt_names {
            let n = clean_name(n)?;
            if !p.names.iter().any(|x| key(&x.name) == key(&n)) {
                p.names.push(PlaceName {
                    name: n,
                    lang: None,
                    date: None,
                });
            }
        }
        if let Some(k) = &edit.kind {
            let k = k.trim();
            if k.is_empty() || k.chars().count() > PLACE_NAME_MAX {
                return Err(ApiError::new(
                    ErrorCode::Invalid,
                    "type is a short word such as village, town, district, region.",
                ));
            }
            p.kind = Some(k.to_string());
        }
        let (_, now) = (self.clock.0)();
        let mut extra: Option<Note> = None;
        if let Some((lat, lon)) = edit.point {
            if !lat.is_finite()
                || !lon.is_finite()
                || !(-90.0..=90.0).contains(&lat)
                || !(-180.0..=180.0).contains(&lon)
            {
                return Err(ApiError::new(
                    ErrorCode::Invalid,
                    "lat must be from -90 to 90 and lon from -180 to 180.",
                ));
            }
            let source = edit
                .source
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    refused(
                        "Give the source of the coordinates, such as GeoNames 702550 or the \
                         Wikipedia article of the village.",
                    )
                })?;
            if source.chars().count() > CORRECTION_REASON_MAX || source.len() > TEXT_PIECE_NOTE_MAX
            {
                return Err(refused("The source holds up to 1000 characters."));
            }
            let (lat, lon) = ((lat * 1e6).round() / 1e6, (lon * 1e6).round() / 1e6);
            let same = old.lat == Some(lat) && old.lon == Some(lon);
            if !same {
                if old.lat.is_some() || old.lon.is_some() {
                    let rows: Vec<Written> = self
                        .project
                        .last_writers(place)?
                        .into_iter()
                        .map(|(by, _)| by)
                        .zip(self.project.rows_of(place)?)
                        .collect();
                    if !point_is_own(&rows, ctx.id) {
                        return Err(refused(
                            "This place already has coordinates that the owner or someone else \
                             set, so they stay as they are. Put yours and their source in a note \
                             and list it in your report.",
                        ));
                    }
                }
                p.lat = Some(lat);
                p.lon = Some(lon);
                extra = Some(research_note(
                    format!(
                        "Coordinates {lat:.6}, {lon:.6} (latitude, longitude). Source: {source}"
                    ),
                    now,
                ));
            }
        } else if edit.source.is_some() {
            return Err(refused("source goes with lat and lon."));
        }
        if p == old {
            return Ok((AgentWrite::AlreadyThere { record: place }, Vec::new()));
        }
        let mut pl = Planner::new(&self.tree, now, self.day_order());
        if let Some(n) = extra {
            p.common.notes.push(n.common.id);
            pl.put(n);
        }
        p.common.changed = Some(now);
        pl.put(p);
        let cs = pl.finish("Update place".into());
        self.commit_as_agent(&cs, ctx, batch, Some(place))
    }

    /// Takes back an event this connection created, in this batch or an earlier one, that nobody
    /// has changed since (`record_is_own`, the test every `update_own_*` edit uses). The event goes
    /// with its citations and notes (those nothing else uses), and a research note on each person
    /// or family it was about records what was removed and why, in the same change, so one undo
    /// brings back all of it. An event the owner or anyone else made is refused. A removal that
    /// would change whether someone counts as alive or dead is refused as everywhere.
    pub fn remove_own_event(
        &mut self,
        event: Uuid,
        reason: &str,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(refused(
                "Give the reason: why this event is wrong and who or what showed it.",
            ));
        }
        if reason.chars().count() > CORRECTION_REASON_MAX {
            return Err(refused(
                "The reason holds up to 1000 characters: put the rest in a note.",
            ));
        }
        let Some(e) = self.tree.events.get(&event).cloned() else {
            // Gone now but made by this connection: removed already.
            if self
                .project
                .last_writers(event)?
                .first()
                .is_some_and(|(by, op)| *by == Some(ctx.id) && *op == gt_store::ChangeOp::Create)
            {
                return Ok((AgentWrite::AlreadyThere { record: event }, Vec::new()));
            }
            return Err(ApiError::not_found("event", event));
        };
        let rows: Vec<Written> = self
            .project
            .last_writers(event)?
            .into_iter()
            .map(|(by, _)| by)
            .zip(self.project.rows_of(event)?)
            .collect();
        if !record_is_own(&rows, ctx.id) {
            return Err(refused(
                "Only an event you made yourself, which nobody has changed since, can be taken \
                 back. An event the owner, an import or another connection made stays: leave a \
                 note and list it in your report.",
            ));
        }
        let (_, now) = (self.clock.0)();
        let place = e
            .place
            .and_then(|p| full_text(&self.tree, p))
            .map(|t| format!(" at {t}"))
            .unwrap_or_default();
        let text = format!(
            "Removed the {:?} event{place} (event {event}), which an agent had entered. Reason: {reason}",
            e.kind
        );
        let mut pl = Planner::new(&self.tree, now, self.day_order());
        pl.abandon(&e.common.citations, &e.common.notes, true);
        pl.remove::<Event>(event);
        let mut targets: Vec<ParticipantRef> = e
            .participants
            .iter()
            .filter(|x| x.role == Role::Principal)
            .map(|x| x.target.clone())
            .collect();
        if targets.is_empty() {
            targets = e.participants.iter().map(|x| x.target.clone()).collect();
        }
        let mut focus = None;
        for t in targets {
            let n = research_note(text.clone(), now);
            match t {
                ParticipantRef::Person(id) => {
                    let Some(mut p) = pl.find::<Person>(id) else {
                        continue;
                    };
                    p.common.notes.push(n.common.id);
                    p.common.changed = Some(now);
                    pl.put(p);
                    focus = focus.or(Some(id));
                }
                ParticipantRef::Family(id) => {
                    let Some(mut f) = pl.find::<Family>(id) else {
                        continue;
                    };
                    f.common.notes.push(n.common.id);
                    f.common.changed = Some(now);
                    pl.put(f);
                    focus = focus.or(Some(id));
                }
            }
            pl.put(n);
        }
        let cs = pl.finish("Remove event".into());
        self.check_living(&cs)?;
        self.commit_as_agent(&cs, ctx, batch, focus)
    }

    /// Takes back a citation this connection created, in this batch or an earlier one, that nobody
    /// has changed since (`record_is_own`). The citation is taken off every record that holds it
    /// (persons, names, families, child links, events, places, notes and the rest) and deleted;
    /// its source stays untouched. A research note on each person or family that held it records
    /// what was removed and why, in the same change, so one undo brings all of it back. A citation
    /// the owner or anyone else made is refused.
    pub fn remove_own_citation(
        &mut self,
        citation: Uuid,
        reason: &str,
        ctx: &AgentCtx,
        batch: Uuid,
    ) -> Result<(AgentWrite, Vec<Uuid>), ApiError> {
        self.check_batch(ctx, batch)?;
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(refused(
                "Give the reason: why this citation is wrong and who or what showed it.",
            ));
        }
        if reason.chars().count() > CORRECTION_REASON_MAX {
            return Err(refused(
                "The reason holds up to 1000 characters: put the rest in a note.",
            ));
        }
        let Some(c) = self.tree.citations.get(&citation).cloned() else {
            if self
                .project
                .last_writers(citation)?
                .first()
                .is_some_and(|(by, op)| *by == Some(ctx.id) && *op == gt_store::ChangeOp::Create)
            {
                return Ok((AgentWrite::AlreadyThere { record: citation }, Vec::new()));
            }
            return Err(ApiError::not_found("citation", citation));
        };
        let rows: Vec<Written> = self
            .project
            .last_writers(citation)?
            .into_iter()
            .map(|(by, _)| by)
            .zip(self.project.rows_of(citation)?)
            .collect();
        if !record_is_own(&rows, ctx.id) {
            return Err(refused(
                "Only a citation you made yourself, which nobody has changed since, can be                  taken back. A citation the owner, an import or another connection made                  stays: leave a note and list it in your report.",
            ));
        }
        let (_, now) = (self.clock.0)();
        let of_source = c
            .source
            .map(|s| format!(" of source {s}"))
            .unwrap_or_default();
        let text = format!(
            "Removed the citation{of_source} (citation {citation}), which an agent had entered.              Reason: {reason}"
        );
        let mut pl = Planner::new(&self.tree, now, self.day_order());
        let mut focus = None;
        let mut noted: Vec<Uuid> = Vec::new();
        let tree = &self.tree;
        macro_rules! strip {
            ($map:ident, $ty:ty) => {
                for (id, r) in &tree.$map {
                    if r.common.citations.contains(&citation) {
                        let mut r: $ty = r.clone();
                        r.common.citations.retain(|x| *x != citation);
                        r.common.changed = Some(now);
                        pl.put(r);
                        let _ = id;
                    }
                }
            };
        }
        for (id, p) in &tree.persons {
            let on_person = p.common.citations.contains(&citation);
            let on_name = p.names.iter().any(|n| n.citations.contains(&citation));
            if on_person || on_name {
                let mut p = p.clone();
                p.common.citations.retain(|x| *x != citation);
                for n in &mut p.names {
                    n.citations.retain(|x| *x != citation);
                }
                let n = research_note(text.clone(), now);
                p.common.notes.push(n.common.id);
                p.common.changed = Some(now);
                pl.put(n);
                pl.put(p);
                noted.push(*id);
                focus = focus.or(Some(*id));
            }
        }
        for (id, f) in &tree.families {
            let on_family = f.common.citations.contains(&citation);
            let on_child = f.children.iter().any(|ch| ch.citations.contains(&citation));
            if on_family || on_child {
                let mut f = f.clone();
                f.common.citations.retain(|x| *x != citation);
                for ch in &mut f.children {
                    ch.citations.retain(|x| *x != citation);
                }
                let n = research_note(text.clone(), now);
                f.common.notes.push(n.common.id);
                f.common.changed = Some(now);
                pl.put(n);
                pl.put(f);
                noted.push(*id);
                focus = focus.or(Some(*id));
            }
        }
        strip!(events, Event);
        strip!(places, Place);
        strip!(sources, gt_core::model::Source);
        strip!(repositories, gt_core::model::Repository);
        strip!(media, gt_core::model::Media);
        strip!(notes, Note);
        for (id, r) in &tree.citations {
            if *id != citation && r.common.citations.contains(&citation) {
                let mut r: Citation = r.clone();
                r.common.citations.retain(|x| *x != citation);
                pl.put(r);
            }
        }
        pl.abandon(&[], &c.common.notes, true);
        pl.remove::<Citation>(citation);
        let cs = pl.finish("Remove citation".into());
        self.commit_as_agent(&cs, ctx, batch, focus.or(Some(citation)))
    }
}
