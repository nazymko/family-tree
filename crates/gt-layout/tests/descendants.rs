mod common;

use std::collections::HashSet;

use common::{MapSource, Union};
use gt_layout::{EdgeKind, Layout, Metrics, Node, descendants};
use proptest::prelude::*;

fn run(s: &MapSource, root: u32) -> Layout {
    descendants(s, root, 20, &Metrics::default(), 100_000)
}

fn node_of(l: &Layout, key: u32) -> &Node {
    l.nodes
        .iter()
        .find(|n| n.key == key && n.dup_of.is_none())
        .expect("key placed")
}

fn cx(n: &Node) -> f32 {
    n.rect.x + n.rect.w / 2.0
}

#[test]
fn parent_centred_over_children() {
    let s = MapSource::plain_tree(&[(0, vec![1, 2, 3]), (1, vec![4, 5]), (3, vec![6])]);
    let l = run(&s, 0);
    let root = node_of(&l, 0);
    let first = node_of(&l, 4);
    let last = node_of(&l, 3);
    // Children of the root are 1, 2, 3 (subtrees below 1 and 3 widen the gaps).
    let mid = (cx(node_of(&l, 1)) + cx(last)) / 2.0;
    assert!((cx(root) - mid).abs() < 1e-3);
    let mid1 = (cx(first) + cx(node_of(&l, 5))) / 2.0;
    assert!((cx(node_of(&l, 1)) - mid1).abs() < 1e-3);
    assert_eq!(l.nodes.len(), 7);
}

#[test]
fn couple_adjacent() {
    let mut s = MapSource::default();
    s.unions.insert(0, vec![(Some(100), vec![1, 2])]);
    let m = Metrics::default();
    let l = run(&s, 0);
    let a = node_of(&l, 0);
    let b = node_of(&l, 100);
    assert_eq!(b.rect.x, a.rect.x + m.node_w + m.couple_gap);
    assert_eq!(b.rect.y, a.rect.y);
    assert_eq!(
        l.edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Partner)
            .count(),
        1
    );
    // Children are centred under the couple box.
    let couple_mid = a.rect.x + (m.node_w * 2.0 + m.couple_gap) / 2.0;
    let kids = (cx(node_of(&l, 1)) + cx(node_of(&l, 2))) / 2.0;
    assert!((couple_mid - kids).abs() < 1e-3);
}

#[test]
fn many_unions_one_node() {
    let mut s = MapSource::default();
    let unions: Vec<Union> = (1..=12).map(|i| (Some(100 + i), vec![i])).collect();
    s.unions.insert(0, unions);
    let l = run(&s, 0);
    assert_eq!(l.nodes.iter().filter(|n| n.key == 0).count(), 1);
    assert_eq!(
        l.nodes
            .iter()
            .filter(|n| (101..=112).contains(&n.key))
            .count(),
        12
    );
    assert_eq!(l.nodes.len(), 25);
    let m = Metrics::default();
    let p5 = node_of(&l, 105);
    assert_eq!(
        p5.rect.x,
        node_of(&l, 0).rect.x + 5.0 * (m.node_w + m.couple_gap)
    );
}

#[test]
fn forty_children_width() {
    let kids: Vec<u32> = (1..=40).collect();
    let s = MapSource::plain_tree(&[(0, kids)]);
    let m = Metrics::default();
    let l = run(&s, 0);
    assert!(l.bounds.w >= 40.0 * m.node_w + 39.0 * m.gap_x - 1e-3);
}

#[test]
fn max_nodes_truncates() {
    let kids: Vec<u32> = (1..=100).collect();
    let s = MapSource::plain_tree(&[(0, kids)]);
    let l = descendants(&s, 0, 5, &Metrics::default(), 10);
    assert!(l.nodes.len() <= 10);
    assert!(l.truncated);
    assert!(l.nodes[0].more_down);
}

#[test]
fn generation_limit_flags_more_down() {
    let s = MapSource::plain_tree(&[(0, vec![1]), (1, vec![2]), (2, vec![3])]);
    let l = descendants(&s, 0, 2, &Metrics::default(), 100);
    assert_eq!(l.nodes.len(), 2);
    assert!(!l.nodes[0].more_down);
    assert!(l.nodes[1].more_down);
    assert!(!l.truncated);
}

#[test]
fn cycle_terminates() {
    let s = MapSource::plain_tree(&[(0, vec![1]), (1, vec![0])]);
    let l = run(&s, 0);
    assert_eq!(l.nodes.len(), 3);
    assert_eq!(l.nodes.iter().filter(|n| n.dup_of.is_some()).count(), 1);
    assert_eq!(
        l.edges.iter().filter(|e| e.kind == EdgeKind::Dup).count(),
        1
    );
}

#[test]
fn edges_are_orthogonal_polylines() {
    let mut s = MapSource::default();
    s.unions
        .insert(0, vec![(Some(50), vec![1, 2]), (None, vec![3])]);
    s.unions.insert(1, vec![(Some(51), vec![4, 0])]); // 0 again: dup
    s.unions.insert(3, vec![(Some(52), vec![5])]);
    let l = run(&s, 0);
    assert!(l.edges.iter().any(|e| e.kind == EdgeKind::Dup));
    assert!(l.edges.iter().any(|e| e.kind == EdgeKind::Parent));
    for e in &l.edges {
        assert!(e.pts.len() >= 4 && e.pts.len() % 2 == 0);
        let pts: Vec<&[f32]> = e.pts.chunks_exact(2).collect();
        for w in pts.windows(2) {
            assert!(w[0][0] == w[1][0] || w[0][1] == w[1][1], "{:?}", e.pts);
        }
    }
}

