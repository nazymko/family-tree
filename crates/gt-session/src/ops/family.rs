//! saveFamily, deleteFamily and the child links of a family.

use super::{Planner, assertion, conflict, invalid, rel_of, union_of};
use crate::dto::{ParentRelId, UnionKindId};
use crate::error::ApiError;
use gt_core::model::*;
use std::collections::HashSet;
use uuid::Uuid;

pub(super) fn save_family(
    p: &mut Planner,
    id: Option<Uuid>,
    partner1: Option<Uuid>,
    partner2: Option<Uuid>,
    kind: UnionKindId,
) -> Result<Option<Uuid>, ApiError> {
    if partner1.is_some() && partner1 == partner2 {
        return Err(invalid("A family needs two different people."));
    }
    for partner in [partner1, partner2].into_iter().flatten() {
        p.real_person(partner)?;
    }
    let mut fam: Family = match id {
        Some(id) => p.get(id)?,
        None => Family {
            common: super::new_common(p.now),
            partner1: None,
            partner2: None,
            kind: UnionKind::Unknown,
            children: Vec::new(),
        },
    };
    if fam
        .children
        .iter()
        .any(|c| Some(c.person) == partner1 || Some(c.person) == partner2)
    {
        return Err(invalid(
            "A partner cannot also be a child of the same family. Unlink the child first.",
        ));
    }
    fam.partner1 = partner1;
    fam.partner2 = partner2;
    fam.kind = union_of(kind, Some(&fam.kind));
    fam.common.changed = Some(p.now);
    p.put(fam);
    Ok(None)
}

pub(super) fn delete_family(p: &mut Planner, id: Uuid) -> Result<Option<Uuid>, ApiError> {
    let fam: Family = p.get(id)?;
    p.abandon(&fam.common.citations, &fam.common.notes, true);
    for c in &fam.children {
        p.abandon(&c.citations, &[], true);
        remove_if_marker(p, c.person);
    }
    p.remove::<Family>(id);
    super::delete::prune_events(p, &HashSet::new(), &HashSet::from([id]));
    Ok(None)
}

pub(super) fn link_child(
    p: &mut Planner,
    family: Uuid,
    child: Uuid,
    rel: (ParentRelId, ParentRelId),
    preferred: bool,
) -> Result<Option<Uuid>, ApiError> {
    let mut fam: Family = p.get(family)?;
    p.real_person(child)?;
    if fam.partner1 == Some(child) || fam.partner2 == Some(child) {
        return Err(invalid("A person cannot be their own child."));
    }
    if fam.children.iter().any(|c| c.person == child) {
        return Err(conflict("This person is already a child in the family."));
    }
    fam.children.push(ChildLink {
        person: child,
        rel1: rel_of(rel.0, None),
        rel2: rel_of(rel.1, None),
        preferred,
        assertion: assertion(false),
        citations: Vec::new(),
        unknown: Vec::new(),
    });
    p.put(fam);
    if preferred {
        unset_preferred_elsewhere(p, child, family);
    }
    Ok(Some(child))
}

/// Adds the unnamed-siblings marker to `family`'s children: a person with no name and no
/// events, at most one per family.
pub(super) fn add_sibling_marker(p: &mut Planner, family: Uuid) -> Result<Option<Uuid>, ApiError> {
    let mut fam: Family = p.get(family)?;
    if has_marker_child(p, &fam) {
        return Err(conflict(
            "This family already has an entry for unnamed brothers and sisters.",
        ));
    }
    Ok(Some(put_marker(p, &mut fam)))
}

/// A family with nobody in it as a partner, for a child whose parents are not known, with that
/// child's unnamed brothers and sisters. The family it makes, and the marker.
pub(super) fn add_family_of_child_with_marker(
    p: &mut Planner,
    child: Uuid,
) -> (Uuid, Option<Uuid>) {
    let mut fam = Family {
        common: super::new_common(p.now),
        partner1: None,
        partner2: None,
        kind: UnionKind::Unknown,
        children: vec![ChildLink {
            person: child,
            rel1: rel_of(ParentRelId::Birth, None),
            rel2: rel_of(ParentRelId::Birth, None),
            preferred: super::families_of_child(p.tree, child).is_empty(),
            assertion: assertion(false),
            citations: Vec::new(),
            unknown: Vec::new(),
        }],
    };
    let id = fam.common.id;
    let marker = put_marker(p, &mut fam);
    (id, Some(marker))
}

