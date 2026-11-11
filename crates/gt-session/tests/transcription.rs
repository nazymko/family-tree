#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The transcription of a picture: pieces of text with their place on it, written by the owner
//! or by an agent, and an agent attaching a picture from a file.

mod common;

use common::*;
use gt_session::*;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn me() -> AgentCtx {
    AgentCtx {
        id: Uuid::from_u128(7),
        name: "Document reader".into(),
    }
}

fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb([200, 190, 160])));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Jpeg).unwrap();
    out.into_inner()
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

fn person(id: Uuid) -> OwnerRef {
    OwnerRef {
        kind: OwnerKind::Person,
        id,
    }
}

fn region(text: &str, x: f32) -> TextRegionInput {
    TextRegionInput {
        id: None,
        area: Some(TextAreaInput {
            x,
            y: 0.1,
            w: 0.3,
            h: 0.1,
        }),
        rotation: None,
        text: text.into(),
    }
}

fn plain(text: &str) -> TextRegionInput {
    TextRegionInput {
        id: None,
        area: None,
        rotation: None,
        text: text.into(),
    }
}

/// A session with Anna and one picture of a letter attached to her by the owner.
fn with_picture() -> (tempfile::TempDir, Session, Uuid, Uuid) {
    let tree = tree_of(SAMPLE);
    let anna = find(&tree, "Anna");
    let (dir, mut s) = open_tree(&tree);
    let file = write(dir.path(), "letter.jpg", &jpeg(40, 30));
    let r = s.attach_files(person(anna), &[file]).unwrap();
    let media = r.attached[0].id;
    (dir, s, anna, media)
}

fn set(id: Uuid, regions: Vec<TextRegionInput>) -> EditCommand {
    EditCommand::SetMediaTranscription { id, regions }
}

fn texts(s: &Session, media: Uuid) -> Vec<String> {
    s.tree().media[&media]
        .transcription
        .iter()
        .map(|r| r.text.clone())
        .collect()
}

#[test]
fn owner_writes_regions_and_they_survive_reopening() {
    let (dir, mut s, anna, media) = with_picture();
    let mut turned = region("Kyiv, 1901", 0.5);
    turned.rotation = Some(90);
    s.apply(set(
        media,
        vec![region("Dear Olha,", 0.1), turned, plain("Signed below")],
    ))
    .unwrap();
    let m = &s.tree().media[&media];
    assert_eq!(m.transcription.len(), 3);
    assert_eq!(m.transcription[1].rotation, 90);
    assert!(m.transcription[2].area.is_none());
    assert_eq!(
        gt_core::model::transcription_text(&m.transcription),
        "Dear Olha,\n\nKyiv, 1901\n\nSigned below"
    );
    // The app reads it with the picture, wherever the picture is attached.
    let dto = s.get_person(anna).unwrap().media[0].media.clone();
    assert_eq!(dto.transcription.len(), 3);
    assert_eq!(dto.transcription[0].text, "Dear Olha,");
    drop(s);
    let s = Session::open(
        &dir.path().join("project"),
        SessionClock::fixed(today(), NOW_MS),
    )
    .unwrap();
    assert_eq!(
        texts(&s, media),
        ["Dear Olha,", "Kyiv, 1901", "Signed below"]
    );
    assert_eq!(s.tree().media[&media].transcription[1].rotation, 90);
}

#[test]
fn edit_keeps_ids_and_undo_brings_back_the_old_text() {
    let (_dir, mut s, _anna, media) = with_picture();
    s.apply(set(media, vec![region("first", 0.1)])).unwrap();
    let id = s.tree().media[&media].transcription[0].id;
    let mut again = region("first, corrected", 0.2);
    again.id = Some(id);
    let r = s.apply(set(media, vec![again])).unwrap();
    assert_eq!(r.label, "Edit transcription");
    assert_eq!(s.tree().media[&media].transcription[0].id, id);
    assert_eq!(texts(&s, media), ["first, corrected"]);
    s.undo().unwrap();
    assert_eq!(texts(&s, media), ["first"]);
    let history = s.history(media, 10).unwrap();
    assert!(history.iter().any(|h| h.label == "Edit transcription"));
}