#[test]
fn nav_links_consistent() {
    let s = MapSource::plain_tree(&[(0, vec![1, 2, 3]), (2, vec![4, 5])]);
    let l = run(&s, 0);
    for n in &l.nodes {
        let Some(p) = n.nav.parent else { continue };
        let mut cur = l.nodes[p as usize].nav.child;
        let mut found = false;
        while let Some(c) = cur {
            found |= c == n.id;
            cur = l.nodes[c as usize].nav.next;
        }
        assert!(found);
    }
}

#[test]
fn deep_chain_does_not_overflow() {
    let chain: Vec<(u32, Vec<u32>)> = (0..254).map(|i| (i, vec![i + 1])).collect();
    let l = descendants(
        &MapSource::plain_tree(&chain),
        0,
        255,
        &Metrics::default(),
        100_000,
    );
    assert_eq!(l.nodes.len(), 255);
}

/// (parent choice seed, union shape) for each person after the first.
fn shape() -> impl Strategy<Value = Vec<(usize, u8)>> {
    prop::collection::vec((any::<usize>(), 0u8..3), 1..200)
}

fn build(shape: &[(usize, u8)]) -> MapSource {
    let n = shape.len() + 1;
    let mut kids: Vec<Vec<u32>> = vec![Vec::new(); n];
    for (i, (seed, _)) in shape.iter().enumerate() {
        kids[seed % (i + 1)].push(i as u32 + 1);
    }
    let mut s = MapSource::default();
    for (p, c) in kids.into_iter().enumerate() {
        let mode = if p == 0 { 1 } else { shape[p - 1].1 };
        let partner = 1000 + p as u32;
        let unions: Vec<Union> = match mode {
            0 => vec![(None, c)],
            1 => vec![(Some(partner), c)],
            _ => {
                let at = c.len() / 2;
                vec![
                    (Some(partner), c[..at].to_vec()),
                    (Some(partner + 5000), c[at..].to_vec()),
                ]
            }
        };
        s.unions.insert(p as u32, unions);
    }
    s
}

proptest! {
    #[test]
    fn no_overlap_property(sh in shape()) {
        let m = Metrics::default();
        let l = descendants(&build(&sh), 0, 255, &m, 100_000);
        let partners: HashSet<(u32, u32)> = l.edges.iter()
            .filter(|e| e.kind == EdgeKind::Partner)
            .map(|e| (e.from, e.to)).collect();
        let mut depths: Vec<i16> = l.nodes.iter().map(|n| n.r#gen).collect();
        depths.sort_unstable();
        depths.dedup();
        for d in depths {
            let mut row: Vec<&Node> = l.nodes.iter().filter(|n| n.r#gen == d).collect();
            row.sort_by(|a, b| a.rect.x.total_cmp(&b.rect.x));
            for w in row.windows(2) {
                let sep = w[1].rect.x - (w[0].rect.x + w[0].rect.w);
                let need = if partners.contains(&(w[0].id, w[1].id)) { m.couple_gap } else { m.gap_x };
                prop_assert!(sep >= need - 1e-2, "depth {d}: sep {sep} < {need}");
            }
        }
    }
}

#[test]
fn mirror_vertical_moves_nodes_edges_and_grid() {
    use gt_layout::{Axis, Rect};
    let s = MapSource::plain_tree(&[(0, vec![1, 2]), (1, vec![3])]);
    let l = run(&s, 0);
    let mut m = l.clone();
    m.mirror(Axis::Vertical);
    let (top, bottom) = (l.bounds.y, l.bounds.y + l.bounds.h);
    assert_eq!(m.bounds, l.bounds);
    for (a, b) in l.nodes.iter().zip(&m.nodes) {
        assert_eq!(a.id, b.id);
        assert_eq!(b.rect.x, a.rect.x);
        assert!((b.rect.y - (top + bottom - a.rect.y - a.rect.h)).abs() < 1e-3);
        assert_eq!(a.nav, b.nav);
    }
    // The root was on top, now it is below its children.
    assert!(node_of(&m, 0).rect.y > node_of(&m, 1).rect.y);
    for (a, b) in l.edges.iter().zip(&m.edges) {
        for (p, q) in a.pts.chunks_exact(2).zip(b.pts.chunks_exact(2)) {
            assert_eq!(p[0], q[0]);
            assert!((q[1] - (top + bottom - p[1])).abs() < 1e-3);
        }
    }
    // The spatial index follows: querying where the root now sits finds it, and the old spot does not.
    let at = |n: &Node| Rect {
        x: n.rect.x + 1.0,
        y: n.rect.y + 1.0,
        w: 1.0,
        h: 1.0,
    };
    let root_now = node_of(&m, 0).clone();
    let (nodes, _) = m.query(&at(&root_now));
    assert!(nodes.iter().any(|n| n.key == 0));
    let (nodes, _) = m.query(&at(node_of(&l, 0)));
    assert!(nodes.iter().all(|n| n.key != 0));
    // Mirroring twice restores the original.
    m.mirror(Axis::Vertical);
    for (a, b) in l.nodes.iter().zip(&m.nodes) {
        assert!((a.rect.y - b.rect.y).abs() < 1e-3);
    }
}

#[test]
fn mirror_horizontal_flips_left_right_only() {
    use gt_layout::Axis;
    let s = MapSource::plain_tree(&[(0, vec![1, 2, 3])]);
    let l = run(&s, 0);
    let mut m = l.clone();
    m.mirror(Axis::Horizontal);
    let (left, right) = (l.bounds.x, l.bounds.x + l.bounds.w);
    for (a, b) in l.nodes.iter().zip(&m.nodes) {
        assert_eq!(b.rect.y, a.rect.y);
        assert!((b.rect.x - (left + right - a.rect.x - a.rect.w)).abs() < 1e-3);
    }
}
