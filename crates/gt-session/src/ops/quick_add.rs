//! quickAdd: a new person placed in one step relative to an existing one.

use super::person::new_person;
use super::{Planner, assertion, conflict, invalid};
use crate::command::{PersonDraft, Relation};
use crate::dto::{ParentRelId, SexKind};
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

/// The link kind when it is not by birth. Only a parent or a child can have one.
fn linked_kind(
    relation: Relation,
    rel: Option<ParentRelId>,
) -> Result<Option<ParentRel>, ApiError> {
    let kind = rel
        .map(|r| super::rel_of(r, None))
        .filter(|k| *k != ParentRel::Birth);
    if kind.is_some()
        && !matches!(
            relation,
            Relation::Father | Relation::Mother | Relation::Child
        )
    {
        return Err(invalid("Only a parent or a child can be linked this way."));
    }
    Ok(kind)
}

/// Sets how `child` is related to `parent`, one of the family's partners.
fn set_rel(f: &mut Family, child: Uuid, parent: Uuid, rel: ParentRel) {
    let first = f.partner1 == Some(parent);
    if let Some(link) = f.children.iter_mut().find(|c| c.person == child) {
        if first {
            link.rel1 = rel;
        } else {
            link.rel2 = rel;
        }
    }
}

/// A child's link to a family, as the child of `anchor` by `kind`. A stepchild is the birth child of the other
/// partner; an adopted or fostered child is usually taken in by both.
fn child_link(
    f: &Family,
    anchor: Uuid,
    child: Uuid,
    preferred: bool,
    kind: Option<ParentRel>,
) -> ChildLink {
    let mut link = birth_link(child, preferred);
    if let Some(k) = kind {
        let first = f.partner1 == Some(anchor);
        if k != ParentRel::Step {
            link.rel1 = k.clone();
            link.rel2 = k;
        } else if first {
            link.rel1 = k;
        } else {
            link.rel2 = k;
        }
    }
    link
}

/// Places `person` as a parent of `anchor` by `kind`.
///
/// A step-parent is the partner of one of the birth parents, so the birth parents stay as they are: with both known
/// the step-parent's couple becomes a second set of parents; a parent recorded alone gets the step-parent as partner
/// in the same family. Any other kind (adoptive, foster, guardian) joins a parent of the same kind who is recorded
/// alone, else starts a set of its own.
fn add_linked_parent(
    p: &mut Planner,
    anchor: Uuid,
    relation: Relation,
    person: Uuid,
    kind: ParentRel,
) -> Result<(), ApiError> {
    let same_side = if relation == Relation::Father {
        Sex::Male
    } else {
        Sex::Female
    };
    let current = parent_family(p, anchor);
    let had_parents = current.is_some();
    let step = kind == ParentRel::Step;

    // The birth parent the step-parent is partnered with: a recorded parent on the other side.
    let spouse = if step {
        let partners = current
            .iter()
            .flat_map(|f| [f.partner1, f.partner2])
            .flatten();
        let mut found = None;
        for id in partners {
            if p.get::<Person>(id)?.sex != same_side {
                found = Some(id);
                break;
            }
        }
        found
    } else {
        None
    };

    // The family with a free slot this person completes.
    let alone = |f: &Family| f.partner1.is_some() != f.partner2.is_some();
    let join: Option<Family> = if step {
        current
            .clone()
            .filter(|f| alone(f) && (f.partner1 == spouse || f.partner2 == spouse))
    } else {
        super::families_of_child(p.tree, anchor)
            .iter()
            .find(|(f, l)| {
                alone(f)
                    && (if f.partner1.is_some() {
                        &l.rel1
                    } else {
                        &l.rel2
                    }) == &kind
            })
            .map(|(f, _)| (*f).clone())
    };

    if let Some(mut f) = join {
        let other = f.partner1.or(f.partner2);
        match other.and_then(|o| couple(p, o, person).map(|t| (o, t))) {
            // The couple exists already: the child moves in, and a family left empty goes.
            Some((o, mut target)) => {
                if let Some(i) = f.children.iter().position(|c| c.person == anchor) {
                    let link = f.children.remove(i);
                    let to_other = if f.partner1 == Some(o) {
                        link.rel1.clone()
                    } else {
                        link.rel2.clone()
                    };
                    target.children.push(link);
                    set_rel(&mut target, anchor, o, to_other);
                    set_rel(&mut target, anchor, person, kind);
                }
                p.put(target);
                if f.children.is_empty() {
                    super::family::delete_family(p, f.common.id)?;
                } else {
                    p.put(f);
                }
            }
            // Nobody in this family is this person's birth child, so all its children get the same link.
            None => {
                fill_slot(&mut f, person);
                let kids: Vec<Uuid> = f.children.iter().map(|c| c.person).collect();
                for kid in kids {
                    set_rel(&mut f, kid, person, kind.clone());
                }
                p.put(f);
            }
        }
        return Ok(());
    }

    if let Some(s) = spouse {
        let mut f = couple(p, s, person).unwrap_or_else(|| new_family(p, Some(s), Some(person)));
        f.children.push(birth_link(anchor, false));
        set_rel(&mut f, anchor, person, kind);
        p.put(f);
        return Ok(());
    }

    let mut f = new_family(p, Some(person), None);
    f.children.push(birth_link(anchor, !had_parents));
    set_rel(&mut f, anchor, person, kind.clone());
    if !step {
        // Whoever joins this set later is the same kind of parent.
        if let Some(link) = f.children.first_mut() {
            link.rel2 = kind;
        }
    }
    p.put(f);
    Ok(())
}

pub(super) fn quick_add(
    p: &mut Planner,
    anchor: Uuid,
    relation: Relation,
    draft: &PersonDraft,
    family: Option<Uuid>,
    new_parent_set: bool,
    parent_rel: Option<ParentRelId>,
) -> Result<Option<Uuid>, ApiError> {
    p.get::<Person>(anchor)?;
    let kind = linked_kind(relation, parent_rel)?;
    let sex = match relation {
        Relation::Father => SexKind::Male,
        Relation::Mother => SexKind::Female,
        _ => draft.sex,
    };

    // Find the target and check everything before anything is written.
    let target: Option<Family> = match relation {
        Relation::Father | Relation::Mother if kind.is_some() => None,
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

    if let (Relation::Father | Relation::Mother, Some(k)) = (relation, kind.clone()) {
        add_linked_parent(p, anchor, relation, new_id, k)?;
        return Ok(Some(new_id));
    }

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
            let link = child_link(&f, anchor, new_id, true, kind);
            f.children.push(link);
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
    parent_rel: Option<ParentRelId>,
) -> Result<Option<Uuid>, ApiError> {
    p.get::<Person>(anchor)?;
    p.get::<Person>(person)?;
    let kind = linked_kind(relation, parent_rel)?;
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
            if let Some(k) = kind {
                add_linked_parent(p, anchor, relation, person, k)?;
                return Ok(Some(person));
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
            let link = child_link(&f, anchor, person, first, kind);
            f.children.push(link);
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
