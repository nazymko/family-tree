#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Attribution, the log, and undoing from it. Agent writes go through `commit_command_as`.

mod common;

use common::*;
use gt_core::tree::Tree;
use gt_session::*;
use gt_store::{Actor, Batch, Project};
use uuid::Uuid;

fn agent() -> Actor {
    Actor::Agent {
        id: Uuid::from_u128(7),
        name: "Transcript reader".into(),
    }
}

fn person_owner(id: Uuid) -> OwnerRef {
    OwnerRef {
        kind: OwnerKind::Person,
        id,
    }
}

fn add_note(id: Uuid, text: &str) -> EditCommand {
    EditCommand::SaveNote {
        target: person_owner(id),
        note: NoteInput {
            id: None,
            kind: NoteKindDto::General,
            text: text.into(),
        },
    }
}

fn edit_note(owner: Uuid, note: Uuid, text: &str) -> EditCommand {
    EditCommand::SaveNote {
        target: person_owner(owner),
        note: NoteInput {
            id: Some(note),
            kind: NoteKindDto::General,
            text: text.into(),
        },
    }
}

fn child_of(anchor: Uuid, name: &str) -> EditCommand {
    EditCommand::QuickAdd {
        anchor,
        relation: Relation::Child,
        draft: draft(name, SexKind::Unknown),
        family: None,
        new_parent_set: None,
        parent_rel: None,
    }
}

fn link_child(family: Uuid, child: Uuid) -> EditCommand {
    EditCommand::LinkChild {
        family,
        child,
        rel: (ParentRelId::Birth, ParentRelId::Birth),
        preferred: true,
    }
}

fn birth(owner: Uuid, raw: &str) -> EditCommand {
    EditCommand::SaveEvent {
        owner: EventOwner {
            kind: EventOwnerKind::Person,
            id: owner,
        },
        event: EventInput {
            id: None,
            kind: "birth".into(),
            date_raw: Some(raw.into()),
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
    }
}

fn cite_source(owner: Uuid, source: SourceRef) -> EditCommand {
    EditCommand::AddCitation {
        target: person_owner(owner),
        citation: CitationArg::New(CitationInput {
            source,
            page: Some("12".into()),
            date_raw: None,
            text: None,
            quality: None,
            evidence: None,
        }),
    }
}

fn cite(owner: Uuid, title: &str) -> EditCommand {
    cite_source(
        owner,
        SourceRef::New(NewSource {
            title: title.into(),
        }),
    )
}

/// The tree with every `changed` stamp cleared.
fn unstamped(t: &Tree) -> Tree {
    let mut t = t.clone();
    t.persons.values_mut().for_each(|r| r.common.changed = None);
    t.families
        .values_mut()
        .for_each(|r| r.common.changed = None);
    t.events.values_mut().for_each(|r| r.common.changed = None);
    t.places.values_mut().for_each(|r| r.common.changed = None);
    t.sources.values_mut().for_each(|r| r.common.changed = None);
    t.citations
        .values_mut()
        .for_each(|r| r.common.changed = None);
    t.notes.values_mut().for_each(|r| r.common.changed = None);
    t.media.values_mut().for_each(|r| r.common.changed = None);
    t.repositories
        .values_mut()
        .for_each(|r| r.common.changed = None);
    t
}

fn setup() -> (tempfile::TempDir, Session, Uuid) {
    let t = tree_of(SAMPLE);
    let anna = find(&t, "Anna");
    let (d, s) = open_tree(&t);
    (d, s, anna)
}

fn rows(s: &Session) -> Vec<LogRowDto> {
    s.log(LogFilterDto::default(), 0, 200).unwrap().items
}

fn newest_group(s: &Session) -> Uuid {
    rows(s)[0].group
}

fn reopen(d: &tempfile::TempDir, s: Session) -> Session {
    s.close();
    Session::open(
        &d.path().join("project"),
        SessionClock::fixed(today(), NOW_MS + 1000),
    )
    .unwrap()
}

fn with_batch(d: &tempfile::TempDir, s: Session, id: Uuid, title: &str) -> Session {
    s.close();
    let dir = d.path().join("project");
    let mut p = Project::open(&dir).unwrap();
    p.batch_create(&Batch {
        id,
        title: title.into(),
        source_file: Some("recording-2.md".into()),
        actor_id: Uuid::from_u128(7),
        actor_name: "Transcript reader".into(),
        started: NOW_MS,
        ended: None,
        report: None,
    })
    .unwrap();
    drop(p);
    Session::open(&dir, SessionClock::fixed(today(), NOW_MS)).unwrap()
}

#[test]
fn user_edit_logged_as_user() {
    let (_d, mut s, anna) = setup();
    s.apply(add_note(anna, "Told by the family")).unwrap();
    let r = rows(&s);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].actor, ActorDto::User);
    assert_eq!(r[0].label, "Save note");
    assert!(!r[0].can_undo, "a user's change is undone with Ctrl+Z");
    assert!(r[0].people.iter().any(|p| p.id == anna));
    assert_eq!(s.unreviewed_people().unwrap(), Vec::<Uuid>::new());
}

#[test]
fn agent_group_lists_people_of_family_link() {
    let (_d, mut s, anna) = setup();
    let karl = find(s.tree(), "Karl");
    let hans = find(s.tree(), "Hans");
    let eva = find(s.tree(), "Eva");
    let family = *s.tree().families.keys().next().unwrap();
    s.commit_command_as(link_child(family, eva), agent(), None)
        .unwrap();
    let r = rows(&s);
    assert_eq!(
        r[0].actor,
        ActorDto::Agent {
            id: Uuid::from_u128(7),
            name: "Transcript reader".into()
        }
    );
    assert!(r[0].can_undo);
    let people: Vec<Uuid> = r[0].people.iter().map(|p| p.id).collect();
    for who in [anna, karl, hans, eva] {
        assert!(people.contains(&who), "{who} missing from {people:?}");
    }
    let mut unreviewed = s.unreviewed_people().unwrap();
    unreviewed.sort();
    let mut want = vec![anna, karl, hans, eva];
    want.sort();
    assert_eq!(unreviewed, want);
}

