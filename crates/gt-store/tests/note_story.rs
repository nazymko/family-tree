#![allow(clippy::unwrap_used)]
//! A note's readable story: a schema 13 project gains the column and its notes have none.

use gt_store::{Project, SCHEMA_VERSION};
use rusqlite::Connection;

#[test]
fn schema_13_project_gains_the_note_story_column_and_old_notes_have_none() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("Old.genetree");
    drop(Project::create(&dir).unwrap());
    {
        // What a schema 13 app left behind: a note, no story column.
        let c = Connection::open(dir.join("tree.db")).unwrap();
        c.execute_batch(
            "DROP TABLE place_record_person; DROP TABLE place_record; ALTER TABLE note DROP COLUMN story; PRAGMA user_version = 13; \
             INSERT INTO note(id, seq, private, kind, text, shared) \
             VALUES ('00000000-0000-0000-0000-0000000000c1', 0, 0, 'General', 'Born 1890.', 0);",
        )
        .unwrap();
    }
    let p = Project::open(&dir).unwrap();
    assert_eq!(p.schema_version(), SCHEMA_VERSION);
    let tree = p.load_tree().unwrap();
    assert_eq!(tree.notes.len(), 1);
    assert!(tree.notes.values().all(|n| n.story.is_none()));
    // A story written by this version reads back.
    p.conn()
        .execute_batch("UPDATE note SET story = 'Born in 1890.'")
        .unwrap();
    let tree = p.load_tree().unwrap();
    assert_eq!(
        tree.notes.values().next().unwrap().story.as_deref(),
        Some("Born in 1890.")
    );
}
