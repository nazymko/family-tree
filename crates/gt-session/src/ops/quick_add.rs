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