#[test]
fn a_marker_in_the_log_says_it_has_no_person_page() {
    let (_d, mut s, _anna) = setup();
    let family = *s.tree().families.keys().next().unwrap();
    s.commit_command_as(EditCommand::AddSiblingMarker { family }, agent(), None)
        .unwrap();
    let marker = s
        .tree()
        .persons
        .values()
        .find(|p| p.sibling_marker)
        .unwrap()
        .common
        .id;
    let r = rows(&s);
    let entry = r[0].people.iter().find(|p| p.id == marker).unwrap();
    assert!(entry.exists);
    assert!(entry.sibling_marker);
    assert!(
        r[0].people
            .iter()
            .filter(|p| p.id != marker)
            .all(|p| !p.sibling_marker)
    );
}

#[test]
fn deleted_people_are_still_named() {
    let (_d, mut s, anna) = setup();
    let p = s
        .commit_command_as(child_of(anna, "Greta"), agent(), None)
        .unwrap()
        .focus
        .unwrap();
    s.commit_command_as(EditCommand::DeletePerson { id: p }, agent(), None)
        .unwrap();
    let r = rows(&s);
    let gone = r[0].people.iter().find(|x| x.id == p).unwrap();
    assert!(!gone.exists);
    assert!(gone.name.contains("Greta"), "{}", gone.name);
}

#[test]
fn undo_group_restores_tree_exactly() {
    let (_d, mut s, anna) = setup();
    let original = unstamped(s.tree());
    s.commit_command_as(child_of(anna, "Greta"), agent(), None)
        .unwrap();
    let g1 = newest_group(&s);
    s.commit_command_as(add_note(anna, "From the recording"), agent(), None)
        .unwrap();
    let g2 = newest_group(&s);
    s.commit_command_as(cite(anna, "Recording 2"), agent(), None)
        .unwrap();
    let g3 = newest_group(&s);
    for g in [g3, g2, g1] {
        let rep = s.undo_group(g).unwrap();
        assert!(rep.undone > 0);
        assert!(rep.skipped.is_empty(), "{:?}", rep.skipped);
    }
    assert_eq!(unstamped(s.tree()), original);
    assert_eq!(unstamped(&s.project().load_tree().unwrap()), original);
    s.verify_derived_state().unwrap();
}

#[test]
fn undo_group_refuses_user_groups_and_unknown_ids() {
    let (_d, mut s, anna) = setup();
    s.apply(add_note(anna, "Mine")).unwrap();
    let e = s.undo_group(newest_group(&s)).unwrap_err();
    assert_eq!(e.code, ErrorCode::Invalid);
    assert_eq!(
        s.undo_group(Uuid::from_u128(5)).unwrap_err().code,
        ErrorCode::NotFound
    );
    assert_eq!(
        s.undo_batch(Uuid::from_u128(5)).unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[test]
fn undo_batch_survives_reopen() {
    let (d, s, anna) = setup();
    let batch = Uuid::from_u128(44);
    let mut s = with_batch(&d, s, batch, "Recording 2 summary");
    let original = unstamped(s.tree());
    s.commit_command_as(child_of(anna, "Greta"), agent(), Some(batch))
        .unwrap();
    let greta = find(s.tree(), "Greta");
    s.commit_command_as(add_note(greta, "From the recording"), agent(), Some(batch))
        .unwrap();
    s.commit_command_as(birth(greta, "1 Jan 1900"), agent(), Some(batch))
        .unwrap();
    s.commit_command_as(cite(anna, "Recording 2"), agent(), Some(batch))
        .unwrap();
    let mut s = reopen(&d, s);
    let rep = s.undo_batch(batch).unwrap();
    assert!(rep.skipped.is_empty(), "{:?}", rep.skipped);
    assert!(rep.undone >= 4);
    assert_eq!(unstamped(s.tree()), original);
    assert_eq!(unstamped(&s.project().load_tree().unwrap()), original);
    s.verify_derived_state().unwrap();
    // And again after another restart: nothing left to do.
    let mut s = reopen(&d, s);
    let again = s.undo_batch(batch).unwrap();
    assert_eq!(again.undone, 0);
    assert!(again.skipped.is_empty());
    assert_eq!(unstamped(s.tree()), original);
}

#[test]
fn undo_batch_keeps_a_person_the_owner_built_on_and_undoes_the_rest() {
    let (d, s, anna) = setup();
    let batch = Uuid::from_u128(44);
    let mut s = with_batch(&d, s, batch, "Recording 2 summary");
    let family = *s.tree().families.keys().next().unwrap();
    s.commit_command_as(child_of(anna, "Greta"), agent(), Some(batch))
        .unwrap();
    let greta = find(s.tree(), "Greta");
    s.commit_command_as(child_of(anna, "Otto"), agent(), Some(batch))
        .unwrap();
    let otto = find(s.tree(), "Otto");
    s.commit_command_as(add_note(anna, "A note to undo"), agent(), Some(batch))
        .unwrap();
    // The owner builds on Greta.
    s.apply(add_note(greta, "The owner's own note")).unwrap();

    let rep = s.undo_batch(batch).unwrap();
    assert!(s.tree().persons.contains_key(&greta), "kept");
    assert!(!s.tree().persons.contains_key(&otto), "undone");
    let kids: Vec<Uuid> = s.tree().families[&family]
        .children
        .iter()
        .map(|c| c.person)
        .collect();
    assert!(kids.contains(&greta), "the link stays with the person");
    assert!(!kids.contains(&otto));
    assert_eq!(s.tree().notes.len(), 1, "only the owner's note is left");
    assert!(s.tree().persons[&anna].common.notes.is_empty());
    assert!(
        rep.skipped
            .iter()
            .any(|f| f.kind == "person" && f.person.as_ref().map(|p| p.id) == Some(greta)),
        "{:?}",
        rep.skipped
    );
    assert_eq!(s.tree(), &s.project().load_tree().unwrap());
    s.verify_derived_state().unwrap();
}

#[test]
fn a_record_something_else_now_points_at_stays() {
    let (_d, mut s, _anna) = setup();
    // The agent adds a person; the owner only links them from elsewhere.
    let p = s
        .commit_command_as(
            EditCommand::CreatePerson {
                draft: draft("Greta", SexKind::Female),
            },
            agent(),
            None,
        )
        .unwrap()
        .focus
        .unwrap();
    let g = newest_group(&s);
    let family = *s.tree().families.keys().next().unwrap();
    s.apply(link_child(family, p)).unwrap();
    let rep = s.undo_group(g).unwrap();
    assert!(s.tree().persons.contains_key(&p));
    assert_eq!(rep.skipped.len(), 1, "{:?}", rep.skipped);
    assert_eq!(s.tree(), &s.project().load_tree().unwrap());
    s.verify_derived_state().unwrap();
}

#[test]
fn undo_twice_is_harmless() {
    let (_d, mut s, anna) = setup();
    s.commit_command_as(add_note(anna, "From the recording"), agent(), None)
        .unwrap();
    let g = newest_group(&s);
    assert!(s.undo_group(g).unwrap().undone > 0);
    let tree = s.tree().clone();
    let n = rows(&s).len();
    let again = s.undo_group(g).unwrap();
    assert_eq!(again.undone, 0);
    assert!(again.skipped.is_empty());
    assert_eq!(s.tree(), &tree);
    assert_eq!(rows(&s).len(), n);
}

#[test]
fn undo_is_logged_as_user() {
    let (_d, mut s, anna) = setup();
    s.commit_command_as(add_note(anna, "From the recording"), agent(), None)
        .unwrap();
    let g = newest_group(&s);
    s.undo_group(g).unwrap();
    let r = rows(&s);
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].label, "Undo: Save note");
    assert_eq!(r[0].actor, ActorDto::User);
    assert!(!r[0].can_undo);
    assert_eq!(r[1].group, g);
    assert!(r[1].reverted);
    assert!(!r[1].can_undo);
    let log = s.project().log_group(r[0].group).unwrap().unwrap();
    assert_eq!(log.reverts, Some(g));
    assert_eq!(s.unreviewed_people().unwrap(), Vec::<Uuid>::new());
}

