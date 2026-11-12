#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! A recording an agent attaches to its source, the clips it links where the story is told,
//! and what get_media and the other reads say of them. Every recording is built here.

mod common;

use common::*;
use gt_session::{EditCommand, OwnerKind, OwnerRef, SourceInput};
use serde_json::{Value, json};
use std::path::PathBuf;
use uuid::Uuid;

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
    person: Uuid,
    note: Uuid,
    rec: Uuid,
}

/// Ivan with a note, a source made by the owner, and a two-minute recording the agent attached.
fn setup() -> Setup {
    let host = empty_host();
    let (source, person, note) = with(&host, |s| {
        let source = s
            .apply(EditCommand::SaveSource {
                source: SourceInput {
                    id: None,
                    title: Some("Interview with Ivan Koval, 1987".into()),
                    author: None,
                    publication: None,
                    abbreviation: None,
                    text: None,
                },
            })
            .unwrap()
            .touched[0];
        let person = add_person(s, draft("Ivan", "Koval", None, Some("1920")));
        add_note(s, person, "He told this one at the table.");
        let note = s.get_person(person).unwrap().notes[0].id;
        (source, person, note)
    });
    let batch = call(&host, "begin_batch", json!({"title": "Interview, 1987"}))["batch"].clone();
    let path = file(&host, "interview.wav", &wav(120_000));
    let answer = call(
        &host,
        "add_media",
        json!({"batch": batch, "file_path": path.to_string_lossy(), "target": {"kind": "source", "id": source}, "title": "Interview, side A"}),
    );
    let rec: Uuid = serde_json::from_value(answer["id"].clone()).unwrap();
    Setup {
        host,
        batch,
        source,
        person,
        note,
        rec,
    }
}

#[test]
fn a_recording_without_a_date_takes_it_from_its_phone_file_name_and_says_so() {
    let t = setup();
    let path = file(&t.host, "Voice 261111_090049.wav", &wav(1_000));
    let answer = call(
        &t.host,
        "add_media",
        json!({"batch": t.batch, "file_path": path.to_string_lossy(), "target": {"kind": "source", "id": t.source}}),
    );
    let said = answer["warnings"][0].as_str().unwrap();
    assert!(
        said.contains("Taken from the file name") && said.contains("11 NOV 2026"),
        "{said}"
    );
    let id: Uuid = serde_json::from_value(answer["id"].clone()).unwrap();
    let (date, title, description) = with(&t.host, |s| {
        let m = &s.tree().media[&id];
        (
            m.date.as_ref().map(|d| d.raw.clone()),
            m.title.clone(),
            m.description.clone(),
        )
    });
    assert_eq!(date.as_deref(), Some("11 NOV 2026"));
    // The session's interface language is Ukrainian by default.
    assert_eq!(
        title.as_deref(),
        Some("Запис розмови 11.11.2026 (Voice 261111_090049)")
    );
    assert!(description.unwrap().contains("09:00:49"));
}

#[test]
fn an_explicit_title_and_date_from_the_agent_win_over_the_file_name() {
    let t = setup();
    let path = file(&t.host, "VID20261031202924.wav", &wav(1_000));
    let answer = call(
        &t.host,
        "add_media",
        json!({"batch": t.batch, "file_path": path.to_string_lossy(), "target": {"kind": "source", "id": t.source},
               "title": "Side B", "date": "5 MAR 1987"}),
    );
    let id: Uuid = serde_json::from_value(answer["id"].clone()).unwrap();
    let (date, title) = with(&t.host, |s| {
        let m = &s.tree().media[&id];
        (m.date.as_ref().map(|d| d.raw.clone()), m.title.clone())
    });
    assert_eq!(date.as_deref(), Some("5 MAR 1987"));
    assert_eq!(title.as_deref(), Some("Side B"));
    let said = answer["warnings"][0].as_str().unwrap_or_default();
    assert!(!said.contains("date"), "{said}");
}

