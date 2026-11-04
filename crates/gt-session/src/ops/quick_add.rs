//! quickAdd: a new person placed in one step relative to an existing one.

use super::person::new_person;
use super::{Planner, assertion, conflict, invalid};
use crate::command::{PersonDraft, Relation};
use crate::dto::SexKind;
use crate::error::ApiError;
use gt_core::model::*;
use uuid::Uuid;

fn birth_link(person: Uuid, preferred: bool) -> ChildLink {
    ChildLink {
        person,
        rel1: ParentRel::Birth,
        rel2: ParentRel::Birth,
        preferred,
        assertion: assertion(false),
        citations: Vec::new(),
        unknown: Vec::new(),
    }
}

fn new_family(p: &Planner, partner1: Option<Uuid>, partner2: Option<Uuid>) -> Family {
    Family {
        common: super::new_common(p.now),
        partner1,
        partner2,
        kind: UnionKind::Unknown,
        children: Vec::new(),
    }
}

/// The family the person is a child of: the one whose link is preferred, else the first.
fn parent_family(p: &Planner, person: Uuid) -> Option<Family> {
    let all: Vec<(&Family, &ChildLink)> = super::families_of_child(p.tree, person);
    all.iter()
        .find(|(_, l)| l.preferred)
        .or_else(|| all.first())
        .map(|(f, _)| (*f).clone())
}

fn fill_slot(f: &mut Family, person: Uuid) {
    if f.partner1.is_none() {
        f.partner1 = Some(person);
    } else if f.partner2.is_none() {
        f.partner2 = Some(person);
    }
}

pub(super) fn quick_add(
    p: &mut Planner,
    anchor: Uuid,
    relation: Relation,
    draft: &PersonDraft,
    family: Option<Uuid>,
    new_parent_set: bool,
) -> Result<Option<Uuid>, ApiError> {
    p.get::<Person>(anchor)?;
    let sex = match relation {
        Relation::Father => SexKind::Male,
        Relation::Mother => SexKind::Female,
        _ => draft.sex,
    };

    // Find the target and check everything before anything is written.
    let target: Option<Family> = match relation {
        Relation::Father | Relation::Mother => match parent_family(p, anchor) {
            Some(f) if !new_parent_set => {
                if f.partner1.is_some() && f.partner2.is_some() {
                    return Err(conflict(
                        "Both parents are already recorded. Add another set of parents to record a second pair.",
                    ));
                }
                Some(f)
            }
            _ => None,
        },
        Relation::Partner => match family {
            Some(id) => {
                let f: Family = p.get(id)?;
                if f.partner1.is_some() && f.partner2.is_some() {
                    return Err(conflict("This family already has two partners."));
                }
                Some(f)
            }
            None => None,
        },
        Relation::Child => match family {
            Some(id) => {
                let f: Family = p.get(id)?;
                if f.partner1 != Some(anchor) && f.partner2 != Some(anchor) {
                    return Err(invalid("That family does not include this person."));
                }
                Some(f)
            }
            None => {
                let mut fams = p.tree.families_as_partner(anchor);
                let first = fams.next();
                if fams.next().is_some() {
                    return Err(invalid("Choose which family the child belongs to."));
                }
                first.cloned()
            }
        },
        Relation::Sibling => parent_family(p, anchor),
    };
    let had_parents = parent_family(p, anchor).is_some();

    let person = new_person(p, draft, sex);
    let new_id = person.common.id;
    p.put(person);
    super::event::add_birth(p, new_id, draft)?;

    match relation {
        Relation::Father | Relation::Mother => match target {
            Some(mut f) => {
                fill_slot(&mut f, new_id);
                p.put(f);
            }
            None => {
                let mut f = new_family(p, Some(new_id), None);
                f.children.push(birth_link(anchor, !had_parents));
                p.put(f);
            }
        },
        Relation::Partner => match target {
            Some(mut f) => {
                fill_slot(&mut f, new_id);
                p.put(f);
            }
            None => {
                let f = new_family(p, Some(anchor), Some(new_id));
                p.put(f);
            }
        },
        Relation::Child => {
            let mut f = target.unwrap_or_else(|| new_family(p, Some(anchor), None));
            f.children.push(birth_link(new_id, true));
            p.put(f);
        }
        Relation::Sibling => match target {
            Some(mut f) => {
                f.children.push(birth_link(new_id, true));
                p.put(f);
            }
            None => {
                let mut f = new_family(p, None, None);
                f.children.push(birth_link(anchor, true));
                f.children.push(birth_link(new_id, true));
                p.put(f);
            }
        },
    }
    Ok(Some(new_id))
}

/// Everyone below `person` (children, their children, ...).
fn descendants(p: &Planner, person: Uuid) -> std::collections::HashSet<Uuid> {
    let mut seen = std::collections::HashSet::new();
    let mut todo = vec![person];
    while let Some(id) = todo.pop() {
        for f in p.tree.families_as_partner(id) {
            for c in &f.children {
                if seen.insert(c.person) {
                    todo.push(c.person);
                }
            }
        }
    }
    seen
}

/// The family of exactly this couple, if there is one.
fn couple(p: &Planner, a: Uuid, b: Uuid) -> Option<Family> {
    p.tree
        .families_as_partner(a)
        .find(|f| f.partner1 == Some(b) || f.partner2 == Some(b))
        .cloned()
}