#[test]
fn ctrl_z_keeps_agent_note_added_later() {
    let (_d, mut s, anna) = setup();
    let original_name = s.tree().persons[&anna].names[0].clone();
    let rename = EditCommand::SaveName {
        person: anna,
        name: NameInput {
            id: Some(original_name.id),
            kind: NameKindDto::Birth,
            kind_text: None,
            preferred: true,
            private: false,
            prefix: None,
            given: Some("Hanna".into()),
            call: None,
            surname_prefix: None,
            surnames: vec!["Schmidt".into()],
            patronymic: None,
            suffix: None,
            nickname: None,
            lang: None,
            date_raw: None,
        },
    };
    s.apply(rename).unwrap();
    s.commit_command_as(add_note(anna, "From the recording"), agent(), None)
        .unwrap();
    let note = s.tree().persons[&anna].common.notes[0];

    let r = s.undo().unwrap().unwrap();
    assert_eq!(r.label, "Undo: Edit name");
    let p = &s.tree().persons[&anna];
    assert_eq!(p.names[0].given.as_deref(), Some("Anna"));
    assert_eq!(p.common.notes, vec![note], "the agent's note stays");
    assert_eq!(p.common.changed, Some(NOW_MS), "a merged write is stamped");
    assert_eq!(s.tree(), &s.project().load_tree().unwrap());

    let r = s.redo().unwrap().unwrap();
    assert_eq!(r.label, "Redo: Edit name");
    let p = &s.tree().persons[&anna];
    assert_eq!(p.names[0].given.as_deref(), Some("Hanna"));
    assert_eq!(p.common.notes, vec![note]);
    assert_eq!(s.tree(), &s.project().load_tree().unwrap());

    s.undo().unwrap();
    assert_eq!(
        s.tree().persons[&anna].names[0].given.as_deref(),
        Some("Anna")
    );
    s.verify_derived_state().unwrap();
}

#[test]
fn ctrl_z_stops_when_agent_changed_same_fact() {
    let (_d, mut s, anna) = setup();
    s.apply(add_note(anna, "Mine")).unwrap();
    let note = s.tree().persons[&anna].common.notes[0];
    s.commit_command_as(edit_note(anna, note, "Mine, corrected"), agent(), None)
        .unwrap();
    let tree = s.tree().clone();
    let state = s.undo_state();
    let e = s.undo().unwrap_err();
    assert_eq!(e.code, ErrorCode::Conflict);
    assert_eq!(
        e.message,
        "An agent changed this since. Undo it from the change log first."
    );
    assert_eq!(s.tree(), &tree);
    assert_eq!(s.undo_state(), state, "the stacks stay as they were");
    // Undoing the agent's change from the log clears the way.
    s.undo_group(newest_group(&s)).unwrap();
    s.undo().unwrap().unwrap();
    assert!(s.tree().notes.is_empty());
}

#[test]
fn ctrl_z_stops_rather_than_dangling_a_later_reference() {
    let (_d, mut s, anna) = setup();
    let eva = find(s.tree(), "Eva");
    s.apply(cite(anna, "Family bible")).unwrap();
    let source = *s.tree().sources.keys().next().unwrap();
    s.commit_command_as(cite_source(eva, SourceRef::Id(source)), agent(), None)
        .unwrap();
    let e = s.undo().unwrap_err();
    assert_eq!(e.code, ErrorCode::Conflict);
    assert!(s.tree().sources.contains_key(&source));
    assert!(s.undo_state().can_undo);
}

#[test]
fn agent_changes_not_on_ctrl_z_stack() {
    let (_d, mut s, anna) = setup();
    s.commit_command_as(add_note(anna, "From the recording"), agent(), None)
        .unwrap();
    assert!(!s.undo_state().can_undo);
    assert!(s.undo().unwrap().is_none());
    assert_eq!(s.tree().notes.len(), 1);
    // And they leave a pending redo alone.
    s.apply(add_note(anna, "Mine")).unwrap();
    s.undo().unwrap();
    s.commit_command_as(add_note(anna, "Another"), agent(), None)
        .unwrap();
    assert!(s.undo_state().can_redo);
}

#[test]
fn log_change_detail_is_readable() {
    let (_d, mut s, anna) = setup();
    let long = "A very long story told at the kitchen table. ".repeat(10);
    s.commit_command_as(add_note(anna, &long), agent(), None)
        .unwrap();
    let r = rows(&s);
    let n = r[0].changes.iter().find(|c| c.kind == "note").unwrap();
    assert_eq!(n.op, HistoryOp::Create);
    assert!(n.detail.starts_with("A very long story"), "{}", n.detail);
    assert!(n.detail.chars().count() <= 81, "{}", n.detail);

    s.commit_command_as(birth(find(s.tree(), "Eva"), "1 Jan 1900"), agent(), None)
        .unwrap();
    let r = rows(&s);
    let e = r[0].changes.iter().find(|c| c.kind == "event").unwrap();
    assert!(e.detail.contains("Birth"), "{}", e.detail);
    assert!(e.detail.contains("1900"), "{}", e.detail);

    s.commit_command_as(cite(anna, "Recording 2"), agent(), None)
        .unwrap();
    let r = rows(&s);
    let c = r[0].changes.iter().find(|c| c.kind == "citation").unwrap();
    assert!(c.detail.contains("Recording 2"), "{}", c.detail);
    assert_eq!(c.sources, vec!["Recording 2".to_string()]);
    let p = r[0].changes.iter().find(|c| c.kind == "person").unwrap();
    assert_eq!(p.sources, vec!["Recording 2".to_string()]);
    assert!(p.detail.contains("Anna"), "{}", p.detail);

    // Still readable once the records are gone.
    let g = r[0].group;
    s.undo_group(g).unwrap();
    let r = rows(&s);
    let c = r[1].changes.iter().find(|c| c.kind == "citation").unwrap();
    assert_eq!(c.sources, vec!["Recording 2".to_string()]);
}

