#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! An agent makes a place (never twice) and merges duplicate places into one, in one undoable step:
//! events, media, place records and child places follow, and an undo of the batch puts it all back.
//! Invented villages only.

use gt_core::date::calendar::{Calendar, first_jdn};
use gt_gedcom::{ImportOptions, import};
use gt_session::*;
use gt_store::{PlaceRecordInput, PlaceRecordPerson, Project};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use std::io::Cursor;
use tempfile::TempDir;
use uuid::Uuid;

const NOW_MS: i64 = 1_793_664_000_000;

const GED: &str = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n\
0 @I1@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n1 BIRT\n2 DATE 1870\n2 PLAC Hopchytsia, North district\n\
0 @I2@ INDI\n1 NAME Maria /Koval/\n1 SEX F\n1 BIRT\n2 DATE 1875\n2 PLAC Hopchytsia, South district\n\
0 @I3@ INDI\n1 NAME Petro /Koval/\n1 SEX M\n1 BIRT\n2 DATE 1900\n2 PLAC Hopchytsia\n\
0 @I4@ INDI\n1 NAME Olena /Koval/\n1 SEX F\n1 BIRT\n2 DATE 1901\n2 PLAC Mill, Hopchytsia, South district\n\
0 TRLR\n";

fn open_ged(ged: &str) -> (TempDir, Session) {
    let today = first_jdn(Calendar::Gregorian, 2026, Some(11), Some(3)).unwrap();
    let out = import(ged.as_bytes(), &ImportOptions::default());
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("project");
    let mut p = Project::create(&proj).unwrap();
    p.save_tree(&out.tree, today).unwrap();
    drop(p);
    let s = Session::open(&proj, SessionClock::fixed(today, NOW_MS)).unwrap();
    (dir, s)
}

fn me() -> AgentCtx {
    AgentCtx {
        id: Uuid::from_u128(7),
        name: "Place keeper".into(),
    }
}

fn place(s: &Session, full: &str) -> Uuid {
    *s.tree()
        .places
        .keys()
        .find(|id| s.place_option(**id).is_some_and(|p| p.full == full))
        .unwrap_or_else(|| panic!("no place {full}"))
}

fn person_place(s: &Session, given: &str) -> Option<Uuid> {
    let p = s
        .tree()
        .persons
        .values()
        .find(|p| p.names[0].given.as_deref() == Some(given))
        .unwrap();
    s.tree()
        .events
        .values()
        .find(|e| {
            e.participants.iter().any(|x| {
                matches!(x.target, gt_core::model::ParticipantRef::Person(id) if id == p.common.id)
            })
        })
        .and_then(|e| e.place)
}

