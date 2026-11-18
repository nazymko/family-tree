#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Godparents of a baptism: the event editor's list, one added or taken off, undo, a deleted
//! godparent, the godchildren on the godparent's page, and what the planner refuses.

mod common;

use common::*;
use gt_core::model::{EventKind, GodparentRole};
use gt_session::*;
use uuid::Uuid;

fn person_owner(id: Uuid) -> EventOwner {
    EventOwner {
        kind: EventOwnerKind::Person,
        id,
    }
}

fn baptism(godparents: Option<Vec<GodparentInput>>) -> EventInput {
    EventInput {
        id: None,
        kind: "baptism".into(),
        date_raw: Some("6 Feb 1906".into()),
        place_text: Some("Ivanivka".into()),
        place_id: None,
        description: None,
        age: None,
        cause: None,
        negative: false,
        private: false,
        preferred: true,
        participants: None,
        godparents,
    }
}

fn by_person(person: Uuid, role: &str) -> GodparentInput {
    GodparentInput {
        id: None,
        person: Some(person),
        name: None,
        role: role.into(),
        note: None,
    }
}

fn by_name(name: &str, role: &str) -> GodparentInput {
    GodparentInput {
        id: None,
        person: None,
        name: Some(name.into()),
        role: role.into(),
        note: Some("a peasant of Ivanivka".into()),
    }
}

fn baptism_of(s: &Session, person: Uuid) -> gt_core::model::Event {
    s.tree()
        .events
        .values()
        .find(|e| e.kind == EventKind::Baptism && gt_session::names::is_principal(e, person))
        .cloned()
        .unwrap()
}

/// Hans has a baptism with Eva (in the tree) and a godmother known only by name.
fn setup() -> (tempfile::TempDir, Session, Uuid, Uuid) {
    let t = tree_of(SAMPLE);
    let hans = find(&t, "Hans");
    let eva = find(&t, "Eva");
    let (d, mut s) = open_tree(&t);
    s.apply(EditCommand::SaveEvent {
        owner: person_owner(hans),
        event: baptism(Some(vec![
            by_person(eva, "godmother"),
            by_name("Ivan Petrovych Koval", "godfather"),
        ])),
    })
    .unwrap();
    (d, s, hans, eva)
}

#[test]
fn the_editor_list_is_stored_read_back_and_shown_both_ways() {
    let (dir, s, hans, eva) = setup();
    let e = baptism_of(&s, hans);
    assert_eq!(e.godparents.len(), 2);
    assert_eq!(e.godparents[0].person, Some(eva));
    assert_eq!(e.godparents[0].role, GodparentRole::Godmother);
    assert_eq!(
        e.godparents[1].name.as_deref(),
        Some("Ivan Petrovych Koval")
    );
    assert_eq!(e.godparents[1].role, GodparentRole::Godfather);

    let detail = s.get_person(hans).unwrap();
    let ev = detail.events.iter().find(|e| e.kind == "baptism").unwrap();
    assert_eq!(ev.godparents.len(), 2);
    assert_eq!(ev.godparents[0].person.as_ref().unwrap().id, eva);
    assert_eq!(
        ev.godparents[1].note.as_deref(),
        Some("a peasant of Ivanivka")
    );

    let godmother = s.get_person(eva).unwrap();
    assert_eq!(godmother.godchildren.len(), 1);
    assert_eq!(godmother.godchildren[0].person.id, hans);
    assert_eq!(godmother.godchildren[0].role, "godmother");
    assert_eq!(godmother.godchildren[0].kind, "baptism");

    // Saved to the project and read back the same.
    let proj = s.project().dir.clone();
    drop(s);
    let s = Session::open(&proj, SessionClock::fixed(today(), NOW_MS)).unwrap();
    assert_eq!(baptism_of(&s, hans).godparents, e.godparents);
    drop(dir);
}

