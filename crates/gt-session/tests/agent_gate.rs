#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The gate on agent writes: what an agent may add, what it may change, and that nothing it does
//! can change whether an existing person counts as alive or dead.

mod common;

use common::*;
use gt_core::model::{EventKind, ParentRel};
use gt_session::*;
use gt_store::{Actor, LogFilter};
use uuid::Uuid;

fn me() -> AgentCtx {
    AgentCtx {
        id: Uuid::from_u128(7),
        name: "Transcript reader".into(),
    }
}

fn other() -> AgentCtx {
    AgentCtx {
        id: Uuid::from_u128(8),
        name: "Second reader".into(),
    }
}

fn batch(s: &mut Session, who: &AgentCtx) -> Uuid {
    s.begin_batch(who, "Recording 2 summary", Some("recording2.txt"))
        .unwrap()
}

fn person_ref(id: Uuid) -> OwnerRef {
    OwnerRef {
        kind: OwnerKind::Person,
        id,
    }
}

fn person_owner(id: Uuid) -> EventOwner {
    EventOwner {
        kind: EventOwnerKind::Person,
        id,
    }
}

fn note(id: Uuid, text: &str) -> EditCommand {
    EditCommand::SaveNote {
        target: person_ref(id),
        note: NoteInput {
            id: None,
            kind: NoteKindDto::General,
            text: text.into(),
        },
    }
}

fn edit_note(owner: Uuid, id: Uuid, text: &str) -> EditCommand {
    EditCommand::SaveNote {
        target: person_ref(owner),
        note: NoteInput {
            id: Some(id),
            kind: NoteKindDto::General,
            text: text.into(),
        },
    }
}

fn event_input(kind: &str, date: Option<&str>) -> EventInput {
    EventInput {
        id: None,
        kind: kind.into(),
        date_raw: date.map(str::to_string),
        place_text: None,
        place_id: None,
        description: None,
        age: None,
        cause: None,
        negative: false,
        private: false,
        preferred: false,
        participants: None,
    }
}

fn event(id: Uuid, kind: &str, date: Option<&str>) -> EditCommand {
    EditCommand::SaveEvent {
        owner: person_owner(id),
        event: event_input(kind, date),
    }
}

fn dated_draft(given: &str, sex: SexKind, birth: &str) -> PersonDraft {
    PersonDraft {
        birth_date_raw: Some(birth.into()),
        ..draft(given, sex)
    }
}

fn quick_add(anchor: Uuid, relation: Relation, d: PersonDraft) -> EditCommand {
    EditCommand::QuickAdd {
        anchor,
        relation,
        draft: d,
        family: None,
        new_parent_set: None,
        parent_rel: None,
    }
}

fn link_existing(anchor: Uuid, relation: Relation, person: Uuid) -> EditCommand {
    EditCommand::LinkExisting {
        anchor,
        relation,
        person,
        family: None,
        new_parent_set: None,
        parent_rel: None,
    }
}

fn cite(id: Uuid, source: &str, page: &str) -> EditCommand {
    EditCommand::AddCitation {
        target: person_ref(id),
        citation: CitationArg::New(CitationInput {
            source: SourceRef::New(NewSource {
                title: source.into(),
            }),
            page: Some(page.into()),
            date_raw: None,
            text: None,
            quality: Some(2),
            evidence: None,
        }),
    }
}

fn number(person: Uuid, file: &str, n: &str) -> EditCommand {
    EditCommand::AddNumber {
        person,
        source_file: file.into(),
        number: n.into(),
    }
}

fn source_field(id: Uuid, author: &str) -> EditCommand {
    EditCommand::SaveSource {
        source: SourceInput {
            id: Some(id),
            title: None,
            author: Some(author.into()),
            publication: None,
            abbreviation: None,
            text: None,
        },
    }
}

fn update_sex(id: Uuid, sex: SexKind) -> EditCommand {
    EditCommand::UpdatePerson {
        id,
        sex: Some(sex),
        sex_text: None,
        living_override: LivingPatch::Keep,
        private: None,
    }
}

fn name_input(s: &Session, person: Uuid, name: Uuid) -> NameInput {
    let n = s.tree().persons[&person]
        .names
        .iter()
        .find(|n| n.id == name)
        .unwrap();
    NameInput {
        id: Some(name),
        kind: NameKindDto::Birth,
        kind_text: None,
        preferred: n.assertion.preferred,
        private: n.private,
        prefix: n.prefix.clone(),
        given: n.given.clone(),
        call: n.call.clone(),
        surname_prefix: n.surname_prefix.clone(),
        surnames: n.surnames.clone(),
        patronymic: n.patronymic.clone(),
        suffix: n.suffix.clone(),
        nickname: n.nickname.clone(),
        lang: n.lang.clone(),
        date_raw: None,
    }
}

fn extra_name(given: &str, preferred: bool) -> NameInput {
    NameInput {
        id: None,
        kind: NameKindDto::Aka,
        kind_text: None,
        preferred,
        private: false,
        prefix: None,
        given: Some(given.into()),
        call: None,
        surname_prefix: None,
        surnames: vec![],
        patronymic: None,
        suffix: None,
        nickname: None,
        lang: None,
        date_raw: None,
    }
}

fn done(r: Result<AgentWrite, ApiError>) -> EditResult {
    match r.unwrap() {
        AgentWrite::Done(x) => x,
        AgentWrite::AlreadyThere { record } => panic!("already there: {record}"),
    }
}

fn logged_groups(s: &Session) -> u32 {
    s.project()
        .log_page(&LogFilter::default(), 0, usize::MAX)
        .unwrap()
        .1
}

/// The write is refused with the gate's code, the tree and the log stay as they were, and the
/// refusal is returned so the test can read its wording.
fn refuse(s: &mut Session, cmd: EditCommand, who: &AgentCtx, b: Uuid) -> String {
    let tree = s.tree().clone();
    let groups = logged_groups(s);
    let err = s
        .apply_as_agent(cmd.clone(), who, b)
        .expect_err(&format!("expected a refusal of {cmd:?}"));
    assert_eq!(err.code, ErrorCode::AgentRefused, "{err:?}");
    assert_eq!(s.tree(), &tree, "tree changed by {cmd:?}");
    assert_eq!(logged_groups(s), groups, "log changed by {cmd:?}");
    s.verify_derived_state().unwrap();
    err.message
}

fn living_of(s: &Session, id: Uuid) -> bool {
    s.person_ref(id).unwrap().living
}

fn newest_note_on(s: &Session, id: Uuid) -> Uuid {
    *s.tree().persons[&id].common.notes.last().unwrap()
}

fn owner_source(s: &mut Session, title: &str) -> Uuid {
    let r = s
        .apply(EditCommand::SaveSource {
            source: SourceInput {
                id: None,
                title: Some(title.into()),
                author: None,
                publication: None,
                abbreviation: None,
                text: None,
            },
        })
        .unwrap();
    r.touched[0]
}

// ---- the owner's values ---------------------------------------------------------------------

