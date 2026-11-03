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
    if rng.below(3) == 0
        && let Some(c) = extended_command(s, rng, n)
    {
        return c;
    }
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

fn owner_ref(kind: OwnerKind, id: Uuid) -> OwnerRef {
    OwnerRef { kind, id }
}

type OwnerInfo = (OwnerRef, Vec<Uuid>, Vec<Uuid>, Vec<Uuid>);

/// A random record that can own notes, citations or media: a person, family, event or name.
/// Returns the owner with its citation, note and media ids.
fn random_owner(s: &Session, rng: &mut Rng) -> Option<OwnerInfo> {
    let t = s.tree();
    let (kind, id, c) = match rng.below(4) {
        0 => {
            let p = t.persons.values().nth(rng.below(t.persons.len().max(1)))?;
            (OwnerKind::Person, p.common.id, &p.common)
        }
        1 => {
            let f = t
                .families
                .values()
                .nth(rng.below(t.families.len().max(1)))?;
            (OwnerKind::Family, f.common.id, &f.common)
        }
        2 => {
            let e = t.events.values().nth(rng.below(t.events.len().max(1)))?;
            (OwnerKind::Event, e.common.id, &e.common)
        }
        _ => {
            let p = t.persons.values().nth(rng.below(t.persons.len().max(1)))?;
            let n = p.names.get(rng.below(p.names.len().max(1)))?;
            return Some((
                owner_ref(OwnerKind::Name, n.id),
                n.citations.clone(),
                n.notes.clone(),
                Vec::new(),
            ));
        }
    };
    Some((
        owner_ref(kind, id),
        c.citations.clone(),
        c.notes.clone(),
        c.media.iter().map(|m| m.media).collect(),
    ))
}

fn cite_input(rng: &mut Rng, s: &Session) -> CitationInput {
    let sources: Vec<Uuid> = s.tree().sources.keys().copied().collect();
    let source = if sources.is_empty() || rng.below(3) == 0 {
        SourceRef::New(NewSource {
            title: format!("Book {}", rng.below(3)),
        })
    } else {
        SourceRef::Id(*rng.pick(&sources))
    };
    CitationInput {
        source,
        page: Some(format!("p. {}", rng.below(90))),
        date_raw: None,
        text: None,
        quality: Some(rng.below(4) as u8),
        evidence: None,
    }
}

fn nth_key<T>(m: &indexmap::IndexMap<Uuid, T>, rng: &mut Rng) -> Option<Uuid> {
    m.keys().nth(rng.below(m.len().max(1))).copied()
}

