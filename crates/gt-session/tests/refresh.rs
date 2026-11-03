#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Edits refresh the derived state (links, living, person index, place index) incrementally.
//! After every step of a random edit / undo / redo sequence the result must equal a fresh
//! rebuild from the tree.

mod common;

use common::*;
use gt_session::*;
use uuid::Uuid;

const GED: &str = "0 @I1@ INDI\n1 NAME Anna /Schmidt/\n1 SEX F\n1 BIRT\n2 DATE 1850\n2 PLAC Berlin, Germany\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Karl /Schmidt/\n1 SEX M\n1 BIRT\n2 DATE 1848\n1 DEAT\n2 DATE 1910\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Hans /Schmidt/\n1 SEX M\n1 BIRT\n2 DATE 1880\n2 PLAC Hamburg, Germany\n1 FAMC @F1@\n1 FAMS @F2@\n\
0 @I4@ INDI\n1 NAME Eva /Meyer/\n1 SEX F\n1 BIRT\n2 DATE 1885\n1 FAMS @F2@\n\
0 @I5@ INDI\n1 NAME Lena /Schmidt/\n1 SEX F\n1 BIRT\n2 DATE 1990\n1 FAMC @F2@\n\
0 @I6@ INDI\n1 NAME Max /Meyer/\n1 SEX M\n1 BIRT\n2 DATE 2015\n2 PLAC Paris, France\n\
0 @I7@ INDI\n1 NAME Zoe /Meyer/\n1 SEX F\n\
0 @F1@ FAM\n1 HUSB @I2@\n1 WIFE @I1@\n1 CHIL @I3@\n1 MARR\n2 DATE 1875\n2 PLAC Berlin, Germany\n\
0 @F2@ FAM\n1 HUSB @I3@\n1 WIFE @I4@\n1 CHIL @I5@\n";

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.below(v.len())]
    }
}

const DATES: [&str; 9] = [
    "1790",
    "abt 1850",
    "bef 1900",
    "aft 1700",
    "3 Apr 1950",
    "1920",
    "2020",
    "bet 1800 and 1820",
    "sometime",
];
const PLACES: [&str; 5] = [
    "Berlin, Germany",
    "Hamburg, Germany",
    "Paris, France",
    "Lyon, France",
    "Oslo",
];

fn draft(rng: &mut Rng, n: usize) -> PersonDraft {
    PersonDraft {
        given: Some(format!("New{n}")),
        surnames: vec![rng.pick(&["Schmidt", "Meyer", "Novak"]).to_string()],
        sex: *rng.pick(&[SexKind::Male, SexKind::Female, SexKind::Unknown]),
        birth_date_raw: rng.pick(&DATES).to_string().into(),
        birth_place_text: rng.pick(&PLACES).to_string().into(),
    }
}

fn event_input(rng: &mut Rng, id: Option<Uuid>) -> EventInput {
    EventInput {
        id,
        kind: rng
            .pick(&["birth", "death", "residence", "burial"])
            .to_string(),
        date_raw: Some(rng.pick(&DATES).to_string()),
        place_text: Some(rng.pick(&PLACES).to_string()),
        place_id: None,
        description: None,
        age: None,
        cause: None,
        negative: rng.below(8) == 0,
        private: false,
        preferred: rng.below(2) == 0,
        participants: None,
    }
}