#[test]
fn bad_regions_are_refused() {
    let (_dir, mut s, _anna, media) = with_picture();
    let mut turned = region("x", 0.1);
    turned.rotation = Some(45);
    assert!(s.apply(set(media, vec![turned])).is_err());
    let off = region("x", 1.5);
    assert!(s.apply(set(media, vec![off])).is_err());
    let mut flat = region("x", 0.1);
    flat.area.as_mut().unwrap().h = 0.0;
    assert!(s.apply(set(media, vec![flat])).is_err());
    let mut nan = region("x", 0.1);
    nan.area.as_mut().unwrap().w = f32::NAN;
    assert!(s.apply(set(media, vec![nan])).is_err());
    let mut twin = region("a", 0.1);
    twin.id = Some(Uuid::from_u128(1));
    let mut twin2 = region("b", 0.2);
    twin2.id = Some(Uuid::from_u128(1));
    assert!(s.apply(set(media, vec![twin, twin2])).is_err());
    assert!(
        s.apply(set(media, vec![plain(&"x".repeat(70 * 1024))]))
            .is_err()
    );
    // An area running over the edge is pulled inside the picture.
    let mut wide = region("x", 0.8);
    wide.area.as_mut().unwrap().w = 0.5;
    s.apply(set(media, vec![wide])).unwrap();
    let a = s.tree().media[&media].transcription[0]
        .area
        .clone()
        .unwrap();
    assert!((a.x + a.w - 1.0).abs() < 1e-6);
}

// ---- agents ----

#[test]
fn agent_fills_an_owners_picture_and_owns_its_pieces() {
    let (_dir, mut s, _anna, media) = with_picture();
    let b = s.begin_batch(&me(), "Letter 1901", None).unwrap();
    let (w, _, kept) = s
        .set_transcription_as_agent(
            media,
            vec![region("Dear Olha,", 0.1), plain("rest")],
            &me(),
            b,
        )
        .unwrap();
    assert!(matches!(w, AgentWrite::Done(_)));
    assert_eq!(kept, 0);
    assert_eq!(s.own_text_regions(media, me().id).unwrap(), [true, true]);
    // Saying it again adds nothing.
    let (w, _, _) = s
        .set_transcription_as_agent(
            media,
            vec![region("Dear Olha,", 0.1), plain("rest")],
            &me(),
            b,
        )
        .unwrap();
    assert!(
        matches!(w, AgentWrite::AlreadyThere { record } if record == media),
        "{w:?}"
    );
    // Its own pieces it may replace, keeping their ids.
    let ids: Vec<Uuid> = s.tree().media[&media]
        .transcription
        .iter()
        .map(|r| r.id)
        .collect();
    s.set_transcription_as_agent(media, vec![region("Dear Olga,", 0.1)], &me(), b)
        .unwrap();
    assert_eq!(texts(&s, media), ["Dear Olga,"]);
    assert_eq!(s.tree().media[&media].transcription[0].id, ids[0]);
}

#[test]
fn agent_never_overwrites_what_the_owner_wrote_or_changed() {
    let (_dir, mut s, _anna, media) = with_picture();
    s.apply(set(media, vec![region("owner's line", 0.1)]))
        .unwrap();
    let b = s.begin_batch(&me(), "Letter 1901", None).unwrap();
    s.set_transcription_as_agent(media, vec![region("agent line", 0.5)], &me(), b)
        .unwrap();
    assert_eq!(texts(&s, media), ["owner's line", "agent line"]);
    assert_eq!(s.own_text_regions(media, me().id).unwrap(), [false, true]);

    // The owner corrects the agent's line: from then on it is the owner's.
    let mut list: Vec<TextRegionInput> = s.tree().media[&media]
        .transcription
        .iter()
        .map(gt_session::agent::region_input)
        .collect();
    list[1].text = "agent line, corrected by the owner".into();
    s.apply(set(media, list.clone())).unwrap();
    assert_eq!(s.own_text_regions(media, me().id).unwrap(), [false, false]);

    // Writing its transcription again keeps both and adds the new piece after them.
    let (_, _, kept) = s
        .set_transcription_as_agent(media, vec![region("a new reading", 0.6)], &me(), b)
        .unwrap();
    assert_eq!(kept, 2);
    assert_eq!(
        texts(&s, media),
        [
            "owner's line",
            "agent line, corrected by the owner",
            "a new reading"
        ]
    );

    // Changing the owner's piece directly is refused.
    let mut list: Vec<TextRegionInput> = s.tree().media[&media]
        .transcription
        .iter()
        .map(gt_session::agent::region_input)
        .collect();
    list[0].text = "rewritten".into();
    let e = s.apply_as_agent(set(media, list), &me(), b).unwrap_err();
    assert_eq!(e.code, ErrorCode::AgentRefused);
    assert_eq!(texts(&s, media)[0], "owner's line");
}

#[test]
fn agent_writes_in_a_closed_batch_are_refused_and_a_batch_undoes() {
    let (_dir, mut s, _anna, media) = with_picture();
    let b = s.begin_batch(&me(), "Letter 1901", None).unwrap();
    s.set_transcription_as_agent(media, vec![plain("whole text")], &me(), b)
        .unwrap();
    s.end_batch(&me(), b, "Read one letter.").unwrap();
    let e = s
        .set_transcription_as_agent(media, vec![plain("more")], &me(), b)
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::AgentRefused);
    s.undo_batch(b).unwrap();
    assert!(texts(&s, media).is_empty());
}

