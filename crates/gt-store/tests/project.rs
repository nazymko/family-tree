#![allow(clippy::unwrap_used)]

use gt_store::{Project, SCHEMA_VERSION, StoreError};
use rusqlite::Connection;

fn table_names(c: &Connection) -> Vec<String> {
    c.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT GLOB 'search_*_[a-z]*'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

#[test]
fn create_then_open() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("My.genetree");
    {
        let p = Project::create(&dir).unwrap();
        assert_eq!(p.schema_version(), SCHEMA_VERSION);
    }
    for sub in ["media", "thumbs", "backups"] {
        assert!(dir.join(sub).is_dir(), "{sub}");
    }
    let p = Project::open(&dir).unwrap();
    assert_eq!(p.schema_version(), SCHEMA_VERSION);
    let mode: String = p
        .conn()
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    let fk: i32 = p
        .conn()
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fk, 1);
    let keys: i32 = p
        .conn()
        .query_row(
            "SELECT count(*) FROM meta WHERE key IN ('app_version','created_ms')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(keys, 2);
}

#[test]
fn create_refuses_existing_project() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    Project::create(&dir).unwrap();
    assert!(matches!(
        Project::create(&dir),
        Err(StoreError::AlreadyExists(_))
    ));
}

#[test]
fn open_non_project() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(matches!(
        Project::open(tmp.path()),
        Err(StoreError::NotAProject(_))
    ));
}

#[test]
fn open_newer_schema_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    drop(Project::create(&dir).unwrap());
    let c = Connection::open(dir.join("tree.db")).unwrap();
    c.pragma_update(None, "user_version", 99).unwrap();
    drop(c);
    match Project::open(&dir) {
        Err(StoreError::NewerSchema { found, supported }) => {
            assert_eq!((found, supported), (99, SCHEMA_VERSION));
        }
        other => panic!("unexpected: {:?}", other.map(|p| p.dir)),
    }
}

#[test]
fn schema_doc_lists_every_table() {
    let doc = include_str!("../../../docs/schema.md");
    let tmp = tempfile::tempdir().unwrap();
    let p = Project::create(&tmp.path().join("p")).unwrap();
    let names = table_names(p.conn());
    assert!(names.len() > 20);
    for n in names {
        assert!(
            doc.contains(&format!("`{n}`")),
            "docs/schema.md misses table {n}"
        );
    }
}

#[test]
fn schema_doc_lists_every_column() {
    let doc = include_str!("../../../docs/schema.md");
    let tmp = tempfile::tempdir().unwrap();
    let p = Project::create(&tmp.path().join("p")).unwrap();
    let c = p.conn();
    for t in table_names(c) {
        let cols: Vec<String> = c
            .prepare(&format!("SELECT name FROM pragma_table_info('{t}')"))
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for col in cols {
            assert!(
                doc.contains(&format!("`{col}`")),
                "docs/schema.md misses column {t}.{col}"
            );
        }
    }
}

#[test]
fn every_foreign_key_column_is_indexed() {
    let tmp = tempfile::tempdir().unwrap();
    let p = Project::create(&tmp.path().join("p")).unwrap();
    let c = p.conn();
    for t in table_names(c) {
        let fks: Vec<String> = c
            .prepare(&format!(
                "SELECT \"from\" FROM pragma_foreign_key_list('{t}')"
            ))
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for col in fks {
            let n: i32 = c
                .query_row(
                    &format!(
                        "SELECT count(*) FROM pragma_index_list('{t}') il \
                         JOIN pragma_index_info(il.name) ii WHERE ii.seqno=0 AND ii.name=?1"
                    ),
                    [&col],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(n > 0, "{t}.{col} has no index");
        }
    }
}

#[test]
fn second_open_is_locked_until_the_first_is_dropped() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("t.genetree");
    drop(Project::create(&dir).unwrap());
    let first = Project::open(&dir).unwrap();
    let err = Project::open(&dir).unwrap_err();
    assert!(matches!(err, gt_store::StoreError::Locked(_)), "{err}");
    assert_eq!(
        err.to_string(),
        "This tree is open in another Family Tree window."
    );
    drop(first);
    drop(Project::open(&dir).unwrap());
}