fn record(book: &str, rec_id: &str) -> PlaceRecordInput {
    PlaceRecordInput {
        kind: "birth".into(),
        date: "1850".into(),
        book: book.into(),
        page: "1".into(),
        rec_id: rec_id.into(),
        people: vec![PlaceRecordPerson {
            role: "child".into(),
            given: "Danylo".into(),
            surname: "Bondar".into(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn jpeg() -> Vec<u8> {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 30, Rgb([200, 190, 160])));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Jpeg).unwrap();
    out.into_inner()
}

struct World {
    _dir: TempDir,
    s: Session,
    north: Uuid,
    south: Uuid,
    plain: Uuid,
    mill: Uuid,
    media: Uuid,
    batch: Uuid,
}

fn world() -> World {
    let (dir, mut s) = open_ged(GED);
    let north = place(&s, "Hopchytsia, North district");
    let south = place(&s, "Hopchytsia, South district");
    let plain = place(&s, "Hopchytsia");
    let mill = place(&s, "Mill, Hopchytsia, South district");
    // A picture taken in the south village.
    let ivan = *s
        .tree()
        .persons
        .iter()
        .find(|(_, p)| p.names[0].given.as_deref() == Some("Ivan"))
        .unwrap()
        .0;
    let file = dir.path().join("photo.jpg");
    std::fs::write(&file, jpeg()).unwrap();
    let media = s
        .attach_files(
            OwnerRef {
                kind: OwnerKind::Person,
                id: ivan,
            },
            &[file],
        )
        .unwrap()
        .attached[0]
        .id;
    s.apply(EditCommand::UpdateMedia {
        id: media,
        title: None,
        description: None,
        date_raw: None,
        place_text: None,
        place_id: Some(south),
    })
    .unwrap();
    // Records about other people: two in the south village, one in the plain one.
    s.add_place_records(south, &[record("BK-1", "r:1"), record("BK-1", "r:2")], None)
        .unwrap();
    s.add_place_records(plain, &[record("BK-1", "r:3")], None)
        .unwrap();
    let batch = s.begin_batch(&me(), "Housekeeping", None).unwrap();
    World {
        _dir: dir,
        s,
        north,
        south,
        plain,
        mill,
        media,
        batch,
    }
}

#[test]
fn a_merge_repoints_everything_and_one_undo_puts_it_back() {
    let mut w = world();
    let (north, south, plain) = (w.north, w.south, w.plain);
    assert_eq!(person_place(&w.s, "Maria"), Some(south));
    assert_eq!(person_place(&w.s, "Petro"), Some(plain));
    let log_before = w.s.place_record_total(south).unwrap();
    assert_eq!(log_before, 2, "the mill has no records");

    let (out, reached) =
        w.s.merge_places_as_agent(north, &[south, plain], &me(), w.batch)
            .unwrap();
    assert_eq!(out.removed, 2);
    assert_eq!(out.counts.events, 2, "Maria and Petro were born there");
    assert_eq!(out.counts.media, 1);
    assert_eq!(
        out.counts.child_places, 1,
        "the mill moves under the kept place"
    );
    assert_eq!(out.place_records, 3);
    assert!(!reached.is_empty());

    // Every reference now names the kept place; the merged places are gone.
    assert_eq!(person_place(&w.s, "Ivan"), Some(north));
    assert_eq!(person_place(&w.s, "Maria"), Some(north));
    assert_eq!(person_place(&w.s, "Petro"), Some(north));
    assert_eq!(w.s.tree().media[&w.media].place, Some(north));
    assert_eq!(w.s.tree().places[&w.mill].parent, Some(north));
    assert!(!w.s.tree().places.contains_key(&south));
    assert!(!w.s.tree().places.contains_key(&plain));
    assert_eq!(
        w.s.place_option(w.mill).unwrap().full,
        "Mill, Hopchytsia, North district"
    );
    // The kept place says what was merged.
    let kept = &w.s.tree().places[&north];
    assert!(
        kept.common
            .notes
            .iter()
            .any(|n| w.s.tree().notes[n].text.contains("Merged places"))
    );
    // The place records follow, and the dedupe on (book, rec_id) still holds.
    assert_eq!(w.s.place_record_total(north).unwrap(), 3);
    let again =
        w.s.add_place_records(north, &[record("BK-1", "r:1"), record("BK-1", "r:9")], None)
            .unwrap();
    assert_eq!((again.added, again.duplicates), (1, 1));
    assert_eq!(w.s.place_record_total(north).unwrap(), 4);

    // One undo step puts all of it back, the place records too.
    let report = w.s.undo_batch(w.batch).unwrap();
    assert!(report.skipped.is_empty(), "{report:?}");
    assert_eq!(person_place(&w.s, "Maria"), Some(south));
    assert_eq!(person_place(&w.s, "Petro"), Some(plain));
    assert_eq!(person_place(&w.s, "Ivan"), Some(north));
    assert_eq!(w.s.tree().media[&w.media].place, Some(south));
    assert_eq!(w.s.tree().places[&w.mill].parent, Some(south));
    assert!(w.s.tree().places.contains_key(&south));
    assert!(w.s.tree().places.contains_key(&plain));
    assert_eq!(w.s.place_record_total(south).unwrap(), 2);
    assert_eq!(w.s.place_record_total(plain).unwrap(), 1);
    // North had none before; the one record added after the merge stays there.
    assert_eq!(w.s.place_record_total(north).unwrap(), 1);
}

#[test]
fn a_merge_is_refused_when_it_cannot_make_sense() {
    let mut w = world();
    let b = w.batch;
    let ctx = me();
    let district = place(&w.s, "South district");
    // Keep among the merged.
    assert!(
        w.s.merge_places_as_agent(w.north, &[w.north, w.south], &ctx, b)
            .is_err()
    );
    // An ancestor of the kept place.
    assert!(
        w.s.merge_places_as_agent(w.south, &[district], &ctx, b)
            .is_err()
    );
    // Unknown places, nothing to merge.
    assert!(
        w.s.merge_places_as_agent(w.north, &[Uuid::from_u128(5)], &ctx, b)
            .is_err()
    );
    assert!(
        w.s.merge_places_as_agent(Uuid::from_u128(5), &[w.south], &ctx, b)
            .is_err()
    );
    assert!(w.s.merge_places_as_agent(w.north, &[], &ctx, b).is_err());
    // Not a batch of this connection.
    assert!(
        w.s.merge_places_as_agent(w.north, &[w.south], &ctx, Uuid::from_u128(9))
            .is_err()
    );
    // Nothing changed.
    assert!(w.s.tree().places.contains_key(&w.south));
    assert_eq!(person_place(&w.s, "Maria"), Some(w.south));
}

#[test]
fn a_place_is_made_once_and_the_batch_undoes_it() {
    let mut w = world();
    let ctx = me();
    let district = place(&w.s, "South district");
    let (made, _) =
        w.s.add_place_as_agent(
            "Verbka",
            Some("village"),
            Some(district),
            Some((46.5, 32.25)),
            Some("GeoNames 1"),
            Some("Named in the books."),
            &ctx,
            w.batch,
        )
        .unwrap();
    let AgentWrite::Done(result) = made else {
        panic!("made")
    };
    let id = result.focus.unwrap();
    let p = &w.s.tree().places[&id];
    assert_eq!(p.names[0].name, "Verbka");
    assert_eq!(p.kind.as_deref(), Some("village"));
    assert_eq!(p.parent, Some(district));
    assert_eq!((p.lat, p.lon), (Some(46.5), Some(32.25)));
    assert_eq!(w.s.place_option(id).unwrap().full, "Verbka, South district");

    // The same name under the same parent, in any case: the existing one.
    let (again, _) =
        w.s.add_place_as_agent(
            "  verbka ",
            None,
            Some(district),
            None,
            None,
            None,
            &ctx,
            w.batch,
        )
        .unwrap();
    assert!(matches!(again, AgentWrite::AlreadyThere { record } if record == id));
    // Another parent is another place.
    let (other, _) =
        w.s.add_place_as_agent("Verbka", None, None, None, None, None, &ctx, w.batch)
            .unwrap();
    assert!(matches!(other, AgentWrite::Done(_)));
    // Coordinates need a source; a missing parent is refused.
    assert!(
        w.s.add_place_as_agent(
            "Boblyv",
            None,
            None,
            Some((1.0, 1.0)),
            None,
            None,
            &ctx,
            w.batch
        )
        .is_err()
    );
    assert!(
        w.s.add_place_as_agent(
            "Boblyv",
            None,
            Some(Uuid::from_u128(3)),
            None,
            None,
            None,
            &ctx,
            w.batch
        )
        .is_err()
    );
    assert!(
        w.s.add_place_as_agent("", None, None, None, None, None, &ctx, w.batch)
            .is_err()
    );

    let places = w.s.tree().places.len();
    w.s.undo_batch(w.batch).unwrap();
    assert_eq!(w.s.tree().places.len(), places - 2);
    assert!(!w.s.tree().places.contains_key(&id));
}

#[test]
fn the_command_path_still_refuses_an_agent_to_make_a_place() {
    let mut w = world();
    let err =
        w.s.apply_as_agent(
            EditCommand::CreatePlace {
                text: "Zarichne".into(),
            },
            &me(),
            w.batch,
        )
        .err();
    assert!(err.is_some());
}
