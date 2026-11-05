#![allow(clippy::unwrap_used)]

mod common;

use common::*;
use gt_core::model::*;
use gt_core::tree::Tree;
use gt_store::Project;

const TODAY: i32 = 2_460_000; // 2023

fn new_project() -> (tempfile::TempDir, Project) {
    let tmp = tempfile::tempdir().unwrap();
    let p = Project::create(&tmp.path().join("t.genetree")).unwrap();
    (tmp, p)
}

fn living_cached(p: &Project, id: gt_core::Uuid) -> Option<i64> {
    p.conn()
        .query_row(
            "SELECT living_cached FROM person WHERE id = ?1",
            [id.to_string()],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn save_load_roundtrip() {
    let (_tmp, mut p) = new_project();
    let tree = rich_tree();
    p.save_tree(&tree, TODAY).unwrap();
    let back = p.load_tree().unwrap();
    assert_eq!(back, tree);
    // Order, not just content.
    assert_eq!(
        back.persons.keys().collect::<Vec<_>>(),
        tree.persons.keys().collect::<Vec<_>>()
    );
}

#[test]
fn sibling_marker_roundtrips_and_is_not_searched() {
    let (_tmp, mut p) = new_project();
    let mut tree = rich_tree();
    let mut marker = tree.persons.values().next().unwrap().clone();
    marker.common.id = gt_core::ids::new_id();
    marker.names.clear();
    marker.sibling_marker = true;
    let id = marker.common.id;
    tree.persons.insert(id, marker);
    p.save_tree(&tree, TODAY).unwrap();
    let back = p.load_tree().unwrap();
    assert_eq!(back, tree);
    assert!(back.persons[&id].sibling_marker);
    assert_eq!(living_cached(&p, id), Some(0));
    let rows: i64 = p
        .conn()
        .query_row(
            "SELECT count(*) FROM search_words WHERE kind = 'person' AND id = ?1",
            [id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows, 0);
}

#[test]
fn person_json_without_sibling_marker_reads_as_false() {
    // Change-log rows written before schema 3 carry no such field.
    let tree = rich_tree();
    let person = tree.persons.values().next().unwrap();
    let mut json = serde_json::to_value(person).unwrap();
    assert_eq!(json["sibling_marker"], false);
    json.as_object_mut().unwrap().remove("sibling_marker");
    let back: Person = serde_json::from_value(json).unwrap();
    assert!(!back.sibling_marker);
}

#[test]
fn save_load_roundtrip_reopened() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("t.genetree");
    let tree = rich_tree();
    Project::create(&dir)
        .unwrap()
        .save_tree(&tree, TODAY)
        .unwrap();
    assert_eq!(Project::open(&dir).unwrap().load_tree().unwrap(), tree);
}

#[test]
fn save_load_big_synthetic() {
    let (_tmp, mut p) = new_project();
    let tree = big_tree(2_000);
    p.save_tree(&tree, TODAY).unwrap();
    assert_eq!(p.load_tree().unwrap(), tree);
}

#[test]
fn save_replaces() {
    let (_tmp, mut p) = new_project();
    let a = rich_tree();
    let mut b = Tree::new();
    add_person(&mut b, person("Only", "Bee"));
    p.save_tree(&a, TODAY).unwrap();
    p.save_tree(&b, TODAY).unwrap();
    assert_eq!(p.load_tree().unwrap(), b);
    p.save_tree(&a, TODAY).unwrap();
    assert_eq!(p.load_tree().unwrap(), a);
}

#[test]
fn record_order_roundtrips_including_after_vacuum() {
    let (_tmp, mut p) = new_project();
    // Ids are UUID v7 (time ordered); insert in reverse and interleaved with deletes' gaps.
    let mut tree = rich_tree();
    let mut extra = Vec::new();
    for i in 0..30 {
        extra.push(person(&format!("P{i}"), "Order"));
    }
    extra.reverse();
    for e in extra {
        add_person(&mut tree, e);
    }
    p.save_tree(&tree, TODAY).unwrap();
    let want: Vec<_> = tree.persons.keys().copied().collect();
    assert_eq!(
        p.load_tree()
            .unwrap()
            .persons
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        want
    );

    // Save a smaller tree then the big one again so row ids have holes, then VACUUM.
    p.save_tree(&Tree::new(), TODAY).unwrap();
    p.save_tree(&tree, TODAY).unwrap();
    p.conn().execute_batch("VACUUM").unwrap();
    let back = p.load_tree().unwrap();
    assert_eq!(back.persons.keys().copied().collect::<Vec<_>>(), want);
    assert_eq!(back, tree);
    // Every top-level table is stored with a seq equal to its position.
    for t in [
        "person",
        "family",
        "event",
        "place",
        "source",
        "repository",
        "citation",
        "media",
        "note",
    ] {
        let seqs: Vec<i64> = p
            .conn()
            .prepare(&format!("SELECT seq FROM {t} ORDER BY seq"))
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let n = i64::try_from(seqs.len()).unwrap();
        assert_eq!(seqs, (0..n).collect::<Vec<_>>(), "{t}");
    }
}

#[test]
fn living_cached_written() {
    let (_tmp, mut p) = new_project();
    let mut tree = Tree::new();
    let undated = add_person(&mut tree, person("No", "Dates"));
    let old = add_person(&mut tree, person("Old", "Timer"));
    let ev = event(EventKind::Birth, old, "1800", None);
    tree.events.insert(ev.common.id, ev);
    let forced = {
        let mut x = person("Forced", "Dead");
        x.living_override = Some(LivingOverride::Deceased);
        add_person(&mut tree, x)
    };
    p.save_tree(&tree, TODAY).unwrap();
    assert_eq!(living_cached(&p, undated), Some(1));
    assert_eq!(living_cached(&p, old), Some(0));
    assert_eq!(living_cached(&p, forced), Some(0));

    // The rule's age comes from meta "living_max_age".
    p.conn()
        .execute(
            "INSERT INTO meta(key, value) VALUES ('living_max_age', '300')",
            [],
        )
        .unwrap();
    p.save_tree(&tree, TODAY).unwrap();
    assert_eq!(living_cached(&p, old), Some(1));
}

#[test]
fn phonetic_rows_written() {
    let (_tmp, mut p) = new_project();
    let mut tree = Tree::new();
    add_person(&mut tree, person("A", "Smith"));
    p.save_tree(&tree, TODAY).unwrap();
    let n: i64 = p
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM name_phonetic WHERE algo='soundex' AND code='S530'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1);
}

#[test]
fn failed_save_keeps_old_tree() {
    let (_tmp, mut p) = new_project();
    let a = rich_tree();
    p.save_tree(&a, TODAY).unwrap();
    // A family pointing at a person that does not exist violates a foreign key at commit.
    let mut bad = Tree::new();
    let f = Family {
        common: common(id()),
        partner1: Some(id()),
        partner2: None,
        kind: UnionKind::Unknown,
        children: vec![],
    };
    bad.families.insert(f.common.id, f);
    assert!(p.save_tree(&bad, TODAY).is_err());
    assert_eq!(p.load_tree().unwrap(), a);
}

#[test]
#[ignore = "release-mode perf: cargo test -p gt-store --release -- --ignored"]
fn bulk_save_perf() {
    let (_tmp, mut p) = new_project();
    let tree = big_tree(50_000);
    let t0 = std::time::Instant::now();
    p.save_tree(&tree, TODAY).unwrap();
    let took = t0.elapsed();
    eprintln!("saved 50k persons in {took:?}");
    assert!(took.as_secs() < 20, "took {took:?}");
}
