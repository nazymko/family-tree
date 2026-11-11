#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! A picture's saved view rotation: set by the owner from the viewer, or by an agent on its own
//! picture or on one nobody has turned yet.

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

fn jpeg(w: u32, h: u32, shade: u8) -> Vec<u8> {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb([shade, 190, 160])));
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

fn turn(id: Uuid, rotation: u16) -> EditCommand {
    EditCommand::SetMediaRotation { id, rotation }
}

/// A session with Anna and one sideways letter the owner attached to her.
fn with_picture() -> (tempfile::TempDir, Session, Uuid, Uuid) {
    let tree = tree_of(SAMPLE);
    let anna = find(&tree, "Anna");
    let (dir, mut s) = open_tree(&tree);
    let file = write(dir.path(), "letter.jpg", &jpeg(40, 30, 200));
    let r = s.attach_files(person(anna), &[file]).unwrap();
    let media = r.attached[0].id;
    (dir, s, anna, media)
}

fn rotation(s: &Session, media: Uuid) -> Option<u16> {
    s.tree().media[&media].rotation
}

#[test]
fn owner_turns_a_picture_it_is_kept_shown_and_undone() {
    let (dir, mut s, anna, media) = with_picture();
    assert_eq!(s.get_person(anna).unwrap().media[0].media.rotation, 0);
    let r = s.apply(turn(media, 90)).unwrap();
    assert_eq!(r.label, "Turn picture");
    assert_eq!(s.undo_state().undo_label.as_deref(), Some("Turn picture"));
    s.apply(turn(media, 180)).unwrap();
    assert_eq!(rotation(&s, media), Some(180));
    // Wherever the picture is shown, it is turned the same way.
    assert_eq!(s.get_person(anna).unwrap().media[0].media.rotation, 180);
    s.undo().unwrap().unwrap();
    assert_eq!(rotation(&s, media), Some(90));
    drop(s);
    let s = Session::open(
        &dir.path().join("project"),
        SessionClock::fixed(today(), NOW_MS),
    )
    .unwrap();
    assert_eq!(rotation(&s, media), Some(90));
}

#[test]
fn turning_keeps_the_transcription_exactly_where_it_was() {
    let (_dir, mut s, _anna, media) = with_picture();
    s.apply(EditCommand::SetMediaTranscription {
        id: media,
        regions: vec![TextRegionInput {
            id: None,
            area: Some(TextAreaInput {
                x: 0.125,
                y: 0.25,
                w: 0.5,
                h: 0.0625,
            }),
            rotation: Some(90),
            text: "Kyiv, 1901".into(),
        }],
        known: None,
    })
    .unwrap();
    let before = s.tree().media[&media].transcription.clone();
    for r in [90, 180, 270, 0] {
        s.apply(turn(media, r)).unwrap();
        assert_eq!(s.tree().media[&media].transcription, before);
    }
}

#[test]
fn only_a_quarter_turn_of_a_picture_is_taken() {
    let (dir, mut s, anna, media) = with_picture();
    for bad in [45, 360, 1] {
        let e = s.apply(turn(media, bad)).unwrap_err();
        assert_eq!(e.code, ErrorCode::Invalid, "{bad}");
    }
    let pdf = write(
        dir.path(),
        "page.pdf",
        b"%PDF-1.4\n1 0 obj<<>>endobj\n%%EOF\n",
    );
    let r = s.attach_files(person(anna), &[pdf]).unwrap();
    let e = s.apply(turn(r.attached[0].id, 90)).unwrap_err();
    assert_eq!(e.code, ErrorCode::Invalid);
    assert!(e.message.contains("picture"), "{}", e.message);
    assert_eq!(rotation(&s, media), None);
}

