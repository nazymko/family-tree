#![allow(clippy::unwrap_used)]

mod common;

use common::*;
use gt_core::Uuid;
use gt_store::migrate::MIGRATIONS;
use gt_store::{
    Actor, Batch, Change, ChangeSet, LogFilter, LogMeta, Project, Record, RecordKind, ReviewScope,
    SCHEMA_VERSION,
};
use rusqlite::Connection;

fn fresh() -> (tempfile::TempDir, Project) {
    let tmp = tempfile::tempdir().unwrap();
    let p = Project::create(&tmp.path().join("t.genetree")).unwrap();
    (tmp, p)
}

fn agent(name: &str) -> Actor {
    Actor::Agent {
        id: Uuid::now_v7(),
        name: name.into(),
    }
}

fn agent_id(a: &Actor) -> Uuid {
    match a {
        Actor::Agent { id, .. } => *id,
        Actor::User => unreachable!(),
    }
}

fn meta(actor: &Actor, batch: Option<Uuid>, people: &[Uuid]) -> LogMeta {
    LogMeta {
        actor: actor.clone(),
        batch,
        people: people.to_vec(),
    }
}

/// A set that creates one new person; returns the set and the person's id.
fn new_person_set(label: &str, ts: i64) -> (ChangeSet, Uuid) {
    let p = person("Ivan", "Koval");
    let pid = p.common.id;
    let cs = ChangeSet {
        group: Uuid::now_v7(),
        reverts: None,
        label: label.into(),
        ts,
        changes: vec![Change {
            kind: RecordKind::Person,
            id: pid,
            before: None,
            after: Some(Record::Person(p)),
        }],
    };
    (cs, pid)
}

fn user() -> LogMeta {
    meta(&Actor::User, None, &[])
}

fn all() -> LogFilter {
    LogFilter::default()
}

#[test]
fn v2_upgrades_and_reads_before_tracking() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("old.genetree");
    std::fs::create_dir_all(&dir).unwrap();
    let target = Uuid::now_v7();
    {
        let c = Connection::open(dir.join("tree.db")).unwrap();
        c.execute_batch(MIGRATIONS[0].1).unwrap();
        c.execute_batch(MIGRATIONS[1].1).unwrap();
        c.pragma_update(None, "user_version", 2).unwrap();
        c.execute(
            "INSERT INTO change_log(ts, target_type, target_id, op, group_id, label) \
             VALUES (5, 'person', ?1, 'update', ?2, 'Old edit')",
            [target.to_string(), Uuid::now_v7().to_string()],
        )
        .unwrap();
        // A schema 1 row: no group at all.
        c.execute(
            "INSERT INTO change_log(ts, target_type, target_id, op) \
             VALUES (4, 'person', ?1, 'update')",
            [target.to_string()],
        )
        .unwrap();
    }
    let p = Project::open(&dir).unwrap();
    assert_eq!(p.schema_version(), 3);
    assert_eq!(SCHEMA_VERSION, 3);
    let (page, total) = p.log_page(&all(), 0, 10).unwrap();
    // The row without a group cannot be addressed, so it is left out of the log.
    assert_eq!(total, 1);
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].label, "Old edit");
    assert_eq!(page[0].actor, None);
    assert!(page[0].people.is_empty());
    // Nothing lost: both rows are still there, and the writers read as unknown.
    let writers = p.last_writers(target).unwrap();
    assert_eq!(writers.len(), 2);
    assert!(writers.iter().all(|(w, _)| w.is_none()));
    let n: i64 = p
        .conn()
        .query_row("SELECT count(*) FROM change_log", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 2);
    assert!(p.unreviewed_people().unwrap().is_empty());
}

#[test]
fn author_and_people_recorded() {
    let (_t, mut p) = fresh();
    let a = agent("Claude");
    let batch = Uuid::now_v7();
    let (cs, pid) = new_person_set("Add person", 100);
    p.apply_changes(&cs, &[], &meta(&a, Some(batch), &[pid, pid]))
        .unwrap();
    let g = p.log_group(cs.group).unwrap().unwrap();
    assert_eq!(g.actor, Some(a.clone()));
    assert_eq!(g.batch, Some(batch));
    assert_eq!(g.label, "Add person");
    assert_eq!(g.ts, 100);
    assert_eq!(g.people.len(), 1);
    assert_eq!(g.people[0].id, pid);
    assert_eq!(g.people[0].reviewed, None);
    assert!(!g.reverted);
    assert_eq!(g.reverts, None);
    assert_eq!(g.changes.len(), 1);

    // A user group records its people too, but they are never "unreviewed".
    let (cs2, pid2) = new_person_set("Add by hand", 200);
    p.apply_changes(&cs2, &[], &meta(&Actor::User, None, &[pid2]))
        .unwrap();
    let g2 = p.log_group(cs2.group).unwrap().unwrap();
    assert_eq!(g2.actor, Some(Actor::User));
    assert_eq!(g2.people.len(), 1);
    assert_eq!(p.unreviewed_people().unwrap().len(), 1);
    assert!(p.unreviewed_people().unwrap().contains(&pid));
    assert!(p.log_group(Uuid::now_v7()).unwrap().is_none());
}

