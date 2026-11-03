#![allow(clippy::unwrap_used)]

mod common;

use common::*;
use gt_core::Uuid;
use gt_core::model::*;
use gt_core::tree::Tree;
use gt_store::migrate::MIGRATIONS;
use gt_store::{
    Change, ChangeOp, ChangeSet, HitKind, Project, Record, RecordKind, SCHEMA_VERSION, StoreError,
};
use proptest::prelude::*;
use rusqlite::Connection;
use std::collections::HashMap;

const TODAY: i32 = 2_460_000;

fn base() -> (tempfile::TempDir, Project, Tree) {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = Project::create(&tmp.path().join("t.genetree")).unwrap();
    let tree = rich_tree();
    p.save_tree(&tree, TODAY).unwrap();
    (tmp, p, tree)
}

fn cs(label: &str, changes: Vec<Change>) -> ChangeSet {
    ChangeSet {
        group: Uuid::now_v7(),
        label: label.into(),
        ts: 1_700_000_000_000,
        changes,
    }
}

fn create(rec: Record) -> Change {
    Change {
        kind: rec.kind(),
        id: rec.id(),
        before: None,
        after: Some(rec),
    }
}

fn update(before: Record, after: Record) -> Change {
    Change {
        kind: after.kind(),
        id: after.id(),
        before: Some(before),
        after: Some(after),
    }
}

fn delete(before: Record) -> Change {
    Change {
        kind: before.kind(),
        id: before.id(),
        before: Some(before),
        after: None,
    }
}

fn count(p: &Project, sql: &str, arg: Uuid) -> i64 {
    p.conn()
        .query_row(sql, [arg.to_string()], |r| r.get(0))
        .unwrap()
}

fn person_named(t: &Tree, surname: &str) -> Uuid {
    t.persons
        .values()
        .find(|p| {
            p.names
                .iter()
                .any(|n| n.surnames.iter().any(|s| s == surname))
        })
        .map(|p| p.common.id)
        .unwrap()
}

fn same_order(a: &Tree, b: &Tree) {
    assert_eq!(
        a.persons.keys().collect::<Vec<_>>(),
        b.persons.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        a.families.keys().collect::<Vec<_>>(),
        b.families.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        a.events.keys().collect::<Vec<_>>(),
        b.events.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        a.places.keys().collect::<Vec<_>>(),
        b.places.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        a.notes.keys().collect::<Vec<_>>(),
        b.notes.keys().collect::<Vec<_>>()
    );
}

fn mirror(t: &mut Tree, cs: &ChangeSet) {
    for c in &cs.changes {
        match (&c.after, c.kind) {
            (Some(Record::Person(r)), _) => {
                t.persons.insert(r.common.id, r.clone());
            }
            (Some(Record::Family(r)), _) => {
                t.families.insert(r.common.id, r.clone());
            }
            (Some(Record::Event(r)), _) => {
                t.events.insert(r.common.id, r.clone());
            }
            (Some(Record::Place(r)), _) => {
                t.places.insert(r.common.id, r.clone());
            }
            (Some(Record::Source(r)), _) => {
                t.sources.insert(r.common.id, r.clone());
            }
            (Some(Record::Repository(r)), _) => {
                t.repositories.insert(r.common.id, r.clone());
            }
            (Some(Record::Citation(r)), _) => {
                t.citations.insert(r.common.id, r.clone());
            }
            (Some(Record::Media(r)), _) => {
                t.media.insert(r.common.id, r.clone());
            }
            (Some(Record::Note(r)), _) => {
                t.notes.insert(r.common.id, r.clone());
            }
            (None, RecordKind::Person) => {
                t.persons.shift_remove(&c.id);
            }
            (None, RecordKind::Family) => {
                t.families.shift_remove(&c.id);
            }
            (None, RecordKind::Event) => {
                t.events.shift_remove(&c.id);
            }
            (None, RecordKind::Place) => {
                t.places.shift_remove(&c.id);
            }
            (None, RecordKind::Source) => {
                t.sources.shift_remove(&c.id);
            }
            (None, RecordKind::Repository) => {
                t.repositories.shift_remove(&c.id);
            }
            (None, RecordKind::Citation) => {
                t.citations.shift_remove(&c.id);
            }
            (None, RecordKind::Media) => {
                t.media.shift_remove(&c.id);
            }
            (None, RecordKind::Note) => {
                t.notes.shift_remove(&c.id);
            }
        }
    }
}