fn random_command(s: &Session, rng: &mut Rng, n: usize) -> EditCommand {
    let persons: Vec<Uuid> = s.tree().persons.keys().copied().collect();
    let families: Vec<Uuid> = s.tree().families.keys().copied().collect();
    let person = |rng: &mut Rng| *rng.pick(&persons);
    match rng.below(14) {
        0 => EditCommand::CreatePerson {
            draft: draft(rng, n),
        },
        1 | 2 => EditCommand::QuickAdd {
            anchor: person(rng),
            relation: *rng.pick(&[
                Relation::Father,
                Relation::Mother,
                Relation::Partner,
                Relation::Child,
                Relation::Sibling,
            ]),
            draft: draft(rng, n),
            family: None,
            new_parent_set: None,
        },
        3 => EditCommand::UpdatePerson {
            id: person(rng),
            sex: None,
            sex_text: None,
            living_override: match rng.below(3) {
                0 => LivingPatch::Clear,
                1 => LivingPatch::Set(LivingOverrideDto::Living),
                _ => LivingPatch::Set(LivingOverrideDto::Deceased),
            },
            private: Some(rng.below(2) == 0),
        },
        4 | 5 => {
            let owner = person(rng);
            EditCommand::SaveEvent {
                owner: EventOwner {
                    kind: EventOwnerKind::Person,
                    id: owner,
                },
                event: event_input(rng, None),
            }
        }
        6 => {
            let owner = person(rng);
            let existing: Vec<Uuid> = s
                .tree()
                .events_of(owner)
                .map(|(e, _)| e.common.id)
                .collect();
            match existing.is_empty() {
                true => EditCommand::SaveEvent {
                    owner: EventOwner {
                        kind: EventOwnerKind::Person,
                        id: owner,
                    },
                    event: event_input(rng, None),
                },
                false => {
                    let id = *rng.pick(&existing);
                    if rng.below(2) == 0 {
                        EditCommand::DeleteEvent {
                            owner: EventOwner {
                                kind: EventOwnerKind::Person,
                                id: owner,
                            },
                            event: id,
                        }
                    } else {
                        EditCommand::SaveEvent {
                            owner: EventOwner {
                                kind: EventOwnerKind::Person,
                                id: owner,
                            },
                            event: event_input(rng, Some(id)),
                        }
                    }
                }
            }
        }
        7 => EditCommand::DeletePerson { id: person(rng) },
        8 => EditCommand::SaveFamily {
            id: None,
            partner1: Some(person(rng)),
            partner2: Some(person(rng)),
            kind: UnionKindId::Married,
        },
        9 if !families.is_empty() => EditCommand::LinkChild {
            family: *rng.pick(&families),
            child: person(rng),
            rel: (ParentRelId::Adopted, ParentRelId::Foster),
            preferred: rng.below(2) == 0,
        },
        10 if !families.is_empty() => EditCommand::UnlinkChild {
            family: *rng.pick(&families),
            child: person(rng),
        },
        11 if !families.is_empty() => EditCommand::DeleteFamily {
            id: *rng.pick(&families),
        },
        12 => EditCommand::SaveName {
            person: person(rng),
            name: NameInput {
                id: None,
                kind: NameKindDto::Birth,
                kind_text: None,
                preferred: rng.below(2) == 0,
                private: false,
                prefix: None,
                given: Some(format!("Alt{n}")),
                call: None,
                surname_prefix: None,
                surnames: vec!["Zed".into()],
                patronymic: None,
                suffix: None,
                nickname: None,
                date_raw: None,
                lang: None,
            },
        },
        _ => EditCommand::CreatePerson {
            draft: draft(rng, n),
        },
    }
}

fn check(s: &Session, seed: u64, step: usize, what: &str) {
    if let Err(e) = s.verify_derived_state() {
        panic!("seed {seed} step {step} ({what}): incremental state differs from rebuild: {e}");
    }
}

#[test]
fn incremental_refresh_equals_rebuild_over_random_edit_undo_redo_sequences() {
    let tree = tree_of(GED);
    let mut applied = 0;
    for seed in 1..=8u64 {
        let (_dir, mut s) = open_tree(&tree);
        check(&s, seed, 0, "open");
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        for step in 1..=120 {
            if s.tree().persons.is_empty() {
                break;
            }
            match rng.below(10) {
                0 | 1 => {
                    let _ = s.undo().unwrap();
                    check(&s, seed, step, "undo");
                }
                2 => {
                    let _ = s.redo().unwrap();
                    check(&s, seed, step, "redo");
                }
                _ => {
                    let cmd = random_command(&s, &mut rng, step);
                    let label = format!("{cmd:?}");
                    if s.apply(cmd).is_ok() {
                        applied += 1;
                    }
                    check(&s, seed, step, &label);
                }
            }
        }
        // Undo everything, then redo everything: the extremes of the stacks.
        while s.undo().unwrap().is_some() {
            check(&s, seed, 999, "undo all");
        }
        while s.redo().unwrap().is_some() {
            check(&s, seed, 999, "redo all");
        }
    }
    assert!(applied > 200, "only {applied} edits applied");
}

#[test]
fn incremental_refresh_keeps_revision_moving_and_sorts_valid() {
    let (_dir, mut s) = open_tree(&tree_of(GED));
    let before = s.revision();
    let anchor = s.tree().persons.keys().next().copied().unwrap();
    s.apply(EditCommand::QuickAdd {
        anchor,
        relation: Relation::Child,
        draft: PersonDraft {
            given: Some("Zara".into()),
            surnames: vec!["Schmidt".into()],
            sex: SexKind::Female,
            birth_date_raw: Some("abt 1850".into()),
            birth_place_text: None,
        },
        family: None,
        new_parent_set: None,
    })
    .unwrap();
    assert!(s.revision() > before);
    let page = s.list_persons(&PersonQuery::default(), 0, 200);
    assert_eq!(page.items.len(), 8);
    assert!(page.items.iter().any(|r| r.person.name.contains("Zara")));
}
