#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! set_note_story: an agent writes the readable retelling beside a note and never touches its text.

mod common;

use common::*;
use gt_store::Record;
use serde_json::{Value, json};
use uuid::Uuid;

fn begin(host: &TestHost) -> Value {
    call(host, "begin_batch", json!({"title": "Story retellings"}))["batch"].clone()
}

/// A person with an owner's note; returns the note.
fn owner_note(host: &TestHost, text: &str) -> Uuid {
    with(host, |s| {
        let person = add_person(s, draft("Ivan", "Testenko", None, Some("1990")));
        add_note(s, person, text);
        s.get_person(person).unwrap().notes[0].id
    })
}

fn story_of(host: &TestHost, note: Uuid) -> Option<String> {
    with(host, |s| s.tree().notes[&note].story.clone())
}

fn text_of(host: &TestHost, note: Uuid) -> String {
    with(host, |s| s.tree().notes[&note].text.clone())
}

#[test]
fn writes_the_story_and_leaves_the_text() {
    let host = empty_host();
    let note = owner_note(&host, "Born 1890 (ДАХеО 1-2-3, арк. 5), maybe in Lisky? H7");
    let batch = begin(&host);
    let done = call(
        &host,
        "set_note_story",
        json!({"batch": batch, "note": note, "story": "Born in 1890, perhaps in Lisky."}),
    );
    assert_eq!(done["id"], json!(note));
    assert_eq!(
        story_of(&host, note).as_deref(),
        Some("Born in 1890, perhaps in Lisky.")
    );
    assert_eq!(
        text_of(&host, note),
        "Born 1890 (ДАХеО 1-2-3, арк. 5), maybe in Lisky? H7"
    );
    // The agent reads it back.
    let shown = call(&host, "get_note", json!({"id": note}));
    assert!(shown.to_string().contains("perhaps in Lisky"), "{shown}");
}

#[test]
fn a_repeat_answers_already_there_and_a_story_there_stays() {
    let host = empty_host();
    let note = owner_note(&host, "Text.");
    let batch = begin(&host);
    let ask = |story: &str| {
        call(
            &host,
            "set_note_story",
            json!({"batch": batch, "note": note, "story": story}),
        )
    };
    ask("A retelling.");
    let again = ask("A retelling.");
    assert_eq!(again["alreadyThere"], json!(true), "{again}");
    let other = call_err(
        &host,
        "set_note_story",
        json!({"batch": batch, "note": note, "story": "Another retelling."}),
    );
    assert!(!other.is_empty());
    assert_eq!(story_of(&host, note).as_deref(), Some("A retelling."));
}

#[test]
fn undo_takes_the_story_off_and_the_log_keeps_both_states() {
    let host = empty_host();
    let note = owner_note(&host, "Text.");
    let batch = begin(&host);
    call(
        &host,
        "set_note_story",
        json!({"batch": batch, "note": note, "story": "A retelling."}),
    );
    let rows = with(&host, |s| s.project().rows_of(note).unwrap());
    let last = rows.last().unwrap();
    let story = |r: &Option<Record>| match r {
        Some(Record::Note(n)) => n.story.clone(),
        _ => panic!("not a note"),
    };
    assert_eq!(story(&last.before), None);
    assert_eq!(story(&last.after).as_deref(), Some("A retelling."));
    with(&host, |s| {
        s.undo_batch(serde_json::from_value(batch.clone()).unwrap())
            .unwrap()
    });
    assert_eq!(story_of(&host, note), None);
    assert_eq!(text_of(&host, note), "Text.");
}

#[test]
fn refusals() {
    let host = empty_host();
    let note = owner_note(&host, "Text.");
    let batch = begin(&host);
    let m = call_err(
        &host,
        "set_note_story",
        json!({"batch": batch, "note": note, "story": "  \n"}),
    );
    assert!(m.contains("story"), "{m}");
    let m = call_err(
        &host,
        "set_note_story",
        json!({"batch": batch, "note": Uuid::from_u128(77), "story": "x"}),
    );
    assert!(m.contains("note"), "{m}");
}
