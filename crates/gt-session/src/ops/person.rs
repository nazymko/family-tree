//! createPerson, updatePerson, saveName, deleteName.

use super::{Planner, assertion, invalid, new_common, parse_date, sex_of, text};
use crate::command::{LivingPatch, NameInput, PersonDraft};
use crate::dto::{LivingOverrideDto, NameKindDto, SexKind};
use crate::error::ApiError;
use gt_core::ids::new_id;
use gt_core::model::*;
use uuid::Uuid;

/// A new person with one preferred birth name (not yet stored).
pub(super) fn new_person(p: &Planner, draft: &PersonDraft, sex: SexKind) -> Person {
    let surnames: Vec<String> = draft
        .surnames
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Person {
        common: new_common(p.now),
        sex: sex_of(sex, None, None),
        names: vec![Name {
            id: new_id(),
            kind: NameKind::Birth,
            assertion: assertion(true),
            private: false,
            prefix: None,
            given: text(draft.given.as_deref()),
            call: None,
            surname_prefix: None,
            surnames,
            patronymic: text(draft.patronymic.as_deref()),
            suffix: None,
            nickname: None,
            lang: None,
            date: None,
            raw: None,
            citations: Vec::new(),
            notes: Vec::new(),
            unknown: Vec::new(),
        }],
        living_override: None,
    }
}

pub(super) fn create_person(
    p: &mut Planner,
    draft: &PersonDraft,
) -> Result<Option<Uuid>, ApiError> {
    let person = new_person(p, draft, draft.sex);
    let id = person.common.id;
    p.put(person);
    super::event::add_birth(p, id, draft)?;
    Ok(Some(id))
}

pub(super) fn update_person(
    p: &mut Planner,
    id: Uuid,
    sex: Option<&SexKind>,
    sex_text: Option<&str>,
    living_override: LivingPatch,
    private: Option<bool>,
) -> Result<Option<Uuid>, ApiError> {
    let mut person: Person = p.get(id)?;
    match sex {
        Some(k) => person.sex = sex_of(*k, sex_text, Some(&person.sex)),
        None => {
            if let (Some(t), Sex::Other(_)) = (sex_text, &person.sex) {
                person.sex = Sex::Other(t.trim().to_string());
            }
        }
    }
    match living_override {
        LivingPatch::Keep => {}
        LivingPatch::Clear => person.living_override = None,
        LivingPatch::Set(LivingOverrideDto::Living) => {
            person.living_override = Some(LivingOverride::Living)
        }
        LivingPatch::Set(LivingOverrideDto::Deceased) => {
            person.living_override = Some(LivingOverride::Deceased)
        }
    }
    if let Some(v) = private {
        person.common.private = v;
    }
    person.common.changed = Some(p.now);
    p.put(person);
    Ok(Some(id))
}

fn name_kind(input: &NameInput, keep: Option<&NameKind>) -> NameKind {
    match input.kind {
        NameKindDto::Birth => NameKind::Birth,
        NameKindDto::Married => NameKind::Married,
        NameKindDto::Aka => NameKind::Aka,
        NameKindDto::Religious => NameKind::Religious,
        NameKindDto::Nickname => NameKind::Nickname,
        NameKindDto::Immigrant => NameKind::Immigrant,
        NameKindDto::Professional => NameKind::Professional,
        NameKindDto::Other => NameKind::Other(
            text(input.kind_text.as_deref())
                .or_else(|| match keep {
                    Some(NameKind::Other(t)) => Some(t.clone()),
                    _ => None,
                })
                .unwrap_or_default(),
        ),
    }
}

