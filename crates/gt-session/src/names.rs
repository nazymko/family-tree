//! Name display and lifespans.

use crate::dates::{Side, lifespan_text};
use crate::dto::{NameOrder, SexKind};
use gt_core::model::{Event, EventKind, Name, NameKind, ParticipantRef, Person, Role, Sex};
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
    // East Slavic names read "given patronymic surname" (Ivan Petrovych Koval).
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

/// Preferred name, else the first, else "Unknown". The word stays English here (MCP agents, exports and the
/// change log read it); the app words it in its own language in `app/src/lib/api/localize.ts`.
pub fn display_name(p: &Person, order: NameOrder) -> String {
    preferred_name(p)
        .map(|n| compose(n, order, false))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown".to_string())
}

/// The surname of the person's birth name, shown under the name when the person goes by another surname
/// (a married name is the main one). `None` when the displayed name is itself the birth name, when there is
/// no birth surname (or it is the same, compared trimmed and case-insensitively), or when the birth name is private.
pub fn birth_surname_shown(p: &Person) -> Option<String> {
    let shown = preferred_name(p)?;
    if shown.kind == NameKind::Birth {
        return None;
    }
    let birth = p
        .names
        .iter()
        .filter(|n| n.kind == NameKind::Birth && !n.private)
        .map(surname_of)
        .find(|s| !s.trim().is_empty())?;
    let birth = birth.trim();
    let shown_surname = surname_of(shown);
    if shown_surname.trim().to_lowercase() == birth.to_lowercase() {
        return None;
    }
    Some(birth.to_string())
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

    fn named(kind: NameKind, surname: &str, preferred: bool, private: bool) -> Name {
        let mut n = name(Some("Olena"), "", Some(surname));
        n.kind = kind;
        n.assertion.preferred = preferred;
        n.private = private;
        n.patronymic = None;
        n
    }

    fn person(names: Vec<Name>) -> Person {
        Person {
            common: gt_core::model::Common {
                id: uuid::Uuid::nil(),
                private: false,
                changed: None,
                citations: vec![],
                notes: vec![],
                media: vec![],
                ext_ids: vec![],
                unknown: vec![],
            },
            sex: Sex::Female,
            names,
            living_override: None,
            sibling_marker: false,
        }
    }

    #[test]
    fn birth_surname_shows_only_beside_another_main_surname() {
        let married = named(NameKind::Married, "Koval", true, false);
        let birth = named(NameKind::Birth, " Bondar ", false, false);
        // A married name is the main one: the birth surname shows, trimmed.
        let p = person(vec![birth.clone(), married.clone()]);
        assert_eq!(birth_surname_shown(&p).as_deref(), Some("Bondar"));
        // No name is marked main: the first one counts.
        let mut first = married.clone();
        first.assertion.preferred = false;
        let mut other = birth.clone();
        other.assertion.preferred = false;
        assert_eq!(
            birth_surname_shown(&person(vec![first, other])).as_deref(),
            Some("Bondar")
        );
        // The birth name is the main one.
        let mut main_birth = birth.clone();
        main_birth.assertion.preferred = true;
        let mut alt = married.clone();
        alt.assertion.preferred = false;
        assert_eq!(birth_surname_shown(&person(vec![main_birth, alt])), None);
        // The same surname in another case.
        let same = named(NameKind::Birth, "KOVAL", false, false);
        assert_eq!(
            birth_surname_shown(&person(vec![same, married.clone()])),
            None
        );
        // No birth name, an empty one, no names at all.
        assert_eq!(birth_surname_shown(&person(vec![married.clone()])), None);
        let empty = named(NameKind::Birth, "", false, false);
        assert_eq!(
            birth_surname_shown(&person(vec![empty, married.clone()])),
            None
        );
        assert_eq!(birth_surname_shown(&person(vec![])), None);
        // A private birth name stays private.
        let private = named(NameKind::Birth, "Bondar", false, true);
        assert_eq!(birth_surname_shown(&person(vec![private, married])), None);
    }
}