#[test]
fn a_picture_on_a_source_and_a_dozen_people_is_one_picture_and_unlinks_from_one_only() {
    let (dir, mut s, anna, _) = with_picture();
    let src = s
        .apply(EditCommand::SaveSource {
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
        .touched[0];
    let file = write(dir.path(), "register.jpg", &jpeg(30, 40, 60));
    let page = s
        .attach_files(
            OwnerRef {
                kind: OwnerKind::Source,
                id: src,
            },
            &[file],
        )
        .unwrap()
        .attached[0]
        .id;
    s.apply(EditCommand::SetMediaTranscription {
        id: page,
        regions: vec![TextRegionInput {
            id: None,
            area: None,
            rotation: None,
            text: "Twelve names".into(),
        }],
        known: None,
    })
    .unwrap();
    s.apply(turn(page, 270)).unwrap();
    let mut people = vec![anna];
    for i in 0..11 {
        let r = s
            .apply(EditCommand::CreatePerson {
                draft: draft(&format!("Person{i}"), SexKind::Unknown),
            })
            .unwrap();
        people.push(r.focus.unwrap());
    }
    for p in &people {
        s.apply(EditCommand::AttachMedia {
            target: person(*p),
            media: vec![page],
        })
        .unwrap();
    }
    // Each person shows the one picture, with its transcription and its turn.
    for p in &people {
        let d = s.get_person(*p).unwrap();
        let link = d.media.iter().find(|l| l.media.id == page).unwrap();
        assert_eq!(link.media.transcription[0].text, "Twelve names");
        assert_eq!(link.media.rotation, 270);
    }
    // Taking it off one person leaves it on everyone else and on the source.
    s.apply(EditCommand::DetachMedia {
        target: person(people[3]),
        media: page,
    })
    .unwrap();
    assert!(
        s.get_person(people[3])
            .unwrap()
            .media
            .iter()
            .all(|l| l.media.id != page)
    );
    for (i, p) in people.iter().enumerate().filter(|(i, _)| *i != 3) {
        assert!(
            s.get_person(*p)
                .unwrap()
                .media
                .iter()
                .any(|l| l.media.id == page),
            "person {i}"
        );
    }
    assert!(
        s.tree().sources[&src]
            .common
            .media
            .iter()
            .any(|l| l.media == page)
    );
    assert_eq!(s.tree().media[&page].transcription.len(), 1);
}

// ---- agents ----

fn agents_picture(dir: &Path, s: &mut Session, anna: Uuid, b: Uuid) -> Uuid {
    let file = write(dir, "register.jpg", &jpeg(30, 40, 90));
    let f = prepare_agent_file(&s.project_dir(), &file).unwrap();
    let (_, _, id) = s
        .attach_as_agent(person(anna), f, &MediaAbout::default(), &me(), b)
        .unwrap();
    id
}

#[test]
fn agent_turns_its_own_picture_as_often_as_it_likes() {
    let (dir, mut s, anna, _) = with_picture();
    let b = s.begin_batch(&me(), "Register 1901", None).unwrap();
    let own = agents_picture(dir.path(), &mut s, anna, b);
    let (w, _) = s.set_media_rotation_as_agent(own, 90, &me(), b).unwrap();
    assert!(matches!(w, AgentWrite::Done(_)), "{w:?}");
    let (w, _) = s.set_media_rotation_as_agent(own, 270, &me(), b).unwrap();
    assert!(matches!(w, AgentWrite::Done(_)), "{w:?}");
    assert_eq!(rotation(&s, own), Some(270));
    let (w, _) = s.set_media_rotation_as_agent(own, 270, &me(), b).unwrap();
    assert!(
        matches!(w, AgentWrite::AlreadyThere { record } if record == own),
        "{w:?}"
    );
    // The batch undoes it with the rest.
    s.undo_batch(b).unwrap();
    assert!(!s.tree().media.contains_key(&own));
}

#[test]
fn agent_turns_someone_elses_picture_only_while_nobody_has() {
    let (_dir, mut s, _anna, media) = with_picture();
    let b = s.begin_batch(&me(), "Letter 1901", None).unwrap();
    // Upright already: nothing to do, and nothing written.
    let (w, _) = s.set_media_rotation_as_agent(media, 0, &me(), b).unwrap();
    assert!(matches!(w, AgentWrite::AlreadyThere { .. }), "{w:?}");
    assert_eq!(rotation(&s, media), None);
    let (w, _) = s.set_media_rotation_as_agent(media, 90, &me(), b).unwrap();
    assert!(matches!(w, AgentWrite::Done(_)), "{w:?}");
    assert_eq!(rotation(&s, media), Some(90));
    // Once set, it is not the agent's to change again: the picture is the owner's.
    let e = s
        .set_media_rotation_as_agent(media, 180, &me(), b)
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Conflict, "{}", e.message);
    assert!(e.message.contains("turned"), "{}", e.message);
    // The owner's own turn, even back to upright, is the owner's.
    s.undo_batch(b).unwrap();
    assert_eq!(rotation(&s, media), None);
    s.apply(turn(media, 0)).unwrap();
    let b2 = s.begin_batch(&me(), "Letter 1901 again", None).unwrap();
    let e = s
        .set_media_rotation_as_agent(media, 90, &me(), b2)
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Conflict);
    assert_eq!(rotation(&s, media), Some(0));
}

#[test]
fn agent_rotation_checks_the_batch_first() {
    let (_dir, mut s, _anna, media) = with_picture();
    let e = s
        .set_media_rotation_as_agent(media, 90, &me(), Uuid::from_u128(99))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::AgentRefused, "{}", e.message);
    let b = s.begin_batch(&me(), "Letter", None).unwrap();
    let e = s
        .set_media_rotation_as_agent(Uuid::from_u128(5), 90, &me(), b)
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
    let e = s
        .set_media_rotation_as_agent(media, 45, &me(), b)
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Invalid);
    assert_eq!(rotation(&s, media), None);
}
