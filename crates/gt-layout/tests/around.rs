//! The chart built around one person: their own line only, ancestors above and descendants below.

mod common;

use std::collections::HashSet;
use std::time::Instant;

use common::MapSource;
use gt_layout::{EdgeKind, Key, Layout, Metrics, Node, PartnerOf, Rel, around};

const ALL: u8 = 255;

fn run(s: &MapSource, root: Key) -> Layout {
    around(s, root, ALL, &Metrics::default(), 100_000)
}

fn node_of(l: &Layout, key: Key) -> &Node {
    l.nodes
        .iter()
        .find(|n| n.key == key && n.dup_of.is_none())
        .unwrap_or_else(|| panic!("key {key} placed"))
}

fn keys(l: &Layout) -> HashSet<Key> {
    l.nodes.iter().map(|n| n.key).collect()
}

fn cx(n: &Node) -> f32 {
    n.rect.x + n.rect.w / 2.0
}

fn assert_no_overlap(l: &Layout) {
    for (i, a) in l.nodes.iter().enumerate() {
        for b in &l.nodes[i + 1..] {
            let apart = a.rect.x + a.rect.w <= b.rect.x
                || b.rect.x + b.rect.w <= a.rect.x
                || a.rect.y + a.rect.h <= b.rect.y
                || b.rect.y + b.rect.h <= a.rect.y;
            assert!(apart, "nodes {} and {} overlap", a.id, b.id);
        }
    }
}

/// No connector passes through a card other than its own two ends.
fn assert_lines_clear_of_cards(l: &Layout) {
    for e in &l.edges {
        if e.kind == EdgeKind::Dup {
            continue;
        }
        let pts: Vec<&[f32]> = e.pts.chunks_exact(2).collect();
        for seg in pts.windows(2) {
            let (x0, x1) = (seg[0][0].min(seg[1][0]), seg[0][0].max(seg[1][0]));
            let (y0, y1) = (seg[0][1].min(seg[1][1]), seg[0][1].max(seg[1][1]));
            for n in &l.nodes {
                if n.id == e.from || n.id == e.to {
                    continue;
                }
                let r = n.rect;
                let hit = x1 > r.x && x0 < r.x + r.w && y1 > r.y && y0 < r.y + r.h;
                assert!(!hit, "edge {}->{} crosses card {}", e.from, e.to, n.id);
            }
        }
    }
}

/// Person 1 (Ivan) with parents 2 (father) and 3 (mother); his siblings 4 and 5; father's parents 6 and 7;
/// mother's parents 8 and 9; the father's brother 10 with a child 11 (a cousin); Ivan's partner 20 and children
/// 30 and 31; 30's partner 40 with child 50 (a grandchild); 40's parents 60 and 61 (in-laws' parents).
fn family() -> MapSource {
    let mut s = MapSource::default();
    s.family(Some(6), Some(7), &[2, 10]);
    s.family(Some(8), Some(9), &[3]);
    s.family(Some(2), Some(3), &[1, 4, 5]);
    s.family(Some(10), None, &[11]);
    s.family(Some(1), Some(20), &[30, 31]);
    s.family(Some(30), Some(40), &[50]);
    s.family(Some(60), Some(61), &[40]);
    s.births.extend([(30, 1), (31, 2)]);
    s
}

#[test]
fn shows_exactly_the_line_and_the_partners() {
    let l = run(&family(), 1);
    let want: HashSet<Key> = [1, 2, 3, 6, 7, 8, 9, 20, 30, 31, 40, 50].into();
    assert_eq!(keys(&l), want);
    for absent in [4, 5, 10, 11, 60, 61] {
        assert!(!keys(&l).contains(&absent), "{absent} must not be shown");
    }
    assert_eq!(l.nodes.len(), want.len());
}

#[test]
fn ancestors_are_above_in_their_rows_and_descendants_below() {
    let l = run(&family(), 1);
    let me = node_of(&l, 1);
    assert_eq!(me.rel, Some(Rel::Focus));
    let row = |k: Key| node_of(&l, k).rect.y;
    for (k, depth) in [(2, 1), (3, 1), (6, 2), (7, 2), (8, 2), (9, 2)] {
        assert_eq!(node_of(&l, k).rel, Some(Rel::Ancestor(depth)));
        assert!(row(k) < me.rect.y, "{k} above");
    }
    assert_eq!(row(2), row(3));
    assert!(row(6) < row(2) && row(6) == row(9));
    for (k, depth) in [(30, 1), (31, 1), (50, 2)] {
        assert_eq!(node_of(&l, k).rel, Some(Rel::Descendant(depth)));
        assert!(row(k) > me.rect.y, "{k} below");
    }
    assert!(row(50) > row(30));
}