#[test]
fn overwrite_refused_tree_unchanged() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let anna = find(s.tree(), "Anna");
    let name = s.tree().persons[&anna].names[0].id;
    let mut input = name_input(&s, anna, name);
    input.given = Some("Hanna".into());
    let msg = refuse(
        &mut s,
        EditCommand::SaveName {
            person: anna,
            name: input,
        },
        &me(),
        b,
    );
    assert!(msg.to_lowercase().contains("add a note"), "{msg}");

    let source = owner_source(&mut s, "Parish book");
    let msg = refuse(
        &mut s,
        EditCommand::SaveSource {
            source: SourceInput {
                id: Some(source),
                title: Some("Another title".into()),
                author: None,
                publication: None,
                abbreviation: None,
                text: None,
            },
        },
        &me(),
        b,
    );
    assert!(msg.contains("title"), "names the fact: {msg}");
    assert!(msg.to_lowercase().contains("add a note"), "{msg}");

    let owner_note = s.apply(note(anna, "The owner's words")).unwrap().touched;
    let id = newest_note_on(&s, anna);
    assert!(owner_note.contains(&id));
    refuse(&mut s, edit_note(anna, id, "An agent's words"), &me(), b);
}

#[test]
fn fill_empty_allowed() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let source = owner_source(&mut s, "Parish book");
    done(s.apply_as_agent(source_field(source, "O. Koval"), &me(), b));
    assert_eq!(
        s.tree().sources[&source].author.as_deref(),
        Some("O. Koval")
    );

    // Anna's birth has a date and no place: the place can be filled, the date must stay.
    let anna = find(s.tree(), "Anna");
    let birth = s
        .tree()
        .events
        .values()
        .find(|e| e.kind == EventKind::Birth)
        .unwrap()
        .clone();
    let input = |date: &str| EventInput {
        id: Some(birth.common.id),
        place_text: Some("Lviv".into()),
        preferred: birth.assertion.preferred,
        ..event_input("birth", Some(date))
    };
    done(s.apply_as_agent(
        EditCommand::SaveEvent {
            owner: person_owner(anna),
            event: input("1850"),
        },
        &me(),
        b,
    ));
    let filled = &s.tree().events[&birth.common.id];
    assert!(filled.place.is_some());
    assert_eq!(filled.date, birth.date);
    s.verify_derived_state().unwrap();
}

#[test]
fn delete_commands_refused() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let anna = find(s.tree(), "Anna");
    let hans = find(s.tree(), "Hans");
    let family = *s.tree().families.keys().next().unwrap();
    let name = s.tree().persons[&anna].names[0].id;
    let birth = *s.tree().events.keys().next().unwrap();
    let any = Uuid::from_u128(99);
    let cmds = vec![
        EditCommand::DeletePerson { id: anna },
        EditCommand::DeleteName { person: anna, name },
        EditCommand::DeleteEvent {
            owner: person_owner(anna),
            event: birth,
        },
        EditCommand::DeleteFamily { id: family },
        EditCommand::UnlinkChild {
            family,
            child: hans,
        },
        EditCommand::UpdateChildLink {
            family,
            child: hans,
            rel: (ParentRelId::Step, ParentRelId::Step),
            preferred: false,
        },
        EditCommand::ReorderChildren {
            family,
            order: vec![hans],
        },
        EditCommand::DeleteNote {
            target: person_ref(anna),
            note: any,
        },
        EditCommand::RemoveCitation {
            target: person_ref(anna),
            citation: any,
        },
        EditCommand::AttachMedia {
            target: person_ref(anna),
            media: vec![any],
        },
        EditCommand::DetachMedia {
            target: person_ref(anna),
            media: any,
        },
        EditCommand::DeleteMedia { id: any },
        EditCommand::SetPrimaryMedia {
            person: anna,
            media: any,
        },
        EditCommand::UpdateMedia {
            id: any,
            title: None,
            description: None,
            date_raw: None,
        },
        EditCommand::ImportDraft {
            persons: vec![],
            families: vec![],
            attach_picture: false,
        },
        EditCommand::SaveFamily {
            id: Some(family),
            partner1: None,
            partner2: None,
            kind: UnionKindId::Unknown,
        },
    ];
    for cmd in cmds {
        let msg = refuse(&mut s, cmd, &me(), b);
        assert!(msg.to_lowercase().contains("note"), "{msg}");
    }
}

// ---- one of a kind --------------------------------------------------------------------------

#[test]
fn second_birth_refused() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let anna = find(s.tree(), "Anna");
    refuse(&mut s, event(anna, "birth", Some("1851")), &me(), b);

    // The same, by naming her as principal of an event on a new person.
    let new = done(s.apply_as_agent(
        EditCommand::CreatePerson {
            draft: draft("Ivan", SexKind::Male),
        },
        &me(),
        b,
    ))
    .focus
    .unwrap();
    let mut input = event_input("birth", Some("1851"));
    input.participants = Some(vec![ParticipantInput {
        person: anna,
        role: "principal".into(),
    }]);
    refuse(
        &mut s,
        EditCommand::SaveEvent {
            owner: person_owner(new),
            event: input,
        },
        &me(),
        b,
    );
}

#[test]
fn own_event_cannot_become_a_second_birth() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let anna = find(s.tree(), "Anna");
    let r = done(s.apply_as_agent(event(anna, "residence", Some("1870")), &me(), b));
    let id = r
        .touched
        .iter()
        .copied()
        .find(|i| s.tree().events.contains_key(i))
        .unwrap();
    let mut input = event_input("birth", Some("1870"));
    input.id = Some(id);
    refuse(
        &mut s,
        EditCommand::SaveEvent {
            owner: person_owner(anna),
            event: input,
        },
        &me(),
        b,
    );
}

#[test]
fn second_father_refused() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let hans = find(s.tree(), "Hans");
    refuse(
        &mut s,
        quick_add(hans, Relation::Father, draft("Ivan", SexKind::Male)),
        &me(),
        b,
    );
    // Asking for a second set of parents does not get round it.
    refuse(
        &mut s,
        EditCommand::QuickAdd {
            anchor: hans,
            relation: Relation::Father,
            draft: draft("Ivan", SexKind::Male),
            family: None,
            new_parent_set: Some(true),
            parent_rel: Some(ParentRelId::Birth),
        },
        &me(),
        b,
    );
    let ivan = done(s.apply_as_agent(
        EditCommand::CreatePerson {
            draft: draft("Ivan", SexKind::Male),
        },
        &me(),
        b,
    ))
    .focus
    .unwrap();
    refuse(
        &mut s,
        EditCommand::LinkExisting {
            anchor: hans,
            relation: Relation::Father,
            person: ivan,
            family: None,
            new_parent_set: Some(true),
            parent_rel: None,
        },
        &me(),
        b,
    );
}

