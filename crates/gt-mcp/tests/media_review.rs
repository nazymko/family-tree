#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! What add_media and the transcription tools refuse, and what a refused call leaves behind.

mod common;

use common::*;
use gt_session::{EditCommand, OwnerKind, OwnerRef, SourceInput, TextRegionInput};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use serde_json::{Value, json};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn begin(host: &TestHost) -> Value {
    call(host, "begin_batch", json!({"title": "Letters of 1901"}))["batch"].clone()
}

fn picture(w: u32, h: u32, format: ImageFormat) -> Vec<u8> {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb([210, 200, 170])));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, format).unwrap();
    out.into_inner()
}

fn root(host: &TestHost) -> PathBuf {
    host._dir.as_ref().unwrap().path().to_path_buf()
}

fn file(host: &TestHost, name: &str, bytes: &[u8]) -> PathBuf {
    let dir = root(host).join("incoming");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

fn source(host: &TestHost, title: &str) -> Uuid {
    with(host, |s| {
        s.apply(EditCommand::SaveSource {
            source: SourceInput {
                id: None,
                title: Some(title.into()),
                author: None,
                publication: None,
                abbreviation: None,
                text: None,
                ..Default::default()
            },
        })
        .unwrap()
        .touched[0]
    })
}

fn add(host: &TestHost, batch: &Value, path: &Path, kind: &str, id: Uuid) -> Value {
    call_raw(
        host,
        "add_media",
        json!({"batch": batch, "file_path": path.to_string_lossy(), "target": {"kind": kind, "id": id}}),
    )
}

fn message(result: &Value) -> String {
    assert_eq!(
        result["isError"],
        json!(true),
        "expected an error: {result}"
    );
    result["content"][0]["text"].as_str().unwrap().to_string()
}

fn ok(result: &Value) -> Value {
    assert_ne!(result["isError"], json!(true), "tool error: {result}");
    serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
}

/// Every file under the project's media folder.
fn stored(host: &TestHost) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                walk(&e.path(), out);
            } else {
                out.push(e.path());
            }
        }
    }
    let mut out = Vec::new();
    walk(&root(host).join("project/media"), &mut out);
    out
}

#[cfg(windows)]
#[test]
fn network_device_and_verbatim_paths_are_refused_before_anything_is_opened() {
    let host = empty_host();
    let src = source(&host, "Letters");
    let batch = begin(&host);
    let good = file(&host, "page.jpg", &picture(20, 20, ImageFormat::Jpeg));
    let verbatim = format!(r"\\?\{}", good.display());
    for path in [
        r"\\files.example\scans\page.jpg".to_string(),
        r"\\.\C:\page.jpg".to_string(),
        verbatim,
        r"\\?\UNC\files.example\scans\page.jpg".to_string(),
    ] {
        let m = message(&add(&host, &batch, Path::new(&path), "source", src));
        assert!(m.contains("local drive"), "{path}: {m}");
    }
    assert!(stored(&host).is_empty());
}

#[test]
fn a_link_to_a_file_is_refused() {
    let host = empty_host();
    let src = source(&host, "Letters");
    let batch = begin(&host);
    let real = root(&host).join("real");
    std::fs::create_dir_all(&real).unwrap();
    std::fs::write(real.join("page.jpg"), picture(20, 20, ImageFormat::Jpeg)).unwrap();
    let link = root(&host).join("linked");
    #[cfg(windows)]
    let made = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&real)
        .output()
        .is_ok_and(|o| o.status.success());
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&real, &link).is_ok();
    assert!(made, "a junction or symlink can be made here");
    let m = message(&add(&host, &batch, &link.join("page.jpg"), "source", src));
    assert!(m.contains("link"), "{m}");
    assert!(stored(&host).is_empty());
}