#[test]
fn v1_project_upgrades_to_2() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("old.genetree");
    std::fs::create_dir_all(&dir).unwrap();
    {
        let c = Connection::open(dir.join("tree.db")).unwrap();
        c.execute_batch(MIGRATIONS[0].1).unwrap();
        c.pragma_update(None, "user_version", 1).unwrap();
    }
    let p = Project::open(&dir).unwrap();
    assert_eq!(p.schema_version(), 2);
    assert_eq!(SCHEMA_VERSION, 2);
    let cols: Vec<String> = p
        .conn()
        .prepare("SELECT name FROM pragma_table_info('change_log')")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    for c in ["group_id", "label", "after_json"] {
        assert!(cols.iter().any(|x| x == c), "missing {c}");
    }
}

#[test]
fn v1_project_with_data_keeps_search_consistent_after_upgrade() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("old.genetree");
    let tree = rich_tree();
    {
        let mut p = Project::create(&dir).unwrap();
        p.save_tree(&tree, TODAY).unwrap();
    }
    {
        // Turn it back into a v1 database whose full-text rows have foreign row ids.
        let c = Connection::open(dir.join("tree.db")).unwrap();
        c.execute_batch(
            "DROP INDEX change_log_target; DROP INDEX change_log_group; DROP INDEX idx_person_seq; DROP INDEX idx_family_seq; DROP INDEX idx_event_seq; DROP INDEX idx_place_seq; DROP INDEX idx_source_seq; DROP INDEX idx_repository_seq; DROP INDEX idx_citation_seq; DROP INDEX idx_media_seq; DROP INDEX idx_note_seq; \
             ALTER TABLE change_log DROP COLUMN group_id; \
             ALTER TABLE change_log DROP COLUMN label; \
             ALTER TABLE change_log DROP COLUMN after_json; \
             UPDATE search_words SET rowid = rowid + 1000003; \
             UPDATE search_trigram SET rowid = rowid + 1000003;",
        )
        .unwrap();
        c.pragma_update(None, "user_version", 1).unwrap();
    }
    let mut p = Project::open(&dir).unwrap();
    assert_eq!(p.schema_version(), 2);
    assert!(!p.search("muller", 10).unwrap().is_empty());
    let who = person_named(&tree, "Müller");
    let before = tree.persons[&who].clone();
    let mut after = before.clone();
    after.names = vec![name("Anna", "Meier")];
    p.apply_changes(
        &cs(
            "Edit name",
            vec![update(Record::Person(before), Record::Person(after))],
        ),
        &[],
    )
    .unwrap();
    assert!(p.search("muller", 10).unwrap().is_empty());
    assert_eq!(
        p.search("meier", 10)
            .unwrap()
            .iter()
            .filter(|h| h.id == who)
            .count(),
        1
    );
}

#[test]
fn upsert_person_replaces_names() {
    let (_t, mut p, tree) = base();
    let who = person_named(&tree, "Müller");
    assert_eq!(
        count(&p, "SELECT count(*) FROM name WHERE person_id=?1", who),
        2
    );
    let before = tree.persons[&who].clone();
    let mut after = before.clone();
    after.names.truncate(1);
    p.apply_changes(
        &cs(
            "Edit name",
            vec![update(
                Record::Person(before),
                Record::Person(after.clone()),
            )],
        ),
        &[],
    )
    .unwrap();
    assert_eq!(
        count(&p, "SELECT count(*) FROM name WHERE person_id=?1", who),
        1
    );
    // The dropped name's citation/note links went with it.
    assert_eq!(
        count(
            &p,
            "SELECT count(*) FROM note_link WHERE target_type='name' AND target_id IN \
             (SELECT id FROM name WHERE person_id=?1)",
            who
        ),
        0
    );
    // Families and child links of the person survived the update.
    let mut want = tree.clone();
    want.persons.insert(who, after);
    let got = p.load_tree().unwrap();
    assert_eq!(got.normalized_for_compare(), want.normalized_for_compare());
    same_order(&got, &want);
}

