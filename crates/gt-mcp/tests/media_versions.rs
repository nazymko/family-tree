#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! group_media_versions: an agent groups files of the same page so the app shows them as one, and
//! never touches a group someone else made.

mod common;

use common::*;
use gt_session::{EditCommand, SourceInput};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use serde_json::{Value, json};
use std::io::Cursor;
use std::path::PathBuf;
use uuid::Uuid;

fn begin(host: &TestHost) -> Value {
    call(
        host,
        "begin_batch",
        json!({"title": "Register of 1901, pages"}),
    )["batch"]
        .clone()
}

fn jpeg(w: u32, h: u32, shade: u8) -> Vec<u8> {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb([shade, 200, 170])));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Jpeg).unwrap();
    out.into_inner()
}

fn file(host: &TestHost, name: &str, bytes: &[u8]) -> PathBuf {
    let dir = host._dir.as_ref().unwrap().path().join("incoming");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

fn source(host: &TestHost) -> Uuid {
    with(host, |s| {
        s.apply(EditCommand::SaveSource {
            source: SourceInput {
                id: None,
                title: Some("Register of 1901".into()),
                ..Default::default()
            },
        })
        .unwrap()
        .touched[0]
    })
}

fn add(host: &TestHost, batch: &Value, source: Uuid, name: &str, w: u32, h: u32) -> Uuid {
    let path = file(host, name, &jpeg(w, h, (w % 200) as u8));
    let added = call(
        host,
        "add_media",
        json!({"batch": batch, "file_path": path.to_string_lossy(), "target": {"kind": "source", "id": source}}),
    );
    serde_json::from_value(added["id"].clone()).unwrap()
}

fn group(host: &TestHost, batch: &Value, primary: Uuid, others: &[Uuid]) -> Value {
    call_raw(
        host,
        "group_media_versions",
        json!({"batch": batch, "primary": primary, "others": others}),
    )
}

fn version_of(host: &TestHost, m: Uuid) -> Option<Uuid> {
    with(host, |s| s.tree().media[&m].version_of)
}

#[test]
fn an_agent_groups_two_copies_and_get_media_shows_the_group() {
    let host = empty_host();
    let src = source(&host);
    let batch = begin(&host);
    let big = add(&host, &batch, src, "page-original.jpg", 90, 60);
    let small = add(&host, &batch, src, "page-small.jpg", 30, 20);
    // Grouping in a batch of its own, so undoing it leaves the files.
    let batch = begin(&host);
    let done = call(
        &host,
        "group_media_versions",
        json!({"batch": batch, "primary": big, "others": [small]}),
    );
    assert_eq!(done["id"], json!(big), "{done}");
    assert_eq!(version_of(&host, small), Some(big));
    let again = call(
        &host,
        "group_media_versions",
        json!({"batch": batch, "primary": big, "others": [small]}),
    );
    assert_eq!(again["alreadyThere"], json!(true), "{again}");

    let read = call_raw(&host, "get_media", json!({"id": small}));
    let details: Value =
        serde_json::from_str(read["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(details["versionPrimary"], json!(big), "{details}");
    assert_eq!(details["versions"], json!([big, small]));
    let listed = call(&host, "list_media", json!({}));
    let rows = listed["items"].as_array().unwrap();
    assert!(
        rows.iter().all(|r| r["versionPrimary"] == json!(big)),
        "{listed}"
    );

    // The owner undoes the batch: the group goes, both files and their links stay.
    let batch_id: Uuid = serde_json::from_value(batch).unwrap();
    with(&host, |s| s.undo_batch(batch_id).unwrap());
    assert_eq!(version_of(&host, small), None);
}

#[test]
fn an_agent_never_changes_a_group_someone_else_made() {
    let host = empty_host();
    let src = source(&host);
    let batch = begin(&host);
    let a = add(&host, &batch, src, "a.jpg", 80, 60);
    let b = add(&host, &batch, src, "b.jpg", 40, 30);
    let c = add(&host, &batch, src, "c.jpg", 20, 15);
    let d = add(&host, &batch, src, "d.jpg", 24, 18);
    // The owner groups a and b, with a shown first.
    with(&host, |s| {
        s.apply(EditCommand::GroupMediaVersions {
            primary: a,
            others: vec![b],
        })
        .unwrap();
    });
    // b is a version in the owner's group: it cannot become a primary.
    let refused = group(&host, &batch, b, &[c]);
    assert_eq!(refused["isError"], json!(true), "{refused}");
    // a heads the owner's group: c cannot be taken into another group led by d.
    let refused = group(&host, &batch, d, &[a]);
    assert_eq!(refused["isError"], json!(true), "{refused}");
    let message = refused["content"][0]["text"].as_str().unwrap();
    assert!(message.contains("another group"), "{message}");
    assert_eq!(version_of(&host, b), Some(a));
    assert_eq!(version_of(&host, d), None);
    // Adding a file to the group that a heads is fine.
    let added = group(&host, &batch, a, &[c]);
    assert_ne!(added["isError"], json!(true), "{added}");
    assert_eq!(version_of(&host, c), Some(a));
}

#[test]
fn group_media_versions_checks_its_arguments() {
    let host = empty_host();
    let src = source(&host);
    let batch = begin(&host);
    let a = add(&host, &batch, src, "a.jpg", 80, 60);
    for bad in [json!([]), json!("x"), json!([1, 2])] {
        let message = call_err(
            &host,
            "group_media_versions",
            json!({"batch": batch, "primary": a, "others": bad}),
        );
        assert!(message.contains("others"), "{message}");
    }
    let message = call_err(
        &host,
        "group_media_versions",
        json!({"batch": batch, "primary": a, "others": [a]}),
    );
    assert!(message.contains("primary"), "{message}");
    let message = call_err(
        &host,
        "group_media_versions",
        json!({"batch": batch, "primary": a, "others": [Uuid::from_u128(5)]}),
    );
    assert!(!message.is_empty());
    // An agent never takes a file out of a group, not even one it made.
    let b = add(&host, &batch, src, "b.jpg", 40, 30);
    group(&host, &batch, a, &[b]);
    let batch_id: Uuid = serde_json::from_value(batch).unwrap();
    let err = with(&host, |s| {
        s.apply_as_agent(
            EditCommand::UngroupMediaVersion {
                media: b,
                new_primary: None,
            },
            &ctx(),
            batch_id,
        )
        .unwrap_err()
    });
    assert!(err.message.contains("Only the owner"), "{}", err.message);
    assert_eq!(version_of(&host, b), Some(a));
}