#[test]
fn second_father_refused_by_the_routes_that_do_not_say_father() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let hans = find(s.tree(), "Hans");
    let ivan = done(s.apply_as_agent(
        EditCommand::CreatePerson {
            draft: draft("Ivan", SexKind::Male),
        },
        &me(),
        b,
    ))
    .focus
    .unwrap();
    // From the parent's side: Hans as Ivan's child.
    refuse(&mut s, link_existing(ivan, Relation::Child, hans), &me(), b);
    // Into a second family directly.
    let family = done(s.apply_as_agent(
        EditCommand::SaveFamily {
            id: None,
            partner1: Some(ivan),
            partner2: None,
            kind: UnionKindId::Unknown,
        },
        &me(),
        b,
    ))
    .touched[0];
    refuse(
        &mut s,
        EditCommand::LinkChild {
            family,
            child: hans,
            rel: (ParentRelId::Birth, ParentRelId::Birth),
            preferred: false,
        },
        &me(),
        b,
    );
}

/// Olena raises the child in F1; Petro is the child's father in F2.
const SPLIT_PARENTS: &str = "0 @I1@ INDI\n1 NAME Olena /Koval/\n1 SEX F\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Petro /Koval/\n1 SEX M\n1 FAMS @F2@\n\
0 @I3@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n1 FAMC @F1@\n1 FAMC @F2@\n\
0 @F1@ FAM\n1 WIFE @I1@\n1 CHIL @I3@\n\
0 @F2@ FAM\n1 HUSB @I2@\n1 CHIL @I3@\n";

#[test]
fn partner_who_would_become_a_second_father_refused() {
    let (_d, mut s) = open_tree(&tree_of(SPLIT_PARENTS));
    let b = batch(&mut s, &me());
    let olena = find(s.tree(), "Olena");
    let f1 = s
        .tree()
        .families
        .values()
        .find(|f| f.partner1 == Some(olena) || f.partner2 == Some(olena))
        .unwrap()
        .common
        .id;
    refuse(
        &mut s,
        EditCommand::QuickAdd {
            anchor: olena,
            relation: Relation::Partner,
            draft: draft("Taras", SexKind::Male),
            family: Some(f1),
            new_parent_set: None,
            parent_rel: None,
        },
        &me(),
        b,
    );
}

#[test]
fn step_father_allowed() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let hans = find(s.tree(), "Hans");
    let step = done(s.apply_as_agent(
        EditCommand::QuickAdd {
            anchor: hans,
            relation: Relation::Father,
            draft: draft("Ivan", SexKind::Male),
            family: None,
            new_parent_set: None,
            parent_rel: Some(ParentRelId::Step),
        },
        &me(),
        b,
    ))
    .focus
    .unwrap();
    let f = s
        .tree()
        .families
        .values()
        .find(|f| f.partner1 == Some(step) || f.partner2 == Some(step))
        .unwrap();
    assert!(f.children.iter().any(|c| c.person == hans));
    s.verify_derived_state().unwrap();
}

/// A mother recorded alone: a step-father takes the free place in her family.
const LONE_MOTHER: &str = "0 @I1@ INDI\n1 NAME Olena /Koval/\n1 SEX F\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 WIFE @I1@\n1 CHIL @I2@\n";

#[test]
fn step_father_fills_the_free_place_of_a_lone_mother() {
    let (_d, mut s) = open_tree(&tree_of(LONE_MOTHER));
    let b = batch(&mut s, &me());
    let ivan = find(s.tree(), "Ivan");
    let step = done(s.apply_as_agent(
        EditCommand::QuickAdd {
            anchor: ivan,
            relation: Relation::Father,
            draft: draft("Taras", SexKind::Male),
            family: None,
            new_parent_set: None,
            parent_rel: Some(ParentRelId::Step),
        },
        &me(),
        b,
    ))
    .focus
    .unwrap();
    let f = s.tree().families.values().next().unwrap();
    assert_eq!(f.partner1, Some(step));
    assert_eq!(f.children[0].rel1, ParentRel::Step);
    assert_eq!(f.children[0].rel2, ParentRel::Birth);
}

#[test]
fn first_father_allowed() {
    let (_d, mut s) = open_tree(&tree_of(LONE_MOTHER));
    let b = batch(&mut s, &me());
    let ivan = find(s.tree(), "Ivan");
    let father = done(s.apply_as_agent(
        quick_add(ivan, Relation::Father, draft("Taras", SexKind::Male)),
        &me(),
        b,
    ))
    .focus
    .unwrap();
    assert_eq!(
        s.tree().families.values().next().unwrap().partner1,
        Some(father)
    );
}

#[test]
fn link_child_into_second_family_refused() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");
    let hans = find(s.tree(), "Hans");
    let family = done(s.apply_as_agent(
        EditCommand::SaveFamily {
            id: None,
            partner1: Some(eva),
            partner2: None,
            kind: UnionKindId::Unknown,
        },
        &me(),
        b,
    ))
    .touched[0];
    refuse(
        &mut s,
        EditCommand::LinkChild {
            family,
            child: hans,
            rel: (ParentRelId::Step, ParentRelId::Step),
            preferred: false,
        },
        &me(),
        b,
    );
}

// ---- alive or dead --------------------------------------------------------------------------

#[test]
fn death_on_living_refused() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");
    assert!(living_of(&s, eva));
    for kind in ["death", "burial", "cremation", "probate"] {
        let msg = refuse(&mut s, event(eva, kind, Some("2001")), &me(), b);
        assert!(msg.contains("Eva"), "{msg}");
        assert!(msg.contains("alive or dead"), "{msg}");
        assert!(msg.contains("Only the owner"), "{msg}");
        assert!(msg.contains("note"), "{msg}");
    }
    // With no date at all it proves death just the same.
    refuse(&mut s, event(eva, "death", None), &me(), b);
    assert!(living_of(&s, eva));
}

#[test]
fn death_of_someone_named_on_another_persons_event_refused() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");
    let karl = find(s.tree(), "Karl");
    let mut input = event_input("death", Some("2001"));
    input.participants = Some(vec![
        ParticipantInput {
            person: eva,
            role: "principal".into(),
        },
        ParticipantInput {
            person: karl,
            role: "witness".into(),
        },
    ]);
    refuse(
        &mut s,
        EditCommand::SaveEvent {
            owner: person_owner(karl),
            event: input,
        },
        &me(),
        b,
    );
    assert!(living_of(&s, eva));
}

#[test]
fn old_birth_date_that_flips_living_refused() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");
    let msg = refuse(&mut s, event(eva, "birth", Some("1800")), &me(), b);
    assert!(msg.contains("alive or dead"), "{msg}");
    // A recent date flips nobody.
    done(s.apply_as_agent(event(eva, "birth", Some("1990")), &me(), b));
    assert!(living_of(&s, eva));
}

#[test]
fn link_that_flips_living_refused() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");
    let anna = find(s.tree(), "Anna");
    // An old child proves a parent dead, whether the child is new...
    let msg = refuse(
        &mut s,
        quick_add(
            eva,
            Relation::Child,
            dated_draft("Ivan", SexKind::Male, "1900"),
        ),
        &me(),
        b,
    );
    assert!(msg.contains("Eva"), "{msg}");
    // ...or already in the tree.
    refuse(&mut s, link_existing(eva, Relation::Child, anna), &me(), b);
    assert!(living_of(&s, eva));
}

#[test]
fn deceased_cannot_flip_to_living() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let anna = find(s.tree(), "Anna");
    assert!(!living_of(&s, anna));
    // A baptism in living memory moves the bound the age rule reads: not a death, not a birth.
    let msg = refuse(&mut s, event(anna, "baptism", Some("2000")), &me(), b);
    assert!(msg.contains("alive or dead"), "{msg}");
    assert!(!living_of(&s, anna));
}