#[test]
fn the_person_stands_between_their_parents_and_their_children() {
    let mut s = MapSource::default();
    s.family(Some(2), Some(3), &[1]);
    s.family(Some(1), None, &[30, 31, 32]);
    let l = run(&s, 1);
    let me = cx(node_of(&l, 1));
    let parents = (cx(node_of(&l, 2)) + cx(node_of(&l, 3))) / 2.0;
    let kids = (cx(node_of(&l, 30)) + cx(node_of(&l, 32))) / 2.0;
    assert!(
        (me - parents).abs() < 0.01,
        "parents centre {parents} vs {me}"
    );
    assert!((me - kids).abs() < 0.01, "children centre {kids} vs {me}");
    // Father on the left, mother on the right, and both lines spread to the sides.
    assert!(cx(node_of(&l, 2)) < me && me < cx(node_of(&l, 3)));
}

#[test]
fn the_person_stays_over_their_parents_with_a_wide_family_below() {
    let mut s = MapSource::default();
    s.family(Some(2), Some(3), &[1]);
    let kids: Vec<Key> = (30..40).collect();
    s.family(Some(1), None, &kids);
    for k in 30..34 {
        s.family(Some(k), None, &[100 + k, 200 + k, 300 + k]);
    }
    let l = run(&s, 1);
    let me = cx(node_of(&l, 1));
    let parents = (cx(node_of(&l, 2)) + cx(node_of(&l, 3))) / 2.0;
    assert!((me - parents).abs() < 0.01);
    assert_no_overlap(&l);
}

#[test]
fn the_person_stays_over_their_parents_with_a_partner() {
    let l = run(&family(), 1);
    let me = cx(node_of(&l, 1));
    let parents = (cx(node_of(&l, 2)) + cx(node_of(&l, 3))) / 2.0;
    assert!(
        (me - parents).abs() < 0.01,
        "parents {parents} vs person {me}"
    );
}

#[test]
fn partners_are_adjacent_and_children_sit_under_the_couple() {
    let l = run(&family(), 1);
    let (me, partner) = (node_of(&l, 1), node_of(&l, 20));
    assert_eq!(partner.rel, Some(Rel::Partner(PartnerOf::Focus)));
    assert_eq!(me.rect.y, partner.rect.y);
    let gap = partner.rect.x - (me.rect.x + me.rect.w);
    assert!((0.0..=Metrics::default().couple_gap + 0.01).contains(&gap));
    let couple = (me.rect.x + partner.rect.x + partner.rect.w) / 2.0;
    // The first child has a partner, so their couple (not just their box) is what is centred.
    let first = (node_of(&l, 30).rect.x + node_of(&l, 40).rect.x + node_of(&l, 40).rect.w) / 2.0;
    let kids = (first + cx(node_of(&l, 31))) / 2.0;
    assert!(
        (couple - kids).abs() < 0.01,
        "couple {couple} vs kids {kids}"
    );
    // A descendant's partner stands beside them, and their child under that couple.
    let (c, cp, g) = (node_of(&l, 30), node_of(&l, 40), node_of(&l, 50));
    assert_eq!(cp.rel, Some(Rel::Partner(PartnerOf::Descendant(1))));
    assert_eq!(c.rect.y, cp.rect.y);
    assert!(cp.rect.x >= c.rect.x + c.rect.w);
    assert!((cx(g) - (c.rect.x + cp.rect.x + cp.rect.w) / 2.0).abs() < 0.01);
}

#[test]
fn nothing_overlaps_and_no_line_crosses_a_card() {
    let l = run(&family(), 1);
    assert_no_overlap(&l);
    assert_lines_clear_of_cards(&l);
    let mut full = MapSource::ahnen_tree(63);
    full.family(Some(1), Some(900), &[800, 801, 802]);
    full.family(Some(800), Some(901), &[700, 701]);
    let l = run(&full, 1);
    assert_no_overlap(&l);
    assert_lines_clear_of_cards(&l);
}

