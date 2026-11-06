#![allow(clippy::unwrap_used)]
//! Schema 5 and the store side of merging a copy: migration, the `copy` author, the ledger and the
//! one-transaction apply.

mod common;

use common::*;
use gt_core::Uuid;
use gt_store::migrate::MIGRATIONS;
use gt_store::{
    Actor, Batch, Change, ChangeSet, LogMeta, MergedGroup, Project, Record, RecordKind,
    SCHEMA_VERSION,
};
use rusqlite::Connection;

fn fresh() -> (tempfile::TempDir, Project) {
    let tmp = tempfile::tempdir().unwrap();
    let p = Project::create(&tmp.path().join("t.genetree")).unwrap();
    (tmp, p)
}

fn batch(id: Uuid) -> Batch {
    Batch {
        id,
        title: "Merged copy: Olena".into(),
        source_file: Some("olena-copy".into()),
        actor_id: Uuid::now_v7(),
        actor_name: "Olena".into(),
        started: 10,
        ended: Some(11),
        report: None,
    }
}

fn create_set(batch_id: Uuid, given: &str) -> ((ChangeSet, LogMeta), Uuid) {
    let p = person(given, "Koval");
    let pid = p.common.id;
    let cs = ChangeSet {
        group: Uuid::now_v7(),
        reverts: None,
        label: format!("From Olena's copy: added {given}"),
        ts: 5,
        changes: vec![Change {
            kind: RecordKind::Person,
            id: pid,
            before: None,
            after: Some(Record::Person(p)),
        }],
    };
    let meta = LogMeta {
        actor: Actor::Copy {
            name: "Olena".into(),
        },
        batch: Some(batch_id),
        people: vec![pid],
    };
    ((cs, meta), pid)
}

#[test]
fn schema_is_five_and_a_v4_project_upgrades_with_a_backup() {
    assert_eq!(SCHEMA_VERSION, 5);
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("old.genetree");
    std::fs::create_dir_all(&dir).unwrap();
    {
        let c = Connection::open(dir.join("tree.db")).unwrap();
        for m in &MIGRATIONS[..4] {
            c.execute_batch(m.1).unwrap();
        }
        c.pragma_update(None, "user_version", 4).unwrap();
    }
    let p = Project::open(&dir).unwrap();
    assert_eq!(p.schema_version(), 5);
    assert!(p.merged_source_groups().unwrap().is_empty());
    let aliases: i64 = p
        .conn()
        .query_row("SELECT COUNT(*) FROM merge_alias", [], |r| r.get(0))
        .unwrap();
    assert_eq!(aliases, 0);
    let backups = std::fs::read_dir(dir.join("backups")).unwrap().count();
    assert_eq!(backups, 1, "a migration is preceded by a backup");
}

#[test]
fn an_older_app_refuses_a_schema_5_project_instead_of_misreading_it() {
    // What the previous app does with a project whose version it does not know: it refuses.
    let (_tmp, p) = fresh();
    let dir = p.dir.clone();
    drop(p);
    {
        let c = Connection::open(dir.join("tree.db")).unwrap();
        c.pragma_update(None, "user_version", SCHEMA_VERSION + 1).unwrap();
    }
    let err = Project::open(&dir).unwrap_err();
    assert!(
        matches!(err, gt_store::StoreError::NewerSchema { found, supported }
            if found == SCHEMA_VERSION + 1 && supported == SCHEMA_VERSION),
        "{err:?}"
    );
}

#[test]
fn a_merge_is_written_with_its_ledger_and_the_copy_author() {
    let (_tmp, mut p) = fresh();
    let bid = Uuid::now_v7();
    let (set, pid) = create_set(bid, "Petro");
    let src = Uuid::now_v7();
    let ledger = [MergedGroup {
        source_group: src,
        merge_group: Some(set.0.group),
        source_name: "Olena".into(),
        source_ts: Some(123),
    }];
    p.apply_merge(&batch(bid), &[set.clone()], &[(pid, false)], &ledger, 99)
        .unwrap();
    assert!(p.load_tree().unwrap().persons.contains_key(&pid));
    assert_eq!(p.merged_source_groups().unwrap().len(), 1);
    assert_eq!(p.merged_groups_of(bid).unwrap(), ledger);
    let g = p.log_group(set.0.group).unwrap().unwrap();
    assert_eq!(
        g.actor,
        Some(Actor::Copy {
            name: "Olena".into()
        })
    );
    assert_eq!(g.batch, Some(bid));
    assert!(p.batch_get(bid).unwrap().is_some());
}

#[test]
fn a_merge_that_fails_part_way_keeps_nothing() {
    let (_tmp, mut p) = fresh();
    let bid = Uuid::now_v7();
    let (a, a_id) = create_set(bid, "Petro");
    let (b, b_id) = create_set(bid, "Hanna");
    let dup = Uuid::now_v7();
    // The second ledger row repeats the first one's key: the last statement of the transaction fails.
    let ledger = [
        MergedGroup {
            source_group: dup,
            merge_group: Some(a.0.group),
            source_name: "Olena".into(),
            source_ts: None,
        },
        MergedGroup {
            source_group: dup,
            merge_group: Some(b.0.group),
            source_name: "Olena".into(),
            source_ts: None,
        },
    ];
    let before = p.load_tree().unwrap();
    assert!(p
        .apply_merge(&batch(bid), &[a, b], &[], &ledger, 99)
        .is_err());
    let after = p.load_tree().unwrap();
    assert_eq!(before, after);
    assert!(!after.persons.contains_key(&a_id) && !after.persons.contains_key(&b_id));
    assert!(p.change_log_is_empty().unwrap());
    assert!(p.batch_get(bid).unwrap().is_none());
    assert!(p.merged_source_groups().unwrap().is_empty());
}

#[test]
fn a_merge_for_another_batch_is_refused() {
    let (_tmp, mut p) = fresh();
    let bid = Uuid::now_v7();
    let (set, _) = create_set(Uuid::now_v7(), "Petro");
    assert!(p.apply_merge(&batch(bid), &[set], &[], &[], 1).is_err());
    assert!(p.change_log_is_empty().unwrap());
}