#[test]
fn the_batch_target_and_date_are_checked_before_the_file_is_touched() {
    let host = empty_host();
    let src = source(&host, "Letters");
    let batch = begin(&host);
    let good = file(&host, "page.jpg", &picture(20, 20, ImageFormat::Jpeg));
    let missing = root(&host).join("nowhere.jpg");

    // A foreign batch says so, whether or not the file is there: no probing of paths.
    let other = gt_mcp::AgentCtx {
        id: Uuid::from_u128(8),
        name: "Another".into(),
    };
    let theirs = with(&host, |s| s.begin_batch(&other, "Theirs", None).unwrap());
    for path in [&good, &missing] {
        let m = message(&add(&host, &json!(theirs), path, "source", src));
        assert!(m.contains("another connection"), "{m}");
    }
    // A record that is not there.
    for path in [&good, &missing] {
        let m = message(&add(&host, &batch, path, "source", Uuid::from_u128(99)));
        assert!(m.contains("No source"), "{m}");
    }
    // A record of the wrong kind for its id.
    let m = message(&add(&host, &batch, &good, "person", src));
    assert!(m.contains("No person"), "{m}");
    // A date that cannot be read.
    for path in [&good, &missing] {
        let r = call_raw(
            &host,
            "add_media",
            json!({"batch": batch, "file_path": path.to_string_lossy(), "target": {"kind": "source", "id": src}, "date": "when the lilacs bloomed"}),
        );
        assert!(message(&r).contains("not understood"), "{r}");
    }
    // A closed batch.
    call(
        &host,
        "end_batch",
        json!({"batch": batch, "report": "Nothing."}),
    );
    for path in [&good, &missing] {
        let m = message(&add(&host, &batch, path, "source", src));
        assert!(m.contains("closed"), "{m}");
    }
    assert!(stored(&host).is_empty(), "{:?}", stored(&host));
    assert!(with(&host, |s| s.tree().media.is_empty()));
}

#[test]
fn the_stored_file_is_named_by_what_it_is_not_by_its_name() {
    let host = empty_host();
    let src = source(&host, "Letters");
    let batch = begin(&host);
    let png = picture(20, 20, ImageFormat::Png);
    let disguised = file(&host, "scan.hta", &png);
    let id: Uuid =
        serde_json::from_value(ok(&add(&host, &batch, &disguised, "source", src))["id"].clone())
            .unwrap();
    let path = with(&host, |s| s.tree().media[&id].path.clone());
    assert!(path.ends_with(".png"), "{path}");
    // The owner's own attach does the same.
    let other = file(&host, "other.exe", &picture(21, 20, ImageFormat::Png));
    let owners = with(&host, |s| {
        let r = s
            .attach_files(
                OwnerRef {
                    kind: OwnerKind::Source,
                    id: src,
                },
                &[other],
            )
            .unwrap();
        s.tree().media[&r.attached[0].id].path.clone()
    });
    assert!(owners.ends_with(".png"), "{owners}");
    assert!(
        stored(&host)
            .iter()
            .all(|p| p.extension().is_some_and(|e| e == "png"))
    );
}

#[test]
fn a_pdf_larger_than_get_media_returns_is_refused() {
    let host = empty_host();
    let src = source(&host, "Letters");
    let batch = begin(&host);
    let mut pdf = b"%PDF-1.4\n".to_vec();
    pdf.resize(
        usize::try_from(gt_session::AGENT_PDF_MAX).unwrap() + 1 - 6,
        b' ',
    );
    pdf.extend_from_slice(b"%%EOF\n");
    let big = file(&host, "register.pdf", &pdf);
    let m = message(&add(&host, &batch, &big, "source", src));
    assert!(m.contains("8 MiB"), "{m}");
    assert!(stored(&host).is_empty());
    // One just under is attached.
    pdf.truncate(pdf.len() - 7);
    pdf.extend_from_slice(b"%%EOF\n");
    let fits = file(&host, "register2.pdf", &pdf);
    ok(&add(&host, &batch, &fits, "source", src));
}