#[test]
fn log_change_fact_is_structured_for_translation() {
    let (_d, mut s, anna) = setup();
    s.commit_command_as(add_note(anna, "From the recording"), agent(), None)
        .unwrap();
    let r = rows(&s);
    let n = r[0].changes.iter().find(|c| c.kind == "note").unwrap();
    assert_eq!(
        n.fact,
        LogFactDto::Text {
            text: "From the recording".into()
        }
    );

    s.commit_command_as(birth(find(s.tree(), "Eva"), "1 Jan 1900"), agent(), None)
        .unwrap();
    let r = rows(&s);
    let e = r[0].changes.iter().find(|c| c.kind == "event").unwrap();
    assert_eq!(
        e.fact,
        LogFactDto::Event {
            event: "birth".into(),
            label: "Birth".into(),
            date: Some("1 Jan 1900".into()),
            place: None,
        }
    );

    let family = *s.tree().families.keys().next().unwrap();
    let eva = find(s.tree(), "Eva");
    s.commit_command_as(link_child(family, eva), agent(), None)
        .unwrap();
    let r = rows(&s);
    let f = r[0].changes.iter().find(|c| c.kind == "family").unwrap();
    let LogFactDto::Family { partners } = &f.fact else {
        panic!("{:?}", f.fact);
    };
    assert_eq!(partners.join(" and "), f.detail);
}

#[test]
fn skipped_fact_is_structured_for_translation() {
    let (_d, mut s, anna) = setup();
    s.commit_command_as(add_note(anna, "From the recording"), agent(), None)
        .unwrap();
    let group = rows(&s)[0].group;
    let note = *s.tree().notes.keys().next().unwrap();
    s.apply(edit_note(anna, note, "Mine now")).unwrap();
    let report = s.undo_group(group).unwrap();
    let skipped = report.skipped.iter().find(|f| f.kind == "note").unwrap();
    // The fact as the agent wrote it: that is what stayed unreverted, whatever it says now.
    assert_eq!(
        skipped.fact,
        LogFactDto::Text {
            text: "From the recording".into()
        }
    );
}

#[test]
fn log_agents_are_every_author_once_with_the_latest_name() {
    let (_d, mut s, anna) = setup();
    assert!(s.log_agents().unwrap().is_empty());
    s.apply(add_note(anna, "Mine")).unwrap();
    let other = Actor::Agent {
        id: Uuid::from_u128(8),
        name: "Archive helper".into(),
    };
    s.commit_command_as(add_note(anna, "One"), agent(), None)
        .unwrap();
    s.commit_command_as(add_note(anna, "Two"), other, None)
        .unwrap();
    let renamed = Actor::Agent {
        id: Uuid::from_u128(7),
        name: "Zoe the reader".into(),
    };
    s.commit_command_as(add_note(anna, "Three"), renamed, None)
        .unwrap();
    let agents = s.log_agents().unwrap();
    let got: Vec<(Uuid, &str)> = agents.iter().map(|a| (a.id, a.name.as_str())).collect();
    assert_eq!(
        got,
        vec![
            (Uuid::from_u128(8), "Archive helper"),
            (Uuid::from_u128(7), "Zoe the reader")
        ]
    );
}

#[test]
fn a_batch_counts_the_changes_that_still_stand() {
    let (d, s, anna) = setup();
    let batch = Uuid::from_u128(44);
    let mut s = with_batch(&d, s, batch, "Recording 2 summary");
    let eva = find(s.tree(), "Eva");
    s.commit_command_as(add_note(anna, "One"), agent(), Some(batch))
        .unwrap();
    let first = newest_group(&s);
    s.commit_command_as(add_note(eva, "Two"), agent(), Some(batch))
        .unwrap();
    assert_eq!(s.batches().unwrap()[0].standing, 2);
    s.undo_group(first).unwrap();
    let b = s.batches().unwrap();
    assert_eq!((b[0].groups, b[0].standing), (2, 1));
    s.undo_batch(batch).unwrap();
    assert_eq!(s.batches().unwrap()[0].standing, 0);
}

#[test]
fn log_filters_reviews_and_batches() {
    let (d, s, anna) = setup();
    let batch = Uuid::from_u128(44);
    let mut s = with_batch(&d, s, batch, "Recording 2 summary");
    let eva = find(s.tree(), "Eva");
    s.apply(add_note(eva, "Mine")).unwrap();
    s.commit_command_as(add_note(anna, "One"), agent(), Some(batch))
        .unwrap();
    s.commit_command_as(add_note(eva, "Two"), agent(), Some(batch))
        .unwrap();

    let by_agent = s
        .log(
            LogFilterDto {
                actor_kind: Some("agent".into()),
                ..Default::default()
            },
            0,
            10,
        )
        .unwrap();
    assert_eq!(by_agent.total, 2);
    assert_eq!(
        by_agent.items[0].batch.as_ref().map(|b| b.title.as_str()),
        Some("Recording 2 summary")
    );
    let page = s
        .log(
            LogFilterDto {
                batch: Some(batch),
                ..Default::default()
            },
            1,
            1,
        )
        .unwrap();
    assert_eq!((page.total, page.offset, page.items.len()), (2, 1, 1));

    let b = s.batches().unwrap();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].title, "Recording 2 summary");
    assert_eq!(b[0].source_file.as_deref(), Some("recording-2.md"));
    assert_eq!(b[0].actor_name, "Transcript reader");
    assert_eq!(b[0].actor_id, Uuid::from_u128(7));
    assert_eq!((b[0].groups, b[0].people, b[0].unreviewed), (2, 2, 2));

    assert_eq!(s.person_agent_changes(anna, 0, 20).unwrap().items.len(), 1);
    assert_eq!(s.person_agent_changes(eva, 0, 20).unwrap().items.len(), 1);

    s.mark_reviewed(ReviewScopeDto::Person { id: anna })
        .unwrap();
    assert_eq!(s.unreviewed_people().unwrap(), vec![eva]);
    assert_eq!(s.batches().unwrap()[0].unreviewed, 1);
    let unreviewed = s
        .log(
            LogFilterDto {
                unreviewed_only: true,
                ..Default::default()
            },
            0,
            10,
        )
        .unwrap();
    assert_eq!(unreviewed.total, 1);
    s.mark_reviewed(ReviewScopeDto::Batch { id: batch })
        .unwrap();
    assert!(s.unreviewed_people().unwrap().is_empty());
    let row = &s.person_agent_changes(anna, 0, 20).unwrap().items[0];
    assert!(row.people.iter().all(|p| p.reviewed.is_some()));

    // Undoing takes the row off the person page but never out of the log.
    let g = row.group;
    s.undo_group(g).unwrap();
    assert!(
        s.person_agent_changes(anna, 0, 20)
            .unwrap()
            .items
            .is_empty()
    );
    assert!(rows(&s).iter().any(|r| r.group == g && r.reverted));
}