#[test]
fn own_event_cannot_be_turned_into_a_flip() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");
    let r = done(s.apply_as_agent(event(eva, "residence", Some("1990")), &me(), b));
    let id = r
        .touched
        .iter()
        .copied()
        .find(|i| s.tree().events.contains_key(i))
        .unwrap();
    let mut input = event_input("death", Some("1990"));
    input.id = Some(id);
    let msg = refuse(
        &mut s,
        EditCommand::SaveEvent {
            owner: person_owner(eva),
            event: input,
        },
        &me(),
        b,
    );
    assert!(msg.contains("alive or dead"), "{msg}");
}

#[test]
fn living_override_refused() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let anna = find(s.tree(), "Anna");
    for patch in [
        LivingPatch::Set(LivingOverrideDto::Living),
        LivingPatch::Set(LivingOverrideDto::Deceased),
        LivingPatch::Clear,
    ] {
        refuse(
            &mut s,
            EditCommand::UpdatePerson {
                id: anna,
                sex: None,
                sex_text: None,
                living_override: patch,
                private: None,
            },
            &me(),
            b,
        );
    }
    // Carrying a permitted field along changes nothing.
    refuse(
        &mut s,
        EditCommand::UpdatePerson {
            id: anna,
            sex: Some(SexKind::Female),
            sex_text: None,
            living_override: LivingPatch::Set(LivingOverrideDto::Living),
            private: None,
        },
        &me(),
        b,
    );
    refuse(
        &mut s,
        EditCommand::UpdatePerson {
            id: anna,
            sex: Some(SexKind::Female),
            sex_text: None,
            living_override: LivingPatch::Keep,
            private: Some(true),
        },
        &me(),
        b,
    );
}

#[test]
fn new_person_with_dates_allowed() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");
    let old = done(s.apply_as_agent(
        EditCommand::CreatePerson {
            draft: dated_draft("Ivan", SexKind::Male, "1800"),
        },
        &me(),
        b,
    ))
    .focus
    .unwrap();
    assert!(!living_of(&s, old), "a new person's status is free");
    done(s.apply_as_agent(
        quick_add(
            eva,
            Relation::Child,
            dated_draft("Olena", SexKind::Female, "1990"),
        ),
        &me(),
        b,
    ));
    assert!(living_of(&s, eva));
    s.verify_derived_state().unwrap();
}

/// A person the agent created in an earlier write is an existing person too: the rule is about
/// who exists before this write, not about who made them.
#[test]
fn person_created_earlier_counts_as_existing() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let new = done(s.apply_as_agent(
        EditCommand::CreatePerson {
            draft: draft("Ivan", SexKind::Male),
        },
        &me(),
        b,
    ))
    .focus
    .unwrap();
    assert!(living_of(&s, new));
    let msg = refuse(&mut s, event(new, "death", Some("1990")), &me(), b);
    assert!(msg.contains("alive or dead"), "{msg}");
}

// ---- what is the agent's own ----------------------------------------------------------------

#[test]
fn own_note_editable() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");
    done(s.apply_as_agent(note(eva, "From the recording"), &me(), b));
    let id = newest_note_on(&s, eva);
    done(s.apply_as_agent(
        edit_note(eva, id, "From the recording, corrected"),
        &me(),
        b,
    ));
    assert_eq!(s.tree().notes[&id].text, "From the recording, corrected");
    done(s.apply_as_agent(edit_note(eva, id, "Corrected again"), &me(), b));
    assert_eq!(s.tree().notes[&id].text, "Corrected again");
}

#[test]
fn own_note_not_editable_after_owner_edit() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");
    done(s.apply_as_agent(note(eva, "From the recording"), &me(), b));
    let id = newest_note_on(&s, eva);
    s.apply(edit_note(eva, id, "The owner's wording")).unwrap();
    refuse(&mut s, edit_note(eva, id, "Mine again"), &me(), b);
    assert_eq!(s.tree().notes[&id].text, "The owner's wording");
}

#[test]
fn owner_note_on_the_same_person_leaves_own_note_editable() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");
    done(s.apply_as_agent(note(eva, "From the recording"), &me(), b));
    let id = newest_note_on(&s, eva);
    s.apply(note(eva, "The owner's own note")).unwrap();
    done(s.apply_as_agent(edit_note(eva, id, "Corrected"), &me(), b));
    assert_eq!(s.tree().notes[&id].text, "Corrected");
}

#[test]
fn other_agent_cannot_edit() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let second = batch(&mut s, &other());
    let eva = find(s.tree(), "Eva");
    done(s.apply_as_agent(note(eva, "From the recording"), &me(), b));
    let id = newest_note_on(&s, eva);
    refuse(&mut s, edit_note(eva, id, "Not yours"), &other(), second);
    // A different agent can still add.
    done(s.apply_as_agent(note(eva, "From the second reader"), &other(), second));
}

#[test]
fn own_person_editable_until_someone_else_changes_it() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let new = done(s.apply_as_agent(
        EditCommand::CreatePerson {
            draft: draft("Ivan", SexKind::Unknown),
        },
        &me(),
        b,
    ))
    .focus
    .unwrap();
    done(s.apply_as_agent(update_sex(new, SexKind::Male), &me(), b));
    done(s.apply_as_agent(update_sex(new, SexKind::Female), &me(), b));
    s.apply(update_sex(new, SexKind::Male)).unwrap();
    refuse(&mut s, update_sex(new, SexKind::Female), &me(), b);
}

#[test]
fn sex_on_existing_person_refused() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let karl = find(s.tree(), "Karl");
    refuse(&mut s, update_sex(karl, SexKind::Female), &me(), b);
}

#[test]
fn filled_field_belongs_to_owner() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let source = owner_source(&mut s, "Parish book");
    done(s.apply_as_agent(source_field(source, "O. Koval"), &me(), b));
    refuse(&mut s, source_field(source, "P. Koval"), &me(), b);
    assert_eq!(
        s.tree().sources[&source].author.as_deref(),
        Some("O. Koval")
    );
}

#[test]
fn added_name_keeps_preferred() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let anna = find(s.tree(), "Anna");
    let old = s.tree().persons[&anna].names[0].clone();
    assert!(old.assertion.preferred);
    done(s.apply_as_agent(
        EditCommand::SaveName {
            person: anna,
            name: extra_name("Hanna", true),
        },
        &me(),
        b,
    ));
    let names = &s.tree().persons[&anna].names;
    assert_eq!(names.len(), 2);
    assert_eq!(names[0], old, "the old name is untouched");
    assert!(!names[1].assertion.preferred);
}

