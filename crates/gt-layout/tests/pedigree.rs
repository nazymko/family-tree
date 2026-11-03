mod common;

use common::MapSource;
use gt_layout::{EdgeKind, Layout, Metrics, Node, Rect, pedigree};

fn by_ahnen(l: &Layout, n: u64) -> &Node {
    l.nodes
        .iter()
        .find(|x| x.ahnen == Some(n) && x.dup_of.is_none())
        .expect("ahnen number present")
}

fn full() -> Layout {
    pedigree(
        &MapSource::ahnen_tree(15),
        1,
        4,
        &Metrics::default(),
        10_000,
    )
}

#[test]
fn ahnentafel_numbers() {
    let l = full();
    assert_eq!(l.nodes.len(), 15);
    let mut nums: Vec<u64> = l.nodes.iter().filter_map(|n| n.ahnen).collect();
    nums.sort_unstable();
    assert_eq!(nums, (1..=15).collect::<Vec<_>>());
    // Node 6 is the father (upper slot) of person 3.
    let six = by_ahnen(&l, 6);
    let three = by_ahnen(&l, 3);
    assert_eq!(three.nav.child, Some(six.id));
    assert_eq!(six.nav.parent, Some(three.id));
    assert_eq!(six.key, 6);
}

#[test]
fn generation_x_positions() {
    for n in &full().nodes {
        assert_eq!(n.rect.x, f32::from(n.r#gen) * 216.0);
    }
}

#[test]
fn sparse_tree_uses_no_empty_slots() {
    let mut s = MapSource::default();
    for k in 1..6 {
        s.parents.insert(k, [Some(k + 1), None]);
    }
    let m = Metrics::default();
    let l = pedigree(&s, 1, 6, &m, 100);
    assert_eq!(l.nodes.len(), 6);
    assert_eq!(l.bounds.h, m.node_h);
}

#[test]
fn parent_centred_between_parents() {
    let l = full();
    for n in 1..=7u64 {
        let a = by_ahnen(&l, n).rect.y;
        let f = by_ahnen(&l, 2 * n).rect.y;
        let mo = by_ahnen(&l, 2 * n + 1).rect.y;
        assert!((a - (f + mo) / 2.0).abs() < 1e-3, "node {n}");
    }
}

#[test]
fn collapse_emits_dup() {
    // Person 5 is a parent of both 2 and 3.
    let mut s = MapSource::default();
    s.parents.insert(1, [Some(2), Some(3)]);
    s.parents.insert(2, [Some(4), Some(5)]);
    s.parents.insert(3, [Some(6), Some(5)]);
    let l = pedigree(&s, 1, 4, &Metrics::default(), 100);
    assert_eq!(l.nodes.iter().filter(|n| n.dup_of.is_some()).count(), 1);
    assert_eq!(
        l.edges.iter().filter(|e| e.kind == EdgeKind::Dup).count(),
        1
    );
    let dup = l.nodes.iter().find(|n| n.dup_of.is_some()).expect("dup");
    let orig = by_ahnen(&l, 5);
    assert_eq!(dup.dup_of, Some(orig.id));
    assert_eq!(dup.key, 5);
}

#[test]
fn generation_limit_flags_more_up() {
    let l = pedigree(&MapSource::ahnen_tree(15), 1, 2, &Metrics::default(), 100);
    assert_eq!(l.nodes.len(), 3);
    assert!(!l.nodes[0].more_up);
    assert!(l.nodes[1..].iter().all(|n| n.more_up));
    assert!(!l.truncated);
}

#[test]
fn cycle_terminates() {
    let mut s = MapSource::default();
    s.parents.insert(1, [Some(2), None]);
    s.parents.insert(2, [Some(1), None]);
    let l = pedigree(&s, 1, 10, &Metrics::default(), 1000);
    assert_eq!(l.nodes.len(), 3);
    assert_eq!(l.nodes.iter().filter(|n| n.dup_of.is_some()).count(), 1);
}

#[test]
fn root_without_parents() {
    let m = Metrics::default();
    let l = pedigree(&MapSource::default(), 7, 5, &m, 100);
    assert_eq!(l.nodes.len(), 1);
    assert!(l.bounds.w >= m.node_w && l.bounds.h >= m.node_h);
    assert!(!l.nodes[0].more_up);
}

#[test]
fn nav_links_consistent() {
    let l = full();
    for n in &l.nodes {
        let Some(p) = n.nav.parent else { continue };
        let mut cur = l.nodes[p as usize].nav.child;
        let mut found = false;
        while let Some(c) = cur {
            if c == n.id {
                found = true;
                break;
            }
            cur = l.nodes[c as usize].nav.next;
        }
        assert!(found, "node {} not reachable from parent {p}", n.id);
    }
}

#[test]
fn max_nodes_truncates() {
    let l = pedigree(&MapSource::ahnen_tree(15), 1, 4, &Metrics::default(), 5);
    assert!(l.nodes.len() <= 5);
    assert!(l.truncated);
}

#[test]
fn query_returns_visible_nodes_only() {
    let l = full();
    let all = Rect {
        x: -10.0,
        y: -10.0,
        w: 10_000.0,
        h: 10_000.0,
    };
    let (n, e) = l.query(&all);
    assert_eq!(n.len(), 15);
    assert_eq!(e.len(), l.edges.len());
    let first_col = Rect {
        x: 0.0,
        y: -10.0,
        w: 100.0,
        h: 10_000.0,
    };
    assert_eq!(l.query(&first_col).0.len(), 1);
}

#[test]
fn edges_are_orthogonal() {
    let mut s = MapSource::ahnen_tree(15);
    s.parents.insert(5, [Some(2), None]); // collapse
    for e in &full().edges {
        assert!(e.pts.len() >= 4 && e.pts.len() % 2 == 0);
    }
    let l = pedigree(&s, 1, 4, &Metrics::default(), 100);
    for e in &l.edges {
        for w in e.pts.chunks_exact(2).collect::<Vec<_>>().windows(2) {
            assert!(w[0][0] == w[1][0] || w[0][1] == w[1][1]);
        }
    }
}