#[test]
fn agent_attaches_a_picture_once_to_a_source() {
    let tree = tree_of("0 @S1@ SOUR\n1 TITL Letters of 1901\n");
    let (dir, mut s) = open_tree(&tree);
    let source = *s.tree().sources.keys().next().unwrap();
    let file = write(dir.path(), "page1.jpg", &jpeg(30, 40));
    check_agent_file(&file).unwrap();
    let b = s.begin_batch(&me(), "Letters", None).unwrap();
    let target = OwnerRef {
        kind: OwnerKind::Source,
        id: source,
    };
    let about = MediaAbout {
        title: Some("Page 1".into()),
        description: None,
        date_raw: Some("1901".into()),
    };
    let prepared = prepare_attach(&s.project_dir(), target.kind, std::slice::from_ref(&file));
    let (w, _) = s
        .attach_as_agent(target, prepared, &about, &me(), b)
        .unwrap();
    let AgentWrite::Done(_) = w else {
        panic!("{w:?}")
    };
    let src = s.source_dto(source).unwrap();
    assert_eq!(src.media.len(), 1);
    assert_eq!(src.media[0].media.title.as_deref(), Some("Page 1"));
    let media = src.media[0].media.id;
    // The same file again is a repeat.
    let prepared = prepare_attach(&s.project_dir(), target.kind, std::slice::from_ref(&file));
    let (w, _) = s
        .attach_as_agent(target, prepared, &about, &me(), b)
        .unwrap();
    assert!(matches!(w, AgentWrite::AlreadyThere { record } if record == media));
    // The agent made the record, so the whole transcription is its own.
    s.set_transcription_as_agent(media, vec![plain("text")], &me(), b)
        .unwrap();
    assert_eq!(s.own_text_regions(media, me().id).unwrap(), [true]);
}

#[test]
fn agent_files_are_checked_before_anything_is_copied() {
    let dir = tempfile::tempdir().unwrap();
    let refused = |p: &Path| check_agent_file(p).unwrap_err().message;
    assert!(refused(Path::new("relative/page.jpg")).contains("full path"));
    assert!(refused(&dir.path().join("missing.jpg")).contains("no file"));
    assert!(refused(dir.path()).contains("folder"));
    let text = write(dir.path(), "notes.txt", b"just some text, not a picture");
    assert!(refused(&text).contains("Only pictures"));
    let mut broken = jpeg(20, 20);
    broken.truncate(40);
    let broken = write(dir.path(), "broken.jpg", &broken);
    assert!(refused(&broken).contains("not a picture"));
    let sound = write(dir.path(), "voice.wav", b"RIFF\x24\0\0\0WAVEfmt \x10\0\0\0");
    assert!(refused(&sound).contains("owner"));
    let half_pdf = write(dir.path(), "half.pdf", b"%PDF-1.4\n1 0 obj\n");
    assert!(refused(&half_pdf).contains("not a whole PDF"));
    let pdf = write(
        dir.path(),
        "whole.pdf",
        b"%PDF-1.4\n1 0 obj\nendobj\n%%EOF\n",
    );
    check_agent_file(&pdf).unwrap();
    let big = dir.path().join("huge.jpg");
    let f = std::fs::File::create(&big).unwrap();
    f.set_len(AGENT_FILE_MAX + 1).unwrap();
    drop(f);
    assert!(refused(&big).contains("50 MiB"));
    let empty = write(dir.path(), "empty.jpg", b"");
    assert!(refused(&empty).contains("empty"));
}

#[test]
fn agent_attach_refuses_a_closed_batch_and_a_note() {
    let (dir, mut s, anna, _media) = with_picture();
    let file = write(dir.path(), "second.jpg", &jpeg(20, 22));
    let b = s.begin_batch(&me(), "Letters", None).unwrap();
    let note = OwnerRef {
        kind: OwnerKind::Note,
        id: anna,
    };
    let prepared = prepare_attach(
        &s.project_dir(),
        OwnerKind::Person,
        std::slice::from_ref(&file),
    );
    let e = s
        .attach_as_agent(note, prepared, &MediaAbout::default(), &me(), b)
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::AgentRefused);
    s.end_batch(&me(), b, "done").unwrap();
    let prepared = prepare_attach(
        &s.project_dir(),
        OwnerKind::Person,
        std::slice::from_ref(&file),
    );
    let e = s
        .attach_as_agent(person(anna), prepared, &MediaAbout::default(), &me(), b)
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::AgentRefused);
}