fn import_two(s: &mut Session) -> (Uuid, Uuid, Uuid) {
    let person = |key: &str| DraftPerson {
        key: key.into(),
        existing: None,
        given: Some(key.into()),
        surname: Some("Koval".into()),
        patronymic: None,
        sex: SexKind::Unknown,
        birth_date_raw: None,
        death_date_raw: None,
        notes: None,
    };
    s.commit_command_as(
        EditCommand::ImportDraft {
            persons: vec![person("Aaa"), person("Bbb")],
            families: vec![],
            attach_picture: false,
        },
        agent(),
        None,
    )
    .unwrap();
    let g = newest_group(s);
    (g, find(s.tree(), "Aaa"), find(s.tree(), "Bbb"))
}

fn row_of(s: &Session, group: Uuid) -> LogRowDto {
    rows(s).into_iter().find(|r| r.group == group).unwrap()
}

fn assert_consistent(s: &Session) {
    assert_eq!(s.tree(), &s.project().load_tree().unwrap());
    s.verify_derived_state().unwrap();
}

fn remove_citation(owner: Uuid, citation: Uuid) -> EditCommand {
    EditCommand::RemoveCitation {
        target: person_owner(owner),
        citation,
    }
}

#[test]
fn partly_undone_group_keeps_its_badge_and_a_retry_finishes_it() {
    let (_d, mut s, _) = setup();
    let original = unstamped(s.tree());
    let (g, aaa, bbb) = import_two(&mut s);
    s.apply(add_note(aaa, "The owner's own note")).unwrap();

    let rep = s.undo_group(g).unwrap();
    assert!(rep.undone > 0, "what could be undone is undone");
    assert!(!rep.skipped.is_empty());
    assert!(s.tree().persons.contains_key(&aaa), "the owner built on it");
    assert!(!s.tree().persons.contains_key(&bbb));
    let row = row_of(&s, g);
    assert!(!row.reverted, "something of it still stands");
    assert!(row.can_undo);
    assert!(s.unreviewed_people().unwrap().contains(&aaa));
    assert_eq!(s.person_agent_changes(aaa, 0, 20).unwrap().items.len(), 1);
    assert_eq!(
        s.project()
            .log_group(rows(&s)[0].group)
            .unwrap()
            .unwrap()
            .reverts,
        None,
        "the partial undo does not claim the group"
    );
    assert_consistent(&s);

    // Nothing has moved: a retry finds the same blocker and writes nothing.
    let n = rows(&s).len();
    let tree = s.tree().clone();
    let same = s.undo_group(g).unwrap();
    assert_eq!(same.undone, 0);
    assert!(!same.skipped.is_empty());
    assert_eq!(s.tree(), &tree);
    assert_eq!(rows(&s).len(), n);
    assert!(!row_of(&s, g).reverted);

    // The owner takes the note back: now it can finish, and only now is it reverted.
    s.undo().unwrap().unwrap();
    let done = s.undo_group(g).unwrap();
    assert!(done.undone > 0);
    assert!(done.skipped.is_empty(), "{:?}", done.skipped);
    assert!(!s.tree().persons.contains_key(&aaa));
    let row = row_of(&s, g);
    assert!(row.reverted);
    assert!(!row.can_undo);
    assert!(s.unreviewed_people().unwrap().is_empty());
    assert!(s.person_agent_changes(aaa, 0, 20).unwrap().items.is_empty());
    assert_eq!(unstamped(s.tree()), original);
    assert_consistent(&s);
}

#[test]
fn a_group_nothing_could_be_undone_of_keeps_its_badge() {
    let (_d, mut s, anna) = setup();
    let original = unstamped(s.tree());
    let greta = s
        .commit_command_as(child_of(anna, "Greta"), agent(), None)
        .unwrap()
        .focus
        .unwrap();
    let g = newest_group(&s);
    s.apply(add_note(greta, "The owner's own note")).unwrap();

    let rep = s.undo_group(g).unwrap();
    assert!(!rep.skipped.is_empty());
    assert!(!row_of(&s, g).reverted);
    assert!(s.unreviewed_people().unwrap().contains(&greta));
    assert_eq!(s.person_agent_changes(greta, 0, 20).unwrap().items.len(), 1);

    s.undo().unwrap().unwrap();
    let done = s.undo_group(g).unwrap();
    assert!(done.skipped.is_empty(), "{:?}", done.skipped);
    assert!(row_of(&s, g).reverted);
    assert_eq!(unstamped(s.tree()), original);
    assert_consistent(&s);
}

