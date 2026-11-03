//! Places typed as comma-separated text ("Berlin, Brandenburg, Germany") become a chain of
//! place records, leaf first; chains that already exist are reused.

use super::{Planner, invalid, new_common, text};
use crate::error::ApiError;
use gt_core::model::{Place, PlaceName};
use gt_core::tree::Tree;
use std::collections::HashMap;
use uuid::Uuid;

fn name_of(p: &Place) -> String {
    p.names
        .first()
        .map(|n| n.name.clone())
        .or_else(|| p.raw.clone())
        .unwrap_or_default()
}

/// "Leaf, Parent, Root" of a stored place (a chain is short; the limit guards against cycles).
pub fn full_text(tree: &Tree, id: Uuid) -> Option<String> {
    let mut place = tree.places.get(&id)?;
    let mut parts = vec![name_of(place)];
    for _ in 0..16 {
        let Some(parent) = place.parent.and_then(|p| tree.places.get(&p)) else {
            break;
        };
        parts.push(name_of(parent));
        place = parent;
    }
    parts.retain(|p| !p.is_empty());
    Some(parts.join(", "))
}

/// The leaf place for `text`, plus the places that had to be created (root first). Matching is
/// by the full text, ignoring case, at every level.
pub fn ensure_chain(tree: &Tree, input: &str) -> Result<(Uuid, Vec<Place>), ApiError> {
    let parts: Vec<&str> = input
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return Err(invalid("Enter a place name."));
    }
    let mut known: HashMap<String, Uuid> = HashMap::new();
    for id in tree.places.keys() {
        if let Some(full) = full_text(tree, *id) {
            known.entry(full.to_lowercase()).or_insert(*id);
        }
    }
    let mut created = Vec::new();
    let mut parent: Option<Uuid> = None;
    for i in (0..parts.len()).rev() {
        let full = parts.get(i..).unwrap_or_default().join(", ");
        let id = match known.get(&full.to_lowercase()) {
            Some(id) => *id,
            None => {
                let mut common = new_common(0);
                common.changed = None;
                let place = Place {
                    common,
                    names: vec![PlaceName {
                        name: parts.get(i).copied().unwrap_or_default().to_string(),
                        lang: None,
                        date: None,
                    }],
                    parent,
                    kind: None,
                    lat: None,
                    lon: None,
                    raw: None,
                };
                let id = place.common.id;
                created.push(place);
                id
            }
        };
        parent = Some(id);
    }
    let leaf = parent.ok_or_else(|| invalid("Enter a place name."))?;
    Ok((leaf, created))
}

/// Resolves place text inside a plan: creates the missing places and returns the leaf id.
/// Blank text means "no place".
pub(super) fn place_for_text(p: &mut Planner, input: &str) -> Result<Option<Uuid>, ApiError> {
    if text(Some(input)).is_none() {
        return Ok(None);
    }
    let (leaf, created) = ensure_chain(p.tree, input)?;
    for place in created {
        p.put(place);
    }
    Ok(Some(leaf))
}