#[test]
fn a_recording_goes_on_its_source_only_and_get_media_reads_it_without_its_sound() {
    let t = setup();
    let path = file(&t.host, "again.wav", &wav(1_000));
    let message = call_err(
        &t.host,
        "add_media",
        json!({"batch": t.batch, "file_path": path.to_string_lossy(), "target": {"kind": "person", "id": t.person}}),
    );
    assert!(message.contains("goes on its source"), "{message}");
    assert!(
        with(&t.host, |s| s.tree().media.len()) == 1,
        "nothing was added for it"
    );

    let raw = call_raw(&t.host, "get_media", json!({"id": t.rec}));
    let details: Value = serde_json::from_str(raw["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(details["durationMs"], json!(120_000));
    assert_eq!(details["duration"], json!("2:00"));
    assert_eq!(details["sources"][0]["id"], json!(t.source));
    assert_eq!(details["clips"], json!([]));
    let second = raw["content"][1]["text"].as_str().unwrap();
    assert!(second.contains("not sent to agents"), "{second}");
    assert!(!raw.to_string().contains("\"blob\""));
}

#[test]
fn add_audio_clip_takes_milliseconds_or_clock_times_and_answers_a_repeat() {
    let t = setup();
    let clip = |start: Value, end: Value, label: &str, target: Value| {
        call_raw(
            &t.host,
            "add_audio_clip",
            json!({"batch": t.batch, "media": t.rec, "start": start, "end": end, "label": label, "target": target}),
        )
    };
    let note = json!({"kind": "note", "id": t.note});
    let first = call(
        &t.host,
        "add_audio_clip",
        json!({"batch": t.batch, "media": t.rec, "start": "1:05", "end": 102_500, "label": "The table", "target": note}),
    );
    let id = first["id"].clone();
    assert!(id.is_string(), "{first}");
    let again = call(
        &t.host,
        "add_audio_clip",
        json!({"batch": t.batch, "media": t.rec, "start": 65_000, "end": "1:42.5", "label": "The table", "target": note}),
    );
    assert_eq!(again, json!({"id": id, "alreadyThere": true}));
    let person = json!({"kind": "person", "id": t.person});
    let r = clip(json!("0:00:10"), json!("0:00:20"), "Hello", person.clone());
    assert_ne!(r["isError"], json!(true), "{r}");

    for (start, end, want) in [
        (json!("1:5"), json!(9000), "start must be"),
        (json!(-5), json!(9000), "start must be"),
        (json!(5000), json!(4000), "must end after it starts"),
        (json!(5000), json!("2:00.001"), "The recording lasts 2:00"),
    ] {
        let r = clip(start.clone(), end.clone(), "x", person.clone());
        assert_eq!(r["isError"], json!(true), "{start} {end}");
        let text = r["content"][0]["text"].as_str().unwrap();
        assert!(text.contains(want), "{start} {end}: {text}");
    }
    let r = clip(
        json!(0),
        json!(10),
        "x",
        json!({"kind": "source", "id": t.source}),
    );
    assert!(
        r["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("target.kind must be one of")
    );

    // Reads of the note and the person list their clips.
    let n = call(&t.host, "get_note", json!({"id": t.note}));
    assert_eq!(n["audioClips"][0]["start"], json!("1:05"));
    assert_eq!(n["audioClips"][0]["endMs"], json!(102_500));
    assert_eq!(n["audioClips"][0]["recording"], json!("Interview, side A"));
    assert!(
        n.get("voiceSamples").is_none(),
        "a clip is not a voice sample: {n}"
    );
    let p = call(&t.host, "get_person", json!({"id": t.person}));
    assert_eq!(p["audioClips"][0]["label"], json!("Hello"), "{p}");

    let details = call(&t.host, "get_media", json!({"id": t.rec}));
    let clips = details["clips"].as_array().unwrap();
    assert_eq!(clips.len(), 2);
    assert_eq!(clips[0]["start"], json!("0:10"));
    assert_eq!(
        clips[0]["target"],
        json!({"kind": "person", "id": t.person})
    );
    assert_eq!(clips[1]["person"]["id"], json!(t.person));
    assert!(clips.iter().all(|c| c["yours"] == json!(true)));
}

#[test]
fn update_own_audio_clip_changes_only_its_own_and_never_deletes() {
    let t = setup();
    let note = json!({"kind": "note", "id": t.note});
    let id = call(
        &t.host,
        "add_audio_clip",
        json!({"batch": t.batch, "media": t.rec, "start": 1000, "end": 5000, "target": note}),
    )["id"]
        .clone();
    let moved = call(
        &t.host,
        "update_own_audio_clip",
        json!({"batch": t.batch, "clip": id, "target": note, "end": "0:06", "label": "Born in winter",
               "move_to": {"kind": "person", "id": t.person}}),
    );
    assert_eq!(moved["id"], id);
    let c = with(&t.host, |s| {
        s.get_recording_clip(
            OwnerRef {
                kind: OwnerKind::Person,
                id: t.person,
            },
            serde_json::from_value(id.clone()).unwrap(),
        )
    })
    .unwrap();
    assert_eq!(
        (c.start_ms, c.end_ms, c.label.as_deref()),
        (1000, 6000, Some("Born in winter"))
    );

    // The owner's clip stays as it is.
    let owners = with(&t.host, |s| {
        s.apply(EditCommand::AddAudioClip {
            target: OwnerRef {
                kind: OwnerKind::Note,
                id: t.note,
            },
            media: t.rec,
            start_ms: 0,
            end_ms: 900,
            label: None,
        })
        .unwrap();
        s.get_recording(t.rec).unwrap().clips[0].clip.id
    });
    let message = call_err(
        &t.host,
        "update_own_audio_clip",
        json!({"batch": t.batch, "clip": owners, "target": note, "label": "mine now"}),
    );
    assert!(
        message.contains("owner") || message.contains("someone"),
        "{message}"
    );
    let details = call(&t.host, "get_media", json!({"id": t.rec}));
    let yours: Vec<bool> = details["clips"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["yours"].as_bool().unwrap())
        .collect();
    assert_eq!(yours, [false, true], "{details}");
}

#[test]
fn a_recording_takes_a_transcript_with_times_and_speakers() {
    let t = setup();
    call(
        &t.host,
        "set_media_transcription",
        json!({"batch": t.batch, "media": t.rec, "regions": [
            {"text": "Where were you born?", "start": 0, "end": "0:02.5", "speaker": "Interviewer"},
            {"text": "In the village.", "start": "0:02.5", "end": 7000, "speaker": "Ivan"},
        ]}),
    );
    let details = call(&t.host, "get_media", json!({"id": t.rec}));
    let pieces = details["transcription"].as_array().unwrap();
    assert_eq!(pieces[1]["startMs"], json!(2500));
    assert_eq!(pieces[1]["end"], json!("0:07"));
    assert_eq!(pieces[1]["speaker"], json!("Ivan"));
    assert_eq!(pieces[1]["yours"], json!(true));
    let message = call_err(
        &t.host,
        "set_media_transcription",
        json!({"batch": t.batch, "media": t.rec, "regions": [
            {"text": "a", "x": 0.1, "y": 0.1, "w": 0.2, "h": 0.2},
        ]}),
    );
    assert!(message.contains("no picture"), "{message}");
}

#[test]
fn undoing_the_batch_takes_the_clips_the_link_and_the_copy() {
    let t = setup();
    let stored = with(&t.host, |s| s.media_original_path(t.rec).unwrap());
    call(
        &t.host,
        "add_audio_clip",
        json!({"batch": t.batch, "media": t.rec, "start": 0, "end": 1000, "target": {"kind": "note", "id": t.note}}),
    );
    call(
        &t.host,
        "end_batch",
        json!({"batch": t.batch, "report": "One recording, one clip."}),
    );
    let batch: Uuid = serde_json::from_value(t.batch.clone()).unwrap();
    with(&t.host, |s| {
        s.undo_batch(batch).unwrap();
        assert!(s.tree().media.is_empty());
        assert!(s.source_dto(t.source).unwrap().media.is_empty());
        assert!(s.get_person(t.person).unwrap().notes[0].media.is_empty());
    });
    assert!(!stored.exists());
}
