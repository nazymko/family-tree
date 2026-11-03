#![allow(clippy::unwrap_used)]

mod common;

use common::*;
use gt_store::{BackupReason, LocationWarning, Project, check_location};
use std::path::Path;

const TODAY: i32 = 2_460_000;

#[test]
fn backup_while_open_is_valid() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = Project::create(&tmp.path().join("t.genetree")).unwrap();
    let tree = rich_tree();
    p.save_tree(&tree, TODAY).unwrap();
    // The connection stays open (WAL mode, unflushed pages possible).
    let path = p.backup(BackupReason::Manual, 5).unwrap();
    assert!(path.starts_with(p.dir.join("backups")));
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with("tree-") && name.ends_with("-manual.db"),
        "{name}"
    );

    let c = rusqlite::Connection::open(&path).unwrap();
    let check: String = c
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(check, "ok");
    let n: i64 = c
        .query_row("SELECT COUNT(*) FROM person", [], |r| r.get(0))
        .unwrap();
    assert_eq!(usize::try_from(n).unwrap(), tree.persons.len());
    // The original is untouched and still usable.
    assert_eq!(p.load_tree().unwrap(), tree);
}

#[test]
fn backup_prunes() {
    let tmp = tempfile::tempdir().unwrap();
    let p = Project::create(&tmp.path().join("t.genetree")).unwrap();
    let made: Vec<_> = (0..7)
        .map(|_| p.backup(BackupReason::DailyOpen, 5).unwrap())
        .collect();
    let mut left: Vec<_> = std::fs::read_dir(p.dir.join("backups"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    left.sort();
    assert_eq!(left.len(), 5);
    // The two oldest are gone, the five newest remain (even when taken in the same second).
    for gone in &made[..2] {
        assert!(!gone.exists(), "{gone:?}");
    }
    for kept in &made[2..] {
        assert!(kept.exists(), "{kept:?}");
    }
}

#[test]
fn same_second_backups_get_a_suffix() {
    let tmp = tempfile::tempdir().unwrap();
    let p = Project::create(&tmp.path().join("t.genetree")).unwrap();
    let paths: Vec<_> = (0..3)
        .map(|_| p.backup(BackupReason::Manual, 50).unwrap())
        .collect();
    let mut unique = paths.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 3);
    // Unless the clock ticked over, the later ones carry -2 and -3.
    let names: Vec<String> = paths
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    if names[0][5..20] == names[2][5..20] {
        assert!(names[1].ends_with("-manual-2.db"), "{names:?}");
        assert!(names[2].ends_with("-manual-3.db"), "{names:?}");
    }
}

#[test]
fn onedrive_warns() {
    assert_eq!(
        check_location(Path::new(r"C:\Users\a\OneDrive - Contoso\Family")),
        Some(LocationWarning::SyncedFolder("OneDrive"))
    );
}

#[test]
fn dropbox_warns() {
    assert_eq!(
        check_location(Path::new("/home/a/Dropbox/tree")),
        Some(LocationWarning::SyncedFolder("Dropbox"))
    );
}

#[test]
fn other_providers_warn() {
    for (path, want) in [
        (
            "/Users/a/Library/Mobile Documents/com~apple~CloudDocs/t",
            "iCloud",
        ),
        ("C:\\Users\\a\\iCloudDrive\\t", "iCloud"),
        ("G:\\My Drive\\t", "Google Drive"),
        ("/home/a/GoogleDrive/t", "Google Drive"),
    ] {
        assert_eq!(
            check_location(Path::new(path)),
            Some(LocationWarning::SyncedFolder(want)),
            "{path}"
        );
    }
}

#[test]
fn unc_warns() {
    assert_eq!(
        check_location(Path::new(r"\\nas\share\tree")),
        Some(LocationWarning::NetworkShare)
    );
    // Verbatim local paths are not shares.
    assert_eq!(check_location(Path::new(r"\\?\C:\Users\a\tree")), None);
}

#[test]
fn mounts_are_not_flagged() {
    assert_eq!(check_location(Path::new("/mnt/data/tree")), None);
    assert_eq!(check_location(Path::new("/media/usb/tree")), None);
}

#[test]
fn plain_dir_ok() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(check_location(tmp.path()), None);
}