#[test]
fn delete_event_removes_participants_and_links() {
    let (_t, mut p, tree) = base();
    let (eid, ev) = tree
        .events
        .iter()
        .find(|(_, e)| e.participants.len() == 3)
        .map(|(i, e)| (*i, e.clone()))
        .unwrap();
    let mut with_links = ev.clone();
    let note = *tree.notes.keys().next().unwrap();
    with_links.common.notes = vec![note];
    p.apply_changes(
        &cs(
            "Add note",
            vec![update(Record::Event(ev), Record::Event(with_links.clone()))],
        ),
        &[],
    )
    .unwrap();
    assert_eq!(
        count(
            &p,
            "SELECT count(*) FROM participant WHERE event_id=?1",
            eid
        ),
        3
    );
    assert_eq!(
        count(&p, "SELECT count(*) FROM note_link WHERE target_id=?1", eid),
        1
    );
    p.apply_changes(
        &cs("Delete event", vec![delete(Record::Event(with_links))]),
        &[],
    )
    .unwrap();
    assert_eq!(
        count(
            &p,
            "SELECT count(*) FROM participant WHERE event_id=?1",
            eid
        ),
        0
    );
    assert_eq!(
        count(&p, "SELECT count(*) FROM note_link WHERE target_id=?1", eid),
        0
    );
    assert_eq!(count(&p, "SELECT count(*) FROM event WHERE id=?1", eid), 0);
}

#[test]
fn search_reflects_edit() {
    let (_t, mut p, tree) = base();
    let who = person_named(&tree, "Müller");
    assert!(p.search("muller", 10).unwrap().iter().any(|h| h.id == who));
    let before = tree.persons[&who].clone();
    let mut after = before.clone();
    after.names = vec![name("Anna", "Meier")];
    p.apply_changes(
        &cs(
            "Edit name",
            vec![update(Record::Person(before), Record::Person(after))],
        ),
        &[],
    )
    .unwrap();
    assert!(p.search("muller", 10).unwrap().is_empty());
    assert!(p.search("meier", 10).unwrap().iter().any(|h| h.id == who));

    // A new event at a new place makes its principal findable by the place.
    let zed = person_named(&tree, "Zulu");
    let hamburg = place("Hamburg");
    let hid = hamburg.common.id;
    let death = event(EventKind::Death, zed, "3 MAR 1930", Some(hid));
    p.apply_changes(
        &cs(
            "Add death",
            vec![
                create(Record::Place(hamburg.clone())),
                create(Record::Event(death)),
            ],
        ),
        &[],
    )
    .unwrap();
    let hits = p.search("hamburg", 10).unwrap();
    assert!(
        hits.iter()
            .any(|h| h.id == zed && h.kind == HitKind::Person)
    );
    assert!(hits.iter().any(|h| h.id == hid && h.kind == HitKind::Place));

    // Renaming the place refreshes the people who have events there.
    let mut renamed = hamburg.clone();
    renamed.names[0].name = "Altona".into();
    renamed.raw = Some("Altona".into());
    p.apply_changes(
        &cs(
            "Edit place",
            vec![update(Record::Place(hamburg), Record::Place(renamed))],
        ),
        &[],
    )
    .unwrap();
    assert!(p.search("hamburg", 10).unwrap().is_empty());
    assert!(p.search("altona", 10).unwrap().iter().any(|h| h.id == zed));
}