#[test]
fn review_per_person_clears_only_that_person() {
    let (_t, mut p) = fresh();
    let a = agent("Claude");
    let (cs, p1) = new_person_set("Add two", 100);
    let p2 = Uuid::now_v7();
    p.apply_changes(&cs, &[], &meta(&a, None, &[p1, p2]))
        .unwrap();
    assert_eq!(p.unreviewed_people().unwrap().len(), 2);
    p.mark_reviewed(ReviewScope::Person(p1), 500).unwrap();
    let left = p.unreviewed_people().unwrap();
    assert_eq!(left.len(), 1);
    assert!(left.contains(&p2));
    let g = p.log_group(cs.group).unwrap().unwrap();
    let r1 = g.people.iter().find(|x| x.id == p1).unwrap();
    assert_eq!(r1.reviewed, Some(500));
    // Reviewing again keeps the first time.
    p.mark_reviewed(ReviewScope::Person(p1), 900).unwrap();
    let g = p.log_group(cs.group).unwrap().unwrap();
    assert_eq!(
        g.people.iter().find(|x| x.id == p1).unwrap().reviewed,
        Some(500)
    );
}

#[test]
fn review_batch_clears_everyone() {
    let (_t, mut p) = fresh();
    let a = agent("Claude");
    let batch = Uuid::now_v7();
    let (c1, p1) = new_person_set("One", 100);
    let (c2, p2) = new_person_set("Two", 110);
    let (c3, p3) = new_person_set("Elsewhere", 120);
    p.apply_changes(&c1, &[], &meta(&a, Some(batch), &[p1]))
        .unwrap();
    p.apply_changes(&c2, &[], &meta(&a, Some(batch), &[p2]))
        .unwrap();
    p.apply_changes(&c3, &[], &meta(&a, None, &[p3])).unwrap();
    p.mark_reviewed(ReviewScope::Batch(batch), 500).unwrap();
    let left = p.unreviewed_people().unwrap();
    assert_eq!(left.len(), 1);
    assert!(left.contains(&p3));
    p.mark_reviewed(ReviewScope::Group(c3.group), 600).unwrap();
    assert!(p.unreviewed_people().unwrap().is_empty());
}

#[test]
fn reverted_group_has_no_badge() {
    let (_t, mut p) = fresh();
    let a = agent("Claude");
    let (cs, pid) = new_person_set("Add", 100);
    p.apply_changes(&cs, &[], &meta(&a, None, &[pid])).unwrap();
    assert!(p.unreviewed_people().unwrap().contains(&pid));
    let inv = cs.inverse();
    p.apply_changes(&inv, &[], &user()).unwrap();
    assert!(p.unreviewed_people().unwrap().is_empty());
    let g = p.log_group(cs.group).unwrap().unwrap();
    assert!(g.reverted);
    let u = p.log_group(inv.group).unwrap().unwrap();
    assert_eq!(u.reverts, Some(cs.group));
    assert!(!u.reverted);
    let f = LogFilter {
        unreviewed_only: true,
        ..LogFilter::default()
    };
    assert_eq!(p.log_page(&f, 0, 10).unwrap().1, 0);
}

