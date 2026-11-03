#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use gt_core::date::calendar::{Calendar, first_jdn};
use gt_import_atdb::fixture::build_family_fixture;
use gt_session::import_format::{ImportFormat, detect};
use gt_session::*;
use std::path::{Path, PathBuf};

const NOW_MS: i64 = 1_793_664_000_000; // 2026-11-03

fn clock() -> SessionClock {
    let today = first_jdn(Calendar::Gregorian, 2026, Some(11), Some(3)).unwrap();
    SessionClock::fixed(today, NOW_MS)
}

fn request(file: &Path, project: &Path, roots: &[&Path]) -> ImportRequest {
    ImportRequest {
        file: file.to_string_lossy().into_owned(),
        project_dir: project.to_string_lossy().into_owned(),
        project_name: "Test".into(),
        media_roots: roots
            .iter()
            .map(|r| r.to_string_lossy().into_owned())
            .collect(),
        day_order: DayOrder::Dmy,
    }
}

fn fixture(dir: &Path) -> PathBuf {
    let p = dir.join("tree.atdb");
    drop(build_family_fixture(&p));
    p
}

#[test]
fn detects_the_atdb_fixture_and_a_ged_file() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(detect(&fixture(tmp.path())).unwrap(), ImportFormat::Atdb);
    let ged = tmp.path().join("a.ged");
    std::fs::write(&ged, "0 HEAD\n0 TRLR\n").unwrap();
    assert_eq!(detect(&ged).unwrap(), ImportFormat::Gedcom);
    assert_eq!(
        detect(&tmp.path().join("missing.atdb")).unwrap_err().code,
        ErrorCode::Io
    );
}

#[test]
fn imports_an_atdb_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let f = fixture(tmp.path());
    let before = std::fs::read(&f).unwrap();
    std::fs::write(tmp.path().join("photo.jpg"), b"not really a jpeg").unwrap();
    // Attach a photo to the database through a second, writable connection.
    let conn = rusqlite::Connection::open(&f).unwrap();
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;
         INSERT INTO Documents (id, path) VALUES (1, 'photo.jpg');
         INSERT INTO DocumentDetails (d_id, rec_table, rec_id, ismain) VALUES (1, 13, 1, 1);",
    )
    .unwrap();
    drop(conn);
    let before_import = std::fs::read(&f).unwrap();
    assert_ne!(before, before_import);

    let proj = tmp.path().join("proj");
    let phases = std::cell::RefCell::new(Vec::new());
    let (s, r) = import_gedcom(&request(&f, &proj, &[]), clock(), &|p| {
        phases.borrow_mut().push(p.phase);
    })
    .unwrap();
    assert_eq!(r.counts["persons"], 5);
    assert_eq!(r.counts["families"], 2);
    assert_eq!(r.project.person_count, 5);
    assert!(r.producer.as_deref().unwrap().contains("Древо Жизни"));
    assert_eq!(r.version, None);
    assert_eq!(r.unmapped_tags["event type 7 (kept as a custom event)"], 1);
    assert!(r.text.contains("person field 69"));
    assert!(phases.borrow().contains(&ImportPhase::Saving));
    assert!(proj.join("tree.db").is_file());
    assert_eq!(s.tree().persons.len(), 5);
    assert_eq!(s.tree().families.len(), 2);
    // The photo next to the database was copied into the project.
    let m = s.tree().media.values().next().unwrap();
    assert!(proj.join(&m.path).is_file());
    // The source file is untouched by the import.
    assert_eq!(std::fs::read(&f).unwrap(), before_import);
    assert!(!tmp.path().join("tree.atdb-journal").exists());
}

#[test]
fn another_sqlite_file_is_refused_and_leaves_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let f = tmp.path().join("other.db");
    let conn = rusqlite::Connection::open(&f).unwrap();
    conn.execute_batch("CREATE TABLE t (x)").unwrap();
    drop(conn);
    let proj = tmp.path().join("proj");
    let e = import_gedcom(&request(&f, &proj, &[]), clock(), &|_| {})
        .err()
        .unwrap();
    assert_eq!(e.code, ErrorCode::Invalid);
    assert!(e.message.contains("Древо Жизни"), "{}", e.message);
    assert!(!proj.exists());
}

#[test]
fn gramps_and_gedzip_files_say_not_supported_yet() {
    let tmp = tempfile::tempdir().unwrap();
    for (name, bytes) in [
        ("a.gramps", &[0x1f_u8, 0x8b, 8, 0][..]),
        ("a.gdz", &b"PK\x03\x04"[..]),
    ] {
        let f = tmp.path().join(name);
        std::fs::write(&f, bytes).unwrap();
        let proj = tmp.path().join("proj");
        let e = import_gedcom(&request(&f, &proj, &[]), clock(), &|_| {})
            .err()
            .unwrap();
        assert_eq!(e.code, ErrorCode::Invalid);
        assert!(
            e.message.contains("cannot be imported yet"),
            "{}",
            e.message
        );
        assert!(!proj.exists());
    }
}
