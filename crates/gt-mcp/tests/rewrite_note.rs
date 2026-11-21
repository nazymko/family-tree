#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! rewrite_note: an agent replaces the text of any note at the owner's request, with a reason.

mod common;

use common::*;
use gt_mcp::{AgentCtx, handle_message};
use gt_session::{EditCommand, SourceInput};
use gt_store::Record;
use serde_json::{Value, json};
use std::path::PathBuf;
use uuid::Uuid;

const REASON: &str = "The owner asked for a story-style rewrite";

fn other() -> AgentCtx {
    AgentCtx {
        id: Uuid::from_u128(8),
        name: "Other reader".into(),
    }
}

fn call_as(host: &TestHost, who: &AgentCtx, tool: &str, args: Value) -> Result<Value, String> {
    let reply = handle_message(
        host,
        who,
        &request("tools/call", json!({"name": tool, "arguments": args})),
    )
    .unwrap();
    let result = reply["result"].clone();
    let text = result["content"][0]["text"].as_str().unwrap().to_string();
    if result["isError"] == json!(true) {
        Err(text)
    } else {
        Ok(serde_json::from_str(&text).unwrap())
    }
}

fn begin(host: &TestHost) -> Value {
    call(host, "begin_batch", json!({"title": "Story rewrites"}))["batch"].clone()
}

/// Ivan (born 1990, so living) with an owner's note; returns (person, note).
fn owner_note(host: &TestHost, text: &str) -> (Uuid, Uuid) {
    with(host, |s| {
        let person = add_person(s, draft("Ivan", "Koval", None, Some("1990")));
        add_note(s, person, text);
        let note = s.get_person(person).unwrap().notes[0].id;
        (person, note)
    })
}

fn text_of(host: &TestHost, note: Uuid) -> String {
    with(host, |s| s.tree().notes[&note].text.clone())
}

fn kind_of(host: &TestHost, note: Uuid) -> String {
    with(host, |s| format!("{:?}", s.tree().notes[&note].kind))
}

fn rewrite(host: &TestHost, batch: &Value, note: Uuid, text: &str) -> Value {
    call(
        host,
        "rewrite_note",
        json!({"batch": batch, "note": note, "text": text, "reason": REASON}),
    )
}

fn refused(host: &TestHost, args: Value) -> String {
    call_err(host, "rewrite_note", args)
}

fn story_of(host: &TestHost, note: Uuid) -> Option<String> {
    with(host, |s| s.tree().notes[&note].story.clone())
}

/// The owner's note with a story of its own, which set_note_story may not replace.
fn owner_note_with_story(host: &TestHost) -> Uuid {
    with(host, |s| {
        let person = add_person(s, draft("Ivan", "Koval", None, Some("1890")));
        s.apply(EditCommand::SaveNote {
            target: gt_session::OwnerRef {
                kind: gt_session::OwnerKind::Person,
                id: person,
            },
            note: gt_session::NoteInput {
                story: Some("Two people for now.".into()),
                id: None,
                kind: gt_session::NoteKindDto::Research,
                text: "Two records, maybe one man.".into(),
            },
        })
        .unwrap();
        s.get_person(person).unwrap().notes[0].id
    })
}

#[test]
fn rewrite_with_a_story_replaces_the_owners_story_too() {
    let host = empty_host();
    let note = owner_note_with_story(&host);
    let batch = begin(&host);
    let err = call_err(
        &host,
        "set_note_story",
        json!({"batch": batch, "note": note, "story": "One man."}),
    );
    assert!(!err.is_empty());
    call(
        &host,
        "rewrite_note",
        json!({"batch": batch, "note": note, "text": "One man, joined by the owner.", "story": "One man.", "reason": REASON}),
    );
    assert_eq!(text_of(&host, note), "One man, joined by the owner.");
    assert_eq!(story_of(&host, note).as_deref(), Some("One man."));
}

#[test]
fn rewrite_without_a_story_keeps_the_story() {
    let host = empty_host();
    let note = owner_note_with_story(&host);
    let batch = begin(&host);
    rewrite(&host, &batch, note, "Two records, maybe one man, reworded.");
    assert_eq!(
        story_of(&host, note).as_deref(),
        Some("Two people for now.")
    );
}