pub(super) fn save_name(
    p: &mut Planner,
    person_id: Uuid,
    input: &NameInput,
) -> Result<Option<Uuid>, ApiError> {
    let mut person: Person = p.get(person_id)?;
    let surnames: Vec<String> = input
        .surnames
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let given = text(input.given.as_deref());
    let patronymic = text(input.patronymic.as_deref());
    if given.is_none() && surnames.is_empty() && patronymic.is_none() {
        return Err(invalid("Enter a given name or a surname."));
    }
    let idx = match input.id {
        Some(id) => Some(
            person
                .names
                .iter()
                .position(|n| n.id == id)
                .ok_or_else(|| ApiError::not_found("name", id))?,
        ),
        None => None,
    };
    let old = idx.and_then(|i| person.names.get(i));
    let made = Name {
        id: input.id.unwrap_or_else(new_id),
        kind: name_kind(input, old.map(|n| &n.kind)),
        assertion: Assertion {
            preferred: input.preferred,
            proof: old.and_then(|n| n.assertion.proof.clone()),
            confidence: old.and_then(|n| n.assertion.confidence),
        },
        private: input.private,
        prefix: text(input.prefix.as_deref()),
        given,
        call: text(input.call.as_deref()),
        surname_prefix: text(input.surname_prefix.as_deref()),
        surnames,
        patronymic,
        suffix: text(input.suffix.as_deref()),
        nickname: text(input.nickname.as_deref()),
        lang: text(input.lang.as_deref()),
        date: parse_date(input.date_raw.as_deref(), p.order),
        raw: None,
        citations: old.map(|n| n.citations.clone()).unwrap_or_default(),
        notes: old.map(|n| n.notes.clone()).unwrap_or_default(),
        unknown: old.map(|n| n.unknown.clone()).unwrap_or_default(),
    };
    let made_id = made.id;
    let make_preferred = made.assertion.preferred;
    match idx {
        Some(i) => person.names[i] = made,
        None => person.names.push(made),
    }
    if make_preferred || person.names.len() == 1 {
        for n in &mut person.names {
            n.assertion.preferred = n.id == made_id;
        }
    } else if !person.names.iter().any(|n| n.assertion.preferred)
        && let Some(first) = person.names.first_mut()
    {
        first.assertion.preferred = true;
    }
    person.common.changed = Some(p.now);
    p.put(person);
    Ok(Some(person_id))
}

/// How an agent's number is stored: the source file is in the scheme, so that two files can share
/// a number. Both parts trimmed; `None` when either is blank.
pub(crate) fn agent_number(source_file: &str, number: &str) -> Option<ExternalId> {
    let (file, value) = (text(Some(source_file))?, text(Some(number))?);
    Some(ExternalId {
        kind: ExtIdKind::Exid {
            scheme: format!("{AGENT_NUMBER_SCHEME}{file}"),
        },
        value,
    })
}

pub(super) fn add_number(
    p: &mut Planner,
    person_id: Uuid,
    source_file: &str,
    number: &str,
) -> Result<Option<Uuid>, ApiError> {
    let id = agent_number(source_file, number)
        .ok_or_else(|| invalid("Give both the source file and the number."))?;
    let mut person: Person = p.get(person_id)?;
    if !person.common.ext_ids.contains(&id) {
        person.common.ext_ids.push(id);
        person.common.changed = Some(p.now);
        p.put(person);
    }
    Ok(Some(person_id))
}

pub(super) fn delete_name(
    p: &mut Planner,
    person_id: Uuid,
    name: Uuid,
) -> Result<Option<Uuid>, ApiError> {
    let mut person: Person = p.get(person_id)?;
    let i = person
        .names
        .iter()
        .position(|n| n.id == name)
        .ok_or_else(|| ApiError::not_found("name", name))?;
    if person.names.len() == 1 {
        return Err(invalid(
            "A person needs at least one name. Edit this name instead.",
        ));
    }
    let gone = person.names.remove(i);
    if gone.assertion.preferred
        && let Some(first) = person.names.first_mut()
    {
        first.assertion.preferred = true;
    }
    p.abandon(&gone.citations, &gone.notes, true);
    person.common.changed = Some(p.now);
    p.put(person);
    Ok(Some(person_id))
}