#[test]
fn a_file_already_in_the_tree_answers_with_its_id() {
    let host = empty_host();
    let first = source(&host, "Letters");
    let second = source(&host, "Parish register");
    let path = file(&host, "page.jpg", &picture(30, 20, ImageFormat::Jpeg));
    let owners = with(&host, |s| {
        s.attach_files(
            OwnerRef {
                kind: OwnerKind::Source,
                id: first,
            },
            std::slice::from_ref(&path),
        )
        .unwrap()
        .attached[0]
            .id
    });
    let batch = begin(&host);
    let added = ok(&add(&host, &batch, &path, "source", second));
    assert_eq!(added["id"], json!(owners), "{added}");
    assert!(added["alreadyThere"].is_null(), "{added}");
    let set = call(
        &host,
        "set_media_transcription",
        json!({"batch": batch, "media": owners, "text": "Baptisms of 1901"}),
    );
    assert_eq!(set["id"], json!(owners), "{set}");
}

#[test]
fn an_agents_picture_never_becomes_the_portrait() {
    let host = empty_host();
    let ivan = with(&host, |s| add_person(s, draft("Ivan", "Koval", None, None)));
    let batch = begin(&host);
    let path = file(&host, "letter.jpg", &picture(30, 20, ImageFormat::Jpeg));
    ok(&add(&host, &batch, &path, "person", ivan));
    let links = with(&host, |s| s.tree().persons[&ivan].common.media.clone());
    assert_eq!(links.len(), 1);
    assert!(!links[0].primary, "the owner chooses the portrait");
}

#[test]
fn an_owners_piece_with_text_takes_no_place_from_an_agent() {
    let host = empty_host();
    let src = source(&host, "Letters");
    let path = file(&host, "letter.jpg", &picture(40, 30, ImageFormat::Jpeg));
    let (media, ids) = with(&host, |s| {
        let media = s
            .attach_files(
                OwnerRef {
                    kind: OwnerKind::Source,
                    id: src,
                },
                std::slice::from_ref(&path),
            )
            .unwrap()
            .attached[0]
            .id;
        let piece = |text: &str| TextRegionInput {
            id: None,
            area: None,
            rotation: None,
            text: text.into(),
            end_ms: None,
            speaker: None,
            start_ms: None,
        };
        s.apply(EditCommand::SetMediaTranscription {
            id: media,
            regions: vec![piece("Owner's whole transcript"), piece("")],
            known: None,
        })
        .unwrap();
        let ids: Vec<Uuid> = s.tree().media[&media]
            .transcription
            .iter()
            .map(|r| r.id)
            .collect();
        (media, ids)
    });
    let batch = begin(&host);
    let m = call_err(
        &host,
        "update_own_media_region",
        json!({"batch": batch, "media": media, "region": ids[0], "x": 0.1, "y": 0.1, "w": 0.5, "h": 0.2}),
    );
    assert!(m.contains("someone else"), "{m}");
    assert!(with(&host, |s| s.tree().media[&media].transcription[0]
        .area
        .is_none()));
    // An empty piece of the owner's it may fill.
    call(
        &host,
        "update_own_media_region",
        json!({"batch": batch, "media": media, "region": ids[1], "text": "Kyiv", "x": 0.1, "y": 0.5, "w": 0.5, "h": 0.2}),
    );
    assert_eq!(
        with(&host, |s| s.tree().media[&media].transcription[1]
            .text
            .clone()),
        "Kyiv"
    );
}

#[test]
fn rotation_may_be_written_as_a_decimal() {
    let host = empty_host();
    let src = source(&host, "Letters");
    let batch = begin(&host);
    let path = file(&host, "letter.jpg", &picture(40, 30, ImageFormat::Jpeg));
    let media = ok(&add(&host, &batch, &path, "source", src))["id"].clone();
    call(
        &host,
        "set_media_transcription",
        json!({"batch": batch, "media": media, "regions": [{"text": "Kyiv", "x": 0.8, "y": 0.1, "w": 0.1, "h": 0.6, "rotation": 90.0}]}),
    );
    let m = call_err(
        &host,
        "set_media_transcription",
        json!({"batch": batch, "media": media, "regions": [{"text": "Kyiv", "rotation": 90.5}]}),
    );
    assert!(m.contains("rotation"), "{m}");
}
