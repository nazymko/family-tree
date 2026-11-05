//! deletePerson, and the event clean-up it shares with deleteFamily.

use super::Planner;
use crate::error::ApiError;
use gt_core::model::*;
use std::collections::HashSet;
use uuid::Uuid;

/// Removes the given persons and families from every event. An event that had a principal and
/// has none left is deleted (with its orphaned notes and citations); any other event just
/// loses those participants.
pub(super) fn prune_events(p: &mut Planner, persons: &HashSet<Uuid>, families: &HashSet<Uuid>) {
    let gone = |x: &Participant| match x.target {
        ParticipantRef::Person(id) => persons.contains(&id),
        ParticipantRef::Family(id) => families.contains(&id),
    };
    let hits: Vec<Event> = p
        .tree
        .events
        .values()
        .filter(|e| e.participants.iter().any(gone))
        .cloned()
        .collect();
    for mut e in hits {
        let had_principal = e.participants.iter().any(|x| x.role == Role::Principal);
        e.participants.retain(|x| !gone(x));
        let has_principal = e.participants.iter().any(|x| x.role == Role::Principal);
        if e.participants.is_empty() || (had_principal && !has_principal) {
            p.abandon(&e.common.citations, &e.common.notes, true);
            p.remove::<Event>(e.common.id);
        } else {
            p.put(e);
        }
    }
}

pub(super) fn delete_person(p: &mut Planner, id: Uuid) -> Result<Option<Uuid>, ApiError> {
    let person: Person = p.get(id)?;
    let mut dead_families: HashSet<Uuid> = HashSet::new();
    let families: Vec<Family> = p
        .tree
        .families
        .values()
        .filter(|f| {
            f.partner1 == Some(id)
                || f.partner2 == Some(id)
                || f.children.iter().any(|c| c.person == id)
        })
        .cloned()
        .collect();
    for mut f in families {
        if f.partner1 == Some(id) {
            f.partner1 = None;
        }
        if f.partner2 == Some(id) {
            f.partner2 = None;
        }
        for link in f.children.iter().filter(|c| c.person == id) {
            p.abandon(&link.citations, &[], true);
        }
        f.children.retain(|c| c.person != id);
        // A family left with only an unnamed-siblings marker has nobody to belong to.
        let only_markers = f.children.iter().all(|c| {
            p.tree
                .persons
                .get(&c.person)
                .is_some_and(|x| x.sibling_marker)
        });
        if f.partner1.is_none() && f.partner2.is_none() && only_markers {
            for c in &f.children {
                super::family::remove_if_marker(p, c.person);
            }
            p.abandon(&f.common.citations, &f.common.notes, true);
            dead_families.insert(f.common.id);
            p.remove::<Family>(f.common.id);
        } else {
            p.put(f);
        }
    }
    prune_events(p, &HashSet::from([id]), &dead_families);
    p.abandon(&person.common.citations, &person.common.notes, true);
    for n in &person.names {
        p.abandon(&n.citations, &n.notes, true);
    }
    p.remove::<Person>(id);
    Ok(None)
}