#[test]
fn rewrites_an_owners_note_and_keeps_the_kind() {
    let host = empty_host();
    let (_, note) = owner_note(&host, "Born in the village, 1990.");
    let batch = begin(&host);
    let kind = kind_of(&host, note);
    let done = rewrite(
        &host,
        &batch,
        note,
        "Ivan came into the world in the village, 1990.",
    );
    assert_eq!(done["id"], json!(note));
    assert_eq!(
        text_of(&host, note),
        "Ivan came into the world in the village, 1990."
    );
    assert_eq!(kind_of(&host, note), kind);
}

#[test]
fn rewrites_an_agents_own_note_and_changes_the_kind() {
    let host = empty_host();
    let (person, _) = owner_note(&host, "Owner's note.");
    let batch = begin(&host);
    let added = call(
        &host,
        "add_note",
        json!({"batch": batch, "target": {"kind": "person", "id": person}, "text": "Plain note."}),
    );
    let note: Uuid = serde_json::from_value(added["id"].clone()).unwrap();
    call(
        &host,
        "rewrite_note",
        json!({"batch": batch, "note": note, "text": "A told story.", "kind": "story", "reason": REASON}),
    );
    assert_eq!(text_of(&host, note), "A told story.");
    assert_eq!(kind_of(&host, note), "Story");
}

#[test]
fn a_repeat_answers_already_there() {
    let host = empty_host();
    let (_, note) = owner_note(&host, "Old.");
    let batch = begin(&host);
    rewrite(&host, &batch, note, "New.");
    let rows = with(&host, |s| s.project().rows_of(note).unwrap().len());
    let again = rewrite(&host, &batch, note, "New.");
    assert_eq!(again["alreadyThere"], json!(true), "{again}");
    assert_eq!(again["id"], json!(note));
    assert_eq!(
        with(&host, |s| s.project().rows_of(note).unwrap().len()),
        rows
    );
}

#[test]
fn undo_restores_the_old_text_and_kind() {
    let host = empty_host();
    let (_, note) = owner_note(&host, "Old text, with a doubt: maybe 1990?");
    let kind = kind_of(&host, note);
    let batch = begin(&host);
    call(
        &host,
        "rewrite_note",
        json!({"batch": batch, "note": note, "text": "New text.", "kind": "conclusion", "reason": REASON}),
    );
    assert_eq!(kind_of(&host, note), "Conclusion");
    with(&host, |s| {
        s.undo_batch(serde_json::from_value(batch.clone()).unwrap())
            .unwrap()
    });
    assert_eq!(text_of(&host, note), "Old text, with a doubt: maybe 1990?");
    assert_eq!(kind_of(&host, note), kind);
}

#[test]
fn the_old_text_and_the_reason_stay_in_the_history() {
    let host = empty_host();
    let (_, note) = owner_note(&host, "The first words of the note.");
    let batch = begin(&host);
    rewrite(&host, &batch, note, "The second words of the note.");
    // The log row keeps the record before and after.
    let rows = with(&host, |s| s.project().rows_of(note).unwrap());
    let last = rows.last().unwrap();
    let text = |r: &Option<Record>| match r {
        Some(Record::Note(n)) => n.text.clone(),
        _ => panic!("not a note"),
    };
    assert_eq!(text(&last.before), "The first words of the note.");
    assert_eq!(text(&last.after), "The second words of the note.");
    // The history label carries the reason.
    let history = with(&host, |s| s.history(note, 10).unwrap());
    assert_eq!(history[0].label, format!("Rewrite note: {REASON}"));
    // And so does what the agent reads.
    let log = call(&host, "get_log", json!({"filters": {"batch": batch}}));
    assert!(log.to_string().contains(REASON), "{log}");
    let hist = call(&host, "get_history", json!({"id": note}));
    assert!(hist.to_string().contains(REASON), "{hist}");
}

