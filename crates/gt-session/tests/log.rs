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
    assert_eq!((b[0].groups, b[0].people, b[0].unreviewed), (2, 2, 2));

    assert_eq!(s.person_agent_changes(anna).unwrap().len(), 1);
    assert_eq!(s.person_agent_changes(eva).unwrap().len(), 1);

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
    let row = &s.person_agent_changes(anna).unwrap()[0];
    assert!(row.people.iter().all(|p| p.reviewed.is_some()));

    // Undoing takes the row off the person page but never out of the log.
    let g = row.group;
    s.undo_group(g).unwrap();
    assert!(s.person_agent_changes(anna).unwrap().is_empty());
    assert!(rows(&s).iter().any(|r| r.group == g && r.reverted));
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
        for step in 0..120 {
            let ids: Vec<Uuid> = s.tree().persons.keys().copied().collect();
            let who = ids[next(ids.len())];
            let notes: Vec<Uuid> = s.tree().persons[&who].common.notes.clone();
            let outcome = match next(10) {
                0 => s.apply(add_note(who, "Mine")).map(|_| ()),
                1 => s.apply(child_of(who, "Olena")).map(|_| ()),
                2 if ids.len() > 2 => s.apply(EditCommand::DeletePerson { id: who }).map(|_| ()),
                3 => s
                    .commit_command_as(add_note(who, "From the recording"), agent(), None)
                    .map(|_| ()),
                4 => s
                    .commit_command_as(child_of(who, "Ivan"), agent(), None)
                    .map(|_| ()),
                5 if !notes.is_empty() => s
                    .commit_command_as(edit_note(who, notes[0], "Corrected"), agent(), None)
                    .map(|_| ()),
                6 | 7 => s.undo().map(|_| ()),
                8 => s.redo().map(|_| ()),
                _ => {
                    let agents: Vec<Uuid> = rows(&s)
                        .iter()
                        .filter(|r| r.can_undo)
                        .map(|r| r.group)
                        .collect();
                    if agents.is_empty() {
                        Ok(())
                    } else {
                        s.undo_group(agents[next(agents.len())]).map(|_| ())
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
    }
}
