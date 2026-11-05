#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! What `tree-changed-by-agent` carries: the records changed and the people they belong to, so an
//! editor open on a person hears about a change to that person's note, event or citation.

mod common;

use common::*;
use gt_core::model::ParticipantRef;
use gt_core::tree::Tree;
use gt_session::*;
use std::collections::HashSet;
use uuid::Uuid;

fn person_named(tree: &Tree, given: &str) -> Uuid {
    tree.persons
        .values()
        .find(|p| p.names.iter().any(|n| n.given.as_deref() == Some(given)))
        .map(|p| p.common.id)
        .unwrap()
}

fn touched(s: &Session, changed: &[Uuid]) -> HashSet<Uuid> {
    let list = s.touched_with_people(changed);
    let set: HashSet<Uuid> = list.iter().copied().collect();
    assert_eq!(set.len(), list.len(), "no id twice");
    set
}

const SOURCED: &str = "0 @I1@ INDI\n1 NAME Olena /Koval/\n1 SEX F\n1 BIRT\n2 DATE 1850\n2 PLAC Kyiv\n1 NOTE Born in the old house\n1 SOUR @S1@\n2 PAGE 12\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Petro /Koval/\n1 SEX M\n1 FAMC @F1@\n\
0 @I4@ INDI\n1 NAME Maria /Melnyk/\n1 SEX F\n\
0 @F1@ FAM\n1 HUSB @I2@\n1 WIFE @I1@\n1 CHIL @I3@\n1 MARR\n2 DATE 1870\n1 NOTE Wed in spring\n\
0 @S1@ SOUR\n1 TITL Parish book\n";

