#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! An agent deletes an empty place, moves a place, edits its names and point, one undoable step
//! each. Invented villages only.

use gt_core::date::calendar::{Calendar, first_jdn};
use gt_core::model::{Citation, Common, Evidence, Source};
use gt_core::tree::Tree;
use gt_gedcom::{ImportOptions, import};
use gt_session::*;
use gt_store::{PlaceRecordInput, PlaceRecordPerson, Project};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use std::io::Cursor;
use tempfile::TempDir;
use uuid::Uuid;

const NOW_MS: i64 = 1_793_664_000_000;

const GED: &str = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n\
0 @I1@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n1 BIRT\n2 DATE 1870\n2 PLAC Verbka, North district\n\
0 TRLR\n";

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x5000);

fn common() -> Common {
    Common {
        id: Uuid::from_u128(u128::from(
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        )),
        private: false,
        changed: None,
        citations: vec![],
        notes: vec![],
        media: vec![],
        ext_ids: vec![],
        unknown: vec![],
    }
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

fn jpeg() -> Vec<u8> {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 30, Rgb([200, 190, 160])));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Jpeg).unwrap();
    out.into_inner()
}

fn open_with(fix: impl FnOnce(&mut Tree)) -> (TempDir, Session) {
    let today = first_jdn(Calendar::Gregorian, 2026, Some(11), Some(3)).unwrap();
    let mut out = import(GED.as_bytes(), &ImportOptions::default());
    fix(&mut out.tree);
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("project");
    let mut p = Project::create(&proj).unwrap();
    p.save_tree(&out.tree, today).unwrap();
    drop(p);
    let s = Session::open(&proj, SessionClock::fixed(today, NOW_MS)).unwrap();
    (dir, s)
}

fn new_place(s: &mut Session, name: &str, parent: Option<Uuid>, batch: Uuid) -> Uuid {
    match s
        .add_place_as_agent(name, None, parent, None, None, None, &me(), batch)
        .unwrap()
        .0
    {
        AgentWrite::Done(r) => r.focus.unwrap(),
        AgentWrite::AlreadyThere { .. } => panic!("already there"),
    }
}

fn refused(r: Result<(AgentWrite, Vec<Uuid>), ApiError>) -> String {
    r.expect_err("should be refused").message
}

