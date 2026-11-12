#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The place of a media record: set from its editor by text or by an existing place, cleared,
//! undone, kept when the tree is opened again, and shown in the details the app reads.

mod common;

use common::*;
use gt_session::*;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use std::io::Cursor;
use uuid::Uuid;

fn jpeg() -> Vec<u8> {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 30, Rgb([200, 190, 160])));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Jpeg).unwrap();
    out.into_inner()
}

fn set_place(id: Uuid, text: Option<&str>, place_id: Option<Uuid>) -> EditCommand {
    EditCommand::UpdateMedia {
        id,
        title: None,
        description: None,
        date_raw: None,
        place_text: text.map(str::to_string),
        place_id,
    }
}

fn with_picture() -> (tempfile::TempDir, Session, Uuid, Uuid) {
    let tree = tree_of(SAMPLE);
    let anna = find(&tree, "Anna");
    let (dir, mut s) = open_tree(&tree);
    let file = dir.path().join("letter.jpg");
    std::fs::write(&file, jpeg()).unwrap();
    let r = s
        .attach_files(
            OwnerRef {
                kind: OwnerKind::Person,
                id: anna,
            },
            &[file],
        )
        .unwrap();
    let media = r.attached[0].id;
    (dir, s, anna, media)
}

fn place_of(s: &Session, media: Uuid) -> Option<String> {
    s.tree().media[&media]
        .place
        .and_then(|p| s.place_option(p))
        .map(|p| p.full)
}

#[test]
fn a_place_is_set_by_text_shown_cleared_and_undone() {
    let (dir, mut s, anna, media) = with_picture();
    assert_eq!(place_of(&s, media), None);
    s.apply(set_place(media, Some("Lviv, Ukraine"), None))
        .unwrap();
    assert_eq!(place_of(&s, media).as_deref(), Some("Lviv, Ukraine"));
    // The app reads it from the details of the picture.
    let shown = s.get_person(anna).unwrap().media[0].media.place.clone();
    assert_eq!(shown.unwrap().full, "Lviv, Ukraine");

    // Other fields leave the place alone.
    s.apply(EditCommand::UpdateMedia {
        id: media,
        title: Some("Letter".into()),
        description: None,
        date_raw: None,
        place_text: None,
        place_id: None,
    })
    .unwrap();
    assert_eq!(place_of(&s, media).as_deref(), Some("Lviv, Ukraine"));

    // Kept when the tree is opened again.
    drop(s);
    let mut s = Session::open(
        &dir.path().join("project"),
        SessionClock::fixed(today(), NOW_MS),
    )
    .unwrap();
    assert_eq!(place_of(&s, media).as_deref(), Some("Lviv, Ukraine"));

    // An existing place by id, then cleared with an empty text.
    let kyiv = s
        .apply(set_place(media, Some("Kyiv, Ukraine"), None))
        .map(|_| s.tree().media[&media].place.unwrap())
        .unwrap();
    s.apply(set_place(media, None, Some(kyiv))).unwrap();
    assert_eq!(place_of(&s, media).as_deref(), Some("Kyiv, Ukraine"));
    s.apply(set_place(media, Some(""), None)).unwrap();
    assert_eq!(place_of(&s, media), None);
    s.undo().unwrap().unwrap();
    assert_eq!(place_of(&s, media).as_deref(), Some("Kyiv, Ukraine"));
}

#[test]
fn a_place_that_is_not_in_the_tree_is_refused() {
    let (_dir, mut s, _anna, media) = with_picture();
    let err = s
        .apply(set_place(media, None, Some(Uuid::from_u128(404))))
        .unwrap_err();
    assert!(!err.message.is_empty());
    assert_eq!(place_of(&s, media), None);
}