#[test]
fn undo_batch_retries_groups_it_could_only_partly_undo() {
    let (d, s, anna) = setup();
    let batch = Uuid::from_u128(44);
    let mut s = with_batch(&d, s, batch, "Recording 2 summary");
    let original = unstamped(s.tree());
    s.commit_command_as(child_of(anna, "Greta"), agent(), Some(batch))
        .unwrap();
    let greta_group = newest_group(&s);
    let greta = find(s.tree(), "Greta");
    s.commit_command_as(child_of(anna, "Otto"), agent(), Some(batch))
        .unwrap();
    let otto_group = newest_group(&s);
    s.apply(add_note(greta, "The owner's own note")).unwrap();

    let rep = s.undo_batch(batch).unwrap();
    assert!(!rep.skipped.is_empty());
    assert!(row_of(&s, otto_group).reverted, "it went in full");
    assert!(!row_of(&s, greta_group).reverted);
    assert!(s.unreviewed_people().unwrap().contains(&greta));

    s.undo().unwrap().unwrap();
    let done = s.undo_batch(batch).unwrap();
    assert!(done.undone > 0);
    assert!(done.skipped.is_empty(), "{:?}", done.skipped);
    assert!(row_of(&s, greta_group).reverted);
    assert!(s.unreviewed_people().unwrap().is_empty());
    assert_eq!(unstamped(s.tree()), original);
    assert_consistent(&s);

    // Finished: nothing left to do.
    let n = rows(&s).len();
    let again = s.undo_batch(batch).unwrap();
    assert_eq!(again.undone, 0);
    assert_eq!(rows(&s).len(), n);
}

fn assert_says_dropped(e: &ApiError) {
    assert_eq!(e.code, ErrorCode::Conflict, "{e:?}");
    assert_eq!(
        e.message,
        "This step was dropped: a record it needs no longer exists."
    );
}

#[test]
fn redo_of_a_child_on_a_person_the_log_undid_is_dropped() {
    let (_d, mut s, anna) = setup();
    let ivan = s
        .commit_command_as(child_of(anna, "Ivan"), agent(), None)
        .unwrap()
        .focus
        .unwrap();
    let g = newest_group(&s);
    // Two redo steps, the blocked one on top.
    s.apply(child_of(ivan, "Olena")).unwrap();
    s.apply(add_note(anna, "Mine")).unwrap();
    s.undo().unwrap().unwrap();
    s.undo().unwrap().unwrap();
    assert!(s.undo_group(g).unwrap().skipped.is_empty());
    assert!(!s.tree().persons.contains_key(&ivan));

    let tree = s.tree().clone();
    let e = s.redo().unwrap_err();
    assert_says_dropped(&e);
    assert_eq!(s.tree(), &tree, "nothing was written");
    assert_consistent(&s);

    // The older step is reachable, and the blocked one is gone.
    assert_eq!(s.undo_state().redo_label.as_deref(), Some("Save note"));
    s.redo().unwrap().unwrap();
    assert_eq!(s.tree().persons[&anna].common.notes.len(), 1);
    assert!(!s.undo_state().can_redo);
    assert_consistent(&s);
}

#[test]
fn redo_of_a_link_to_a_person_the_log_undid_is_dropped() {
    let (_d, mut s, _) = setup();
    let p = s
        .commit_command_as(
            EditCommand::CreatePerson {
                draft: draft("Greta", SexKind::Female),
            },
            agent(),
            None,
        )
        .unwrap()
        .focus
        .unwrap();
    let g = newest_group(&s);
    let family = *s.tree().families.keys().next().unwrap();
    s.apply(link_child(family, p)).unwrap();
    s.undo().unwrap().unwrap();
    assert!(s.undo_group(g).unwrap().undone > 0);

    assert_says_dropped(&s.redo().unwrap_err());
    assert!(!s.undo_state().can_redo);
    assert!(s.redo().unwrap().is_none());
    assert_consistent(&s);
}

#[test]
fn redo_of_a_citation_of_a_source_the_log_undid_is_dropped() {
    let (_d, mut s, anna) = setup();
    let eva = find(s.tree(), "Eva");
    s.commit_command_as(cite(anna, "Recording 2"), agent(), None)
        .unwrap();
    let g = newest_group(&s);
    let source = *s.tree().sources.keys().next().unwrap();
    s.apply(cite_source(eva, SourceRef::Id(source))).unwrap();
    s.undo().unwrap().unwrap();
    assert!(s.undo_group(g).unwrap().undone > 0);
    assert!(!s.tree().sources.contains_key(&source));

    assert_says_dropped(&s.redo().unwrap_err());
    assert!(!s.undo_state().can_redo);
    assert_consistent(&s);
}

#[test]
fn ctrl_z_restoring_a_citation_of_a_source_the_log_undid_is_dropped() {
    let (_d, mut s, anna) = setup();
    let eva = find(s.tree(), "Eva");
    s.commit_command_as(cite(anna, "Recording 2"), agent(), None)
        .unwrap();
    let g = newest_group(&s);
    let source = *s.tree().sources.keys().next().unwrap();
    s.apply(add_note(eva, "Mine")).unwrap();
    s.apply(cite_source(eva, SourceRef::Id(source))).unwrap();
    let citation = s.tree().persons[&eva].common.citations[0];
    s.apply(remove_citation(eva, citation)).unwrap();
    assert!(s.undo_group(g).unwrap().skipped.is_empty());
    assert!(!s.tree().sources.contains_key(&source));

    let tree = s.tree().clone();
    assert_says_dropped(&s.undo().unwrap_err());
    assert_eq!(s.tree(), &tree);
    assert_consistent(&s);
    // The steps under it are still there.
    s.undo().unwrap().unwrap();
    s.undo().unwrap().unwrap();
    assert!(s.tree().persons[&eva].common.notes.is_empty());
    assert!(!s.undo_state().can_undo);
    assert_consistent(&s);
}

#[test]
fn ctrl_z_of_a_step_updating_a_person_the_log_undid_is_dropped() {
    let (_d, mut s, anna) = setup();
    let ivan = s
        .commit_command_as(child_of(anna, "Ivan"), agent(), None)
        .unwrap()
        .focus
        .unwrap();
    let g = newest_group(&s);
    s.apply(add_note(anna, "Mine")).unwrap();
    s.apply(add_note(ivan, "Also mine")).unwrap();
    let note = s.tree().persons[&ivan].common.notes[0];
    s.apply(EditCommand::DeleteNote {
        target: person_owner(ivan),
        note,
    })
    .unwrap();
    assert!(s.undo_group(g).unwrap().skipped.is_empty());
    assert!(!s.tree().persons.contains_key(&ivan));

    let tree = s.tree().clone();
    assert_says_dropped(&s.undo().unwrap_err());
    assert_eq!(s.tree(), &tree);
    assert_says_dropped(&s.undo().unwrap_err());
    assert_eq!(s.undo_state().undo_label.as_deref(), Some("Save note"));
    s.undo().unwrap().unwrap();
    assert!(s.tree().persons[&anna].common.notes.is_empty());
    assert!(!s.undo_state().can_undo);
    assert_consistent(&s);
}