#[test]
fn an_empty_place_is_deleted_and_undone_but_a_referenced_one_stays() {
    let (dir, mut s) = open_with(|t| {
        // A place that carries a citation.
        let source = Source {
            common: common(),
            title: Some("Book".into()),
            author: None,
            publication: None,
            abbreviation: None,
            text: None,
            repositories: vec![],
            kind: None,
            issuer: None,
            doc_number: None,
            doc_date: None,
        };
        let sid = source.common.id;
        let cit = Citation {
            common: common(),
            source: Some(sid),
            page: None,
            date: None,
            text: None,
            quality: None,
            evidence: Evidence {
                original: None,
                primary_info: None,
                direct: None,
            },
        };
        let cid = cit.common.id;
        t.sources.insert(sid, source);
        t.citations.insert(cid, cit);
        let mut cited = gt_core::model::Place {
            common: common(),
            names: vec![gt_core::model::PlaceName {
                name: "Cited".into(),
                lang: None,
                date: None,
            }],
            parent: None,
            kind: None,
            lat: None,
            lon: None,
            raw: None,
        };
        cited.common.citations.push(cid);
        t.places.insert(cited.common.id, cited);
    });
    let b = s.begin_batch(&me(), "Housekeeping", None).unwrap();
    let verbka = place(&s, "Verbka, North district");
    let empty = new_place(&mut s, "Empty", None, b);
    let under = new_place(&mut s, "Under", Some(empty), b);
    let cited = place(&s, "Cited");

    // Each kind of reference is refused, and the answer names the counts.
    let msg = refused(s.delete_place_as_agent(verbka, &me(), b));
    assert!(msg.contains("1 events"), "{msg}");
    let msg = refused(s.delete_place_as_agent(empty, &me(), b));
    assert!(msg.contains("1 places under it"), "{msg}");
    let msg = refused(s.delete_place_as_agent(cited, &me(), b));
    assert!(msg.contains("1 citations"), "{msg}");
    s.add_place_records(
        under,
        &[PlaceRecordInput {
            kind: "birth".into(),
            book: "BK-1".into(),
            rec_id: "r:1".into(),
            people: vec![PlaceRecordPerson {
                role: "child".into(),
                given: "Anna".into(),
                ..Default::default()
            }],
            ..Default::default()
        }],
        None,
    )
    .unwrap();
    let msg = refused(s.delete_place_as_agent(under, &me(), b));
    assert!(msg.contains("1 place records"), "{msg}");
    // A picture made at the place.
    let ivan = *s.tree().persons.keys().next().unwrap();
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
        place_id: Some(empty),
    })
    .unwrap();
    let msg = refused(s.delete_place_as_agent(empty, &me(), b));
    assert!(msg.contains("1 pictures"), "{msg}");

    // An empty place goes, in one step that the batch undoes.
    let lone = new_place(&mut s, "Lone", None, b);
    let before = s.tree().places.len();
    let (w, _) = s.delete_place_as_agent(lone, &me(), b).unwrap();
    assert!(matches!(w, AgentWrite::Done(_)));
    assert_eq!(s.tree().places.len(), before - 1);
    assert!(!s.tree().places.contains_key(&lone));
    let b2 = s.begin_batch(&me(), "Second", None).unwrap();
    let again = new_place(&mut s, "Again", None, b2);
    s.delete_place_as_agent(again, &me(), b2).unwrap();
    assert!(!s.tree().places.contains_key(&again));
    s.undo_batch(b2).unwrap();
    assert!(
        !s.tree().places.contains_key(&again),
        "made and deleted in the batch"
    );
}

#[test]
fn undo_of_a_delete_brings_the_place_back() {
    let (_dir, mut s) = open_with(|_| {});
    let b0 = s.begin_batch(&me(), "Make", None).unwrap();
    let id = new_place(&mut s, "Lone", None, b0);
    s.end_batch(&me(), b0, "made").unwrap();
    let b = s.begin_batch(&me(), "Delete", None).unwrap();
    s.delete_place_as_agent(id, &me(), b).unwrap();
    assert!(!s.tree().places.contains_key(&id));
    s.undo_batch(b).unwrap();
    assert!(s.tree().places.contains_key(&id));
}

#[test]
fn a_place_moves_under_another_parent_never_into_itself_and_undoes() {
    let (_dir, mut s) = open_with(|_| {});
    let b = s.begin_batch(&me(), "Move", None).unwrap();
    let a = new_place(&mut s, "A", None, b);
    let child = new_place(&mut s, "Child", Some(a), b);
    let grand = new_place(&mut s, "Grand", Some(child), b);
    let other = new_place(&mut s, "Other", None, b);
    s.end_batch(&me(), b, "made").unwrap();

    let b = s.begin_batch(&me(), "Move", None).unwrap();
    // A cycle: itself, a child, a grandchild.
    for np in [a, child, grand] {
        let msg = refused(s.move_place_as_agent(a, Some(np), &me(), b));
        assert!(msg.contains("under itself"), "{msg}");
    }
    assert_eq!(s.tree().places[&a].parent, None);
    s.move_place_as_agent(child, Some(other), &me(), b).unwrap();
    assert_eq!(s.tree().places[&child].parent, Some(other));
    assert_eq!(s.place_option(grand).unwrap().full, "Grand, Child, Other");
    let (w, _) = s.move_place_as_agent(child, Some(other), &me(), b).unwrap();
    assert!(matches!(w, AgentWrite::AlreadyThere { .. }));
    s.move_place_as_agent(child, None, &me(), b).unwrap();
    assert_eq!(s.tree().places[&child].parent, None);
    s.move_place_as_agent(child, Some(a), &me(), b).unwrap();
    s.undo_batch(b).unwrap();
    assert_eq!(s.tree().places[&child].parent, Some(a));
    let missing = Uuid::from_u128(99);
    let b = s.begin_batch(&me(), "Move", None).unwrap();
    assert!(
        s.move_place_as_agent(child, Some(missing), &me(), b)
            .is_err()
    );
}