#[test]
fn a_person_with_no_ancestors_or_no_descendants_still_shows() {
    let mut s = MapSource::default();
    s.family(Some(1), None, &[30]);
    let l = run(&s, 1);
    assert_eq!(keys(&l), HashSet::from([1, 30]));
    let mut s = MapSource::default();
    s.family(Some(2), Some(3), &[1]);
    let l = run(&s, 1);
    assert_eq!(keys(&l), HashSet::from([1, 2, 3]));
    let l = run(&MapSource::default(), 1);
    assert_eq!(l.nodes.len(), 1);
}

#[test]
fn a_repeated_ancestor_appears_earlier_instead_of_twice() {
    // Cousin marriage: the parents 2 and 3 share the grandparents 6 and 7.
    let mut s = MapSource::default();
    s.family(Some(6), Some(7), &[2, 3]);
    s.family(Some(2), Some(3), &[1]);
    let l = run(&s, 1);
    let dups: Vec<&Node> = l.nodes.iter().filter(|n| n.dup_of.is_some()).collect();
    assert_eq!(
        dups.len(),
        2,
        "grandparents are drawn once more as references"
    );
    for d in dups {
        assert!(matches!(d.key, 6 | 7));
        let first = &l.nodes[d.dup_of.unwrap() as usize];
        assert_eq!(first.key, d.key);
        assert!(first.dup_of.is_none());
    }
    assert_no_overlap(&l);
}

#[test]
fn a_loop_in_the_data_ends() {
    // 2 is a parent of 1 and 1 a parent of 2; 3 and 1 have 4, who is a parent of 3.
    let mut s = MapSource::default();
    s.family(Some(2), None, &[1]);
    s.family(Some(1), None, &[2]);
    s.family(Some(3), Some(1), &[4]);
    s.family(Some(4), None, &[3]);
    let l = run(&s, 1);
    assert!(l.nodes.len() < 20);
    assert_no_overlap(&l);
    let l = run(&s, 3);
    assert!(l.nodes.len() < 20);
}

#[test]
fn a_generation_limit_marks_the_cut_and_all_does_not_cut() {
    let mut s = MapSource::default();
    // A line of ten ancestors and ten descendants around person 100.
    for k in 0..10u32 {
        s.family(Some(101 + k), None, &[100 + k]);
        s.family(Some(200 + k), None, &[201 + k]);
    }
    s.family(Some(100), None, &[200]);
    let all = around(&s, 100, ALL, &Metrics::default(), 100_000);
    assert_eq!(all.nodes.len(), 22);
    assert!(all.nodes.iter().all(|n| !n.more_up && !n.more_down));
    let two = around(&s, 100, 2, &Metrics::default(), 100_000);
    // Two rows each way: the person, two ancestors and two descendants.
    assert_eq!(two.nodes.len(), 5);
    assert!(two.nodes.iter().any(|n| n.more_up) && two.nodes.iter().any(|n| n.more_down));
}

/// A layout of about 5,000 people stays fluid.
#[test]
#[ignore = "heavy tier"]
fn heavy_around_5000_people_is_fast() {
    let mut s = MapSource::default();
    // Ancestors: a full tree of 1,023 around person 1 (Ahnentafel numbers), plus about 3,900 descendants.
    for n in 1..512u32 {
        s.parents.insert(n, [Some(2 * n), Some(2 * n + 1)]);
    }
    let mut next = 10_000u32;
    let mut frontier = vec![1u32];
    let mut made = 0;
    while made < 3_900 {
        let mut nf = Vec::new();
        for p in frontier {
            let kids: Vec<Key> = (0..3).map(|i| next + i).collect();
            next += 3;
            made += 3;
            s.family(Some(p), None, &kids);
            nf.extend(kids);
        }
        frontier = nf;
    }
    let t = Instant::now();
    let l = around(&s, 1, ALL, &Metrics::default(), 1_000_000);
    let el = t.elapsed();
    assert!(l.nodes.len() > 4_900, "{} nodes", l.nodes.len());
    assert!(el.as_millis() < 1_500, "took {el:?}");
    assert_no_overlap(&l);
}