#[test]
fn own_added_name_editable_until_the_owner_edits_it() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let anna = find(s.tree(), "Anna");
    done(s.apply_as_agent(
        EditCommand::SaveName {
            person: anna,
            name: extra_name("Hanna", false),
        },
        &me(),
        b,
    ));
    let id = s.tree().persons[&anna].names[1].id;
    // A later note by the owner rewrites the person, but does not touch this name.
    s.apply(note(anna, "The owner's note")).unwrap();
    let mut input = name_input(&s, anna, id);
    input.kind = NameKindDto::Aka;
    input.given = Some("Hannah".into());
    done(s.apply_as_agent(
        EditCommand::SaveName {
            person: anna,
            name: input.clone(),
        },
        &me(),
        b,
    ));
    assert_eq!(
        s.tree().persons[&anna].names[1].given.as_deref(),
        Some("Hannah")
    );

    input.given = Some("Hanne".into());
    s.apply(EditCommand::SaveName {
        person: anna,
        name: input.clone(),
    })
    .unwrap();
    input.given = Some("Hanka".into());
    refuse(
        &mut s,
        EditCommand::SaveName {
            person: anna,
            name: input,
        },
        &me(),
        b,
    );
}

#[test]
fn second_birth_already_in_the_owners_data_can_still_get_a_note() {
    let two_births = "0 @I1@ INDI\n1 NAME Eva /Meyer/\n1 SEX F\n\
                      1 BIRT\n2 DATE 1950\n1 BIRT\n2 DATE 1951\n";
    let (_d, mut s) = open_tree(&tree_of(two_births));
    let b = batch(&mut s, &me());
    let birth = *s.tree().events.keys().next().unwrap();
    done(s.apply_as_agent(
        EditCommand::SaveNote {
            target: OwnerRef {
                kind: OwnerKind::Event,
                id: birth,
            },
            note: NoteInput {
                id: None,
                kind: NoteKindDto::General,
                text: "The sources disagree".into(),
            },
        },
        &me(),
        b,
    ));
}

// ---- repeats --------------------------------------------------------------------------------

#[test]
fn identical_repeat_adds_nothing() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");

    let placed = |id: Uuid| EditCommand::SaveEvent {
        owner: person_owner(id),
        event: EventInput {
            place_text: Some("Lviv, Ukraine".into()),
            ..event_input("residence", Some("1990"))
        },
    };
    for cmd in [
        note(eva, "From the recording"),
        placed(eva),
        cite(eva, "Recording 2", "12:30"),
    ] {
        done(s.apply_as_agent(cmd.clone(), &me(), b));
        let tree = s.tree().clone();
        let groups = logged_groups(&s);
        let AgentWrite::AlreadyThere { record } = s.apply_as_agent(cmd.clone(), &me(), b).unwrap()
        else {
            panic!("{cmd:?} was written twice");
        };
        assert_eq!(s.tree(), &tree);
        assert_eq!(logged_groups(&s), groups);
        assert!(
            tree.notes.contains_key(&record)
                || tree.events.contains_key(&record)
                || tree.citations.contains_key(&record)
        );
    }

    // Anything different is a new fact.
    done(s.apply_as_agent(note(eva, "From the recording, again"), &me(), b));
    done(s.apply_as_agent(event(eva, "residence", Some("1991")), &me(), b));
    done(s.apply_as_agent(cite(eva, "Recording 2", "12:31"), &me(), b));
}

#[test]
fn repeat_is_per_connection_and_per_target() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let second = batch(&mut s, &other());
    let eva = find(s.tree(), "Eva");
    let karl = find(s.tree(), "Karl");
    s.apply(note(eva, "Same words")).unwrap();
    done(s.apply_as_agent(note(eva, "Same words"), &me(), b));
    done(s.apply_as_agent(note(eva, "Same words"), &other(), second));
    done(s.apply_as_agent(note(karl, "Same words"), &me(), b));
    assert_eq!(s.tree().persons[&eva].common.notes.len(), 3);
}

#[test]
fn repeat_of_a_one_of_a_kind_event_is_not_a_refusal() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");
    done(s.apply_as_agent(event(eva, "birth", Some("1990")), &me(), b));
    let again = s
        .apply_as_agent(event(eva, "birth", Some("1990")), &me(), b)
        .unwrap();
    assert!(matches!(again, AgentWrite::AlreadyThere { .. }));
    refuse(&mut s, event(eva, "birth", Some("1991")), &me(), b);
}

// ---- numbers --------------------------------------------------------------------------------

#[test]
fn two_files_share_a_number() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let anna = find(s.tree(), "Anna");
    let karl = find(s.tree(), "Karl");
    done(s.apply_as_agent(number(anna, "one.txt", "12"), &me(), b));
    done(s.apply_as_agent(number(karl, "two.txt", "12"), &me(), b));
    assert_eq!(s.find_by_number("one.txt", "12"), Some(anna));
    assert_eq!(s.find_by_number("two.txt", "12"), Some(karl));
    assert_eq!(s.find_by_number("two.txt", "13"), None);
    assert_eq!(s.find_by_number("three.txt", "12"), None);
}

#[test]
fn number_points_at_one_person() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let anna = find(s.tree(), "Anna");
    let karl = find(s.tree(), "Karl");
    let ids_before = s.tree().persons[&anna].common.ext_ids.len();
    done(s.apply_as_agent(number(anna, "one.txt", "12"), &me(), b));
    refuse(&mut s, number(karl, "one.txt", "12"), &me(), b);
    let again = s
        .apply_as_agent(number(anna, "one.txt", "12"), &me(), b)
        .unwrap();
    assert_eq!(again, AgentWrite::AlreadyThere { record: anna });
    assert_eq!(s.find_by_number("one.txt", "12"), Some(anna));
    assert_eq!(s.tree().persons[&anna].common.ext_ids.len(), ids_before + 1);
}

#[test]
fn numbers_survive_reopening() {
    let (d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let anna = find(s.tree(), "Anna");
    done(s.apply_as_agent(number(anna, "one.txt", "12"), &me(), b));
    s.close();
    let s = Session::open(
        &d.path().join("project"),
        SessionClock::fixed(today(), NOW_MS),
    )
    .unwrap();
    assert_eq!(s.find_by_number("one.txt", "12"), Some(anna));
}

#[test]
fn numbers_not_exported() {
    let (d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let anna = find(s.tree(), "Anna");
    done(s.apply_as_agent(number(anna, "secret-file.txt", "4217"), &me(), b));
    for version in [ExportVersion::V551, ExportVersion::V70] {
        let o = ExportOptions {
            version,
            living: LivingExport::All,
            include_private: true,
        };
        let p = s.preview_export(&o).unwrap();
        let out = d.path().join(format!("out-{version:?}.ged"));
        s.export_gedcom(&o, &out, &p.token).unwrap();
        let text = String::from_utf8_lossy(&std::fs::read(&out).unwrap()).into_owned();
        assert!(text.contains("Anna"), "the person is exported");
        assert!(!text.contains("agent:"), "{text}");
        assert!(!text.contains("secret-file"), "{text}");
        assert!(!text.contains("4217"), "{text}");
    }
}

// ---- batches --------------------------------------------------------------------------------

#[test]
fn closed_batch_refused() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");
    done(s.apply_as_agent(note(eva, "From the recording"), &me(), b));
    s.end_batch(&me(), b, "Added one note.").unwrap();
    let stored = s.project().batch_get(b).unwrap().unwrap();
    assert_eq!(stored.report.as_deref(), Some("Added one note."));
    assert_eq!(stored.ended, Some(NOW_MS));
    refuse(&mut s, note(eva, "After the end"), &me(), b);
    assert_eq!(
        s.end_batch(&me(), b, "Twice").unwrap_err().code,
        ErrorCode::AgentRefused
    );
    let stored = s.project().batch_get(b).unwrap().unwrap();
    assert_eq!(stored.report.as_deref(), Some("Added one note."));
}

#[test]
fn batch_of_someone_else_or_unknown_refused() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");
    refuse(&mut s, note(eva, "Borrowed"), &other(), b);
    refuse(&mut s, note(eva, "Invented"), &me(), Uuid::from_u128(1234));
    assert_eq!(
        s.end_batch(&other(), b, "Not mine").unwrap_err().code,
        ErrorCode::AgentRefused
    );
    assert_eq!(s.project().batch_get(b).unwrap().unwrap().ended, None);
}

