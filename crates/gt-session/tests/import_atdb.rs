#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use gt_core::date::calendar::{Calendar, first_jdn};
use gt_gedcom::ImportOptions;
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

mod scratch_cleanup {
    use super::*;
    use gt_session::import_format::{Converted, TempDirGuard};
    use std::cell::RefCell;

    fn ged_file(dir: &Path) -> PathBuf {
        let f = dir.join("a.ged");
        std::fs::write(&f, "0 HEAD\n0 @I1@ INDI\n1 NAME A /B/\n0 TRLR\n").unwrap();
        f
    }

    /// A converter that makes a scratch folder; it fails after that when `fail` is set.
    fn run(fail: bool, fail_open: bool) -> (Result<(), ErrorCode>, PathBuf, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let f = ged_file(tmp.path());
        let proj = tmp.path().join("proj");
        let made: RefCell<PathBuf> = RefCell::new(PathBuf::new());
        let convert = |_: ImportFormat, p: &Path, o: &ImportOptions| {
            let guard = TempDirGuard::create("gt-seam-test")?;
            std::fs::write(guard.path().join("x.jpg"), b"x").unwrap();
            *made.borrow_mut() = guard.path().to_path_buf();
            if fail {
                return Err(ApiError::new(ErrorCode::Invalid, "later step failed"));
            }
            let bytes = std::fs::read(p).unwrap();
            Ok(Converted {
                outcome: gt_gedcom::import(&bytes, o),
                extra_media_roots: vec![guard.path().to_path_buf()],
                cleanup: Some(guard),
            })
        };
        let open = |d: &Path, c: SessionClock| {
            if fail_open {
                Err(ApiError::new(ErrorCode::Io, "cannot open"))
            } else {
                Session::open(d, c)
            }
        };
        let r =
            import_gedcom_with_hooks(&request(&f, &proj, &[]), clock(), &|_| {}, &open, &convert)
                .map(|_| ())
                .map_err(|e| e.code);
        let scratch = made.borrow().clone();
        // keep tmp alive until here
        drop(tmp);
        (r, scratch.clone(), scratch)
    }

    #[test]
    fn scratch_folder_is_removed_on_success() {
        let (r, scratch, _) = run(false, false);
        assert!(r.is_ok());
        assert!(!scratch.as_os_str().is_empty() && !scratch.exists());
    }

    #[test]
    fn scratch_folder_is_removed_when_conversion_fails_after_creating_it() {
        let (r, scratch, _) = run(true, false);
        assert_eq!(r, Err(ErrorCode::Invalid));
        assert!(!scratch.as_os_str().is_empty() && !scratch.exists());
    }

    #[test]
    fn scratch_folder_is_removed_when_opening_the_new_project_fails() {
        let (r, scratch, _) = run(false, true);
        assert_eq!(r, Err(ErrorCode::Io));
        assert!(!scratch.as_os_str().is_empty() && !scratch.exists());
    }
}