fn put_marker(p: &mut Planner, fam: &mut Family) -> Uuid {
    let mut common = super::new_common(p.now);
    let id = common.id;
    common.changed = Some(p.now);
    p.put(Person {
        common,
        sex: Sex::Unknown,
        names: Vec::new(),
        living_override: None,
        sibling_marker: true,
    });
    fam.children.push(ChildLink {
        person: id,
        rel1: rel_of(ParentRelId::Birth, None),
        rel2: rel_of(ParentRelId::Birth, None),
        preferred: true,
        assertion: assertion(false),
        citations: Vec::new(),
        unknown: Vec::new(),
    });
    fam.common.changed = Some(p.now);
    p.put(fam.clone());
    id
}

/// An unnamed-siblings marker exists only as a child of one family: it goes with the link.
pub(super) fn remove_if_marker(p: &mut Planner, person: Uuid) {
    if p.find::<Person>(person).is_some_and(|x| x.sibling_marker) {
        p.remove::<Person>(person);
    }
}

fn has_marker_child(p: &Planner, fam: &Family) -> bool {
    fam.children.iter().any(|c| {
        p.tree
            .persons
            .get(&c.person)
            .is_some_and(|x| x.sibling_marker)
    })
}

pub(super) fn update_child_link(
    p: &mut Planner,
    family: Uuid,
    child: Uuid,
    rel: (ParentRelId, ParentRelId),
    preferred: bool,
) -> Result<Option<Uuid>, ApiError> {
    let mut fam: Family = p.get(family)?;
    let link = fam
        .children
        .iter_mut()
        .find(|c| c.person == child)
        .ok_or_else(|| ApiError::not_found("child link", child))?;
    link.rel1 = rel_of(rel.0, Some(&link.rel1));
    link.rel2 = rel_of(rel.1, Some(&link.rel2));
    link.preferred = preferred;
    p.put(fam);
    if preferred {
        unset_preferred_elsewhere(p, child, family);
    }
    Ok(Some(child))
}

pub(super) fn unlink_child(
    p: &mut Planner,
    family: Uuid,
    child: Uuid,
) -> Result<Option<Uuid>, ApiError> {
    let mut fam: Family = p.get(family)?;
    let i = fam
        .children
        .iter()
        .position(|c| c.person == child)
        .ok_or_else(|| ApiError::not_found("child link", child))?;
    let gone = fam.children.remove(i);
    p.abandon(&gone.citations, &[], true);
    remove_if_marker(p, child);
    p.put(fam);
    Ok(Some(child))
}

pub(super) fn reorder_children(
    p: &mut Planner,
    family: Uuid,
    order: &[Uuid],
) -> Result<Option<Uuid>, ApiError> {
    let mut fam: Family = p.get(family)?;
    let have: HashSet<Uuid> = fam.children.iter().map(|c| c.person).collect();
    let given: HashSet<Uuid> = order.iter().copied().collect();
    if order.len() != have.len() || given != have {
        return Err(invalid(
            "The new order must list each child of the family exactly once.",
        ));
    }
    let mut old = std::mem::take(&mut fam.children);
    for id in order {
        if let Some(i) = old.iter().position(|c| c.person == *id) {
            fam.children.push(old.remove(i));
        }
    }
    p.put(fam);
    Ok(None)
}

/// A child has one preferred parent set: setting it here clears it in the other families.
fn unset_preferred_elsewhere(p: &mut Planner, child: Uuid, keep: Uuid) {
    let others: Vec<Uuid> = p
        .tree
        .families_as_child(child)
        .filter(|(f, l)| f.common.id != keep && l.preferred)
        .map(|(f, _)| f.common.id)
        .collect();
    for fid in others {
        if let Some(mut f) = p.find::<Family>(fid) {
            for l in &mut f.children {
                if l.person == child {
                    l.preferred = false;
                }
            }
            p.put(f);
        }
    }
}
