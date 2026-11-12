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
fn schema_5_project_gains_the_transcription_column() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("Old.genetree");
    drop(Project::create(&dir).unwrap());
    {
        // What a schema 5 app left behind: no transcription column.
        let c = Connection::open(dir.join("tree.db")).unwrap();
        c.execute_batch(
            &format!("{DROP_CLIPS} ALTER TABLE media DROP COLUMN rotation; ALTER TABLE media DROP COLUMN transcription_json; PRAGMA user_version = 5;"),
        )
        .unwrap();
    }
    let p = Project::open(&dir).unwrap();
    assert_eq!(p.schema_version(), SCHEMA_VERSION);
    let has: i64 = p
        .conn()
        .query_row(
            "SELECT count(*) FROM pragma_table_info('media') WHERE name = 'transcription_json'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(has, 1);
}

#[test]
fn schema_6_project_gains_the_rotation_column_and_its_pictures_stay_unturned() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("Old.genetree");
    drop(Project::create(&dir).unwrap());
    {
        // What a schema 6 app left behind: a picture, no rotation column.
        let c = Connection::open(dir.join("tree.db")).unwrap();
        c.execute_batch(
            &format!("{DROP_CLIPS} ALTER TABLE media DROP COLUMN rotation; PRAGMA user_version = 6; \
             INSERT INTO media(id, seq, private, path, mime) \
             VALUES ('00000000-0000-0000-0000-0000000000a1', 0, 0, 'media/ab/ab.jpg', 'image/jpeg');"),
        )
        .unwrap();
    }
    let p = Project::open(&dir).unwrap();
    assert_eq!(p.schema_version(), SCHEMA_VERSION);
    let has: i64 = p
        .conn()
        .query_row(
            "SELECT count(*) FROM pragma_table_info('media') WHERE name = 'rotation'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(has, 1);
    let tree = p.load_tree().unwrap();
    let m = tree.media.values().next().unwrap();
    assert_eq!((m.rotation, m.shown_rotation()), (None, 0));
}

/// Takes a project back from schema 8: no place on media, no clip columns on media links.
const DROP_CLIPS: &str = "ALTER TABLE media DROP COLUMN place_id; \
    ALTER TABLE media_link DROP COLUMN clip_id; \
    ALTER TABLE media_link DROP COLUMN clip_start_ms; ALTER TABLE media_link DROP COLUMN clip_end_ms; \
    ALTER TABLE media_link DROP COLUMN clip_label;";

#[test]
fn schema_7_project_gains_the_clip_columns_and_its_links_stay_whole() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("Old.genetree");
    drop(Project::create(&dir).unwrap());
    {
        // What a schema 7 app left behind: a person linked to a recording, no clip columns.
        let c = Connection::open(dir.join("tree.db")).unwrap();
        c.execute_batch(&format!(
            "{DROP_CLIPS} PRAGMA user_version = 7; \
             INSERT INTO media(id, seq, private, path, mime) \
             VALUES ('00000000-0000-0000-0000-0000000000a1', 0, 0, 'media/ab/ab.ogg', 'audio/ogg'); \
             INSERT INTO person(id, seq, private, sex) \
             VALUES ('00000000-0000-0000-0000-0000000000b1', 0, 0, 'unknown'); \
             INSERT INTO media_link(id, target_type, target_id, pos, media_id, \"primary\") \
             VALUES ('00000000-0000-0000-0000-0000000000c1', 'person', \
             '00000000-0000-0000-0000-0000000000b1', 0, '00000000-0000-0000-0000-0000000000a1', 0);"
        ))
        .unwrap();
    }
    let p = Project::open(&dir).unwrap();
    assert_eq!(p.schema_version(), SCHEMA_VERSION);
    let has: i64 = p
        .conn()
        .query_row(
            "SELECT count(*) FROM pragma_table_info('media_link') WHERE name LIKE 'clip_%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(has, 4);
    let tree = p.load_tree().unwrap();
    let person = tree.persons.values().next().unwrap();
    assert_eq!(person.common.media.len(), 1);
    assert_eq!(
        person.common.media[0].clip, None,
        "an old link is to the whole recording"
    );
}

#[test]
fn schema_8_project_gains_the_media_place_column_and_its_media_have_no_place() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("Old.genetree");
    drop(Project::create(&dir).unwrap());
    {
        // What a schema 8 app left behind: a recording, no place column.
        let c = Connection::open(dir.join("tree.db")).unwrap();
        c.execute_batch(
            "ALTER TABLE media DROP COLUMN place_id; PRAGMA user_version = 8; \
             INSERT INTO media(id, seq, private, path, mime) \
             VALUES ('00000000-0000-0000-0000-0000000000a1', 0, 0, 'media/ab/ab.ogg', 'audio/ogg');",
        )
        .unwrap();
    }
    let p = Project::open(&dir).unwrap();
    assert_eq!(p.schema_version(), SCHEMA_VERSION);
    assert_eq!(SCHEMA_VERSION, 9);
    let has: i64 = p
        .conn()
        .query_row(
            "SELECT count(*) FROM pragma_table_info('media') WHERE name = 'place_id'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(has, 1);
    let tree = p.load_tree().unwrap();
    assert_eq!(tree.media.values().next().unwrap().place, None);
    // The upgrade left a backup of the schema 8 file beside it.
    let backups = std::fs::read_dir(dir.join("backups"))
        .map(|d| d.count())
        .unwrap_or(0);
    assert!(backups >= 1, "no pre-migration backup");
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
