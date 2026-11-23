//! Plans for an agent's place housekeeping: a new place, and the merge of duplicate places.

use super::places::full_text;
use super::{Planner, new_common};
use crate::error::ApiError;
use gt_core::model::{Note, NoteKind, Place, PlaceName};
use gt_core::tree::Tree;
use gt_store::ChangeSet;
use std::collections::HashSet;
use uuid::Uuid;

/// What a merge repointed: the numbers an agent's merge answers with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaceMergeCounts {
    /// Events whose place moved to the kept place.
    pub events: usize,
    /// Media records whose place moved.
    pub media: usize,
    /// Places that were under a merged place and are now under the kept one.
    pub child_places: usize,
    /// Alternative names the kept place gained.
    pub names_added: usize,
    /// Notes, citations and pictures of the merged places that now belong to the kept place.
    pub notes: usize,
    pub citations: usize,
    pub pictures: usize,
}

/// A name for comparing places: trimmed, lower case, one apostrophe.
pub(crate) fn name_key(name: &str) -> String {
    gt_store::fold_search(name.trim())
}

/// The place under `parent` that already goes by `name` (any of its names), ignoring case.
pub(crate) fn find_by_name(tree: &Tree, name: &str, parent: Option<Uuid>) -> Option<Uuid> {
    let key = name_key(name);
    tree.places
        .values()
        .filter(|p| p.parent == parent)
        .filter(|p| {
            p.names.iter().any(|n| name_key(&n.name) == key)
                || p.raw.as_deref().is_some_and(|r| name_key(r) == key)
        })
        .map(|p| p.common.id)
        .min()
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

/// A new place with a name, a kind, a parent, optionally a point (decimal degrees) and a research
/// note. Returns the change set and the place's id.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_add_place(
    tree: &Tree,
    name: &str,
    kind: Option<&str>,
    parent: Option<Uuid>,
    point: Option<(f64, f64)>,
    note: Option<&str>,
    now: i64,
    order: gt_core::date::DayOrder,
) -> Result<(ChangeSet, Uuid), ApiError> {
    let mut p = Planner::new(tree, now, order);
    let mut common = new_common(now);
    let id = common.id;
    if let Some(text) = note {
        let n = research_note(text.to_string(), now);
        common.notes.push(n.common.id);
        p.put(n);
    }
    p.put(Place {
        common,
        names: vec![PlaceName {
            name: name.trim().to_string(),
            lang: None,
            date: None,
        }],
        parent,
        kind: kind.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()),
        lat: point.map(|c| c.0),
        lon: point.map(|c| c.1),
        raw: None,
    });
    Ok((p.finish("Add place".into()), id))
}

/// Merges `merge` into `keep`: what pointed at a merged place points at `keep`, the places under a
/// merged place go under `keep`, `keep` takes over the names, notes, citations and pictures it
/// lacks (and a point or kind it has none of), and the merged places are removed. A research note
/// on `keep` records what was merged. The place records of the merged places are moved by the
/// store in the same transaction (`PlaceRecordAdjust`), not here.
pub(crate) fn plan_merge(
    tree: &Tree,
    keep: Uuid,
    merge: &[Uuid],
    now: i64,
    order: gt_core::date::DayOrder,
) -> Result<(ChangeSet, PlaceMergeCounts), ApiError> {
    let mut counts = PlaceMergeCounts::default();
    let mut p = Planner::new(tree, now, order);
    let mut kept: Place = p.get(keep)?;
    let gone: HashSet<Uuid> = merge.iter().copied().collect();
    let mut described = Vec::new();
    for id in merge {
        let m: Place = p.get(*id)?;
        described.push(format!(
            "{} [{id}]",
            full_text(tree, *id).unwrap_or_default()
        ));
        for n in &m.names {
            let key = name_key(&n.name);
            if !key.is_empty() && !kept.names.iter().any(|k| name_key(&k.name) == key) {
                kept.names.push(n.clone());
                counts.names_added += 1;
            }
        }
        for n in &m.common.notes {
            if !kept.common.notes.contains(n) {
                kept.common.notes.push(*n);
                counts.notes += 1;
            }
        }
        for c in &m.common.citations {
            if !kept.common.citations.contains(c) {
                kept.common.citations.push(*c);
                counts.citations += 1;
            }
        }
        for l in &m.common.media {
            if !kept.common.media.iter().any(|k| k.media == l.media) {
                kept.common.media.push(l.clone());
                counts.pictures += 1;
            }
        }
        for e in &m.common.ext_ids {
            if !kept.common.ext_ids.contains(e) {
                kept.common.ext_ids.push(e.clone());
            }
        }
        if kept.lat.is_none() && kept.lon.is_none() {
            kept.lat = m.lat;
            kept.lon = m.lon;
        }
        if kept.kind.is_none() {
            kept.kind.clone_from(&m.kind);
        }
        p.remove::<Place>(*id);
    }
    // Everything that named a merged place names the kept one.
    for e in tree.events.values() {
        if e.place.is_some_and(|pl| gone.contains(&pl)) {
            let mut e = e.clone();
            e.place = Some(keep);
            e.common.changed = Some(now);
            p.put(e);
            counts.events += 1;
        }
    }
    for m in tree.media.values() {
        if m.place.is_some_and(|pl| gone.contains(&pl)) {
            let mut m = m.clone();
            m.place = Some(keep);
            m.common.changed = Some(now);
            p.put(m);
            counts.media += 1;
        }
    }
    for child in tree.places.values() {
        if child.parent.is_some_and(|pl| gone.contains(&pl)) && !gone.contains(&child.common.id) {
            let mut c = child.clone();
            c.parent = Some(keep);
            c.common.changed = Some(now);
            p.put(c);
            counts.child_places += 1;
        }
    }
    let n = research_note(
        format!("Merged places into this one: {}.", described.join("; ")),
        now,
    );
    kept.common.notes.push(n.common.id);
    kept.common.changed = Some(now);
    p.put(n);
    p.put(kept);
    Ok((p.finish("Merge places".into()), counts))
}
