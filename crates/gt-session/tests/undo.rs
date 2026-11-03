#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use common::*;
use gt_session::*;
use gt_store::Project;
use uuid::Uuid;

fn child_of(anchor: Uuid, name: &str) -> EditCommand {
    EditCommand::QuickAdd {
        anchor,
        relation: Relation::Child,
        draft: draft(name, SexKind::Unknown),
        family: None,
        new_parent_set: None,
    }
}

fn disk_tree(s: &Session) -> gt_core::tree::Tree {
    s.project().load_tree().unwrap()
}

fn setup() -> (tempfile::TempDir, Session, Uuid) {
    let t = tree_of(SAMPLE);
    let anna = find(&t, "Anna");
    let (d, s) = open_tree(&t);
    (d, s, anna)
}

#[test]
fn undo_restores_multi_row_command() {
    let (_d, mut s, anna) = setup();
    let original = disk_tree(&s);
    let mut cmd = child_of(anna, "Greta");
    if let EditCommand::QuickAdd { draft, .. } = &mut cmd {
        draft.birth_date_raw = Some("1880".into());
        draft.birth_place_text = Some("Berlin, Germany".into());
    }
    s.apply(cmd).unwrap();
    assert_ne!(disk_tree(&s), original);
    assert_eq!(s.tree(), &disk_tree(&s));
    let r = s.undo().unwrap().unwrap();
    assert_eq!(r.label, "Undo: Add child");
    assert_eq!(disk_tree(&s), original);
    assert_eq!(s.tree(), &original);
}

#[test]
fn undo_of_delete_restores_order() {
    let (_d, mut s, anna) = setup();
    let original = disk_tree(&s);
    s.apply(EditCommand::DeletePerson { id: anna }).unwrap();
    assert!(!s.tree().persons.contains_key(&anna));
    s.undo().unwrap();
    assert_eq!(disk_tree(&s), original);
    let ids = |t: &gt_core::tree::Tree| t.persons.keys().copied().collect::<Vec<_>>();
    assert_eq!(ids(s.tree()), ids(&original));
    assert_eq!(ids(&disk_tree(&s)), ids(&original));
    // Redo deletes again.
    s.redo().unwrap();
    assert!(!s.tree().persons.contains_key(&anna));
    assert!(!disk_tree(&s).persons.contains_key(&anna));
}

#[test]
fn redo_after_undo() {
    let (_d, mut s, anna) = setup();
    s.apply(child_of(anna, "Greta")).unwrap();
    let after = disk_tree(&s);
    s.undo().unwrap();
    let st = s.undo_state();
    assert!(!st.can_undo && st.can_redo);
    assert_eq!(st.redo_label.as_deref(), Some("Add child"));
    let r = s.redo().unwrap().unwrap();
    assert_eq!(r.label, "Redo: Add child");
    assert_eq!(disk_tree(&s), after);
    assert_eq!(s.tree(), &after);
    assert!(s.redo().unwrap().is_none());
    // And it can be undone again.
    s.undo().unwrap();
    assert_eq!(s.tree().persons.len(), 4);
}

#[test]
fn new_edit_clears_redo() {
    let (_d, mut s, anna) = setup();
    s.apply(child_of(anna, "Greta")).unwrap();
    s.undo().unwrap();
    assert!(s.undo_state().can_redo);
    s.apply(child_of(anna, "Otto")).unwrap();
    assert!(!s.undo_state().can_redo);
    assert!(s.redo().unwrap().is_none());
}

#[test]
fn undo_label_and_state() {
    let (_d, mut s, anna) = setup();
    assert_eq!(
        s.undo_state(),
        UndoState {
            can_undo: false,
            can_redo: false,
            undo_label: None,
            redo_label: None
        }
    );
    assert!(s.undo().unwrap().is_none());
    let r = s.apply(child_of(anna, "Greta")).unwrap();
    assert_eq!(r.label, "Add child");
    assert_eq!(r.undo.undo_label.as_deref(), Some("Add child"));
    assert_eq!(
        r.focus,
        r.touched
            .iter()
            .copied()
            .find(|id| s.tree().persons.contains_key(id) && *id != anna)
    );
    s.apply(EditCommand::DeletePerson { id: anna }).unwrap();
    assert_eq!(s.undo_state().undo_label.as_deref(), Some("Delete person"));
    let u = s.undo().unwrap().unwrap();
    assert_eq!(u.undo.undo_label.as_deref(), Some("Add child"));
    assert_eq!(u.undo.redo_label.as_deref(), Some("Delete person"));
}

#[test]
fn stack_capped_at_200() {
    let (_d, mut s, anna) = setup();
    for i in 0..205 {
        s.apply(EditCommand::UpdatePerson {
            id: anna,
            sex: None,
            sex_text: None,
            living_override: LivingPatch::Keep,
            private: Some(i % 2 == 0),
        })
        .unwrap();
    }
    let mut n = 0;
    while s.undo().unwrap().is_some() {
        n += 1;
    }
    assert_eq!(n, 200);
}

