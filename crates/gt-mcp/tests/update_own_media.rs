#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! update_own_media: an agent corrects the title, description, date and place of a media record.

mod common;

use common::*;
use gt_mcp::{AgentCtx, handle_message};
use gt_session::{EditCommand, SourceInput};
use serde_json::{Value, json};
use std::path::PathBuf;
use uuid::Uuid;

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

/// A WAV of silence, `ms` long.
fn wav(ms: u64) -> Vec<u8> {
    let rate: u32 = 8000;
    let data = (rate as u64 * ms / 1000) as u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&8u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    out.resize(out.len() + data as usize, 0);
    out
}

fn file(host: &TestHost, name: &str, bytes: &[u8]) -> PathBuf {
    let dir = host._dir.as_ref().unwrap().path().join("incoming");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

struct Setup {
    host: TestHost,
    batch: Value,
    source: Uuid,
    note: Uuid,
    rec: Uuid,
}

fn begin(host: &TestHost) -> Value {
    call(host, "begin_batch", json!({"title": "Interview, 1987"}))["batch"].clone()
}

/// A source, a note, and a recording the agent attached with a title, description and date, then
/// gave a transcript piece and a clip; the batch is closed and a new one is open.
fn setup() -> Setup {
    let host = empty_host();
    let (source, note) = with(&host, |s| {
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
        let person = add_person(s, draft("Ivan", "Koval", None, Some("1920")));
        add_note(s, person, "He told this one at the table.");
        (source, s.get_person(person).unwrap().notes[0].id)
    });
    let first = begin(&host);
    let path = file(&host, "interview.wav", &wav(120_000));
    let added = call(
        &host,
        "add_media",
        json!({"batch": first, "file_path": path.to_string_lossy(),
               "target": {"kind": "source", "id": source},
               "title": "Interview, side A", "description": "Recorded at the table.",
               "date": "5 MAR 1987", "place": "Lviv, Ukraine"}),
    );
    let rec: Uuid = serde_json::from_value(added["id"].clone()).unwrap();
    call(
        &host,
        "set_media_transcription",
        json!({"batch": first, "media": rec, "text": "Hello."}),
    );
    call(
        &host,
        "add_audio_clip",
        json!({"batch": first, "media": rec, "start": 0, "end": 1000, "target": {"kind": "note", "id": note}}),
    );
    call(
        &host,
        "end_batch",
        json!({"batch": first, "report": "Done."}),
    );
    let batch = begin(&host);
    Setup {
        host,
        batch,
        source,
        note,
        rec,
    }
}

fn update(t: &Setup, fields: Value) -> Value {
    let mut args = json!({"batch": t.batch, "media": t.rec});
    for (k, v) in fields.as_object().unwrap() {
        args[k] = v.clone();
    }
    call(&t.host, "update_own_media", args)
}

fn media_fields(
    t: &Setup,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    with(&t.host, |s| {
        let m = &s.tree().media[&t.rec];
        (
            m.title.clone(),
            m.description.clone(),
            m.date.as_ref().map(|d| d.raw.clone()),
            m.place.and_then(|p| s.place_option(p)).map(|p| p.full),
        )
    })
}

fn untouched_parts(t: &Setup) -> (usize, Option<u16>, usize, String) {
    with(&t.host, |s| {
        let m = &s.tree().media[&t.rec];
        let clips = s.tree().notes[&t.note]
            .common
            .media
            .iter()
            .filter(|l| l.clip.is_some())
            .count();
        (m.transcription.len(), m.rotation, clips, m.path.clone())
    })
}

#[test]
fn changes_the_title_only() {
    let t = setup();
    let parts = untouched_parts(&t);
    let answer = update(&t, json!({"title": "Interview with Ivan, side A"}));
    assert_eq!(answer["touched"][0], json!(t.rec), "{answer}");
    assert_eq!(
        media_fields(&t),
        (
            Some("Interview with Ivan, side A".into()),
            Some("Recorded at the table.".into()),
            Some("5 MAR 1987".into()),
            Some("Lviv, Ukraine".into()),
        )
    );
    assert_eq!(
        untouched_parts(&t),
        parts,
        "file, transcript, clips and turn stay"
    );
    assert_eq!(parts.0, 1);
    assert_eq!(parts.2, 1);
    let sources = with(&t.host, |s| s.source_dto(t.source).unwrap().media.len());
    assert_eq!(sources, 1, "still linked");
}

#[test]
fn an_empty_text_clears_title_and_description() {
    let t = setup();
    update(&t, json!({"description": ""}));
    let f = media_fields(&t);
    assert_eq!(f.0.as_deref(), Some("Interview, side A"));
    assert_eq!(f.1, None);
    update(&t, json!({"title": ""}));
    assert_eq!(media_fields(&t).0, None);
    assert_eq!(media_fields(&t).2.as_deref(), Some("5 MAR 1987"));
}

#[test]
fn sets_the_date_like_add_media_does() {
    let t = setup();
    update(&t, json!({"date": "11 NOV 2026"}));
    assert_eq!(media_fields(&t).2.as_deref(), Some("11 NOV 2026"));
    let message = call_err(
        &t.host,
        "update_own_media",
        json!({"batch": t.batch, "media": t.rec, "date": "sometime in spring"}),
    );
    assert!(message.contains("is not understood as a date"), "{message}");
    assert_eq!(media_fields(&t).2.as_deref(), Some("11 NOV 2026"));
}

#[test]
fn refuses_without_a_batch_and_with_no_field() {
    let t = setup();
    let message = call_err(
        &t.host,
        "update_own_media",
        json!({"media": t.rec, "title": "x"}),
    );
    assert!(message.contains("begin_batch"), "{message}");
    let message = call_err(
        &t.host,
        "update_own_media",
        json!({"batch": t.batch, "media": t.rec}),
    );
    assert!(message.contains("at least one of"), "{message}");
    let message = call_err(
        &t.host,
        "update_own_media",
        json!({"batch": Uuid::from_u128(77).to_string(), "media": t.rec, "title": "x"}),
    );
    assert!(!message.is_empty());
    assert_eq!(media_fields(&t).0.as_deref(), Some("Interview, side A"));
}

#[test]
fn refuses_media_someone_else_added_and_media_the_owner_changed() {
    let t = setup();
    // Another agent, in a batch of its own, may not change what is not its own.
    let theirs =
        call_as(&t.host, &other(), "begin_batch", json!({"title": "Other"})).unwrap()["batch"]
            .clone();
    let message = call_as(
        &t.host,
        &other(),
        "update_own_media",
        json!({"batch": theirs, "media": t.rec, "title": "Mine now"}),
    )
    .unwrap_err();
    assert!(message.contains("did not create"), "{message}");
    assert_eq!(media_fields(&t).0.as_deref(), Some("Interview, side A"));

    // The owner changes the title; the agent that added the record may no longer change it.
    with(&t.host, |s| {
        s.apply(EditCommand::UpdateMedia {
            id: t.rec,
            title: Some("Interview, the owner's words".into()),
            description: None,
            date_raw: None,
            place_text: None,
            place_id: None,
        })
        .unwrap();
    });
    let message = call_err(
        &t.host,
        "update_own_media",
        json!({"batch": t.batch, "media": t.rec, "title": "Agent again"}),
    );
    assert!(message.contains("someone has changed"), "{message}");
    assert_eq!(
        media_fields(&t).0.as_deref(),
        Some("Interview, the owner's words")
    );
}

#[test]
fn history_label_names_the_title_and_the_batch_undo_restores_the_old_values() {
    let t = setup();
    // The record was added in an earlier batch; this batch only corrects it.
    update(
        &t,
        json!({"title": "Interview with Ivan", "description": "", "date": "6 MAR 1987", "place": "Kyiv, Ukraine"}),
    );
    assert_eq!(
        media_fields(&t),
        (
            Some("Interview with Ivan".into()),
            None,
            Some("6 MAR 1987".into()),
            Some("Kyiv, Ukraine".into()),
        )
    );
    let batch: Uuid = t.batch.as_str().unwrap().parse().unwrap();
    with(&t.host, |s| {
        let filter = gt_store::LogFilter {
            batch: Some(batch),
            ..Default::default()
        };
        let (groups, _) = s.project().log_page(&filter, 0, 10).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].label, "Update media: Interview with Ivan");
    });
    let log = call(&t.host, "get_log", json!({"batch": t.batch})).to_string();
    assert!(
        log.contains("Interview, side A"),
        "the old title is in the log: {log}"
    );
    call(
        &t.host,
        "end_batch",
        json!({"batch": t.batch, "report": "Corrected."}),
    );
    with(&t.host, |s| {
        s.undo_batch(batch).unwrap();
    });
    assert_eq!(
        media_fields(&t),
        (
            Some("Interview, side A".into()),
            Some("Recorded at the table.".into()),
            Some("5 MAR 1987".into()),
            Some("Lviv, Ukraine".into()),
        )
    );
}

