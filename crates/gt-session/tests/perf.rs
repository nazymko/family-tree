#![allow(clippy::unwrap_used)]
//! Budgets from spec 7.4 on a synthetic 50k tree. Run with
//! `cargo test -p gt-session --release -- --ignored`.

use gt_gedcom::synthetic::synthetic_tree;
use gt_session::*;
use gt_store::Project;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const TODAY: i32 = 2_461_343; // 2026-11-03

fn build(dir: &Path) {
    let tree = synthetic_tree(50_000, 15_000);
    let mut p = Project::create(dir).unwrap();
    p.save_tree(&tree, TODAY).unwrap();
}

fn open(dir: &Path) -> (Session, Duration) {
    let t = Instant::now();
    let s = Session::open(dir, SessionClock::fixed(TODAY, 0)).unwrap();
    (s, t.elapsed())
}

/// One 50k project shared by the budget tests (building it takes seconds).
fn fixture() -> PathBuf {
    static DIR: OnceLock<(tempfile::TempDir, PathBuf)> = OnceLock::new();
    DIR.get_or_init(|| {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("big");
        build(&dir);
        (tmp, dir)
    })
    .1
    .clone()
}

#[test]
#[ignore = "release-only budget check"]
fn open_under_2s() {
    let (s, opened) = open(&fixture());
    assert_eq!(s.info().person_count, 50_000);
    assert!(opened < Duration::from_secs(2), "open took {opened:?}");
}

#[test]
#[ignore = "release-only budget check"]
fn first_sort_build_under_300ms() {
    let (s, _) = open(&fixture());
    let t = Instant::now();
    let first = s.list_persons(&PersonQuery::default(), 0, 200);
    let took = t.elapsed();
    assert_eq!(first.items.len(), 200);
    assert!(
        took < Duration::from_millis(300),
        "first sort took {took:?}"
    );
}

#[test]
#[ignore = "release-only budget check"]
fn list_page_under_30ms() {
    let (s, _) = open(&fixture());
    let q = PersonQuery::default();
    let _ = s.list_persons(&q, 0, 200); // builds the order cache
    let t = Instant::now();
    let page = s.list_persons(&q, 20_000, 200);
    let took = t.elapsed();
    assert_eq!(page.items.len(), 200);
    assert!(took < Duration::from_millis(30), "list page took {took:?}");
    let filtered = PersonQuery {
        text: "ann".into(),
        ..PersonQuery::default()
    };
    let t = Instant::now();
    let _ = s.list_persons(&filtered, 0, 200);
    let took = t.elapsed();
    assert!(
        took < Duration::from_millis(30),
        "text filter took {took:?}"
    );
}

#[test]
#[ignore = "release-only budget check"]
fn search_under_100ms() {
    let (s, _) = open(&fixture());
    let t = Instant::now();
    let hits = s.search("Anna", 50).unwrap();
    let took = t.elapsed();
    assert!(!hits.is_empty());
    assert!(took < Duration::from_millis(100), "search took {took:?}");
}