#[test]
fn log_filters_by_author_batch_unreviewed() {
    let (_t, mut p) = fresh();
    let a = agent("Claude");
    let b = agent("Other");
    let batch = Uuid::now_v7();
    let (c1, p1) = new_person_set("A1", 100);
    let (c2, p2) = new_person_set("B1", 200);
    let (c3, p3) = new_person_set("U1", 300);
    p.apply_changes(&c1, &[], &meta(&a, Some(batch), &[p1]))
        .unwrap();
    p.apply_changes(&c2, &[], &meta(&b, None, &[p2])).unwrap();
    p.apply_changes(&c3, &[], &meta(&Actor::User, None, &[p3]))
        .unwrap();

    let (page, total) = p.log_page(&all(), 0, 10).unwrap();
    assert_eq!(total, 3);
    let labels: Vec<_> = page.iter().map(|g| g.label.as_str()).collect();
    assert_eq!(labels, ["U1", "B1", "A1"], "newest first");

    let (page, total) = p.log_page(&all(), 1, 1).unwrap();
    assert_eq!((page.len(), total), (1, 3));
    assert_eq!(page[0].label, "B1");

    let f = LogFilter {
        actor_id: Some(agent_id(&a)),
        ..LogFilter::default()
    };
    let (page, total) = p.log_page(&f, 0, 10).unwrap();
    assert_eq!((page.len(), total), (1, 1));
    assert_eq!(page[0].label, "A1");

    let f = LogFilter {
        actor_kind: Some("agent".into()),
        ..LogFilter::default()
    };
    assert_eq!(p.log_page(&f, 0, 10).unwrap().1, 2);
    let f = LogFilter {
        actor_kind: Some("user".into()),
        ..LogFilter::default()
    };
    assert_eq!(p.log_page(&f, 0, 10).unwrap().1, 1);

    let f = LogFilter {
        batch: Some(batch),
        ..LogFilter::default()
    };
    assert_eq!(p.log_page(&f, 0, 10).unwrap().0[0].label, "A1");

    let f = LogFilter {
        person: Some(p2),
        ..LogFilter::default()
    };
    let (page, total) = p.log_page(&f, 0, 10).unwrap();
    assert_eq!((page.len(), total), (1, 1));
    assert_eq!(page[0].label, "B1");

    // Unreviewed counts agent groups only, so the user group is not in it.
    let f = LogFilter {
        unreviewed_only: true,
        ..LogFilter::default()
    };
    assert_eq!(p.log_page(&f, 0, 10).unwrap().1, 2);
    p.mark_reviewed(ReviewScope::Person(p1), 900).unwrap();
    assert_eq!(p.log_page(&f, 0, 10).unwrap().1, 1);
}

#[test]
fn group_changes_roundtrip_records() {
    let (_t, mut p) = fresh();
    let (cs, pid) = new_person_set("Add", 100);
    p.apply_changes(&cs, &[], &user()).unwrap();
    let got = p.group_changes(cs.group).unwrap();
    assert_eq!(got, cs.changes);
    assert!(p.group_changes(Uuid::now_v7()).unwrap().is_empty());
    assert_eq!(p.rows_of(pid).unwrap(), cs.changes);
}

#[test]
fn last_writers_name_the_agent_and_none_for_user() {
    let (_t, mut p) = fresh();
    let a = agent("Claude");
    let (cs, pid) = new_person_set("Add", 100);
    p.apply_changes(&cs, &[], &meta(&a, None, &[pid])).unwrap();
    let Some(Record::Person(before)) = cs.changes[0].after.clone() else {
        unreachable!()
    };
    let mut after = before.clone();
    after.names = vec![name("Olena", "Koval")];
    let edit = ChangeSet {
        group: Uuid::now_v7(),
        reverts: None,
        label: "Rename".into(),
        ts: 200,
        changes: vec![Change {
            kind: RecordKind::Person,
            id: pid,
            before: Some(Record::Person(before)),
            after: Some(Record::Person(after)),
        }],
    };
    p.apply_changes(&edit, &[], &user()).unwrap();
    let w = p.last_writers(pid).unwrap();
    assert_eq!(w.len(), 2);
    assert_eq!(w[0].0, Some(agent_id(&a)));
    assert_eq!(w[1].0, None);
}

#[test]
fn batches_open_close_and_list() {
    let (_t, mut p) = fresh();
    let id = Uuid::now_v7();
    let b = Batch {
        id,
        title: "Import notes".into(),
        source_file: Some("notes.txt".into()),
        actor_id: Uuid::now_v7(),
        actor_name: "Claude".into(),
        started: 10,
        ended: None,
        report: None,
    };
    p.batch_create(&b).unwrap();
    assert_eq!(p.batch_get(id).unwrap(), Some(b.clone()));
    p.batch_close(id, "Added 3 people", 20).unwrap();
    let got = p.batch_get(id).unwrap().unwrap();
    assert_eq!(got.ended, Some(20));
    assert_eq!(got.report.as_deref(), Some("Added 3 people"));
    let later = Batch {
        id: Uuid::now_v7(),
        started: 30,
        ..b
    };
    p.batch_create(&later).unwrap();
    let list = p.batch_list().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, later.id, "newest first");
    assert_eq!(p.batch_get(Uuid::now_v7()).unwrap(), None);
}
