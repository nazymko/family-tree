#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! add_media, set_media_transcription and update_own_media_region, and what get_media reads back.

mod common;

use common::*;
use gt_session::{EditCommand, OwnerKind, OwnerRef, SourceInput, TextAreaInput, TextRegionInput};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use serde_json::{Value, json};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn begin(host: &TestHost) -> Value {
    call(host, "begin_batch", json!({"title": "Letters of 1901"}))["batch"].clone()
}

fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb([210, 200, 170])));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Jpeg).unwrap();
    out.into_inner()
}

/// A file next to the project, outside its media folder.
fn file(host: &TestHost, name: &str, bytes: &[u8]) -> PathBuf {
    let dir = host._dir.as_ref().unwrap().path().join("incoming");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

/// A source the owner made.
fn owners_source(host: &TestHost) -> Uuid {
    with(host, |s| {
        let r = s
            .apply(EditCommand::SaveSource {
                source: SourceInput {
                    id: None,
                    title: Some("Letters of 1901".into()),
                    author: None,
                    publication: None,
                    abbreviation: None,
                    text: None,
                },
            })
            .unwrap();
        r.touched[0]
    })
}

fn attach(host: &TestHost, batch: &Value, source: Uuid, path: &Path) -> Value {
    call(
        host,
        "add_media",
        json!({
            "batch": batch,
            "file_path": path.to_string_lossy(),
            "target": {"kind": "source", "id": source},
            "title": "Letter, page 1",
            "date": "12 MAR 1901",
        }),
    )
}

fn media_texts(host: &TestHost, media: Uuid) -> Vec<String> {
    with(host, |s| {
        s.tree().media[&media]
            .transcription
            .iter()
            .map(|r| r.text.clone())
            .collect()
    })
}

#[test]
fn add_media_attaches_once_and_get_media_reads_the_transcription() {
    let host = empty_host();
    let source = owners_source(&host);
    let batch = begin(&host);
    let path = file(&host, "letter.jpg", &jpeg(60, 40));
    let added = attach(&host, &batch, source, &path);
    let media: Uuid = serde_json::from_value(added["id"].clone()).unwrap();
    assert!(added["alreadyThere"].is_null(), "{added}");
    assert!(path.exists(), "the original stays where it was");
    // The copy is in the project's media folder, under its content hash.
    let stored = with(&host, |s| s.tree().media[&media].path.clone());
    assert!(stored.starts_with("media/"), "{stored}");
    let again = attach(&host, &batch, source, &path);
    assert_eq!(again["alreadyThere"], json!(true));
    assert_eq!(again["id"], json!(media));

    let set = call(
        &host,
        "set_media_transcription",
        json!({"batch": batch, "media": media, "regions": [
            {"text": "Dear Olha,", "x": 0.1, "y": 0.1, "w": 0.5, "h": 0.08},
            {"text": "Kyiv", "x": 0.9, "y": 0.2, "w": 0.05, "h": 0.6, "rotation": 90},
        ]}),
    );
    assert_eq!(set["id"], json!(media));
    let again = call(
        &host,
        "set_media_transcription",
        json!({"batch": batch, "media": media, "regions": [
            {"text": "Dear Olha,", "x": 0.1, "y": 0.1, "w": 0.5, "h": 0.08},
            {"text": "Kyiv", "x": 0.9, "y": 0.2, "w": 0.05, "h": 0.6, "rotation": 90},
        ]}),
    );
    assert_eq!(again["alreadyThere"], json!(true), "{again}");

    let read = call_raw(&host, "get_media", json!({"id": media}));
    let details: Value =
        serde_json::from_str(read["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(details["fullText"], json!("Dear Olha,\n\nKyiv"));
    assert_eq!(details["transcription"][1]["rotation"], json!(90));
    assert_eq!(details["transcription"][0]["yours"], json!(true));
    assert_eq!(details["title"], json!("Letter, page 1"));
    let listed = call(&host, "list_media", json!({}));
    let row = &listed["items"][0];
    assert_eq!(row["fullText"], json!("Dear Olha,\n\nKyiv"), "{listed}");
    assert_eq!(row["transcriptionParts"], json!(2));
}

#[test]
fn add_media_refuses_what_is_not_a_picture() {
    let host = empty_host();
    let source = owners_source(&host);
    let batch = begin(&host);
    let missing = host._dir.as_ref().unwrap().path().join("nowhere.jpg");
    let message = call_err(
        &host,
        "add_media",
        json!({"batch": batch, "file_path": missing.to_string_lossy(), "target": {"kind": "source", "id": source}}),
    );
    assert!(message.contains("no file"), "{message}");
    let text = file(&host, "secrets.txt", b"user=me\npassword=nothing\n");
    let message = call_err(
        &host,
        "add_media",
        json!({"batch": batch, "file_path": text.to_string_lossy(), "target": {"kind": "source", "id": source}}),
    );
    assert!(message.contains("Only pictures"), "{message}");
    let mut fake = jpeg(10, 10);
    fake.truncate(30);
    let fake = file(&host, "fake.jpg", &fake);
    let message = call_err(
        &host,
        "add_media",
        json!({"batch": batch, "file_path": fake.to_string_lossy(), "target": {"kind": "source", "id": source}}),
    );
    assert!(message.contains("not a picture"), "{message}");
    let big = file(&host, "huge.jpg", b"");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&big)
        .unwrap()
        .set_len(gt_session::AGENT_FILE_MAX + 1)
        .unwrap();
    let message = call_err(
        &host,
        "add_media",
        json!({"batch": batch, "file_path": big.to_string_lossy(), "target": {"kind": "source", "id": source}}),
    );
    assert!(message.contains("50 MiB"), "{message}");
    let good = file(&host, "page.jpg", &jpeg(20, 20));
    let message = call_err(
        &host,
        "add_media",
        json!({"batch": batch, "file_path": good.to_string_lossy(), "target": {"kind": "note", "id": source}}),
    );
    assert!(message.contains("target.kind"), "{message}");
    // Nothing reached the tree, and nothing was copied into it.
    assert!(with(&host, |s| s.tree().media.is_empty()));
    let stored = host._dir.as_ref().unwrap().path().join("project/media");
    let copied = walk(&stored);
    assert_eq!(copied, 0, "refused files are never copied");
}

fn walk(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| {
                    if e.path().is_dir() {
                        walk(&e.path())
                    } else {
                        1
                    }
                })
                .sum()
        })
        .unwrap_or(0)
}