#[test]
fn family_event_place_reaches_both_spouses() {
    let (_t, mut p, tree) = base();
    let (fid, fam) = tree
        .families
        .iter()
        .next()
        .map(|(i, f)| (*i, f.clone()))
        .unwrap();
    let pl = place("Lübeck");
    let plid = pl.common.id;
    let mut ev = event(
        EventKind::Marriage,
        fam.partner1.unwrap(),
        "1880",
        Some(plid),
    );
    ev.participants = vec![Participant {
        target: ParticipantRef::Family(fid),
        role: Role::Principal,
    }];
    p.apply_changes(
        &cs(
            "Add marriage",
            vec![create(Record::Place(pl)), create(Record::Event(ev))],
        ),
        &[],
    )
    .unwrap();
    let hits = p.search("lübeck", 10).unwrap();
    for partner in [fam.partner1.unwrap(), fam.partner2.unwrap()] {
        assert!(hits.iter().any(|h| h.id == partner), "{partner}");
    }
}

#[test]
fn living_cached_applied() {
    let (_t, mut p, tree) = base();
    let who = person_named(&tree, "Zulu");
    let before = tree.persons[&who].clone();
    let mut after = before.clone();
    after.sex = Sex::Male;
    p.apply_changes(
        &cs(
            "Edit sex",
            vec![update(Record::Person(before), Record::Person(after))],
        ),
        &[(who, false)],
    )
    .unwrap();
    let get = |p: &Project| -> i64 {
        p.conn()
            .query_row(
                "SELECT living_cached FROM person WHERE id=?1",
                [who.to_string()],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(get(&p), 0);
    let mut all = HashMap::new();
    all.insert(who, true);
    p.set_living_cached_all(&all).unwrap();
    assert_eq!(get(&p), 1);
}

#[test]
fn change_log_rows_and_history() {
    let (_t, mut p, tree) = base();
    assert!(p.change_log_is_empty().unwrap());
    let who = person_named(&tree, "Müller");
    let before = tree.persons[&who].clone();
    let mut after = before.clone();
    after.names = vec![name("Anna", "Meier")];
    let hamburg = place("Hamburg");
    let ev = event(EventKind::Residence, who, "1900", Some(hamburg.common.id));
    let set = cs(
        "Edit name",
        vec![
            create(Record::Place(hamburg)),
            update(Record::Person(before), Record::Person(after.clone())),
            create(Record::Event(ev)),
        ],
    );
    p.apply_changes(&set, &[]).unwrap();
    assert!(!p.change_log_is_empty().unwrap());
    let n: i64 = p
        .conn()
        .query_row(
            "SELECT count(*) FROM change_log WHERE group_id=?1 AND label='Edit name'",
            [set.group.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 3);
    let mut later = cs("Edit sex", vec![]);
    later.ts += 5;
    let mut a2 = after.clone();
    a2.sex = Sex::Male;
    later.changes = vec![update(Record::Person(after), Record::Person(a2))];
    p.apply_changes(&later, &[]).unwrap();
    let h = p.history(who, 10).unwrap();
    assert_eq!(h.len(), 2);
    assert_eq!(h[0].label, "Edit sex");
    assert_eq!(h[0].op, ChangeOp::Update);
    assert_eq!(h[0].kind, RecordKind::Person);
    assert_eq!(h[1].label, "Edit name");
    assert_eq!(h[1].group, set.group);
    assert_eq!(p.history(who, 1).unwrap().len(), 1);
}

#[test]
fn inverse_roundtrip() {
    let (_t, mut p, tree) = base();
    let who = person_named(&tree, "Müller");
    let before = tree.persons[&who].clone();
    let mut after = before.clone();
    after.names = vec![name("Anna", "Meier")];
    let pl = place("Hamburg");
    let ev = event(EventKind::Death, who, "1900", Some(pl.common.id));
    let gone = tree.events.values().next().unwrap().clone();
    let set = cs(
        "Mixed",
        vec![
            create(Record::Place(pl)),
            update(Record::Person(before), Record::Person(after)),
            create(Record::Event(ev)),
            delete(Record::Event(gone)),
        ],
    );
    p.apply_changes(&set, &[]).unwrap();
    assert_ne!(
        p.load_tree().unwrap().normalized_for_compare(),
        tree.normalized_for_compare()
    );
    let inv = set.inverse();
    assert_eq!(inv.changes.len(), 4);
    assert_eq!(inv.changes[0].kind, RecordKind::Event);
    assert!(inv.changes[0].before.is_none());
    p.apply_changes(&inv, &[]).unwrap();
    let got = p.load_tree().unwrap();
    assert_eq!(got.normalized_for_compare(), tree.normalized_for_compare());
    assert!(p.search("hamburg", 10).unwrap().is_empty());
    assert!(p.search("muller", 10).unwrap().iter().any(|h| h.id == who));
}

#[test]
fn failed_apply_rolls_back() {
    let (_t, mut p, tree) = base();
    let newp = person("Neu", "Person");
    let newid = newp.common.id;
    let bad = event(EventKind::Birth, newid, "1900", Some(Uuid::now_v7()));
    let r = p.apply_changes(
        &cs(
            "Broken",
            vec![create(Record::Person(newp)), create(Record::Event(bad))],
        ),
        &[],
    );
    assert!(matches!(r, Err(StoreError::Sql(_))), "{r:?}");
    assert_eq!(
        count(&p, "SELECT count(*) FROM person WHERE id=?1", newid),
        0
    );
    assert!(p.change_log_is_empty().unwrap());
    assert_eq!(p.load_tree().unwrap(), tree);
    assert!(p.search("neu", 10).unwrap().is_empty());
}

#[test]
fn meta_roundtrip_and_survives_reopen() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("m.genetree");
    {
        let mut p = Project::create(&dir).unwrap();
        assert_eq!(p.meta_get("home_person").unwrap(), None);
        p.meta_set("home_person", "abc").unwrap();
        p.meta_set("home_person", "def").unwrap();
        p.meta_set("living_max_age", "90").unwrap();
        assert_eq!(p.meta_get("home_person").unwrap().as_deref(), Some("def"));
    }
    let p = Project::open(&dir).unwrap();
    assert_eq!(p.meta_get("home_person").unwrap().as_deref(), Some("def"));
    assert_eq!(p.meta_get("living_max_age").unwrap().as_deref(), Some("90"));
}

#[test]
#[ignore = "release-mode timing: cargo test -p gt-store --release -- --ignored"]
fn apply_is_fast_on_50k() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = Project::create(&tmp.path().join("big.genetree")).unwrap();
    let tree = big_tree(50_000);
    p.save_tree(&tree, TODAY).unwrap();
    let (&who, person0) = tree.persons.get_index(25_000).unwrap();
    let mut after = person0.clone();
    after.names = vec![name("Changed", "Name")];
    let ev = event(EventKind::Death, who, "1 JAN 1950", None);
    let pl = place("Fresh");
    let set = cs(
        "Edit",
        vec![
            create(Record::Place(pl)),
            update(Record::Person(person0.clone()), Record::Person(after)),
            create(Record::Event(ev)),
        ],
    );
    let t = std::time::Instant::now();
    p.apply_changes(&set, &[(who, true)]).unwrap();
    let dt = t.elapsed();
    assert!(dt.as_millis() < 50, "{dt:?}");
}

// ---- randomised: incremental edits equal a full save -------------------------------------

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a, V>(&mut self, m: &'a indexmap::IndexMap<Uuid, V>) -> Option<(Uuid, &'a V)> {
        if m.is_empty() {
            return None;
        }
        let i = self.below(m.len());
        m.get_index(i).map(|(k, v)| (*k, v))
    }
}