#[test]
fn saving_the_event_without_a_list_keeps_the_godparents_and_ids_stay() {
    let (_d, mut s, hans, _) = setup();
    let e = baptism_of(&s, hans);
    let mut input = baptism(None);
    input.id = Some(e.common.id);
    input.date_raw = Some("7 Feb 1906".into());
    s.apply(EditCommand::SaveEvent {
        owner: person_owner(hans),
        event: input.clone(),
    })
    .unwrap();
    assert_eq!(baptism_of(&s, hans).godparents, e.godparents);

    // The editor sends the list back with the ids it was given: they stay.
    let mut kept = by_name("Ivan Petrovych Koval, a peasant", "godfather");
    kept.id = Some(e.godparents[1].id);
    input.godparents = Some(vec![kept]);
    s.apply(EditCommand::SaveEvent {
        owner: person_owner(hans),
        event: input,
    })
    .unwrap();
    let now = baptism_of(&s, hans).godparents;
    assert_eq!(now.len(), 1);
    assert_eq!(now[0].id, e.godparents[1].id);
    assert_eq!(
        now[0].name.as_deref(),
        Some("Ivan Petrovych Koval, a peasant")
    );
}

#[test]
fn add_and_remove_one_and_undo_brings_it_back() {
    let (_d, mut s, hans, _) = setup();
    let event = baptism_of(&s, hans).common.id;
    let karl = find(s.tree(), "Karl");
    let r = s
        .apply(EditCommand::AddGodparent {
            event,
            godparent: by_person(karl, "godfather"),
        })
        .unwrap();
    assert_eq!(r.focus, Some(hans));
    assert_eq!(baptism_of(&s, hans).godparents.len(), 3);
    assert_eq!(s.get_person(karl).unwrap().godchildren.len(), 1);

    s.undo().unwrap();
    assert_eq!(baptism_of(&s, hans).godparents.len(), 2);
    assert!(s.get_person(karl).unwrap().godchildren.is_empty());
    s.redo().unwrap();
    assert_eq!(baptism_of(&s, hans).godparents.len(), 3);

    let gone = baptism_of(&s, hans).godparents[0].id;
    s.apply(EditCommand::RemoveGodparent {
        event,
        godparent: gone,
    })
    .unwrap();
    assert!(!baptism_of(&s, hans).godparents.iter().any(|g| g.id == gone));
    s.undo().unwrap();
    assert_eq!(baptism_of(&s, hans).godparents[0].id, gone);
}

#[test]
fn a_deleted_godparent_stays_as_the_name_they_had() {
    let (_d, mut s, hans, eva) = setup();
    s.apply(EditCommand::DeletePerson { id: eva }).unwrap();
    let e = baptism_of(&s, hans);
    assert_eq!(e.godparents.len(), 2);
    assert_eq!(e.godparents[0].person, None);
    assert_eq!(e.godparents[0].name.as_deref(), Some("Eva Meyer"));
    s.undo().unwrap();
    assert_eq!(baptism_of(&s, hans).godparents[0].person, Some(eva));
}

#[test]
fn the_planner_refuses_what_is_no_godparent() {
    let (_d, mut s, hans, eva) = setup();
    let event = baptism_of(&s, hans).common.id;
    let refuse = |s: &mut Session, g: GodparentInput| {
        s.apply(EditCommand::AddGodparent {
            event,
            godparent: g,
        })
        .unwrap_err()
        .message
    };
    let both = GodparentInput {
        name: Some("Ivan".into()),
        ..by_person(eva, "godfather")
    };
    assert!(refuse(&mut s, both).contains("one of the two"));
    let neither = GodparentInput {
        name: Some("   ".into()),
        ..by_name("x", "godfather")
    };
    assert!(refuse(&mut s, neither).contains("one of the two"));
    assert!(refuse(&mut s, by_person(hans, "godfather")).contains("own godparent"));
    assert!(refuse(&mut s, by_name("Ivan", "uncle")).contains("not a godparent"));
    assert!(refuse(&mut s, by_person(eva, "godmother")).contains("already"));
    assert!(refuse(&mut s, by_name(" ivan  petrovych KOVAL ", "godfather")).contains("already"));

    // Only a baptism or christening has godparents.
    let birth = s
        .tree()
        .events
        .values()
        .find(|e| e.kind == EventKind::Birth)
        .unwrap()
        .common
        .id;
    let err = s
        .apply(EditCommand::AddGodparent {
            event: birth,
            godparent: by_name("Ivan", "godfather"),
        })
        .unwrap_err();
    assert!(err.message.contains("baptism or christening"), "{err:?}");

    // Twice in one list.
    let mut input = baptism(Some(vec![
        by_name("Maria", "godmother"),
        by_name("maria", "godmother"),
    ]));
    input.id = Some(event);
    let err = s
        .apply(EditCommand::SaveEvent {
            owner: person_owner(hans),
            event: input,
        })
        .unwrap_err();
    assert!(err.message.contains("twice"), "{err:?}");
}