#[test]
fn closed_batch_is_refused_for_media_writes() {
    let host = empty_host();
    let source = owners_source(&host);
    let batch = begin(&host);
    let path = file(&host, "letter.jpg", &jpeg(30, 30));
    let media: Uuid =
        serde_json::from_value(attach(&host, &batch, source, &path)["id"].clone()).unwrap();
    call(
        &host,
        "end_batch",
        json!({"batch": batch, "report": "One letter."}),
    );
    let message = call_err(
        &host,
        "set_media_transcription",
        json!({"batch": batch, "media": media, "text": "Dear Olha"}),
    );
    assert!(message.contains("closed"), "{message}");
    let other = file(&host, "other.jpg", &jpeg(31, 30));
    let message = call_err(
        &host,
        "add_media",
        json!({"batch": batch, "file_path": other.to_string_lossy(), "target": {"kind": "source", "id": source}}),
    );
    assert!(message.contains("closed"), "{message}");
}

#[test]
fn an_owners_region_stays_and_only_own_regions_change() {
    let host = empty_host();
    let source = owners_source(&host);
    // The owner attached the picture and wrote one line of it.
    let path = file(&host, "letter.jpg", &jpeg(40, 30));
    let media = with(&host, |s| {
        let target = OwnerRef {
            kind: OwnerKind::Source,
            id: source,
        };
        let r = s.attach_files(target, std::slice::from_ref(&path)).unwrap();
        let media = r.attached[0].id;
        s.apply(EditCommand::SetMediaTranscription {
            id: media,
            regions: vec![TextRegionInput {
                id: None,
                area: Some(TextAreaInput {
                    x: 0.1,
                    y: 0.1,
                    w: 0.4,
                    h: 0.1,
                }),
                rotation: None,
                text: "Owner's line".into(),
            }],
            known: None,
        })
        .unwrap();
        media
    });
    let batch = begin(&host);
    let set = call(
        &host,
        "set_media_transcription",
        json!({"batch": batch, "media": media, "regions": [{"text": "Agent's line", "x": 0.1, "y": 0.5, "w": 0.4, "h": 0.1}]}),
    );
    assert!(
        set["warnings"][0].as_str().unwrap().contains("1 piece"),
        "{set}"
    );
    assert_eq!(media_texts(&host, media), ["Owner's line", "Agent's line"]);
    let ids: Vec<Uuid> = with(&host, |s| {
        s.tree().media[&media]
            .transcription
            .iter()
            .map(|r| r.id)
            .collect()
    });
    // Its own line it may correct.
    call(
        &host,
        "update_own_media_region",
        json!({"batch": batch, "media": media, "region": ids[1], "text": "Agent's line, read again", "rotation": 180}),
    );
    assert_eq!(media_texts(&host, media)[1], "Agent's line, read again");
    let again = call(
        &host,
        "update_own_media_region",
        json!({"batch": batch, "media": media, "region": ids[1], "text": "Agent's line, read again"}),
    );
    assert_eq!(again["alreadyThere"], json!(true), "{again}");
    // The owner's line it may not.
    let message = call_err(
        &host,
        "update_own_media_region",
        json!({"batch": batch, "media": media, "region": ids[0], "text": "Rewritten"}),
    );
    assert!(message.contains("someone else"), "{message}");
    assert_eq!(media_texts(&host, media)[0], "Owner's line");
    // Bad shapes are refused before anything is planned.
    let half = call_err(
        &host,
        "set_media_transcription",
        json!({"batch": batch, "media": media, "regions": [{"text": "x", "x": 0.1}]}),
    );
    assert!(half.contains("together"), "{half}");
    let turned = call_err(
        &host,
        "set_media_transcription",
        json!({"batch": batch, "media": media, "regions": [{"text": "x", "rotation": 45}]}),
    );
    assert!(turned.contains("rotation"), "{turned}");
    let both = call_err(
        &host,
        "set_media_transcription",
        json!({"batch": batch, "media": media, "regions": [], "text": "x"}),
    );
    assert!(both.contains("one of them"), "{both}");
    // An empty list takes the agent's own lines away, never the owner's.
    call(
        &host,
        "set_media_transcription",
        json!({"batch": batch, "media": media, "regions": []}),
    );
    assert_eq!(media_texts(&host, media), ["Owner's line"]);
}