#[test]
fn refusals() {
    let host = empty_host();
    let (_, note) = owner_note(&host, "Keep me.");
    let batch = begin(&host);
    let ask = |args: Value| refused(&host, args);

    let m = ask(json!({"batch": batch, "note": note, "text": "New."}));
    assert!(m.contains("reason"), "{m}");
    let m = ask(json!({"batch": batch, "note": note, "text": "New.", "reason": "   "}));
    assert!(m.contains("reason"), "{m}");
    let m = ask(json!({"batch": batch, "note": note, "text": "New.", "reason": "x".repeat(1001)}));
    assert!(m.contains("reason"), "{m}");
    let m = ask(json!({"batch": batch, "note": note, "text": "  \n", "reason": REASON}));
    assert!(m.contains("text"), "{m}");
    let m = ask(
        json!({"batch": batch, "note": note, "text": "x".repeat(64 * 1024 + 1), "reason": REASON}),
    );
    assert!(m.contains("64 KiB"), "{m}");
    let m =
        ask(json!({"batch": batch, "note": Uuid::from_u128(77), "text": "New.", "reason": REASON}));
    assert!(m.contains("note"), "{m}");
    // Exactly the limit is fine.
    let big = "y".repeat(64 * 1024);
    rewrite(&host, &batch, note, &big);
    rewrite(&host, &batch, note, "Keep me.");

    // Another connection's batch.
    let m = call_as(
        &host,
        &other(),
        "rewrite_note",
        json!({"batch": batch, "note": note, "text": "New.", "reason": REASON}),
    )
    .unwrap_err();
    assert!(m.contains("batch"), "{m}");
    assert_eq!(text_of(&host, note), "Keep me.");

    // A closed batch.
    call(
        &host,
        "end_batch",
        json!({"batch": batch, "report": "Nothing more."}),
    );
    let m = ask(json!({"batch": batch, "note": note, "text": "New.", "reason": REASON}));
    assert!(m.contains("closed"), "{m}");
    assert_eq!(text_of(&host, note), "Keep me.");
}

/// `ms` of 8 kHz mono 16-bit silence.
fn wav(ms: u64) -> Vec<u8> {
    let data = (16 * ms) as u32;
    let mut out = b"RIFF".to_vec();
    out.extend((36 + data).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(8000u32.to_le_bytes());
    out.extend(16000u32.to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(data.to_le_bytes());
    out.resize(out.len() + data as usize, 0);
    out
}

#[test]
fn an_audio_clip_on_the_note_survives() {
    let host = empty_host();
    let (source, _, note) = with(&host, |s| {
        let source = s
            .apply(EditCommand::SaveSource {
                source: SourceInput {
                    id: None,
                    title: Some("Interview, 1987".into()),
                    author: None,
                    publication: None,
                    abbreviation: None,
                    text: None,
                    ..Default::default()
                },
            })
            .unwrap()
            .touched[0];
        let person = add_person(s, draft("Ivan", "Koval", None, Some("1990")));
        add_note(s, person, "He told this one at the table.");
        let note = s.get_person(person).unwrap().notes[0].id;
        (source, person, note)
    });
    let batch = begin(&host);
    let dir = host._dir.as_ref().unwrap().path().join("incoming");
    std::fs::create_dir_all(&dir).unwrap();
    let path: PathBuf = dir.join("interview.wav");
    std::fs::write(&path, wav(10_000)).unwrap();
    let rec: Uuid = serde_json::from_value(
        call(
            &host,
            "add_media",
            json!({"batch": batch, "file_path": path.to_string_lossy(), "target": {"kind": "source", "id": source}}),
        )["id"]
            .clone(),
    )
    .unwrap();
    call(
        &host,
        "add_audio_clip",
        json!({"batch": batch, "media": rec, "start": 1000, "end": 5000, "target": {"kind": "note", "id": note}}),
    );
    let clips = |host: &TestHost| {
        with(host, |s| {
            s.get_recording(rec)
                .unwrap()
                .clips
                .iter()
                .map(|c| (c.clip.start_ms, c.clip.end_ms))
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(clips(&host), [(1000, 5000)]);
    rewrite(&host, &batch, note, "Told at the table, in his own words.");
    assert_eq!(text_of(&host, note), "Told at the table, in his own words.");
    assert_eq!(clips(&host), [(1000, 5000)]);
}

#[test]
fn nobody_changes_from_alive_to_dead() {
    let host = empty_host();
    let (person, note) = owner_note(&host, "He lives in the village.");
    let status = |host: &TestHost| {
        with(host, |s| {
            let p = s.get_person(person).unwrap();
            (p.living_computed, format!("{:?}", p.living_override))
        })
    };
    let before = status(&host);
    let batch = begin(&host);
    rewrite(
        &host,
        &batch,
        note,
        "Ivan lives in the village, as he always did.",
    );
    assert_eq!(status(&host), before);
    assert!(before.0, "born in 1990 counts as living");
}