#[test]
fn batch_records_title_file_and_connection() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let stored = s.project().batch_get(b).unwrap().unwrap();
    assert_eq!(stored.title, "Recording 2 summary");
    assert_eq!(stored.source_file.as_deref(), Some("recording2.txt"));
    assert_eq!(stored.actor_id, me().id);
    assert_eq!(stored.actor_name, "Transcript reader");
    assert_eq!(stored.ended, None);
}

// ---- how the write is stored ----------------------------------------------------------------

#[test]
fn agent_write_is_logged_and_stays_off_the_owners_stacks() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");
    s.apply(note(eva, "First")).unwrap();
    s.apply(note(eva, "Second")).unwrap();
    s.undo().unwrap();
    let before = s.undo_state();
    assert!(before.can_redo);

    let r = done(s.apply_as_agent(note(eva, "From the recording"), &me(), b));
    assert_eq!(r.undo, before, "undo and redo are untouched");
    assert_eq!(s.undo_state(), before);

    let (groups, _) = s
        .project()
        .log_page(
            &LogFilter {
                batch: Some(b),
                ..Default::default()
            },
            0,
            usize::MAX,
        )
        .unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(
        groups[0].actor,
        Some(Actor::Agent {
            id: me().id,
            name: "Transcript reader".into(),
        })
    );
    assert!(groups[0].people.iter().any(|p| p.id == eva));
}

#[test]
fn reuse_citation_refused_and_last_citation_left_alone() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");
    s.apply(cite(eva, "Owner's book", "p. 1")).unwrap();
    let last = s.reuse_citation().cloned();
    assert!(last.is_some());
    refuse(
        &mut s,
        EditCommand::AddCitation {
            target: person_ref(eva),
            citation: CitationArg::Reuse(ReuseLast { reuse_last: true }),
        },
        &me(),
        b,
    );
    done(s.apply_as_agent(cite(eva, "Recording 2", "12:30"), &me(), b));
    assert_eq!(s.reuse_citation().cloned(), last, "still the owner's");
}

// ---- review round 1 -------------------------------------------------------------------------

/// Ivan has a father recorded alone; Oksana a mother recorded alone. No dates: both count as alive.
const TWO_LINES: &str = "0 @I1@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n1 FAMC @F1@\n\
0 @I2@ INDI\n1 NAME Petro /Koval/\n1 SEX M\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Olena /Koval/\n1 SEX F\n1 FAMS @F2@\n\
0 @I4@ INDI\n1 NAME Oksana /Koval/\n1 SEX F\n1 FAMC @F2@\n\
0 @F1@ FAM\n1 HUSB @I2@\n1 CHIL @I1@\n\
0 @F2@ FAM\n1 WIFE @I3@\n1 CHIL @I4@\n";

fn set_override(s: &mut Session, id: Uuid, patch: LivingPatch) {
    s.apply(EditCommand::UpdatePerson {
        id,
        sex: None,
        sex_text: None,
        living_override: patch,
        private: None,
    })
    .unwrap();
}

fn already(r: Result<AgentWrite, ApiError>) -> Uuid {
    match r.unwrap() {
        AgentWrite::AlreadyThere { record } => record,
        AgentWrite::Done(x) => panic!("stored, not folded: {x:?}"),
    }
}

#[test]
fn death_under_an_owners_living_mark_refused() {
    let (_d, mut s) = open_tree(&tree_of(TWO_LINES));
    let b = batch(&mut s, &me());
    let ivan = find(s.tree(), "Ivan");
    set_override(&mut s, ivan, LivingPatch::Set(LivingOverrideDto::Living));
    let msg = refuse(&mut s, event(ivan, "death", Some("1990")), &me(), b);
    assert!(msg.contains("alive or dead"), "{msg}");
    set_override(&mut s, ivan, LivingPatch::Clear);
    assert!(living_of(&s, ivan), "taking the mark off leaves him alive");
}

#[test]
fn death_under_an_owners_deceased_mark_refused() {
    let (_d, mut s) = open_tree(&tree_of(TWO_LINES));
    let b = batch(&mut s, &me());
    let ivan = find(s.tree(), "Ivan");
    set_override(&mut s, ivan, LivingPatch::Set(LivingOverrideDto::Deceased));
    refuse(&mut s, event(ivan, "death", Some("1990")), &me(), b);
    set_override(&mut s, ivan, LivingPatch::Clear);
    assert!(living_of(&s, ivan));
}

#[test]
fn late_baptism_under_an_owners_deceased_mark_refused() {
    // No relatives: only her own status can change.
    let alone = "0 @I1@ INDI\n1 NAME Eva /Meyer/\n1 SEX F\n1 BIRT\n2 DATE 1850\n";
    let (_d, mut s) = open_tree(&tree_of(alone));
    let b = batch(&mut s, &me());
    let eva = find(s.tree(), "Eva");
    assert!(!living_of(&s, eva), "born 1850");
    set_override(&mut s, eva, LivingPatch::Set(LivingOverrideDto::Deceased));
    refuse(&mut s, event(eva, "baptism", Some("2000")), &me(), b);
    set_override(&mut s, eva, LivingPatch::Clear);
    assert!(!living_of(&s, eva), "taking the mark off leaves her dead");
}

#[test]
fn a_write_that_leaves_the_rule_alone_passes_under_a_mark() {
    let (_d, mut s) = open_tree(&tree_of(TWO_LINES));
    let b = batch(&mut s, &me());
    let ivan = find(s.tree(), "Ivan");
    set_override(&mut s, ivan, LivingPatch::Set(LivingOverrideDto::Deceased));
    done(s.apply_as_agent(note(ivan, "From the recording"), &me(), b));
}

#[test]
fn negative_death_cannot_be_made_positive() {
    let (_d, mut s) = open_tree(&tree_of(TWO_LINES));
    let b = batch(&mut s, &me());
    let ivan = find(s.tree(), "Ivan");
    let negative = EditCommand::SaveEvent {
        owner: person_owner(ivan),
        event: EventInput {
            negative: true,
            ..event_input("death", Some("1991"))
        },
    };
    done(s.apply_as_agent(negative, &me(), b));
    let id = s
        .tree()
        .events
        .values()
        .find(|e| e.negative)
        .unwrap()
        .common
        .id;
    let positive = EditCommand::SaveEvent {
        owner: person_owner(ivan),
        event: EventInput {
            id: Some(id),
            ..event_input("death", Some("1991"))
        },
    };
    refuse(&mut s, positive, &me(), b);
    assert!(living_of(&s, ivan));
}

