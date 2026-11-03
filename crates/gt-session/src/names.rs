//! Name display and lifespans.

use crate::dates::{Side, lifespan_text};
use crate::dto::{NameOrder, SexKind};
use gt_core::model::{Event, EventKind, Name, ParticipantRef, Person, Role, Sex};
use gt_core::tree::Tree;

pub const BIRTH_KINDS: [EventKind; 3] =
    [EventKind::Birth, EventKind::Christening, EventKind::Baptism];
pub const DEATH_KINDS: [EventKind; 3] = [EventKind::Death, EventKind::Burial, EventKind::Cremation];

/// The preferred name, else the first one.
pub fn preferred_name(p: &Person) -> Option<&Name> {
    p.names
        .iter()
        .find(|n| n.assertion.preferred)
        .or_else(|| p.names.first())
}

fn join(parts: &[Option<&str>]) -> String {
    parts
        .iter()
        .flatten()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Surname part of a name: prefix ("van der") and surnames.
pub fn surname_of(n: &Name) -> String {
    let surnames = n.surnames.join(" ");
    join(&[n.surname_prefix.as_deref(), Some(&surnames)])
}

/// Text for one name. `full` adds the prefix ("Dr.") and suffix ("Jr.") used in the name editor.
pub fn compose(n: &Name, order: NameOrder, full: bool) -> String {
    let given = n.given.as_deref().map(str::trim).unwrap_or("");
    // East Slavic names read "given patronymic surname" (Іван Петрович Коваль).
    let patronymic = n.patronymic.as_deref().map(str::trim).unwrap_or("");
    let surname = surname_of(n);
    let (prefix, suffix) = if full {
        (n.prefix.as_deref(), n.suffix.as_deref())
    } else {
        (None, None)
    };
    let text = match order {
        NameOrder::GivenFirst => join(&[
            prefix,
            Some(given),
            Some(patronymic),
            Some(&surname),
            suffix,
        ]),
        NameOrder::SurnameFirst => {
            let rest = join(&[prefix, Some(given), Some(patronymic), suffix]);
            match (surname.is_empty(), rest.is_empty()) {
                (false, false) => format!("{surname}, {rest}"),
                (false, true) => surname,
                (true, _) => rest,
            }
        }
    };
    if !text.is_empty() {
        return text;
    }
    [&n.raw, &n.nickname]
        .into_iter()
        .flatten()
        .map(|s| s.replace('/', " "))
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .find(|s| !s.is_empty())
        .unwrap_or_default()
}

/// Preferred name, else the first, else "Unknown".
pub fn display_name(p: &Person, order: NameOrder) -> String {
    preferred_name(p)
        .map(|n| compose(n, order, false))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown".to_string())
}

pub fn sex_kind(s: &Sex) -> (SexKind, Option<String>) {
    match s {
        Sex::Male => (SexKind::Male, None),
        Sex::Female => (SexKind::Female, None),
        Sex::Intersex => (SexKind::Intersex, None),
        Sex::Unknown => (SexKind::Unknown, None),
        Sex::Other(t) => (SexKind::Other, Some(t.clone())),
    }
}

pub fn is_principal(e: &Event, person: uuid::Uuid) -> bool {
    e.participants
        .iter()
        .any(|p| p.target == ParticipantRef::Person(person) && p.role == Role::Principal)
}

/// The event that best states a birth or death for `person`: a non-negated event in which they
/// are the principal, earlier kinds first, then the preferred one, then tree order.
pub fn pick_event<'a>(
    events: impl Iterator<Item = &'a Event>,
    person: uuid::Uuid,
    kinds: &[EventKind],
) -> Option<&'a Event> {
    let mut best: Option<(usize, bool, &Event)> = None;
    for e in events {
        if e.negative || !is_principal(e, person) {
            continue;
        }
        let Some(rank) = kinds.iter().position(|k| *k == e.kind) else {
            continue;
        };
        let key = (rank, !e.assertion.preferred);
        if best.is_none_or(|(r, np, _)| key < (r, np)) {
            best = Some((key.0, key.1, e));
        }
    }
    best.map(|(_, _, e)| e)
}

pub fn side_of(event: Option<&Event>) -> Side {
    match event {
        None => Side::Absent,
        Some(e) => Side::of(e.date.as_ref()),
    }
}

/// "c. 1850 – 1921", "b. 1850", "d. 1921" or "".
pub fn lifespan(tree: &Tree, p: &Person) -> String {
    let id = p.common.id;
    let birth = pick_event(tree.events_of(id).map(|(e, _)| e), id, &BIRTH_KINDS);
    let death = pick_event(tree.events_of(id).map(|(e, _)| e), id, &DEATH_KINDS);
    lifespan_text(&side_of(birth), &side_of(death))
}

#[cfg(test)]
mod patronymic_tests {
    use super::*;
    use gt_core::model::{Assertion, NameKind};

    fn name(given: Option<&str>, patronymic: &str, surname: Option<&str>) -> Name {
        Name {
            id: uuid::Uuid::nil(),
            kind: NameKind::Birth,
            assertion: Assertion {
                preferred: true,
                proof: None,
                confidence: None,
            },
            private: false,
            prefix: None,
            given: given.map(Into::into),
            call: None,
            surname_prefix: None,
            surnames: surname.map(|s| vec![s.to_string()]).unwrap_or_default(),
            patronymic: Some(patronymic.into()),
            suffix: None,
            nickname: None,
            lang: None,
            date: None,
            raw: None,
            citations: vec![],
            notes: vec![],
            unknown: vec![],
        }
    }

    #[test]
    fn patronymic_sits_between_given_name_and_surname() {
        let n = name(Some("Ivan"), "Petrovych", Some("Koval"));
        assert_eq!(
            compose(&n, NameOrder::GivenFirst, false),
            "Ivan Petrovych Koval"
        );
        assert_eq!(
            compose(&n, NameOrder::SurnameFirst, false),
            "Koval, Ivan Petrovych"
        );
    }

    #[test]
    fn patronymic_alone_still_names_the_person() {
        let n = name(None, "Petrovych", None);
        assert_eq!(compose(&n, NameOrder::GivenFirst, false), "Petrovych");
    }
}
