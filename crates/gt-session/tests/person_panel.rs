#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! What the person page reads, round two: paging of the agent's rows, the cheap MCP history, where a
//! note or citation lives, untracked groups, partner lists, and undo against real writes.

mod common;

use common::*;
use gt_session::field_diff::LIST_SEP;
use gt_session::*;
use gt_store::Actor;
use uuid::Uuid;

fn agent() -> Actor {
    Actor::Agent {
        id: Uuid::from_u128(7),
        name: "Transcript reader".into(),
    }
}

fn person(id: Uuid) -> OwnerRef {
    OwnerRef {
        kind: OwnerKind::Person,
        id,
    }
}

fn add_note(target: OwnerRef, text: &str) -> EditCommand {
    EditCommand::SaveNote {
        target,
        note: NoteInput {
            id: None,
            kind: NoteKindDto::General,
            text: text.into(),
        },
    }
}

fn event(owner: EventOwner, kind: &str) -> EditCommand {
    EditCommand::SaveEvent {
        owner,
        event: EventInput {
            id: None,
            kind: kind.into(),
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
    }
}

fn partner_of(anchor: Uuid, name: &str) -> EditCommand {
    EditCommand::QuickAdd {
        anchor,
        relation: Relation::Partner,
        draft: draft(name, SexKind::Female),
        family: None,
        new_parent_set: None,
        parent_rel: None,
    }
}

fn setup() -> (tempfile::TempDir, Session, Uuid) {
    let t = tree_of(SAMPLE);
    let eva = find(&t, "Eva");
    let (d, s) = open_tree(&t);
    (d, s, eva)
}

#[test]
fn the_agents_rows_page_and_an_undone_group_is_not_counted() {
    let (_d, mut s, eva) = setup();
    for i in 0..205 {
        s.commit_command_as(add_note(person(eva), &format!("Note {i}")), agent(), None)
            .unwrap();
    }
    let first = s.person_agent_changes(eva, 0, 20).unwrap();
    assert_eq!((first.total, first.items.len()), (205, 20));
    // A page is never longer than 200, whatever is asked.
    assert_eq!(
        s.person_agent_changes(eva, 0, 5000).unwrap().items.len(),
        200
    );
    let tail = s.person_agent_changes(eva, 200, 20).unwrap();
    assert_eq!((tail.offset, tail.items.len()), (200, 5));
    s.undo_group(first.items[0].group).unwrap();
    let after = s.person_agent_changes(eva, 0, 20).unwrap();
    assert_eq!(after.total, 204, "the undone group left the total");
}

#[test]
fn only_a_person_record_has_fields_in_the_short_history() {
    let (_d, mut s, eva) = setup();
    s.commit_command_as(add_note(person(eva), "Kept short"), agent(), None)
        .unwrap();
    let note = *s.tree().notes.keys().last().unwrap();
    let short = s.history(note, 10).unwrap();
    assert_eq!(short.len(), 1);
    assert!(short[0].fields.is_empty(), "the MCP history stays cheap");
    s.apply(EditCommand::UpdatePerson {
        id: eva,
        sex: Some(SexKind::Male),
        sex_text: None,
        living_override: LivingPatch::Keep,
        private: None,
    })
    .unwrap();
    assert!(!s.history(eva, 10).unwrap()[0].fields.is_empty());
}

#[test]
fn a_note_and_a_citation_are_found_with_the_record_that_holds_them() {
    let (_d, mut s, eva) = setup();
    s.commit_command_as(partner_of(eva, "Marta"), agent(), None)
        .unwrap();
    let family = *s.tree().families.keys().last().unwrap();
    let on_family = OwnerRef {
        kind: OwnerKind::Family,
        id: family,
    };
    s.apply(add_note(on_family, "On the family")).unwrap();
    let note = *s.tree().notes.keys().last().unwrap();
    let home = s.record_home("note", note).unwrap().unwrap();
    assert_eq!(home.owner, on_family);
    assert_eq!(home.note.unwrap().text, "On the family");

    s.apply(event(
        EventOwner {
            kind: EventOwnerKind::Person,
            id: eva,
        },
        "birth",
    ))
    .unwrap();
    let ev = *s.tree().events.keys().last().unwrap();
    let on_event = OwnerRef {
        kind: OwnerKind::Event,
        id: ev,
    };
    s.apply(EditCommand::AddCitation {
        target: on_event,
        citation: CitationArg::New(CitationInput {
            source: SourceRef::New(NewSource {
                title: "Register".into(),
            }),
            page: None,
            date_raw: None,
            text: None,
            quality: None,
            evidence: None,
        }),
    })
    .unwrap();
    let cite = *s.tree().citations.keys().last().unwrap();
    let home = s.record_home("citation", cite).unwrap().unwrap();
    assert_eq!(home.owner, on_event);
    assert!(home.citation.is_some() && home.note.is_none());
    assert!(
        s.record_home("note", Uuid::from_u128(99))
            .unwrap()
            .is_none()
    );
    assert!(s.record_home("event", ev).is_err());
}

#[test]
fn groups_without_an_author_are_told_apart() {
    let (d, mut s, eva) = setup();
    s.apply(add_note(person(eva), "Mine")).unwrap();
    assert!(!s.has_untracked_changes().unwrap());
    s.close();
    let dir = d.path().join("project");
    let c = rusqlite::Connection::open(dir.join("tree.db")).unwrap();
    c.execute(
        "UPDATE change_log SET actor_kind = NULL, actor_id = NULL, actor_name = NULL",
        [],
    )
    .unwrap();
    drop(c);
    let s = Session::open(&dir, SessionClock::fixed(today(), NOW_MS + 1000)).unwrap();
    assert!(s.has_untracked_changes().unwrap());
}

#[test]
fn the_partners_of_a_family_are_joined_by_a_separator_the_page_words() {
    let (_d, mut s, eva) = setup();
    s.commit_command_as(partner_of(eva, "Marta"), agent(), None)
        .unwrap();
    let family = *s.tree().families.keys().last().unwrap();
    s.commit_command_as(
        event(
            EventOwner {
                kind: EventOwnerKind::Family,
                id: family,
            },
            "marriage",
        ),
        agent(),
        None,
    )
    .unwrap();
    let rows = s.person_agent_changes(eva, 0, 20).unwrap().items;
    let ev = rows[0].changes.iter().find(|c| c.kind == "event").unwrap();
    let of = ev
        .fields
        .iter()
        .find(|f| f.field == "participant")
        .unwrap()
        .of
        .clone()
        .unwrap();
    assert!(of.contains(LIST_SEP), "{of:?}");
    assert!(!of.contains(" and "), "{of:?}");
}

#[test]
fn undo_after_the_owner_edited_the_record_drops_that_step_and_keeps_the_edit() {
    let (_d, mut s, eva) = setup();
    s.commit_command_as(add_note(person(eva), "Agent text"), agent(), None)
        .unwrap();
    let note = *s.tree().notes.keys().last().unwrap();
    s.apply(EditCommand::SaveNote {
        target: person(eva),
        note: NoteInput {
            id: Some(note),
            kind: NoteKindDto::General,
            text: "Owner text".into(),
        },
    })
    .unwrap();
    let group = s.person_agent_changes(eva, 0, 20).unwrap().items[0].group;
    let report = s.undo_group(group).unwrap();
    assert!(
        report.skipped.iter().any(|k| k.kind == "note"),
        "{report:?}"
    );
    assert_eq!(s.tree().notes[&note].text, "Owner text");
}

#[test]
fn undoing_the_agents_creation_of_a_person_removes_the_person() {
    let (_d, mut s, eva) = setup();
    s.commit_command_as(partner_of(eva, "Marta"), agent(), None)
        .unwrap();
    let marta = find(s.tree(), "Marta");
    let group = s.person_agent_changes(marta, 0, 20).unwrap().items[0].group;
    let report = s.undo_group(group).unwrap();
    assert!(report.undone > 0 && report.skipped.is_empty(), "{report:?}");
    assert!(!s.tree().persons.contains_key(&marta));
    assert!(s.get_person(marta).is_err());
}