#[test]
fn a_person_stays_in_the_list_and_brings_no_relatives() {
    let tree = tree_of(SOURCED);
    let (_d, s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    assert_eq!(touched(&s, &[olena]), HashSet::from([olena]));
}

#[test]
fn a_note_brings_its_person() {
    let tree = tree_of(SOURCED);
    let (_d, s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    let note = tree.persons[&olena].common.notes[0];
    assert_eq!(touched(&s, &[note]), HashSet::from([note, olena]));
}

#[test]
fn a_citation_brings_its_person() {
    let tree = tree_of(SOURCED);
    let (_d, s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    let citation = tree.persons[&olena].common.citations[0];
    assert_eq!(touched(&s, &[citation]), HashSet::from([citation, olena]));
}

#[test]
fn an_event_brings_its_people() {
    let tree = tree_of(SOURCED);
    let (_d, s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    let birth = tree
        .events
        .values()
        .find(|e| {
            e.participants
                .iter()
                .any(|p| p.target == ParticipantRef::Person(olena))
        })
        .unwrap();
    assert_eq!(
        touched(&s, &[birth.common.id]),
        HashSet::from([birth.common.id, olena])
    );
}

#[test]
fn a_family_note_brings_partners_and_children_but_not_strangers() {
    let tree = tree_of(SOURCED);
    let (_d, s) = open_tree(&tree);
    let family = tree.families.values().next().unwrap();
    let note = family.common.notes[0];
    let got = touched(&s, &[note]);
    assert!(got.contains(&note));
    assert!(got.contains(&person_named(&tree, "Olena")));
    assert!(got.contains(&person_named(&tree, "Ivan")));
    assert!(got.contains(&person_named(&tree, "Petro")));
    assert!(!got.contains(&person_named(&tree, "Maria")));
}

#[test]
fn a_family_event_brings_the_partners() {
    let tree = tree_of(SOURCED);
    let (_d, s) = open_tree(&tree);
    let family = tree.families.values().next().unwrap().common.id;
    let marriage = tree
        .events
        .values()
        .find(|e| {
            e.participants
                .iter()
                .any(|p| p.target == ParticipantRef::Family(family))
        })
        .unwrap();
    let got = touched(&s, &[marriage.common.id]);
    assert!(got.contains(&person_named(&tree, "Olena")));
    assert!(got.contains(&person_named(&tree, "Ivan")));
}

#[test]
fn a_source_and_a_place_reach_the_people_through_what_uses_them() {
    let tree = tree_of(SOURCED);
    let (_d, s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    let source = tree.sources.keys().next().copied().unwrap();
    assert!(touched(&s, &[source]).contains(&olena));
    let place = tree.places.keys().next().copied().unwrap();
    assert!(touched(&s, &[place]).contains(&olena));
    assert!(!touched(&s, &[place]).contains(&person_named(&tree, "Maria")));
}

#[test]
fn an_unknown_id_is_kept_and_adds_nobody() {
    let tree = tree_of(SOURCED);
    let (_d, s) = open_tree(&tree);
    let unknown = Uuid::from_u128(99);
    assert_eq!(touched(&s, &[unknown]), HashSet::from([unknown]));
    assert!(s.touched_with_people(&[]).is_empty());
}

// ---- who a write reaches: what the tree showed before it, and what it shows after ----

const FAMILY: &str = "0 @I1@ INDI\n1 NAME Olena /Koval/\n1 SEX F\n1 BIRT\n2 DATE 1850\n1 SOUR @S1@\n2 PAGE 12\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Petro /Koval/\n1 SEX M\n1 FAMC @F1@\n\
0 @I5@ INDI\n1 NAME Stefan /Koval/\n1 SEX M\n1 FAMC @F1@\n\
0 @I4@ INDI\n1 NAME Maria /Melnyk/\n1 SEX F\n\
0 @F1@ FAM\n1 HUSB @I2@\n1 WIFE @I1@\n1 CHIL @I3@\n1 CHIL @I5@\n1 MARR\n2 DATE 1870\n\
0 @S1@ SOUR\n1 TITL Parish book\n";

fn the_family(t: &Tree) -> Uuid {
    t.families.values().next().unwrap().common.id
}

fn marriage_of(t: &Tree) -> Uuid {
    let family = the_family(t);
    t.events
        .values()
        .find(|e| {
            e.participants
                .iter()
                .any(|p| p.target == ParticipantRef::Family(family))
        })
        .unwrap()
        .common
        .id
}

fn birth_of(t: &Tree, person: Uuid) -> Uuid {
    t.events
        .values()
        .find(|e| {
            e.participants
                .iter()
                .any(|p| p.target == ParticipantRef::Person(person))
        })
        .unwrap()
        .common
        .id
}

fn given_names(t: &Tree, ids: &HashSet<Uuid>) -> Vec<String> {
    let mut names: Vec<String> = t
        .persons
        .values()
        .filter(|p| ids.contains(&p.common.id))
        .map(|p| p.names[0].given.clone().unwrap())
        .collect();
    names.sort();
    names
}

/// An owner's command, as `apply_as_agent_reaching` would see it: every command, also those an agent
/// is refused, must reach whoever the tree linked to it before or links to it after.
fn reach_of(s: &mut Session, cmd: EditCommand) -> Vec<String> {
    let before = s.tree().clone();
    let touched = s.apply(cmd).unwrap().touched;
    let mut all: HashSet<Uuid> = people_reached(&before, &touched).into_iter().collect();
    all.extend(people_reached(s.tree(), &touched));
    given_names(&before, &all)
}

fn owner_ref(kind: OwnerKind, id: Uuid) -> OwnerRef {
    OwnerRef { kind, id }
}

fn event_input(
    kind: &str,
    id: Option<Uuid>,
    participants: Option<Vec<ParticipantInput>>,
) -> EventInput {
    EventInput {
        id,
        kind: kind.into(),
        date_raw: Some("1850".into()),
        place_text: None,
        place_id: None,
        description: None,
        age: None,
        cause: None,
        negative: false,
        private: false,
        preferred: true,
        participants,
    }
}

fn taking_part(person: Uuid, role: &str) -> ParticipantInput {
    ParticipantInput {
        person,
        role: role.into(),
    }
}

fn person_owner(id: Uuid) -> EventOwner {
    EventOwner {
        kind: EventOwnerKind::Person,
        id,
    }
}

fn plain_note(text: &str) -> NoteInput {
    NoteInput {
        id: None,
        kind: NoteKindDto::General,
        text: text.into(),
    }
}

fn new_citation(source: Uuid) -> CitationArg {
    CitationArg::New(CitationInput {
        source: SourceRef::Id(source),
        page: Some("3".into()),
        date_raw: None,
        text: None,
        quality: None,
        evidence: None,
    })
}

fn media_record(name: &str) -> gt_core::model::Media {
    gt_core::model::Media {
        common: gt_core::model::Common {
            id: Uuid::now_v7(),
            private: false,
            changed: None,
            citations: vec![],
            notes: vec![],
            media: vec![],
            ext_ids: vec![],
            unknown: vec![],
        },
        path: name.into(),
        original_name: None,
        mime: None,
        title: None,
        date: None,
        description: None,
        sha256: None,
    }
}

/// `FAMILY` with one picture on Olena and on the family.
fn with_media() -> (Tree, Uuid, Uuid) {
    let mut tree = tree_of(FAMILY);
    let picture = media_record("wedding.jpg");
    let on_person = media_record("portrait.jpg");
    let (picture_id, on_person_id) = (picture.common.id, on_person.common.id);
    tree.media.insert(picture_id, picture);
    tree.media.insert(on_person_id, on_person);
    let family = the_family(&tree);
    let link = |media| gt_core::model::MediaLink {
        media,
        primary: false,
        region: None,
    };
    tree.families
        .get_mut(&family)
        .unwrap()
        .common
        .media
        .push(link(picture_id));
    let olena = person_named(&tree, "Olena");
    tree.persons
        .get_mut(&olena)
        .unwrap()
        .common
        .media
        .push(link(on_person_id));
    (tree, picture_id, on_person_id)
}

#[test]
fn deleting_a_family_reaches_its_partners_and_children() {
    let tree = tree_of(FAMILY);
    let (_d, mut s) = open_tree(&tree);
    let names = reach_of(
        &mut s,
        EditCommand::DeleteFamily {
            id: the_family(&tree),
        },
    );
    assert_eq!(names, ["Ivan", "Olena", "Petro", "Stefan"]);
}

#[test]
fn deleting_a_family_event_reaches_both_partners() {
    let tree = tree_of(FAMILY);
    let (_d, mut s) = open_tree(&tree);
    let names = reach_of(
        &mut s,
        EditCommand::DeleteEvent {
            owner: EventOwner {
                kind: EventOwnerKind::Family,
                id: the_family(&tree),
            },
            event: marriage_of(&tree),
        },
    );
    assert_eq!(names, ["Ivan", "Olena"]);
}

#[test]
fn unlinking_a_child_reaches_the_child() {
    let tree = tree_of(FAMILY);
    let (_d, mut s) = open_tree(&tree);
    let petro = person_named(&tree, "Petro");
    let names = reach_of(
        &mut s,
        EditCommand::UnlinkChild {
            family: the_family(&tree),
            child: petro,
        },
    );
    assert!(names.contains(&"Petro".to_string()), "{names:?}");
}

#[test]
fn replacing_a_partner_reaches_the_old_and_the_new() {
    let tree = tree_of(FAMILY);
    let (_d, mut s) = open_tree(&tree);
    let names = reach_of(
        &mut s,
        EditCommand::SaveFamily {
            id: Some(the_family(&tree)),
            partner1: Some(person_named(&tree, "Ivan")),
            partner2: Some(person_named(&tree, "Maria")),
            kind: UnionKindId::Married,
        },
    );
    for who in ["Olena", "Maria", "Ivan"] {
        assert!(names.contains(&who.to_string()), "{who} in {names:?}");
    }
}

#[test]
fn dropping_a_witness_reaches_the_witness() {
    let tree = tree_of(FAMILY);
    let (_d, mut s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    let maria = person_named(&tree, "Maria");
    let birth = birth_of(&tree, olena);
    s.apply(EditCommand::SaveEvent {
        owner: person_owner(olena),
        event: event_input(
            "birth",
            Some(birth),
            Some(vec![
                taking_part(olena, "principal"),
                taking_part(maria, "witness"),
            ]),
        ),
    })
    .unwrap();
    let names = reach_of(
        &mut s,
        EditCommand::SaveEvent {
            owner: person_owner(olena),
            event: event_input(
                "birth",
                Some(birth),
                Some(vec![taking_part(olena, "principal")]),
            ),
        },
    );
    assert!(names.contains(&"Maria".to_string()), "{names:?}");
}

#[test]
fn deleting_a_name_reaches_its_person() {
    let tree = tree_of(FAMILY);
    let (_d, mut s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    s.apply(EditCommand::SaveName {
        person: olena,
        name: NameInput {
            id: None,
            kind: NameKindDto::Aka,
            kind_text: None,
            preferred: false,
            private: false,
            prefix: None,
            given: Some("Lena".into()),
            call: None,
            surname_prefix: None,
            surnames: vec!["Koval".into()],
            patronymic: None,
            suffix: None,
            nickname: None,
            lang: None,
            date_raw: None,
        },
    })
    .unwrap();
    let name = s.tree().persons[&olena].names[1].id;
    let names = reach_of(
        &mut s,
        EditCommand::DeleteName {
            person: olena,
            name,
        },
    );
    assert_eq!(names, ["Olena"]);
}

#[test]
fn reordering_children_reaches_the_whole_family() {
    let tree = tree_of(FAMILY);
    let (_d, mut s) = open_tree(&tree);
    let names = reach_of(
        &mut s,
        EditCommand::ReorderChildren {
            family: the_family(&tree),
            order: vec![person_named(&tree, "Stefan"), person_named(&tree, "Petro")],
        },
    );
    assert_eq!(names, ["Ivan", "Olena", "Petro", "Stefan"]);
}

#[test]
fn media_commands_reach_those_who_show_the_picture() {
    let (tree, on_family, on_olena) = with_media();
    let olena = person_named(&tree, "Olena");
    let maria = person_named(&tree, "Maria");
    let (_d, mut s) = open_tree(&tree);
    let everyone_in_family = ["Ivan", "Olena", "Petro", "Stefan"];

    let edit = EditCommand::UpdateMedia {
        id: on_family,
        title: Some("The wedding".into()),
        description: None,
        date_raw: None,
    };
    assert_eq!(reach_of(&mut s, edit), everyone_in_family);

    let attach = EditCommand::AttachMedia {
        target: owner_ref(OwnerKind::Person, maria),
        media: vec![on_olena, on_family],
    };
    assert_eq!(reach_of(&mut s, attach), ["Maria"]);

    let primary = EditCommand::SetPrimaryMedia {
        person: maria,
        media: on_family,
    };
    assert_eq!(reach_of(&mut s, primary), ["Maria"]);

    let detach = EditCommand::DetachMedia {
        target: owner_ref(OwnerKind::Person, olena),
        media: on_olena,
    };
    assert_eq!(reach_of(&mut s, detach), ["Olena"]);

    // Deleting a picture: its holders are known only from before the commit.
    let delete = EditCommand::DeleteMedia { id: on_family };
    assert_eq!(
        reach_of(&mut s, delete),
        ["Ivan", "Maria", "Olena", "Petro", "Stefan"]
    );
}

#[test]
fn an_owner_of_kind_name_reaches_its_person() {
    let tree = tree_of(FAMILY);
    let (_d, mut s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    let name = tree.persons[&olena].names[0].id;
    let source = *tree.sources.keys().next().unwrap();
    let on_name = owner_ref(OwnerKind::Name, name);
    let note = EditCommand::SaveNote {
        target: on_name,
        note: plain_note("Spelled so in the parish book"),
    };
    assert_eq!(reach_of(&mut s, note), ["Olena"]);
    let cite = EditCommand::AddCitation {
        target: on_name,
        citation: new_citation(source),
    };
    assert_eq!(reach_of(&mut s, cite), ["Olena"]);

    // Taking a citation off the name: the citation alone is gone from it afterwards.
    let citation = s.tree().persons[&olena].names[0].citations[0];
    let remove = EditCommand::RemoveCitation {
        target: on_name,
        citation,
    };
    assert_eq!(reach_of(&mut s, remove), ["Olena"]);
}

#[test]
fn an_owner_of_kind_citation_reaches_whoever_cites_it() {
    let tree = tree_of(FAMILY);
    let (_d, mut s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    let citation = tree.persons[&olena].common.citations[0];
    let note = EditCommand::SaveNote {
        target: owner_ref(OwnerKind::Citation, citation),
        note: plain_note("Copied by hand"),
    };
    assert_eq!(reach_of(&mut s, note), ["Olena"]);

    let note_id = s.tree().citations[&citation].common.notes[0];
    let delete = EditCommand::DeleteNote {
        target: owner_ref(OwnerKind::Citation, citation),
        note: note_id,
    };
    assert_eq!(reach_of(&mut s, delete), ["Olena"]);
}

#[test]
fn a_citation_on_a_child_link_reaches_the_whole_family() {
    let mut tree = tree_of(FAMILY);
    let olena = person_named(&tree, "Olena");
    let citation = tree.persons[&olena].common.citations[0];
    tree.persons
        .get_mut(&olena)
        .unwrap()
        .common
        .citations
        .clear();
    let family = the_family(&tree);
    tree.families.get_mut(&family).unwrap().children[0]
        .citations
        .push(citation);
    let (_d, mut s) = open_tree(&tree);
    let edit = EditCommand::UpdateCitation {
        id: citation,
        citation: CitationPatch {
            source: None,
            page: Some("99".into()),
            date_raw: None,
            text: None,
            quality: None,
            evidence: None,
        },
    };
    assert_eq!(reach_of(&mut s, edit), ["Ivan", "Olena", "Petro", "Stefan"]);
}

// ---- the call a write tool makes ----

fn agent() -> AgentCtx {
    AgentCtx {
        id: Uuid::from_u128(7),
        name: "Transcript reader".into(),
    }
}

#[test]
fn an_agents_note_gives_the_write_and_the_ids_with_their_person() {
    let tree = tree_of(FAMILY);
    let (_d, mut s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    let batch = s.begin_batch(&agent(), "Recording 2", None).unwrap();
    let (write, reached) = s
        .apply_as_agent_reaching(
            EditCommand::SaveNote {
                target: owner_ref(OwnerKind::Person, olena),
                note: plain_note("Lived in the old house"),
            },
            &agent(),
            batch,
        )
        .unwrap();
    let AgentWrite::Done(result) = write else {
        panic!("written");
    };
    for id in &result.touched {
        assert!(reached.contains(id), "every changed id is listed");
    }
    assert!(reached.contains(&olena));
    let unique: HashSet<Uuid> = reached.iter().copied().collect();
    assert_eq!(unique.len(), reached.len(), "no id twice");
}

#[test]
fn a_repeat_that_wrote_nothing_reaches_nobody() {
    let tree = tree_of(FAMILY);
    let (_d, mut s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    let batch = s.begin_batch(&agent(), "Recording 2", None).unwrap();
    let cmd = EditCommand::SaveNote {
        target: owner_ref(OwnerKind::Person, olena),
        note: plain_note("Lived in the old house"),
    };
    s.apply_as_agent_reaching(cmd.clone(), &agent(), batch)
        .unwrap();
    let (write, reached) = s.apply_as_agent_reaching(cmd, &agent(), batch).unwrap();
    assert!(matches!(write, AgentWrite::AlreadyThere { .. }));
    assert!(reached.is_empty());
}

#[test]
fn an_agent_dropping_a_witness_of_its_own_event_reaches_the_witness() {
    let tree = tree_of(FAMILY);
    let (_d, mut s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    let maria = person_named(&tree, "Maria");
    let batch = s.begin_batch(&agent(), "Recording 2", None).unwrap();
    let (write, reached) = s
        .apply_as_agent_reaching(
            EditCommand::SaveEvent {
                owner: person_owner(olena),
                event: event_input(
                    "occupation",
                    None,
                    Some(vec![
                        taking_part(olena, "principal"),
                        taking_part(maria, "witness"),
                    ]),
                ),
            },
            &agent(),
            batch,
        )
        .unwrap();
    assert!(reached.contains(&maria), "the witness of a new event");
    let AgentWrite::Done(result) = write else {
        panic!("written");
    };
    let event = result
        .touched
        .iter()
        .copied()
        .find(|id| s.tree().events.contains_key(id))
        .unwrap();

    let (_, reached) = s
        .apply_as_agent_reaching(
            EditCommand::SaveEvent {
                owner: person_owner(olena),
                event: event_input(
                    "occupation",
                    Some(event),
                    Some(vec![taking_part(olena, "principal")]),
                ),
            },
            &agent(),
            batch,
        )
        .unwrap();
    assert!(
        reached.contains(&maria),
        "no longer linked after the commit"
    );
    assert!(reached.contains(&olena));
}

#[test]
fn a_refused_write_reports_the_refusal_and_reaches_nobody() {
    let tree = tree_of(FAMILY);
    let (_d, mut s) = open_tree(&tree);
    let batch = s.begin_batch(&agent(), "Recording 2", None).unwrap();
    let e = s
        .apply_as_agent_reaching(
            EditCommand::DeleteFamily {
                id: the_family(&tree),
            },
            &agent(),
            batch,
        )
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::AgentRefused);
}
