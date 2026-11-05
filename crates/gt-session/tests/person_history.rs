#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! What the person page reads: every change group that reached the person, and the agent's own,
//! with the fields of each record in words. Agent writes go through `commit_command_as`.

mod common;

use common::*;
use gt_session::*;
use gt_store::Actor;
use uuid::Uuid;

fn agent() -> Actor {
    Actor::Agent {
        id: Uuid::from_u128(7),
        name: "Transcript reader".into(),
    }
}

fn add_note(id: Uuid, text: &str) -> EditCommand {
    EditCommand::SaveNote {
        target: OwnerRef {
            kind: OwnerKind::Person,
            id,
        },
        note: NoteInput {
            id: None,
            kind: NoteKindDto::General,
            text: text.into(),
        },
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
            place_text: Some("Lviv".into()),
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

fn cite(owner: Uuid, title: &str) -> EditCommand {
    EditCommand::AddCitation {
        target: OwnerRef {
            kind: OwnerKind::Person,
            id: owner,
        },
        citation: CitationArg::New(CitationInput {
            source: SourceRef::New(NewSource {
                title: title.into(),
            }),
            page: Some("12".into()),
            date_raw: None,
            text: None,
            quality: None,
            evidence: None,
        }),
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

fn field<'a>(c: &'a LogChangeDto, name: &str) -> &'a FieldChangeDto {
    c.fields.iter().find(|f| f.field == name).unwrap()
}

fn change<'a>(r: &'a LogRowDto, kind: &str) -> &'a LogChangeDto {
    r.changes.iter().find(|c| c.kind == kind).unwrap()
}

#[test]
fn a_note_the_agent_added_shows_whole_and_undo_shows_it_removed() {
    let (_d, mut s, eva) = setup();
    let text = "A long remembered story. ".repeat(40);
    s.commit_command_as(add_note(eva, &text), agent(), None)
        .unwrap();
    let rows = s.person_agent_changes(eva).unwrap();
    assert_eq!(rows.len(), 1);
    let note = change(&rows[0], "note");
    assert_eq!(field(note, "text").before, None);
    assert_eq!(field(note, "text").after.as_deref(), Some(text.as_str()));
    assert_eq!(field(note, "noteKind").after.as_deref(), Some("general"));
    assert!(note.fields.iter().all(|f| f.field != "shared"));
    assert_eq!(note.id, s.tree().notes.keys().copied().next().unwrap());

    // The change log's pages cut the same value short.
    let page = s.log(LogFilterDto::default(), 0, 10).unwrap();
    let cut = field(change(&page.items[0], "note"), "text")
        .after
        .clone()
        .unwrap();
    assert!(cut.chars().count() < text.chars().count());

    let report = s.undo_group(rows[0].group).unwrap();
    assert!(report.undone > 0 && report.skipped.is_empty());
    assert!(s.person_agent_changes(eva).unwrap().is_empty());
    // The history keeps both: the agent's group, undone, and the undo, which removes the note.
    let history = s.person_history(eva, 0, 20).unwrap();
    assert_eq!(history.total, 2);
    assert!(history.items[1].reverted);
    let undo = change(&history.items[0], "note");
    assert_eq!(field(undo, "text").after, None);
    assert_eq!(field(undo, "text").before.as_deref(), Some(text.as_str()));
}

#[test]
fn an_event_the_owner_added_is_in_the_history_with_its_place_in_words() {
    let (_d, mut s, eva) = setup();
    s.apply(birth(eva, "1 Jan 1900")).unwrap();
    let history = s.person_history(eva, 0, 20).unwrap();
    assert_eq!(history.total, 1);
    assert_eq!(history.items[0].actor, ActorDto::User);
    let event = change(&history.items[0], "event");
    assert_eq!(field(event, "eventKind").after.as_deref(), Some("birth"));
    assert_eq!(field(event, "place").after.as_deref(), Some("Lviv"));
    assert_eq!(field(event, "when").after.as_deref(), Some("1 Jan 1900"));
    let who = field(event, "participant");
    assert_eq!(who.of.as_deref(), Some("Eva Meyer"));
    assert_eq!(who.after.as_deref(), Some("principal"));
    assert!(
        s.person_agent_changes(eva).unwrap().is_empty(),
        "the owner's own change is not the agent's"
    );
}

#[test]
fn a_family_the_agent_added_names_its_partners() {
    let (_d, mut s, eva) = setup();
    s.commit_command_as(partner_of(eva, "Marta"), agent(), None)
        .unwrap();
    let rows = s.person_agent_changes(eva).unwrap();
    let family = change(&rows[0], "family");
    let partners: Vec<_> = ["partner1", "partner2"]
        .iter()
        .filter_map(|f| family.fields.iter().find(|x| x.field == *f))
        .filter_map(|f| f.after.clone())
        .collect();
    assert!(partners.contains(&"Eva Meyer".to_string()), "{partners:?}");
    assert!(
        partners.contains(&"Marta Schmidt".to_string()),
        "{partners:?}"
    );
    assert_eq!(s.person_history(eva, 0, 20).unwrap().total, 1);
}

#[test]
fn a_citation_names_its_source_and_a_deleted_source_is_still_named() {
    let (_d, mut s, eva) = setup();
    s.commit_command_as(cite(eva, "Parish book"), agent(), None)
        .unwrap();
    let rows = s.person_agent_changes(eva).unwrap();
    let citation = change(&rows[0], "citation");
    assert_eq!(
        field(citation, "source").after.as_deref(),
        Some("Parish book")
    );
    assert_eq!(field(citation, "page").after.as_deref(), Some("12"));
    s.undo_group(rows[0].group).unwrap();
    // The undo deleted the citation and its source; the log still names the source.
    let history = s.person_history(eva, 0, 20).unwrap();
    let gone = change(&history.items[0], "citation");
    assert_eq!(field(gone, "source").before.as_deref(), Some("Parish book"));
    assert_eq!(field(gone, "source").after, None);
}

#[test]
fn a_person_with_no_changes_has_no_history() {
    let (_d, s, eva) = setup();
    let history = s.person_history(eva, 0, 20).unwrap();
    assert_eq!((history.total, history.items.len()), (0, 0));
}

#[test]
fn the_history_pages() {
    let (_d, mut s, eva) = setup();
    for i in 0..25 {
        s.apply(add_note(eva, &format!("Note {i}"))).unwrap();
    }
    let first = s.person_history(eva, 0, 20).unwrap();
    assert_eq!((first.total, first.items.len()), (25, 20));
    let second = s.person_history(eva, 20, 20).unwrap();
    assert_eq!((second.offset, second.items.len()), (20, 5));
    assert_ne!(first.items[0].group, second.items[0].group);
}

#[test]
fn groups_from_before_people_were_listed_reach_the_person_record_they_changed() {
    let (d, mut s, eva) = setup();
    s.apply(add_note(eva, "Old note")).unwrap();
    s.close();
    let dir = d.path().join("project");
    let c = rusqlite::Connection::open(dir.join("tree.db")).unwrap();
    c.execute("DELETE FROM change_person", []).unwrap();
    drop(c);
    let s = Session::open(&dir, SessionClock::fixed(today(), NOW_MS + 1000)).unwrap();
    let history = s.person_history(eva, 0, 20).unwrap();
    assert_eq!(history.total, 1, "the note also changed the person record");
}
