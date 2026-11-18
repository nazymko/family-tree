//! Godparents of a baptism or christening: the whole list from the event editor, one added, one
//! taken off.

use super::{Planner, invalid, text};
use crate::command::GodparentInput;
use crate::error::ApiError;
use gt_core::model::*;
use uuid::Uuid;

/// The kinds of event that have godparents.
pub fn takes_godparents(k: &EventKind) -> bool {
    matches!(k, EventKind::Baptism | EventKind::Christening)
}

fn role(id: &str) -> Result<GodparentRole, ApiError> {
    GodparentRole::parse(id).ok_or_else(|| {
        invalid(format!(
            "\"{}\" is not a godparent's role. Use godfather or godmother.",
            id.trim()
        ))
    })
}

fn short(is_name: bool, t: Option<&str>) -> Result<Option<String>, ApiError> {
    let t = text(t);
    if t.as_ref()
        .is_some_and(|t| t.chars().count() > GODPARENT_TEXT_MAX)
    {
        return Err(if is_name {
            invalid(format!(
                "A godparent's name holds up to {GODPARENT_TEXT_MAX} characters. Put the rest in \
                 the godparent's note."
            ))
        } else {
            invalid(format!(
                "A godparent's note holds up to {GODPARENT_TEXT_MAX} characters. Put the rest in a \
                 note on the event."
            ))
        });
    }
    Ok(t)
}

/// Whitespace and case do not make another name.
pub fn same_name(a: &str, b: &str) -> bool {
    let fold = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    fold(a) == fold(b)
}

/// Whether `g` is the same godparent as `other`: the same person, or the same written name.
pub fn same_godparent(g: &Godparent, other: &Godparent) -> bool {
    match (g.person, other.person) {
        (Some(a), Some(b)) => a == b,
        (None, None) => match (&g.name, &other.name) {
            (Some(a), Some(b)) => same_name(a, b),
            _ => false,
        },
        _ => false,
    }
}

/// One godparent from the input, keeping `id` when it names one the event has.
fn godparent(
    p: &Planner,
    event: &Event,
    old: &[Godparent],
    g: &GodparentInput,
) -> Result<Godparent, ApiError> {
    let name = short(true, g.name.as_deref())?;
    let person = g.person;
    match (person, &name) {
        (Some(_), Some(_)) | (None, None) => {
            return Err(invalid(
                "A godparent is either a person in the tree or a name as the record writes it: \
                 give one of the two.",
            ));
        }
        _ => {}
    }
    if let Some(id) = person {
        p.real_person(id)?;
        let principal = event
            .participants
            .iter()
            .any(|x| x.target == ParticipantRef::Person(id) && x.role == Role::Principal);
        if principal {
            return Err(invalid("Nobody is their own godparent."));
        }
    }
    let id =
        g.id.filter(|id| old.iter().any(|o| o.id == *id))
            .unwrap_or_else(gt_core::ids::new_id);
    Ok(Godparent {
        id,
        person,
        name,
        role: role(&g.role)?,
        note: short(false, g.note.as_deref())?,
    })
}

fn no_twice(list: &[Godparent]) -> Result<(), ApiError> {
    for (i, g) in list.iter().enumerate() {
        if list[..i].iter().any(|o| same_godparent(o, g)) {
            return Err(invalid("The same godparent is in the list twice."));
        }
    }
    Ok(())
}

/// The event editor's whole list, checked.
pub(super) fn godparent_list(
    p: &Planner,
    event: &Event,
    old: &[Godparent],
    input: &[GodparentInput],
) -> Result<Vec<Godparent>, ApiError> {
    let list = input
        .iter()
        .map(|g| godparent(p, event, old, g))
        .collect::<Result<Vec<_>, _>>()?;
    no_twice(&list)?;
    Ok(list)
}

/// The first person the event is about: whose `changed` stamp moves and who the edit is about.
fn principal_person(e: &Event) -> Option<Uuid> {
    e.participants.iter().find_map(|x| match x.target {
        ParticipantRef::Person(id) if x.role == Role::Principal => Some(id),
        _ => None,
    })
}

pub(super) fn add_godparent(
    p: &mut Planner,
    event: Uuid,
    input: &GodparentInput,
) -> Result<Option<Uuid>, ApiError> {
    let mut e: Event = p.get(event)?;
    if !takes_godparents(&e.kind) {
        return Err(invalid(
            "Godparents go on a baptism or christening. Add that event first, then the godparents \
             to it.",
        ));
    }
    let mut input = input.clone();
    input.id = None;
    let g = godparent(p, &e, &[], &input)?;
    if e.godparents.iter().any(|o| same_godparent(o, &g)) {
        return Err(invalid("This godparent is on the event already."));
    }
    e.godparents.push(g);
    e.common.changed = Some(p.now);
    let focus = principal_person(&e);
    p.put(e);
    if let Some(id) = focus {
        p.bump_person(id);
    }
    Ok(focus)
}

pub(super) fn remove_godparent(
    p: &mut Planner,
    event: Uuid,
    godparent: Uuid,
) -> Result<Option<Uuid>, ApiError> {
    let mut e: Event = p.get(event)?;
    let before = e.godparents.len();
    e.godparents.retain(|g| g.id != godparent);
    if e.godparents.len() == before {
        return Err(ApiError::not_found("godparent", godparent));
    }
    e.common.changed = Some(p.now);
    let focus = principal_person(&e);
    p.put(e);
    if let Some(id) = focus {
        p.bump_person(id);
    }
    Ok(focus)
}
