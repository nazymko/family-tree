//! saveEvent, deleteEvent and the birth event of a new person.

use super::places::place_for_text;
use super::{Planner, assertion, invalid, new_common, parse_date, text};
use crate::command::{EventInput, EventOwner, EventOwnerKind, PersonDraft};
use crate::error::ApiError;
use gt_core::model::*;
use uuid::Uuid;

/// Every kind of event but `custom:`, by id.
const EVENT_KINDS: &str = "birth, christening, baptism, death, burial, cremation, adoption, \
    marriage, marriage_banns, marriage_contract, marriage_license, marriage_settlement, \
    engagement, divorce, divorce_filed, annulment, separation, census, residence, occupation, \
    education, religion, nationality, title, emigration, immigration, naturalization, probate, \
    will, graduation, retirement, military, physical_description";

/// `marriage_banns` -> `MarriageBanns`.
fn pascal(s: &str) -> String {
    s.split('_')
        .map(|w| {
            let mut cs = w.chars();
            cs.next()
                .map(|f| f.to_uppercase().collect::<String>() + cs.as_str())
                .unwrap_or_default()
        })
        .collect()
}

/// `custom:<text>` -> `Custom`; the other ids are the lower_snake form of the variant name.
pub fn parse_event_kind(id: &str) -> Result<EventKind, ApiError> {
    let id = id.trim();
    if let Some(t) = id.strip_prefix("custom:") {
        let t = t.trim();
        if t.is_empty() {
            return Err(invalid("Name the kind of event."));
        }
        return Ok(EventKind::Custom(t.to_string()));
    }
    serde_json::from_value(serde_json::Value::String(pascal(id))).map_err(|_| {
        invalid(format!(
            "\"{id}\" is not a kind of event. Use one of: {EVENT_KINDS}, or custom:<your words>."
        ))
    })
}

pub fn parse_role(id: &str) -> Role {
    serde_json::from_value(serde_json::Value::String(pascal(id.trim())))
        .unwrap_or_else(|_| Role::Other(id.trim().to_string()))
}

fn owner_target(owner: EventOwner) -> ParticipantRef {
    match owner.kind {
        EventOwnerKind::Person => ParticipantRef::Person(owner.id),
        EventOwnerKind::Family => ParticipantRef::Family(owner.id),
    }
}

fn check_owner(p: &Planner, owner: EventOwner) -> Result<(), ApiError> {
    match owner.kind {
        EventOwnerKind::Person => {
            if p.get::<Person>(owner.id)?.sibling_marker {
                return Err(super::invalid(
                    "The entry for unnamed brothers and sisters cannot have events.",
                ));
            }
            Ok(())
        }
        EventOwnerKind::Family => p.get::<Family>(owner.id).map(|_| ()),
    }
}

fn is_principal(e: &Event, t: &ParticipantRef) -> bool {
    e.participants
        .iter()
        .any(|x| &x.target == t && x.role == Role::Principal)
}

pub(super) fn save_event(
    p: &mut Planner,
    owner: EventOwner,
    input: &EventInput,
) -> Result<Option<Uuid>, ApiError> {
    check_owner(p, owner)?;
    let target = owner_target(owner);
    let kind = parse_event_kind(&input.kind)?;
    let old: Option<Event> = match input.id {
        Some(id) => {
            let e: Event = p.get(id)?;
            if !e.participants.iter().any(|x| x.target == target) {
                return Err(ApiError::not_found("event", id));
            }
            Some(e)
        }
        None => None,
    };

    let place = if let Some(id) = input.place_id {
        p.get::<Place>(id)?;
        Some(id)
    } else if let Some(t) = &input.place_text {
        place_for_text(p, t)?
    } else {
        old.as_ref().and_then(|e| e.place)
    };

    // Persons named in the input replace the person participants; the owner is always one of
    // the participants and family participants stay as they were.
    let mut participants: Vec<Participant> = Vec::new();
    if let Some(e) = &old {
        participants.extend(
            e.participants
                .iter()
                .filter(|x| matches!(x.target, ParticipantRef::Family(_)))
                .cloned(),
        );
    }
    match &input.participants {
        Some(list) => {
            for x in list {
                p.real_person(x.person)?;
                participants.push(Participant {
                    target: ParticipantRef::Person(x.person),
                    role: parse_role(&x.role),
                });
            }
        }
        None => {
            if let Some(e) = &old {
                participants.extend(
                    e.participants
                        .iter()
                        .filter(|x| matches!(x.target, ParticipantRef::Person(_)))
                        .cloned(),
                );
            }
        }
    }
    if !participants.iter().any(|x| x.target == target) {
        participants.insert(
            0,
            Participant {
                target,
                role: Role::Principal,
            },
        );
    }

    let mut common = old
        .as_ref()
        .map(|e| e.common.clone())
        .unwrap_or_else(|| new_common(p.now));
    common.private = input.private;
    common.changed = Some(p.now);
    let event = Event {
        common,
        kind,
        date: parse_date(input.date_raw.as_deref(), p.order),
        place,
        description: text(input.description.as_deref()),
        age: text(input.age.as_deref()),
        cause: text(input.cause.as_deref()),
        negative: input.negative,
        assertion: Assertion {
            preferred: input.preferred,
            proof: old.as_ref().and_then(|e| e.assertion.proof.clone()),
            confidence: old.as_ref().and_then(|e| e.assertion.confidence),
        },
        participants,
    };
    p.put(event);
    if owner.kind == EventOwnerKind::Person {
        p.bump_person(owner.id);
        Ok(Some(owner.id))
    } else {
        Ok(None)
    }
}

pub(super) fn delete_event(
    p: &mut Planner,
    owner: EventOwner,
    event: Uuid,
) -> Result<Option<Uuid>, ApiError> {
    check_owner(p, owner)?;
    let target = owner_target(owner);
    let mut e: Event = p.get(event)?;
    if !e.participants.iter().any(|x| x.target == target) {
        return Err(ApiError::not_found("event", event));
    }
    let others = e
        .participants
        .iter()
        .any(|x| x.role == Role::Principal && x.target != target);
    if is_principal(&e, &target) && others {
        // A shared event stays for the other principals.
        e.participants.retain(|x| x.target != target);
        p.put(e);
    } else {
        p.abandon(&e.common.citations, &e.common.notes, true);
        p.remove::<Event>(event);
    }
    if owner.kind == EventOwnerKind::Person {
        p.bump_person(owner.id);
        Ok(Some(owner.id))
    } else {
        Ok(None)
    }
}

/// Adds the Birth event a quick-add or create form asked for (date and/or place).
pub(super) fn add_birth(
    p: &mut Planner,
    person: Uuid,
    draft: &PersonDraft,
) -> Result<(), ApiError> {
    let date = parse_date(draft.birth_date_raw.as_deref(), p.order);
    let place = match draft.birth_place_text.as_deref() {
        Some(t) => place_for_text(p, t)?,
        None => None,
    };
    if date.is_none() && place.is_none() {
        return Ok(());
    }
    p.put(Event {
        common: new_common(p.now),
        kind: EventKind::Birth,
        date,
        place,
        description: None,
        age: None,
        cause: None,
        negative: false,
        assertion: assertion(true),
        participants: vec![Participant {
            target: ParticipantRef::Person(person),
            role: Role::Principal,
        }],
    });
    Ok(())
}