#[test]
fn saying_it_again_answers_already_there() {
    let t = setup();
    update(&t, json!({"title": "Interview with Ivan"}));
    let again = update(&t, json!({"title": "Interview with Ivan"}));
    assert_eq!(again["alreadyThere"], json!(true), "{again}");
}

#[test]
fn place_is_set_on_add_media_changed_and_cleared_and_get_media_returns_it() {
    let t = setup();
    let raw = call_raw(&t.host, "get_media", json!({"id": t.rec}));
    let details: Value = serde_json::from_str(raw["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        details["place"]["full"],
        json!("Lviv, Ukraine"),
        "{details}"
    );

    update(&t, json!({"place": "Kyiv, Ukraine"}));
    assert_eq!(media_fields(&t).3.as_deref(), Some("Kyiv, Ukraine"));
    let places = with(&t.host, |s| s.tree().places.len());
    // The same place again is found, not made twice.
    update(&t, json!({"place": "kyiv, ukraine"}));
    assert_eq!(with(&t.host, |s| s.tree().places.len()), places);

    update(&t, json!({"place": ""}));
    assert_eq!(media_fields(&t).3, None);
    let raw = call_raw(&t.host, "get_media", json!({"id": t.rec}));
    let details: Value = serde_json::from_str(raw["content"][0]["text"].as_str().unwrap()).unwrap();
    assert!(details["place"].is_null(), "{details}");
}