#[test]
fn redo_of_a_step_updating_a_person_the_log_undid_is_dropped() {
    let (_d, mut s, anna) = setup();
    let ivan = s
        .commit_command_as(child_of(anna, "Ivan"), agent(), None)
        .unwrap()
        .focus
        .unwrap();
    let g = newest_group(&s);
    s.apply(add_note(ivan, "Mine")).unwrap();
    s.undo().unwrap().unwrap();
    assert!(s.undo_group(g).unwrap().skipped.is_empty());

    assert_says_dropped(&s.redo().unwrap_err());
    assert!(!s.undo_state().can_redo);
    assert!(s.redo().unwrap().is_none());
    assert_consistent(&s);
}

#[test]
fn a_step_on_a_record_a_dropped_redo_step_would_have_made_is_dropped() {
    let (_d, mut s, anna) = setup();
    let ivan = s
        .commit_command_as(child_of(anna, "Ivan"), agent(), None)
        .unwrap()
        .focus
        .unwrap();
    let g = newest_group(&s);
    let olena = s.apply(child_of(ivan, "Olena")).unwrap().focus.unwrap();
    s.apply(add_note(olena, "Mine")).unwrap();
    s.undo().unwrap().unwrap();
    s.undo().unwrap().unwrap();
    assert!(s.undo_group(g).unwrap().skipped.is_empty());

    assert_says_dropped(&s.redo().unwrap_err());
    assert_says_dropped(&s.redo().unwrap_err());
    assert!(!s.undo_state().can_redo);
    assert_consistent(&s);
}

#[test]
fn a_step_is_dropped_whatever_removed_its_record() {
    // No log undo here: the owner unlinks a child and an agent then deletes that person.
    let (_d, mut s, _) = setup();
    let (family, child) = s
        .tree()
        .families
        .iter()
        .find_map(|(id, f)| f.children.first().map(|c| (*id, c.person)))
        .unwrap();
    s.apply(EditCommand::UnlinkChild { family, child }).unwrap();
    s.commit_command_as(EditCommand::DeletePerson { id: child }, agent(), None)
        .unwrap();

    let tree = s.tree().clone();
    assert_says_dropped(&s.undo().unwrap_err());
    assert_eq!(s.tree(), &tree);
    assert!(!s.undo_state().can_undo);
    assert_consistent(&s);
}

#[test]
fn a_step_blocked_by_what_a_dropped_step_left_behind_is_dropped_too() {
    let (_d, mut s, anna) = setup();
    s.commit_command_as(cite(anna, "Recording 2"), agent(), None)
        .unwrap();
    let g = newest_group(&s);
    let source = *s.tree().sources.keys().next().unwrap();
    let petro = s
        .apply(EditCommand::CreatePerson {
            draft: draft("Petro", SexKind::Male),
        })
        .unwrap()
        .focus
        .unwrap();
    s.apply(cite_source(petro, SourceRef::Id(source))).unwrap();
    s.apply(child_of(petro, "Olena")).unwrap();
    s.apply(EditCommand::DeletePerson { id: petro }).unwrap();
    assert!(s.undo_group(g).unwrap().skipped.is_empty());

    // The delete cannot be undone: the citation it would restore stood on the source. So its
    // change to the family stays, and no agent's change is left to blame for it.
    assert_says_dropped(&s.undo().unwrap_err());
    let tree = s.tree().clone();
    let e = s.undo().unwrap_err();
    assert_eq!(e.code, ErrorCode::Conflict);
    assert!(e.message.starts_with("This step was dropped"), "{e:?}");
    assert!(!e.message.contains("no longer exists"), "{e:?}");
    assert_eq!(s.tree(), &tree);
    assert_eq!(s.undo_state().undo_label.as_deref(), Some("Add citation"));
    assert_consistent(&s);
}

#[test]
fn redo_blocked_by_an_agent_stays_on_its_stack() {
    let (_d, mut s, anna) = setup();
    s.apply(add_note(anna, "Mine")).unwrap();
    let note = s.tree().persons[&anna].common.notes[0];
    s.apply(edit_note(anna, note, "Mine, again")).unwrap();
    s.undo().unwrap().unwrap();
    s.commit_command_as(edit_note(anna, note, "Corrected"), agent(), None)
        .unwrap();
    let g = newest_group(&s);
    let state = s.undo_state();
    let e = s.redo().unwrap_err();
    assert_eq!(e.code, ErrorCode::Conflict);
    assert!(e.message.contains("An agent changed"), "{e:?}");
    assert_eq!(
        s.undo_state(),
        state,
        "it can go ahead once the way is clear"
    );
    s.undo_group(g).unwrap();
    s.redo().unwrap().unwrap();
    assert_eq!(s.tree().notes[&note].text, "Mine, again");
    assert_consistent(&s);
}

#[test]
fn log_undo_skips_what_would_point_at_a_removed_record() {
    let (_d, mut s, anna) = setup();
    s.apply(cite(anna, "Family bible")).unwrap();
    let citation = s.tree().persons[&anna].common.citations[0];
    let source = *s.tree().sources.keys().next().unwrap();
    s.commit_command_as(remove_citation(anna, citation), agent(), None)
        .unwrap();
    let g = newest_group(&s);
    // The owner takes their citation back; with it goes the source the citation stood on.
    s.undo().unwrap().unwrap();
    assert!(!s.tree().sources.contains_key(&source));

    let tree = s.tree().clone();
    let n = rows(&s).len();
    let rep = s.undo_group(g).unwrap();
    assert_eq!(rep.undone, 0, "{rep:?}");
    assert_eq!(
        rep.skipped.len(),
        s.project().log_group(g).unwrap().unwrap().changes.len(),
        "{rep:?}"
    );
    assert_eq!(s.tree(), &tree, "neither the citation nor the link to it");
    assert_eq!(rows(&s).len(), n);
    assert!(!row_of(&s, g).reverted);
    assert_consistent(&s);

    // The owner's redo brings the citation back itself; nothing is left for the log undo to do.
    s.redo().unwrap().unwrap();
    assert!(s.tree().citations.contains_key(&citation));
    let again = s.undo_group(g).unwrap();
    assert_eq!(again.undone, 0);
    assert!(again.skipped.is_empty(), "{:?}", again.skipped);
    assert_consistent(&s);
}

