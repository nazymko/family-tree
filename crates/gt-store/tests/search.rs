#![allow(clippy::unwrap_used)]

mod common;

use common::*;
use gt_core::model::*;
use gt_core::tree::Tree;
use gt_store::{HitKind, Project};

const TODAY: i32 = 2_460_000;

fn project_with(people: &[(&str, &str)]) -> (tempfile::TempDir, Project, Vec<gt_core::Uuid>) {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = Project::create(&tmp.path().join("t.genetree")).unwrap();
    let mut tree = Tree::new();
    let ids = people
        .iter()
        .map(|(g, s)| add_person(&mut tree, person(g, s)))
        .collect();
    p.save_tree(&tree, TODAY).unwrap();
    (tmp, p, ids)
}

#[test]
fn search_diacritics() {
    let (_t, p, ids) = project_with(&[("Hans", "Müller"), ("Eva", "Berg")]);
    let hits = p.search("muller", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, ids[0]);
    assert_eq!(hits[0].kind, HitKind::Person);
    assert_eq!(hits[0].label, "Hans Müller");
    // Prefix of a word, any case.
    assert_eq!(p.search("MÜL", 10).unwrap().len(), 1);
}

#[test]
fn search_cyrillic() {
    let (_t, p, ids) = project_with(&[("Иван", "Петров"), ("Anna", "Smith")]);
    let hits = p.search("Иван", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, ids[0]);
    assert_eq!(p.search("иван", 10).unwrap().len(), 1);
}

#[test]
fn search_cjk() {
    let (_t, p, ids) = project_with(&[("", "王小明"), ("", "李大")]);
    let hits = p.search("小明", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, ids[0]);
    assert_eq!(p.search("王小明", 10).unwrap().len(), 1);
}

#[test]
fn search_phonetic() {
    let (_t, p, ids) = project_with(&[("John", "Smith"), ("Jane", "Jones")]);
    let hits = p.search("Smyth", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, ids[0]);
}

#[test]
fn search_ranks_words_before_phonetic_and_dedups() {
    let (_t, p, ids) = project_with(&[("A", "Smyth"), ("B", "Smith")]);
    let hits = p.search("Smyth", 10).unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].id, ids[0]);
    assert_eq!(hits[1].id, ids[1]);
    assert_eq!(p.search("Smyth", 1).unwrap().len(), 1);
}

#[test]
fn search_finds_places_in_person_body_and_other_kinds() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = Project::create(&tmp.path().join("t.genetree")).unwrap();
    let mut tree = rich_tree();
    let _ = &mut tree;
    p.save_tree(&tree, TODAY).unwrap();
    // Karl's event took place in Springfield.
    let hits = p.search("springfield", 20).unwrap();
    let kinds: Vec<_> = hits.iter().map(|h| h.kind).collect();
    assert!(kinds.contains(&HitKind::Place));
    assert!(kinds.contains(&HitKind::Person));
    assert!(
        p.search("baptisms", 10)
            .unwrap()
            .iter()
            .any(|h| h.kind == HitKind::Source)
    );
    assert!(
        p.search("story", 10)
            .unwrap()
            .iter()
            .any(|h| h.kind == HitKind::Note)
    );
}

#[test]
fn search_odd_input_does_not_error() {
    let (_t, p, _ids) = project_with(&[("A", "B")]);
    for q in ["", "   ", "\"", "a\"b", "*", "AND OR", "(", "%", "_"] {
        p.search(q, 5).unwrap();
    }
}

#[test]
fn search_after_replace_has_no_stale_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = Project::create(&tmp.path().join("t.genetree")).unwrap();
    let mut a = Tree::new();
    add_person(&mut a, person("Old", "Name"));
    p.save_tree(&a, TODAY).unwrap();
    let mut b = Tree::new();
    add_person(&mut b, person("New", "Other"));
    p.save_tree(&b, TODAY).unwrap();
    assert!(p.search("Name", 10).unwrap().is_empty());
    assert_eq!(p.search("Other", 10).unwrap().len(), 1);
}

#[test]
fn collation_orders_accents() {
    let (_t, p, _) = project_with(&[]);
    let c = p.conn();
    c.execute_batch(
        "CREATE TEMP TABLE l(label TEXT);
         INSERT INTO l VALUES ('Zoe'), ('Émile'), ('Eva');",
    )
    .unwrap();
    let got: Vec<String> = c
        .prepare("SELECT label FROM l ORDER BY label COLLATE icu_root")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(got, ["Émile", "Eva", "Zoe"]);
    // Plain binary order puts the accented one last: the collation is doing the work.
    let bin: Vec<String> = c
        .prepare("SELECT label FROM l ORDER BY label")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(bin.last().unwrap(), "Émile");
}

#[allow(dead_code)]
fn unused(_: Event) {}
