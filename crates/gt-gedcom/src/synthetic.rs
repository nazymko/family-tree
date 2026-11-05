//! Deterministic synthetic family trees for performance tests and benchmarks.

use crate::idgen::{derive_id, det_id, place_id};
use gt_core::DayOrder;
use gt_core::date::GenDate;
use gt_core::model::*;
use gt_core::tree::Tree;
use uuid::Uuid;

const GIVEN: [&str; 16] = [
    "Anna",
    "Karl",
    "Maria",
    "Johann",
    "Elisabeth",
    "Friedrich",
    "Margarethe",
    "Heinrich",
    "Sophie",
    "Wilhelm",
    "Charlotte",
    "Ludwig",
    "Katharina",
    "August",
    "Louise",
    "Georg",
];
const SURNAME: [&str; 12] = [
    "Mueller",
    "Schmidt",
    "Schneider",
    "Fischer",
    "Weber",
    "Meyer",
    "Wagner",
    "Becker",
    "Hoffmann",
    "Schulz",
    "Koch",
    "Richter",
];
const COUNTRIES: [&str; 3] = ["Germany", "France", "Poland"];

fn common(id: Uuid) -> Common {
    Common {
        id,
        private: false,
        changed: None,
        citations: vec![],
        notes: vec![],
        media: vec![],
        ext_ids: vec![],
        unknown: vec![],
    }
}

fn assertion() -> Assertion {
    Assertion {
        preferred: false,
        proof: None,
        confidence: None,
    }
}

fn add_place(t: &mut Tree, name: &str, parent: Option<Uuid>, chain: &str) -> Uuid {
    let id = place_id(chain);
    t.places.entry(id).or_insert_with(|| Place {
        common: common(id),
        names: vec![PlaceName {
            name: name.to_string(),
            lang: None,
            date: None,
        }],
        parent,
        kind: None,
        lat: None,
        lon: None,
        raw: Some(chain.to_string()),
    });
    id
}

fn add_event(
    t: &mut Tree,
    owner: ParticipantRef,
    kind: EventKind,
    year: i32,
    place: Uuid,
    key: &[u8],
) {
    let id = det_id(key);
    t.events.insert(
        id,
        Event {
            common: common(id),
            kind,
            date: Some(GenDate::parse(&format!("{year}"), DayOrder::Dmy)),
            place: Some(place),
            description: None,
            age: None,
            cause: None,
            negative: false,
            assertion: assertion(),
            participants: vec![Participant {
                target: owner,
                role: Role::Principal,
            }],
        },
    );
}

fn pick<'a>(list: &'a [&'a str], i: usize) -> &'a str {
    list.get(i % list.len().max(1)).copied().unwrap_or("")
}

/// A deterministic tree: `persons` people with names, sex, a birth (and for every second person
/// a death) at one of 120 three-level places, and `families` couples with a marriage whose
/// children are the people after the couples.
pub fn synthetic_tree(persons: usize, families: usize) -> Tree {
    let mut t = Tree::new();
    t.header.producer = Some("SYNTHETIC".into());
    let mut leaves: Vec<Uuid> = Vec::new();
    for (ci, country) in COUNTRIES.iter().enumerate() {
        let c = add_place(&mut t, country, None, country);
        for r in 0..4 {
            let rname = format!("Region{ci}{r}");
            let rchain = format!("{rname}, {country}");
            let rid = add_place(&mut t, &rname, Some(c), &rchain);
            for k in 0..10 {
                let name = format!("Town{ci}{r}{k}");
                let chain = format!("{name}, {rchain}");
                leaves.push(add_place(&mut t, &name, Some(rid), &chain));
            }
        }
    }
    let leaf = |i: usize| leaves.get(i % leaves.len().max(1)).copied();
    let ids: Vec<Uuid> = (0..persons)
        .map(|i| det_id(format!("synthetic-person-{i}").as_bytes()))
        .collect();
    for (i, id) in ids.iter().enumerate() {
        let given = pick(&GIVEN, i);
        let sur = pick(&SURNAME, i / 7);
        let name = Name {
            id: derive_id(*id, "name", 0),
            kind: NameKind::Birth,
            assertion: Assertion {
                preferred: true,
                ..assertion()
            },
            private: false,
            prefix: None,
            given: Some(given.to_string()),
            call: None,
            surname_prefix: None,
            surnames: vec![sur.to_string()],
            patronymic: None,
            suffix: None,
            nickname: None,
            lang: None,
            date: None,
            raw: Some(format!("{given} /{sur}/")),
            citations: vec![],
            notes: vec![],
            unknown: vec![],
        };
        t.persons.insert(
            *id,
            Person {
                common: common(*id),
                sex: if i % 2 == 0 { Sex::Male } else { Sex::Female },
                names: vec![name],
                living_override: None,
                sibling_marker: false,
            },
        );
        let year = 1700 + i32::try_from(i % 250).unwrap_or(0);
        if let Some(place) = leaf(i) {
            add_event(
                &mut t,
                ParticipantRef::Person(*id),
                EventKind::Birth,
                year,
                place,
                format!("birth-{i}").as_bytes(),
            );
        }
        if i % 2 == 0
            && let Some(place) = leaf(i * 7 + 3)
        {
            add_event(
                &mut t,
                ParticipantRef::Person(*id),
                EventKind::Death,
                year + 60,
                place,
                format!("death-{i}").as_bytes(),
            );
        }
    }
    let couples = families.min(persons / 2);
    let mut next_child = couples * 2;
    for f in 0..couples {
        let fid = det_id(format!("synthetic-family-{f}").as_bytes());
        let mut children = Vec::new();
        if let Some(child) = ids.get(next_child) {
            children.push(ChildLink {
                person: *child,
                rel1: ParentRel::Birth,
                rel2: ParentRel::Birth,
                preferred: true,
                assertion: assertion(),
                citations: vec![],
                unknown: vec![],
            });
            next_child += 1;
        }
        t.families.insert(
            fid,
            Family {
                common: common(fid),
                partner1: ids.get(f * 2).copied(),
                partner2: ids.get(f * 2 + 1).copied(),
                kind: UnionKind::Married,
                children,
            },
        );
        if let Some(place) = leaf(f * 11) {
            add_event(
                &mut t,
                ParticipantRef::Family(fid),
                EventKind::Marriage,
                1720 + i32::try_from(f % 250).unwrap_or(0),
                place,
                format!("marr-{f}").as_bytes(),
            );
        }
    }
    t
}