#[test]
fn second_father_with_the_link_unknown_refused() {
    let (_d, mut s) = open_tree(&tree_of(TWO_LINES));
    let b = batch(&mut s, &me());
    let ivan = find(s.tree(), "Ivan");
    refuse(
        &mut s,
        EditCommand::QuickAdd {
            anchor: ivan,
            relation: Relation::Father,
            draft: draft("Second", SexKind::Male),
            family: None,
            new_parent_set: None,
            parent_rel: Some(ParentRelId::Unknown),
        },
        &me(),
        b,
    );
    let second = done(s.apply_as_agent(
        EditCommand::CreatePerson {
            draft: draft("Second", SexKind::Male),
        },
        &me(),
        b,
    ))
    .focus
    .unwrap();
    refuse(
        &mut s,
        EditCommand::LinkExisting {
            anchor: ivan,
            relation: Relation::Father,
            person: second,
            family: None,
            new_parent_set: None,
            parent_rel: Some(ParentRelId::Unknown),
        },
        &me(),
        b,
    );
}

#[test]
fn partner_joining_as_a_second_father_refused_when_the_free_slot_is_unknown() {
    let (_d, mut s) = open_tree(&tree_of(TWO_LINES));
    let b = batch(&mut s, &me());
    let (ivan, petro) = (find(s.tree(), "Ivan"), find(s.tree(), "Petro"));
    let family = *s
        .tree()
        .families
        .iter()
        .find(|(_, f)| f.children.iter().any(|c| c.person == ivan))
        .unwrap()
        .0;
    // The owner says nothing is known about how Ivan relates to anyone in this family.
    s.apply(EditCommand::UpdateChildLink {
        family,
        child: ivan,
        rel: (ParentRelId::Unknown, ParentRelId::Unknown),
        preferred: true,
    })
    .unwrap();
    // The new partner takes the free slot, and an Unknown link holds a role as a birth link does.
    refuse(
        &mut s,
        EditCommand::QuickAdd {
            anchor: petro,
            relation: Relation::Partner,
            draft: draft("Second", SexKind::Male),
            family: Some(family),
            new_parent_set: None,
            parent_rel: None,
        },
        &me(),
        b,
    );
}

#[test]
fn link_child_refused_when_the_birth_family_link_is_unknown() {
    let (_d, mut s) = open_tree(&tree_of(TWO_LINES));
    let b = batch(&mut s, &me());
    let (ivan, olena) = (find(s.tree(), "Ivan"), find(s.tree(), "Olena"));
    let (first, second) = {
        let t = s.tree();
        let of = |p: Uuid| {
            *t.families
                .iter()
                .find(|(_, f)| f.children.iter().any(|c| c.person == p))
                .unwrap()
                .0
        };
        let second = *t
            .families
            .iter()
            .find(|(_, f)| f.partner2 == Some(olena) || f.partner1 == Some(olena))
            .unwrap()
            .0;
        (of(ivan), second)
    };
    s.apply(EditCommand::UpdateChildLink {
        family: first,
        child: ivan,
        rel: (ParentRelId::Unknown, ParentRelId::Unknown),
        preferred: true,
    })
    .unwrap();
    refuse(
        &mut s,
        EditCommand::LinkChild {
            family: second,
            child: ivan,
            rel: (ParentRelId::Birth, ParentRelId::Birth),
            preferred: false,
        },
        &me(),
        b,
    );
}

#[test]
fn second_father_through_a_later_sex_change_refused() {
    let (_d, mut s) = open_tree(&tree_of(TWO_LINES));
    let b = batch(&mut s, &me());
    let ivan = find(s.tree(), "Ivan");
    let xeno = done(s.apply_as_agent(
        EditCommand::CreatePerson {
            draft: draft("Xeno", SexKind::Unknown),
        },
        &me(),
        b,
    ))
    .focus
    .unwrap();
    done(s.apply_as_agent(link_existing(ivan, Relation::Mother, xeno), &me(), b));
    refuse(&mut s, update_sex(xeno, SexKind::Male), &me(), b);
    done(s.apply_as_agent(update_sex(xeno, SexKind::Female), &me(), b));
}

#[test]
fn second_birth_family_refused_by_every_route() {
    let (_d, mut s) = open_tree(&tree_of(TWO_LINES));
    let b = batch(&mut s, &me());
    let (ivan, oksana, olena) = (
        find(s.tree(), "Ivan"),
        find(s.tree(), "Oksana"),
        find(s.tree(), "Olena"),
    );
    refuse(
        &mut s,
        link_existing(oksana, Relation::Sibling, ivan),
        &me(),
        b,
    );
    refuse(
        &mut s,
        link_existing(olena, Relation::Child, ivan),
        &me(),
        b,
    );
    // Not by birth: she is his step-mother, and he keeps his own family.
    done(s.apply_as_agent(
        EditCommand::LinkExisting {
            anchor: olena,
            relation: Relation::Child,
            person: ivan,
            family: None,
            new_parent_set: None,
            parent_rel: Some(ParentRelId::Step),
        },
        &me(),
        b,
    ));
}

#[test]
fn two_birth_families_in_the_owners_data_still_take_a_note() {
    let two = "0 @I1@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n1 FAMC @F1@\n1 FAMC @F2@\n\
               0 @I2@ INDI\n1 NAME Petro /Koval/\n1 SEX M\n1 FAMS @F1@\n\
               0 @I3@ INDI\n1 NAME Olena /Koval/\n1 SEX F\n1 FAMS @F2@\n\
               0 @F1@ FAM\n1 HUSB @I2@\n1 CHIL @I1@\n\
               0 @F2@ FAM\n1 WIFE @I3@\n1 CHIL @I1@\n";
    let (_d, mut s) = open_tree(&tree_of(two));
    let b = batch(&mut s, &me());
    let ivan = find(s.tree(), "Ivan");
    let family = *s.tree().families.keys().next().unwrap();
    done(s.apply_as_agent(note(ivan, "The sources disagree"), &me(), b));
    done(s.apply_as_agent(
        EditCommand::SaveNote {
            target: OwnerRef {
                kind: OwnerKind::Family,
                id: family,
            },
            note: NoteInput {
                id: None,
                kind: NoteKindDto::General,
                text: "The sources disagree".into(),
            },
        },
        &me(),
        b,
    ));
}

