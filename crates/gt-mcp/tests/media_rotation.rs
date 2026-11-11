#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! set_media_rotation, and how get_media and the media rows report a picture's saved turn.

mod common;

use base64::Engine;
use common::*;
use gt_session::{EditCommand, SourceInput};
use image::{DynamicImage, GenericImageView, ImageFormat, Rgb, RgbImage};
use serde_json::{Value, json};
use std::io::Cursor;
use std::path::PathBuf;
use uuid::Uuid;

fn begin(host: &TestHost) -> Value {
    call(host, "begin_batch", json!({"title": "Register of 1901"}))["batch"].clone()
}

/// A landscape picture, wider than it is tall, so a turned copy would show.
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
                author: None,
                publication: None,
                abbreviation: None,
                text: None,
            },
        })
        .unwrap()
        .touched[0]
    })
}

fn add(host: &TestHost, batch: &Value, source: Uuid, name: &str, shade: u8) -> Uuid {
    let path = file(host, name, &jpeg(60, 40, shade));
    let added = call(
        host,
        "add_media",
        json!({"batch": batch, "file_path": path.to_string_lossy(), "target": {"kind": "source", "id": source}}),
    );
    serde_json::from_value(added["id"].clone()).unwrap()
}

fn turn(host: &TestHost, batch: &Value, media: Uuid, rotation: u16) -> Value {
    call_raw(
        host,
        "set_media_rotation",
        json!({"batch": batch, "media": media, "rotation": rotation}),
    )
}

#[test]
fn an_agent_turns_its_picture_and_get_media_says_so_with_the_picture_unturned() {
    let host = empty_host();
    let src = source(&host);
    let batch = begin(&host);
    let media = add(&host, &batch, src, "register.jpg", 120);
    let set = call(
        &host,
        "set_media_rotation",
        json!({"batch": batch, "media": media, "rotation": 90}),
    );
    assert_eq!(set["id"], json!(media), "{set}");
    let again = call(
        &host,
        "set_media_rotation",
        json!({"batch": batch, "media": media, "rotation": 90.0}),
    );
    assert_eq!(again["alreadyThere"], json!(true), "{again}");
    assert_eq!(with(&host, |s| s.tree().media[&media].rotation), Some(90));

    let read = call_raw(&host, "get_media", json!({"id": media}));
    let details: Value =
        serde_json::from_str(read["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(details["rotation"], json!(90));
    // The image is the picture upright by EXIF only: the transcription's places are on it.
    let image = &read["content"][1];
    assert_eq!(image["type"], json!("image"));
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(image["data"].as_str().unwrap())
        .unwrap();
    let (w, h) = image::load_from_memory(&bytes).unwrap().dimensions();
    assert!(w > h, "{w}x{h}: the picture is not turned");
    let listed = call(&host, "list_media", json!({}));
    assert_eq!(listed["items"][0]["rotation"], json!(90), "{listed}");
}

#[test]
fn an_agent_turns_the_owners_picture_only_while_nobody_has() {
    let host = empty_host();
    let src = source(&host);
    let batch = begin(&host);
    let media = add(&host, &batch, src, "letter.jpg", 30);
    // The owner turns it upright on purpose.
    with(&host, |s| {
        s.apply(EditCommand::SetMediaRotation {
            id: media,
            rotation: 180,
        })
        .unwrap();
    });
    let refused = turn(&host, &batch, media, 270);
    assert_eq!(refused["isError"], json!(true), "{refused}");
    let message = refused["content"][0]["text"].as_str().unwrap();
    assert!(message.contains("already turned"), "{message}");
    assert_eq!(with(&host, |s| s.tree().media[&media].rotation), Some(180));
}

#[test]
fn set_media_rotation_checks_the_batch_and_the_turn_first() {
    let host = empty_host();
    let src = source(&host);
    let batch = begin(&host);
    let media = add(&host, &batch, src, "page.jpg", 60);
    call(
        &host,
        "end_batch",
        json!({"batch": batch, "report": "Added one page."}),
    );
    let message = call_err(
        &host,
        "set_media_rotation",
        json!({"batch": batch, "media": Uuid::from_u128(1), "rotation": 90}),
    );
    assert!(message.contains("closed"), "{message}");
    let open = begin(&host);
    for bad in [json!(45), json!(90.5), json!("90")] {
        let message = call_err(
            &host,
            "set_media_rotation",
            json!({"batch": open, "media": media, "rotation": bad}),
        );
        assert!(message.contains("0, 90, 180 or 270"), "{message}");
    }
    assert_eq!(with(&host, |s| s.tree().media[&media].rotation), None);
}

#[test]
fn the_tool_texts_say_where_the_places_are() {
    let host = empty_host();
    let list = gt_mcp::handle_message(&host, &ctx(), &request("tools/list", json!({}))).unwrap();
    let tools = list["result"]["tools"].as_array().unwrap().clone();
    let text = |name: &str| {
        tools
            .iter()
            .find(|t| t["name"] == json!(name))
            .unwrap_or_else(|| panic!("{name} is listed"))["description"]
            .as_str()
            .unwrap()
            .to_string()
    };
    for name in ["get_media", "set_media_rotation", "set_media_transcription"] {
        assert!(
            text(name).contains("upright by its EXIF orientation"),
            "{name}: {}",
            text(name)
        );
    }
    assert!(text("get_media").contains("not turned"));
}