fn extended_command(s: &Session, rng: &mut Rng, n: usize) -> Option<EditCommand> {
    let t = s.tree();
    let persons: Vec<Uuid> = t.persons.keys().copied().collect();
    let (owner, cites, notes, media) = random_owner(s, rng)?;
    Some(match rng.below(15) {
        0 => {
            let p = t.persons.get(&nth_key(&t.persons, rng)?)?;
            EditCommand::DeleteName {
                person: p.common.id,
                name: p.names.get(rng.below(p.names.len().max(1)))?.id,
            }
        }
        1 => {
            let f = t.families.get(&nth_key(&t.families, rng)?)?;
            let c = f.children.get(rng.below(f.children.len().max(1)))?;
            EditCommand::UpdateChildLink {
                family: f.common.id,
                child: c.person,
                rel: (
                    *rng.pick(&[
                        ParentRelId::Adopted,
                        ParentRelId::Foster,
                        ParentRelId::Birth,
                    ]),
                    ParentRelId::Birth,
                ),
                preferred: rng.below(2) == 0,
            }
        }
        2 => {
            let f = t.families.get(&nth_key(&t.families, rng)?)?;
            let mut order: Vec<Uuid> = f.children.iter().map(|c| c.person).collect();
            for i in (1..order.len()).rev() {
                order.swap(i, rng.below(i + 1));
            }
            EditCommand::ReorderChildren {
                family: f.common.id,
                order,
            }
        }
        3 => EditCommand::SaveNote {
            target: if rng.below(2) == 0 {
                owner_ref(OwnerKind::Person, *rng.pick(&persons))
            } else {
                owner
            },
            note: NoteInput {
                id: notes.first().filter(|_| rng.below(2) == 0).copied(),
                kind: NoteKindDto::General,
                text: format!("note {n}"),
            },
        },
        4 => {
            let _ = &notes;
            let p = t.persons.values().find(|p| !p.common.notes.is_empty())?;
            EditCommand::DeleteNote {
                target: owner_ref(OwnerKind::Person, p.common.id),
                note: p.common.notes[0],
            }
        }
        5 => EditCommand::SaveSource {
            source: SourceInput {
                id: nth_key(&t.sources, rng).filter(|_| rng.below(2) == 0),
                title: Some(format!("Source {n}")),
                author: Some("Author".into()),
                publication: None,
                abbreviation: None,
                text: None,
            },
        },
        6 | 7 => EditCommand::AddCitation {
            target: owner,
            citation: match (rng.below(3), t.citations.keys().next(), s.reuse_citation()) {
                (1, Some(c), _) => CitationArg::Clone(CloneOf { clone_of: *c }),
                (2, _, Some(_)) => CitationArg::Reuse(ReuseLast { reuse_last: true }),
                _ => CitationArg::New(cite_input(rng, s)),
            },
        },
        8 => EditCommand::UpdateCitation {
            id: nth_key(&t.citations, rng)?,
            citation: CitationPatch {
                source: nth_key(&t.sources, rng),
                page: Some(format!("p. {n}")),
                date_raw: None,
                text: None,
                quality: Some(rng.below(4) as u8),
                evidence: None,
            },
        },
        9 => {
            let _ = (&cites, &owner);
            let p = t
                .persons
                .values()
                .find(|p| !p.common.citations.is_empty())?;
            EditCommand::RemoveCitation {
                target: owner_ref(OwnerKind::Person, p.common.id),
                citation: p.common.citations[rng.below(p.common.citations.len())],
            }
        }
        10 => match rng.below(4) {
            0 => EditCommand::AttachMedia {
                target: owner,
                media: vec![nth_key(&t.media, rng)?],
            },
            1 => {
                let _ = (&media, &owner);
                let p = t.persons.values().find(|p| !p.common.media.is_empty())?;
                EditCommand::DetachMedia {
                    target: owner_ref(OwnerKind::Person, p.common.id),
                    media: p.common.media[rng.below(p.common.media.len())].media,
                }
            }
            2 => {
                let p = t.persons.values().find(|p| !p.common.media.is_empty())?;
                EditCommand::SetPrimaryMedia {
                    person: p.common.id,
                    media: p.common.media[rng.below(p.common.media.len())].media,
                }
            }
            _ => EditCommand::UpdateMedia {
                id: nth_key(&t.media, rng)?,
                title: Some(format!("Title {n}")),
                description: None,
                date_raw: Some(rng.pick(&DATES).to_string()),
            },
        },
        11 => EditCommand::SaveFamily {
            // Edits an existing family: new partners and kind.
            id: Some(nth_key(&t.families, rng)?),
            partner1: Some(*rng.pick(&persons)),
            partner2: if rng.below(3) == 0 {
                None
            } else {
                Some(*rng.pick(&persons))
            },
            kind: *rng.pick(&[
                UnionKindId::Married,
                UnionKindId::Unmarried,
                UnionKindId::CivilUnion,
            ]),
        },
        12 => {
            // An event owned by a family (marriage and the like).
            let fam = nth_key(&t.families, rng)?;
            let existing: Vec<Uuid> = t
                .events
                .values()
                .filter(|e| {
                    e.participants
                        .iter()
                        .any(|p| p.target == gt_core::model::ParticipantRef::Family(fam))
                })
                .map(|e| e.common.id)
                .collect();
            let owner = EventOwner {
                kind: EventOwnerKind::Family,
                id: fam,
            };
            if !existing.is_empty() && rng.below(3) == 0 {
                EditCommand::DeleteEvent {
                    owner,
                    event: *rng.pick(&existing),
                }
            } else {
                let reuse = existing.first().copied().filter(|_| rng.below(2) == 0);
                let mut event = event_input(rng, reuse);
                event.kind = rng.pick(&["marriage", "divorce", "residence"]).to_string();
                EditCommand::SaveEvent { owner, event }
            }
        }
        _ => {
            // An event with several participants.
            let mut event = event_input(rng, None);
            event.kind = rng
                .pick(&["residence", "baptism", "burial", "census"])
                .to_string();
            event.participants = Some(
                (0..1 + rng.below(3))
                    .map(|_| ParticipantInput {
                        person: *rng.pick(&persons),
                        role: rng.pick(&["principal", "witness", "godparent"]).to_string(),
                    })
                    .collect(),
            );
            EditCommand::SaveEvent {
                owner: EventOwner {
                    kind: EventOwnerKind::Person,
                    id: *rng.pick(&persons),
                },
                event,
            }
        }
    })
}

/// Writes tiny distinct PNGs and attaches them (one undoable step each) to random persons.
fn attach_images(s: &mut Session, dir: &std::path::Path, rng: &mut Rng, n: usize) {
    let id = *s
        .tree()
        .persons
        .keys()
        .nth(rng.below(s.tree().persons.len()))
        .unwrap();
    let path = dir.join(format!("img{n}.png"));
    let shade = (n % 250) as u8;
    image::RgbImage::from_pixel(4, 4, image::Rgb([shade, 20, 200]))
        .save(&path)
        .unwrap();
    let _ = s.attach_files(owner_ref(OwnerKind::Person, id), &[path]);
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
    let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
    for seed in 1..=12u64 {
        let (dir, mut s) = open_tree(&tree);
        check(&s, seed, 0, "open");
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        for step in 1..=120 {
            if s.tree().persons.is_empty() {
                break;
            }
            if step % 8 == 1 {
                attach_images(&mut s, dir.path(), &mut rng, (seed as usize) * 1000 + step);
                check(&s, seed, step, "attach files");
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
                    let kind = label
                        .split([' ', '{'])
                        .next()
                        .unwrap_or_default()
                        .to_string();
                    if s.apply(cmd).is_ok() {
                        applied += 1;
                        *kinds.entry(kind).or_default() += 1;
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
    eprintln!("applied by kind: {kinds:?}");
    for k in [
        "DeleteName",
        "UpdateChildLink",
        "ReorderChildren",
        "SaveNote",
        "DeleteNote",
        "SaveSource",
        "AddCitation",
        "UpdateCitation",
        "RemoveCitation",
        "AttachMedia",
        "DetachMedia",
        "SetPrimaryMedia",
        "UpdateMedia",
        "SaveFamily",
        "SaveEvent",
        "DeleteEvent",
    ] {
        assert!(kinds.contains_key(k), "{k} was never applied: {kinds:?}");
    }
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