const GIVEN: [&str; 4] = ["Anna", "Józef", "Мария", "Ève"];
const FAMILY: [&str; 5] = ["Müller", "Meier", "Новак", "Kovač", "Stein"];

fn child(person: Uuid) -> ChildLink {
    ChildLink {
        person,
        rel1: ParentRel::Birth,
        rel2: ParentRel::Birth,
        preferred: false,
        assertion: Assertion {
            preferred: false,
            proof: None,
            confidence: None,
        },
        citations: vec![],
        unknown: vec![],
    }
}

fn gen_name(r: &mut Rng) -> Name {
    name(GIVEN[r.below(GIVEN.len())], FAMILY[r.below(FAMILY.len())])
}

fn gen_cs(r: &mut Rng, t: &Tree) -> Option<ChangeSet> {
    let changes = match r.below(14) {
        0 => {
            let mut p = person(GIVEN[r.below(4)], FAMILY[r.below(5)]);
            p.sex = if r.below(2) == 0 {
                Sex::Male
            } else {
                Sex::Female
            };
            vec![create(Record::Person(p))]
        }
        1 => {
            let (_, b) = r.pick(&t.persons)?;
            let mut a = b.clone();
            a.names = (0..=r.below(2)).map(|_| gen_name(r)).collect();
            a.sex = Sex::Female;
            vec![update(Record::Person(b.clone()), Record::Person(a))]
        }
        2 => {
            let mut pl = place(&format!("Town{}", r.below(100)));
            let parent = r.pick(&t.places).map(|(k, _)| k);
            if r.below(2) == 0 {
                pl.parent = parent;
            }
            vec![create(Record::Place(pl))]
        }
        3 => {
            let (_, b) = r.pick(&t.places)?;
            let mut a = b.clone();
            a.names[0].name = format!("Renamed{}", r.below(100));
            vec![update(Record::Place(b.clone()), Record::Place(a))]
        }
        4 => {
            let (pid, _) = r.pick(&t.persons)?;
            let at = r.pick(&t.places).map(|(k, _)| k);
            let kind = if r.below(2) == 0 {
                EventKind::Birth
            } else {
                EventKind::Death
            };
            vec![create(Record::Event(event(kind, pid, "1 JAN 1900", at)))]
        }
        5 => {
            let (_, b) = r.pick(&t.events)?;
            let mut a = b.clone();
            a.place = r.pick(&t.places).map(|(k, _)| k);
            a.description = Some(format!("d{}", r.below(10)));
            vec![update(Record::Event(b.clone()), Record::Event(a))]
        }
        6 => {
            let (_, b) = r.pick(&t.events)?;
            vec![delete(Record::Event(b.clone()))]
        }
        7 => {
            let (a, _) = r.pick(&t.persons)?;
            let (b, _) = r.pick(&t.persons)?;
            let (c, _) = r.pick(&t.persons)?;
            if a == b {
                return None;
            }
            let fam = Family {
                common: common(Uuid::now_v7()),
                partner1: Some(a),
                partner2: Some(b),
                kind: UnionKind::Married,
                children: vec![child(c)],
            };
            vec![create(Record::Family(fam))]
        }
        8 => {
            let (_, b) = r.pick(&t.families)?;
            let (c, _) = r.pick(&t.persons)?;
            let mut a = b.clone();
            if r.below(3) == 0 {
                a.children.pop();
            } else {
                a.children.push(child(c));
            }
            vec![update(Record::Family(b.clone()), Record::Family(a))]
        }
        9 => {
            let (_, b) = r.pick(&t.families)?;
            vec![delete(Record::Family(b.clone()))]
        }
        10 => {
            // Delete a person together with everything that points at them.
            let (pid, pr) = r.pick(&t.persons)?;
            let mut out = Vec::new();
            for f in t.families.values() {
                if f.partner1 == Some(pid)
                    || f.partner2 == Some(pid)
                    || f.children.iter().any(|c| c.person == pid)
                {
                    let mut a = f.clone();
                    if a.partner1 == Some(pid) {
                        a.partner1 = None;
                    }
                    if a.partner2 == Some(pid) {
                        a.partner2 = None;
                    }
                    a.children.retain(|c| c.person != pid);
                    out.push(update(Record::Family(f.clone()), Record::Family(a)));
                }
            }
            for e in t.events.values() {
                let target = ParticipantRef::Person(pid);
                if e.participants.iter().any(|x| x.target == target) {
                    let mut a = e.clone();
                    a.participants.retain(|x| x.target != target);
                    if a.participants.is_empty() {
                        out.push(delete(Record::Event(e.clone())));
                    } else {
                        out.push(update(Record::Event(e.clone()), Record::Event(a)));
                    }
                }
            }
            out.push(delete(Record::Person(pr.clone())));
            out
        }
        11 => {
            let (_, b) = r.pick(&t.persons)?;
            let n = Note {
                common: common(Uuid::now_v7()),
                kind: NoteKind::General,
                text: format!("Note {}", r.below(1000)),
                lang: None,
                shared: false,
            };
            let mut a = b.clone();
            a.common.notes.push(n.common.id);
            vec![
                create(Record::Note(n)),
                update(Record::Person(b.clone()), Record::Person(a)),
            ]
        }
        12 => {
            // Detach a note from every person that links it, then delete it.
            let (nid, n) = r.pick(&t.notes)?;
            let used_elsewhere = t.families.values().any(|f| f.common.notes.contains(&nid))
                || t.events.values().any(|e| e.common.notes.contains(&nid))
                || t.places.values().any(|e| e.common.notes.contains(&nid))
                || t.sources.values().any(|e| e.common.notes.contains(&nid))
                || t.citations.values().any(|e| e.common.notes.contains(&nid));
            if used_elsewhere {
                return None;
            }
            let mut out = Vec::new();
            for p in t.persons.values() {
                if p.common.notes.contains(&nid) || p.names.iter().any(|x| x.notes.contains(&nid)) {
                    let mut a = p.clone();
                    a.common.notes.retain(|x| *x != nid);
                    a.names
                        .iter_mut()
                        .for_each(|x| x.notes.retain(|y| *y != nid));
                    out.push(update(Record::Person(p.clone()), Record::Person(a)));
                }
            }
            out.push(delete(Record::Note(n.clone())));
            out
        }
        _ => {
            let (pid, pl) = r.pick(&t.places)?;
            let used = t.events.values().any(|e| e.place == Some(pid))
                || t.places.values().any(|x| x.parent == Some(pid));
            if used {
                return None;
            }
            vec![delete(Record::Place(pl.clone()))]
        }
    };
    Some(cs("Random", changes))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn incremental_equals_full_save(seed in any::<u64>()) {
        let (_t, mut p, mut mirrored) = base();
        let mut rng = Rng(seed);
        let mut applied = 0;
        let mut attempts = 0;
        while applied < 50 && attempts < 1000 {
            attempts += 1;
            let Some(set) = gen_cs(&mut rng, &mirrored) else { continue };
            p.apply_changes(&set, &[]).unwrap();
            mirror(&mut mirrored, &set);
            applied += 1;
        }
        prop_assert_eq!(applied, 50);
        let got = p.load_tree().unwrap();
        same_order(&got, &mirrored);
        prop_assert_eq!(got.normalized_for_compare(), mirrored.normalized_for_compare());
        // The incrementally maintained search rows answer like a full save's.
        let (_t2, mut full, _) = base();
        full.save_tree(&mirrored, TODAY).unwrap();
        for q in ["meier", "muller", "novak", "town", "renamed", "note"] {
            let a: std::collections::BTreeSet<_> =
                p.search(q, 500).unwrap().into_iter().map(|h| (h.id, h.kind as u8)).collect();
            let b: std::collections::BTreeSet<_> =
                full.search(q, 500).unwrap().into_iter().map(|h| (h.id, h.kind as u8)).collect();
            prop_assert_eq!(a, b, "query {}", q);
        }
    }
}