#[test]
fn events_differing_in_any_detail_are_all_stored() {
    let (_d, mut s) = open_tree(&tree_of(TWO_LINES));
    let b = batch(&mut s, &me());
    let ivan = find(s.tree(), "Ivan");
    let occupation = |d: &str| EditCommand::SaveEvent {
        owner: person_owner(ivan),
        event: EventInput {
            description: Some(d.into()),
            ..event_input("occupation", None)
        },
    };
    done(s.apply_as_agent(occupation("Farmer"), &me(), b));
    done(s.apply_as_agent(occupation("Smith"), &me(), b));
    already(s.apply_as_agent(occupation("Smith"), &me(), b));
    let jobs = s
        .tree()
        .events
        .values()
        .filter(|e| e.kind == EventKind::Occupation)
        .count();
    assert_eq!(jobs, 2);

    let emigration = |negative: bool| EditCommand::SaveEvent {
        owner: person_owner(ivan),
        event: EventInput {
            negative,
            ..event_input("emigration", Some("1950"))
        },
    };
    done(s.apply_as_agent(emigration(false), &me(), b));
    done(s.apply_as_agent(emigration(true), &me(), b));

    let aged = |age: Option<&str>, cause: Option<&str>| EditCommand::SaveEvent {
        owner: person_owner(ivan),
        event: EventInput {
            age: age.map(str::to_string),
            cause: cause.map(str::to_string),
            ..event_input("emigration", Some("1960"))
        },
    };
    done(s.apply_as_agent(aged(None, None), &me(), b));
    done(s.apply_as_agent(aged(Some("30"), None), &me(), b));
    done(s.apply_as_agent(aged(None, Some("War")), &me(), b));
}

#[test]
fn citations_differing_in_text_or_date_are_all_stored() {
    let (_d, mut s) = open_tree(&tree_of(TWO_LINES));
    let b = batch(&mut s, &me());
    let ivan = find(s.tree(), "Ivan");
    let quote = |text: &str, date: Option<&str>| EditCommand::AddCitation {
        target: person_ref(ivan),
        citation: CitationArg::New(CitationInput {
            source: SourceRef::New(NewSource {
                title: "Recording".into(),
            }),
            page: None,
            date_raw: date.map(str::to_string),
            text: Some(text.into()),
            quality: None,
            evidence: None,
        }),
    };
    done(s.apply_as_agent(quote("Quote one", None), &me(), b));
    done(s.apply_as_agent(quote("Quote two", None), &me(), b));
    done(s.apply_as_agent(quote("Quote two", Some("1990")), &me(), b));
    already(s.apply_as_agent(quote("Quote two", Some("1990")), &me(), b));
    assert_eq!(s.tree().citations.len(), 3);
}

#[test]
fn identical_source_is_folded_and_any_difference_stored() {
    let (_d, mut s) = open_tree(&tree_of(TWO_LINES));
    let b = batch(&mut s, &me());
    let source = |author: Option<&str>| EditCommand::SaveSource {
        source: SourceInput {
            id: None,
            title: Some("Recording 2".into()),
            author: author.map(str::to_string),
            publication: None,
            abbreviation: None,
            text: None,
        },
    };
    let first = done(s.apply_as_agent(source(None), &me(), b)).touched[0];
    assert_eq!(already(s.apply_as_agent(source(None), &me(), b)), first);
    assert_eq!(s.tree().sources.len(), 1);
    done(s.apply_as_agent(source(Some("Olena Koval")), &me(), b));
    assert_eq!(s.tree().sources.len(), 2);
}

#[test]
fn identical_name_is_folded_and_any_difference_stored() {
    let (_d, mut s) = open_tree(&tree_of(TWO_LINES));
    let b = batch(&mut s, &me());
    let ivan = find(s.tree(), "Ivan");
    let name = |nick: Option<&str>| EditCommand::SaveName {
        person: ivan,
        name: NameInput {
            nickname: nick.map(str::to_string),
            ..extra_name("Vanya", false)
        },
    };
    done(s.apply_as_agent(name(None), &me(), b));
    let id = s.tree().persons[&ivan].names[1].id;
    assert_eq!(already(s.apply_as_agent(name(None), &me(), b)), id);
    assert_eq!(s.tree().persons[&ivan].names.len(), 2);
    done(s.apply_as_agent(name(Some("Vanka")), &me(), b));
    assert_eq!(s.tree().persons[&ivan].names.len(), 3);
}

#[test]
fn identical_family_is_folded_and_any_difference_stored() {
    let (_d, mut s) = open_tree(&tree_of(TWO_LINES));
    let b = batch(&mut s, &me());
    let (ivan, oksana) = (find(s.tree(), "Ivan"), find(s.tree(), "Oksana"));
    let family = |one: Uuid, two: Option<Uuid>, kind: UnionKindId| EditCommand::SaveFamily {
        id: None,
        partner1: Some(one),
        partner2: two,
        kind,
    };
    let count = s.tree().families.len();
    let first = done(s.apply_as_agent(family(ivan, Some(oksana), UnionKindId::Unknown), &me(), b))
        .touched[0];
    assert_eq!(
        already(s.apply_as_agent(family(ivan, Some(oksana), UnionKindId::Unknown), &me(), b)),
        first
    );
    assert_eq!(
        already(s.apply_as_agent(family(oksana, Some(ivan), UnionKindId::Unknown), &me(), b)),
        first,
        "the same two people in the other order"
    );
    assert_eq!(s.tree().families.len(), count + 1);
    done(s.apply_as_agent(family(ivan, Some(oksana), UnionKindId::Married), &me(), b));
    done(s.apply_as_agent(family(ivan, None, UnionKindId::Unknown), &me(), b));
    assert_eq!(s.tree().families.len(), count + 3);
}

#[test]
fn note_repeat_ignores_surrounding_space() {
    let (_d, mut s) = open_tree(&tree_of(TWO_LINES));
    let b = batch(&mut s, &me());
    let ivan = find(s.tree(), "Ivan");
    done(s.apply_as_agent(note(ivan, "Same words"), &me(), b));
    let again = already(s.apply_as_agent(note(ivan, "  Same words\n"), &me(), b));
    assert_eq!(again, newest_note_on(&s, ivan));
}

#[test]
fn second_one_of_a_kind_refusal_names_the_kind() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let b = batch(&mut s, &me());
    let anna = find(s.tree(), "Anna");
    let msg = refuse(&mut s, event(anna, "birth", Some("1851")), &me(), b);
    assert!(msg.contains("birth"), "{msg}");
}

#[test]
fn a_birthday_crossing_the_rule_does_not_stop_every_write() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, Ordering};
    // Alive today at 98. Five years on the rule says dead, but the session has not recomputed.
    let tree = tree_of(
        "0 @I1@ INDI\n1 NAME Eva /Meyer/\n1 SEX F\n1 BIRT\n2 DATE 1928\n\
         0 @I2@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n",
    );
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("project");
    let mut p = gt_store::Project::create(&proj).unwrap();
    p.save_tree(&tree, today()).unwrap();
    drop(p);
    let day = Arc::new(AtomicI32::new(today()));
    let shared = Arc::clone(&day);
    let clock = SessionClock(Box::new(move || (shared.load(Ordering::Relaxed), NOW_MS)));
    let mut s = Session::open(&proj, clock).unwrap();
    let b = batch(&mut s, &me());
    let ivan = find(s.tree(), "Ivan");
    let eva = find(s.tree(), "Eva");
    assert!(living_of(&s, eva));
    day.store(today() + 5 * 366, Ordering::Relaxed);
    done(s.apply_as_agent(note(ivan, "From the recording"), &me(), b));
}