#[test]
fn update_place_edits_names_type_and_coordinates() {
    let (_dir, mut s) = open_with(|_| {});
    let b = s.begin_batch(&me(), "Edit", None).unwrap();
    let id = new_place(&mut s, "Church", None, b);
    s.end_batch(&me(), b, "made").unwrap();
    let names = |s: &Session| -> Vec<String> {
        s.tree().places[&id]
            .names
            .iter()
            .map(|n| n.name.clone())
            .collect()
    };
    let b = s.begin_batch(&me(), "Edit", None).unwrap();

    // Adds alternative names (once each), and a type.
    let edit = PlaceEdit {
        add_alt_names: vec!["Hopchytsia".into(), "Gopchitsa".into(), "hopchytsia".into()],
        kind: Some("church".into()),
        ..Default::default()
    };
    s.update_place_as_agent(id, &edit, &me(), b).unwrap();
    assert_eq!(names(&s), ["Church", "Hopchytsia", "Gopchitsa"]);
    assert_eq!(s.tree().places[&id].kind.as_deref(), Some("church"));

    // Removes an alternative name; the primary name stays.
    let edit = PlaceEdit {
        remove_alt_names: vec!["HOPCHYTSIA".into()],
        ..Default::default()
    };
    s.update_place_as_agent(id, &edit, &me(), b).unwrap();
    assert_eq!(names(&s), ["Church", "Gopchitsa"]);
    let edit = PlaceEdit {
        remove_alt_names: vec!["Church".into()],
        ..Default::default()
    };
    let msg = refused(s.update_place_as_agent(id, &edit, &me(), b));
    assert!(msg.contains("primary name"), "{msg}");
    assert_eq!(names(&s), ["Church", "Gopchitsa"]);
    let edit = PlaceEdit {
        remove_alt_names: vec!["Nowhere".into()],
        ..Default::default()
    };
    refused(s.update_place_as_agent(id, &edit, &me(), b));

    // A rename of the primary name.
    let edit = PlaceEdit {
        name: Some("Trinity church".into()),
        ..Default::default()
    };
    s.update_place_as_agent(id, &edit, &me(), b).unwrap();
    assert_eq!(names(&s), ["Trinity church", "Gopchitsa"]);

    // Coordinates need a source.
    let edit = PlaceEdit {
        point: Some((46.5, 32.25)),
        ..Default::default()
    };
    let msg = refused(s.update_place_as_agent(id, &edit, &me(), b));
    assert!(msg.contains("source"), "{msg}");
    assert_eq!(s.tree().places[&id].lat, None);
    let edit = PlaceEdit {
        point: Some((46.5, 32.25)),
        source: Some("GeoNames 1".into()),
        ..Default::default()
    };
    s.update_place_as_agent(id, &edit, &me(), b).unwrap();
    assert_eq!(s.tree().places[&id].lat, Some(46.5));
    assert!(
        s.tree().places[&id]
            .common
            .notes
            .iter()
            .any(|n| s.tree().notes[n].text.contains("GeoNames 1"))
    );
    // The same again changes nothing.
    let (w, _) = s.update_place_as_agent(id, &edit, &me(), b).unwrap();
    assert!(matches!(w, AgentWrite::AlreadyThere { .. }));

    // One undo of the batch takes all of it back.
    s.undo_batch(b).unwrap();
    assert_eq!(names(&s), ["Church"]);
    assert_eq!(s.tree().places[&id].lat, None);
    assert_eq!(s.tree().places[&id].kind, None);
}