#[test]
fn a_retry_with_nothing_left_to_do_writes_nothing() {
    let (_d, mut s, _) = setup();
    let (g, aaa, _) = import_two(&mut s);
    s.apply(add_note(aaa, "The owner's own note")).unwrap();
    assert!(!s.undo_group(g).unwrap().skipped.is_empty());
    // The owner removes the kept person by hand, so the rest of the undo has already happened.
    s.apply(EditCommand::DeletePerson { id: aaa }).unwrap();
    let n = rows(&s).len();
    let rep = s.undo_group(g).unwrap();
    assert_eq!(rep.undone, 0);
    assert!(rep.skipped.is_empty(), "{:?}", rep.skipped);
    assert_eq!(rows(&s).len(), n);
    assert_consistent(&s);
}

/// Undoes from the log everything that can be, as many times as it makes a difference.
fn undo_all_from_log(s: &mut Session, context: &str) {
    for _ in 0..50 {
        let before = s.tree().clone();
        let open: Vec<Uuid> = rows(s)
            .iter()
            .filter(|r| r.can_undo)
            .map(|r| r.group)
            .collect();
        for g in open {
            s.undo_group(g)
                .unwrap_or_else(|e| panic!("{context}: log undo: {e:?}"));
        }
        if s.tree() == &before {
            return;
        }
    }
    panic!("{context}: the log undos never settle");
}

/// Runs one stack down to nothing. A step may be dropped, and may wait for a log undo, but once
/// the log has nothing more to give the same step must not refuse again.
fn drain(
    s: &mut Session,
    mut step: impl FnMut(&mut Session) -> Result<bool, ApiError>,
    context: &str,
) {
    let mut waiting = false;
    loop {
        match step(s) {
            Ok(false) => return,
            Ok(true) => waiting = false,
            Err(e) if e.message.starts_with("This step was dropped") => waiting = false,
            Err(e) => {
                assert_eq!(e.code, ErrorCode::Conflict, "{context}: {e:?}");
                assert!(!waiting, "{context}: stuck on {:?}: {e:?}", s.undo_state());
                waiting = true;
            }
        }
        undo_all_from_log(s, context);
        assert_eq!(s.tree(), &s.project().load_tree().unwrap(), "{context}");
        s.verify_derived_state()
            .unwrap_or_else(|e| panic!("{context}: {e}"));
    }
}

#[test]
fn owner_and_agent_interleaved_stay_consistent() {
    for seed in 0..8u64 {
        let (_d, mut s, _) = setup();
        let mut x = seed.wrapping_mul(2_654_435_761).wrapping_add(11);
        let mut next = move |n: usize| {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            usize::try_from(x >> 33).unwrap() % n
        };
        // Half the owner's work builds on what an agent just added, so that undoing that from the
        // log pulls the ground from under the owner's undo and redo steps.
        let mut agent_person: Option<Uuid> = None;
        for step in 0..120 {
            let ids: Vec<Uuid> = s.tree().persons.keys().copied().collect();
            let drawn = ids[next(ids.len())];
            let who = agent_person
                .filter(|p| ids.contains(p) && next(2) == 0)
                .unwrap_or(drawn);
            let notes: Vec<Uuid> = s.tree().persons[&who].common.notes.clone();
            let families: Vec<Uuid> = s.tree().families.keys().copied().collect();
            let sources: Vec<Uuid> = s.tree().sources.keys().copied().collect();
            let source = if sources.is_empty() {
                SourceRef::New(NewSource {
                    title: "Recording".into(),
                })
            } else {
                SourceRef::Id(sources[next(sources.len())])
            };
            let cited: Vec<Uuid> = s.tree().persons[&who].common.citations.clone();
            let outcome = match next(20) {
                0 => s.apply(add_note(who, "Mine")).map(|_| ()),
                1 => s.apply(child_of(who, "Olena")).map(|_| ()),
                2 if ids.len() > 2 => s.apply(EditCommand::DeletePerson { id: who }).map(|_| ()),
                3 => s
                    .commit_command_as(add_note(who, "From the recording"), agent(), None)
                    .map(|_| ()),
                4 => s
                    .commit_command_as(child_of(who, "Ivan"), agent(), None)
                    .map(|r| agent_person = r.focus.or(agent_person)),
                5 if !notes.is_empty() => s
                    .commit_command_as(edit_note(who, notes[0], "Corrected"), agent(), None)
                    .map(|_| ()),
                6 | 7 => s.undo().map(|_| ()),
                8..=10 => s.redo().map(|_| ()),
                11 if !families.is_empty() => s
                    .apply(link_child(families[next(families.len())], who))
                    .map(|_| ()),
                12 if !families.is_empty() => s
                    .commit_command_as(
                        link_child(families[next(families.len())], who),
                        agent(),
                        None,
                    )
                    .map(|_| ()),
                13 => s.apply(cite_source(who, source)).map(|_| ()),
                14 => s
                    .commit_command_as(cite_source(who, source), agent(), None)
                    .map(|_| ()),
                15 if !cited.is_empty() => s.apply(remove_citation(who, cited[0])).map(|_| ()),
                16 if !cited.is_empty() => s
                    .commit_command_as(remove_citation(who, cited[0]), agent(), None)
                    .map(|_| ()),
                _ => {
                    let agents: Vec<Uuid> = rows(&s)
                        .iter()
                        .filter(|r| r.can_undo)
                        .map(|r| r.group)
                        .collect();
                    if agents.is_empty() {
                        Ok(())
                    } else {
                        let newest = next(2) == 0;
                        let g = if newest {
                            agents[0]
                        } else {
                            agents[next(agents.len())]
                        };
                        s.undo_group(g).map(|_| ())
                    }
                }
            };
            if let Err(e) = outcome {
                // A plan can refuse the person it drew; the undo paths may only stop for a conflict.
                assert!(
                    matches!(
                        e.code,
                        ErrorCode::Conflict | ErrorCode::Invalid | ErrorCode::NotFound
                    ),
                    "seed {seed} step {step}: {e:?}"
                );
            }
            assert_eq!(
                s.tree(),
                &s.project().load_tree().unwrap(),
                "seed {seed} step {step}"
            );
            s.verify_derived_state()
                .unwrap_or_else(|e| panic!("seed {seed} step {step}: {e}"));
        }
        // With everything undone from the log, both stacks run out whatever has happened.
        let context = format!("seed {seed}");
        undo_all_from_log(&mut s, &context);
        drain(&mut s, |s| s.undo().map(|r| r.is_some()), &context);
        assert!(!s.undo_state().can_undo, "{context}");
        drain(&mut s, |s| s.redo().map(|r| r.is_some()), &context);
        assert!(!s.undo_state().can_redo, "{context}");
    }
}