/// linkExisting: like quickAdd, but the relative is someone already in the tree. A parent who already has a
/// family with the other parent gets the child in that family (the single-parent family it leaves goes when
/// empty), so a couple is never recorded twice.
pub(super) fn link_existing(
    p: &mut Planner,
    anchor: Uuid,
    relation: Relation,
    person: Uuid,
    family: Option<Uuid>,
    new_parent_set: bool,
) -> Result<Option<Uuid>, ApiError> {
    p.get::<Person>(anchor)?;
    p.get::<Person>(person)?;
    if person == anchor {
        return Err(invalid("A person cannot be their own relative."));
    }
    match relation {
        Relation::Father | Relation::Mother => {
            if descendants(p, anchor).contains(&person) {
                return Err(invalid("A descendant cannot also be a parent."));
            }
            if super::families_of_child(p.tree, anchor)
                .iter()
                .any(|(f, _)| f.partner1 == Some(person) || f.partner2 == Some(person))
            {
                return Err(conflict("This person is already a parent."));
            }
            let current = parent_family(p, anchor);
            let had_parents = current.is_some();
            match current.filter(|_| !new_parent_set) {
                Some(f) => {
                    let other = match (f.partner1, f.partner2) {
                        (Some(_), Some(_)) => {
                            return Err(conflict(
                                "Both parents are already recorded. Add another set of parents to record a second pair.",
                            ));
                        }
                        (a, b) => a.or(b),
                    };
                    match other.and_then(|o| couple(p, o, person)) {
                        // The couple exists already: move the child into it.
                        Some(mut target) => {
                            let mut from = f;
                            if let Some(i) = from.children.iter().position(|c| c.person == anchor) {
                                let link = from.children.remove(i);
                                if !target.children.iter().any(|c| c.person == anchor) {
                                    target.children.push(link);
                                }
                            }
                            p.put(target);
                            if from.children.is_empty() {
                                super::family::delete_family(p, from.common.id)?;
                            } else {
                                p.put(from);
                            }
                        }
                        None => {
                            let mut f = f;
                            fill_slot(&mut f, person);
                            p.put(f);
                        }
                    }
                }
                None => {
                    let mut f = new_family(p, Some(person), None);
                    f.children.push(birth_link(anchor, !had_parents));
                    p.put(f);
                }
            }
        }
        Relation::Partner => {
            if couple(p, anchor, person).is_some() {
                return Err(conflict("These two are already partners."));
            }
            match family {
                Some(id) => {
                    let mut f: Family = p.get(id)?;
                    if f.partner1.is_some() && f.partner2.is_some() {
                        return Err(conflict("This family already has two partners."));
                    }
                    if f.children.iter().any(|c| c.person == person) {
                        return Err(invalid(
                            "A partner cannot also be a child of the same family.",
                        ));
                    }
                    fill_slot(&mut f, person);
                    p.put(f);
                }
                None => {
                    let f = new_family(p, Some(anchor), Some(person));
                    p.put(f);
                }
            }
        }
        Relation::Child => {
            if descendants(p, person).contains(&anchor) {
                return Err(invalid("An ancestor cannot also be a child."));
            }
            let mut f = match family {
                Some(id) => {
                    let f: Family = p.get(id)?;
                    if f.partner1 != Some(anchor) && f.partner2 != Some(anchor) {
                        return Err(invalid("That family does not include this person."));
                    }
                    f
                }
                None => {
                    let mut fams = p.tree.families_as_partner(anchor);
                    let first = fams.next().cloned();
                    if fams.next().is_some() {
                        return Err(invalid("Choose which family the child belongs to."));
                    }
                    first.unwrap_or_else(|| new_family(p, Some(anchor), None))
                }
            };
            if f.partner1 == Some(person) || f.partner2 == Some(person) {
                return Err(invalid(
                    "A partner cannot also be a child of the same family.",
                ));
            }
            if f.children.iter().any(|c| c.person == person) {
                return Err(conflict("This person is already a child in the family."));
            }
            let first = parent_family(p, person).is_none();
            f.children.push(birth_link(person, first));
            p.put(f);
        }
        Relation::Sibling => {
            let mine = parent_family(p, anchor);
            let theirs = parent_family(p, person);
            match (mine, theirs) {
                (Some(a), Some(b)) if a.common.id == b.common.id => {
                    return Err(conflict("These two are already siblings."));
                }
                (Some(mut f), theirs) => {
                    if f.partner1 == Some(person) || f.partner2 == Some(person) {
                        return Err(invalid("A parent cannot also be a sibling."));
                    }
                    f.children.push(birth_link(person, theirs.is_none()));
                    p.put(f);
                }
                (None, Some(mut f)) => {
                    if f.partner1 == Some(anchor) || f.partner2 == Some(anchor) {
                        return Err(invalid("A parent cannot also be a sibling."));
                    }
                    f.children.push(birth_link(anchor, true));
                    p.put(f);
                }
                (None, None) => {
                    let mut f = new_family(p, None, None);
                    f.children.push(birth_link(anchor, true));
                    f.children.push(birth_link(person, true));
                    p.put(f);
                }
            }
        }
    }
    Ok(Some(person))
}