#[test]
fn edit_persisted_before_ack() {
    let t = tree_of(SAMPLE);
    let anna = find(&t, "Anna");
    let (d, mut s) = open_tree(&t);
    let r = s.apply(child_of(anna, "Greta")).unwrap();
    let new = r.focus.unwrap();
    drop(s); // as if the process died right after the acknowledgement
    let s2 = Session::open(
        &d.path().join("project"),
        SessionClock::fixed(today(), NOW_MS),
    )
    .unwrap();
    assert!(s2.tree().persons.contains_key(&new));
    assert_eq!(s2.tree().persons.len(), 5);
}

#[test]
fn living_cache_updates_after_edit() {
    let (_d, mut s, _) = setup();
    let anna = find(s.tree(), "Eva");
    let q = PersonQuery::default();
    let living_of = |s: &Session| {
        s.list_persons(&q, 0, 50)
            .items
            .iter()
            .find(|r| r.person.id == anna)
            .unwrap()
            .person
            .living
    };
    let cached = |s: &Session| -> i64 {
        s.project()
            .conn()
            .query_row(
                "SELECT living_cached FROM person WHERE id = ?1",
                [anna.to_string()],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert!(living_of(&s));
    assert_eq!(cached(&s), 1);
    let death = EditCommand::SaveEvent {
        owner: EventOwner {
            kind: EventOwnerKind::Person,
            id: anna,
        },
        event: EventInput {
            id: None,
            kind: "death".into(),
            date_raw: Some("1 Jan 1900".into()),
            place_text: None,
            place_id: None,
            description: None,
            age: None,
            cause: None,
            negative: false,
            private: false,
            preferred: true,
            participants: None,
        },
    };
    s.apply(death).unwrap();
    assert!(!living_of(&s));
    assert_eq!(cached(&s), 0);
    assert!(!s.get_person(anna).unwrap().person_ref.living);
    s.undo().unwrap();
    assert!(living_of(&s));
    assert_eq!(cached(&s), 1);
}

#[test]
fn history_contains_group_labels() {
    let (_d, mut s, anna) = setup();
    s.apply(EditCommand::UpdatePerson {
        id: anna,
        sex: None,
        sex_text: None,
        living_override: LivingPatch::Keep,
        private: Some(true),
    })
    .unwrap();
    s.undo().unwrap();
    let h = s.history(anna, 10).unwrap();
    let labels: Vec<&str> = h.iter().map(|e| e.label.as_str()).collect();
    assert_eq!(labels, vec!["Undo: Edit person", "Edit person"]);
    assert!(h.iter().all(|e| e.op == HistoryOp::Update));
    assert_eq!(s.history(anna, 1).unwrap().len(), 1);
}

#[test]
fn sorted_list_reflects_rename_without_restart() {
    let (_d, mut s, _) = setup();
    let eva = find(s.tree(), "Eva");
    let q = PersonQuery::default();
    let first = |s: &Session| s.list_persons(&q, 0, 50).items[0].person.name.clone();
    assert_eq!(first(&s), "Eva Meyer");
    let name = s.tree().persons[&eva].names[0].clone();
    s.apply(EditCommand::SaveName {
        person: eva,
        name: NameInput {
            id: Some(name.id),
            kind: NameKindDto::Birth,
            kind_text: None,
            preferred: true,
            private: false,
            prefix: None,
            given: Some("Eva".into()),
            call: None,
            surname_prefix: None,
            surnames: vec!["Zander".into()],
            patronymic: None,
            suffix: None,
            nickname: None,
            lang: None,
            date_raw: None,
        },
    })
    .unwrap();
    let names: Vec<String> = s
        .list_persons(&q, 0, 50)
        .items
        .iter()
        .map(|r| r.person.name.clone())
        .collect();
    assert_eq!(names.last().unwrap(), "Eva Zander");
    assert!(!names.contains(&"Eva Meyer".to_string()));
}

#[test]
fn failed_command_changes_nothing() {
    let (_d, mut s, anna) = setup();
    let before = s.tree().clone();
    let e = s
        .apply(EditCommand::QuickAdd {
            anchor: find(s.tree(), "Hans"),
            relation: Relation::Father,
            draft: draft("X", SexKind::Male),
            family: None,
            new_parent_set: None,
        })
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Conflict);
    assert_eq!(s.tree(), &before);
    assert!(!s.undo_state().can_undo);
    let _ = anna;
}

#[test]
fn reuse_last_citation_through_session_survives_undo() {
    let (_d, mut s, anna) = setup();
    let eva = find(s.tree(), "Eva");
    let add = |target: Uuid, arg: CitationArg| EditCommand::AddCitation {
        target: OwnerRef {
            kind: OwnerKind::Person,
            id: target,
        },
        citation: arg,
    };
    let reuse = || CitationArg::Reuse(ReuseLast { reuse_last: true });
    assert_eq!(
        s.apply(add(anna, reuse())).unwrap_err().code,
        ErrorCode::Invalid
    );
    s.apply(add(
        anna,
        CitationArg::New(CitationInput {
            source: SourceRef::New(NewSource {
                title: "Census".into(),
            }),
            page: Some("12".into()),
            date_raw: None,
            text: None,
            quality: None,
            evidence: None,
        }),
    ))
    .unwrap();
    s.undo().unwrap();
    assert!(s.tree().citations.is_empty());
    s.apply(add(eva, reuse())).unwrap();
    let c = s.tree().citations.values().next().unwrap();
    assert_eq!(c.page.as_deref(), Some("12"));
    assert_eq!(s.tree(), &disk_tree(&s));
    let _ = Project::open;
}
