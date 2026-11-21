#![allow(clippy::unwrap_used)]
//! The compact print layout of the chart built around one person: long runs of leaf boxes (no partner, nothing
//! below) fold into a grid of sub-rows under one bracket, so a big chart comes closer to a sheet of paper.

mod common;

use std::collections::HashSet;

use common::MapSource;
use gt_layout::{
    AroundBase, BranchDepth, EdgeKind, Key, Layout, ManyKin, Metrics, OpenSpec, around,
    around_all_kin, around_many, around_many_kin, around_open, around_with_siblings,
};

const ALL: u8 = 255;

/// The fixture of `around.rs`: Ivan (1) with parents 2 and 3, siblings 4 and 5, grandparents 6, 7, 8, 9, the
/// father's brother 10 with a child 11, partner 20, children 30 and 31, 30's partner 40 with child 50, 40's parents 60
/// and 61.
fn around_family() -> MapSource {
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

/// The fixture of `around_open.rs`.
fn open_family() -> MapSource {
    let mut s = MapSource::default();
    s.family(Some(60), Some(61), &[6, 62]);
    s.family(Some(6), Some(7), &[2, 10]);
    s.family(Some(8), Some(9), &[3]);
    s.family(Some(2), Some(3), &[1, 4]);
    s.family(Some(1), Some(20), &[30, 31]);
    s.family(Some(30), Some(40), &[50]);
    s.family(Some(4), Some(70), &[71]);
    s.family(Some(71), None, &[72]);
    s.family(Some(10), Some(11), &[12, 13]);
    s.family(Some(12), Some(14), &[15]);
    s.family(Some(15), None, &[16]);
    s.family(Some(62), Some(63), &[64]);
    s.family(Some(64), None, &[65]);
    s.births.extend([(30, 1), (31, 2), (4, 5), (1, 3)]);
    s
}

fn sib(key: Key, d: BranchDepth) -> OpenSpec {
    OpenSpec {
        key,
        siblings: Some(d),
        children: None,
    }
}

fn kids(key: Key, d: BranchDepth) -> OpenSpec {
    OpenSpec {
        key,
        siblings: None,
        children: Some(d),
    }
}

/// The fixture of `around.rs` for wide families: parents 2 and 3 (with their own parents 6/7 and 8/9) over person 1
/// and the siblings 4 (older) and 5 (younger); `wide_for` gets twelve children.
fn wide_siblings_family(wide_for: &[Key]) -> MapSource {
    let mut s = MapSource::default();
    s.family(Some(6), Some(7), &[2]);
    s.family(Some(8), Some(9), &[3]);
    s.family(Some(2), Some(3), &[4, 1, 5]);
    s.births.extend([(4, 1), (1, 2), (5, 3)]);
    s.family(Some(1), None, &[30, 31]);
    let mut next = 1_000;
    for &w in wide_for {
        let kids: Vec<Key> = (next..next + 12).collect();
        next += 12;
        s.family(Some(w), None, &kids);
    }
    s
}

/// Everything that is drawn: every card and every line.
fn assert_same(a: &Layout, b: &Layout) {
    assert_eq!(a.nodes, b.nodes);
    assert_eq!(a.edges, b.edges);
    assert_eq!(a.bounds, b.bounds);
}

fn node(l: &Layout, key: Key) -> &gt_layout::Node {
    l.nodes
        .iter()
        .find(|n| n.key == key && n.dup_of.is_none())
        .unwrap_or_else(|| panic!("key {key} placed"))
}

fn assert_no_overlap(l: &Layout) {
    for (i, a) in l.nodes.iter().enumerate() {
        for b in &l.nodes[i + 1..] {
            let apart = a.rect.x + a.rect.w <= b.rect.x
                || b.rect.x + b.rect.w <= a.rect.x
                || a.rect.y + a.rect.h <= b.rect.y
                || b.rect.y + b.rect.h <= a.rect.y;
            assert!(apart, "nodes {} and {} overlap", a.key, b.key);
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
                assert!(!hit, "edge {}->{} crosses card {}", e.from, e.to, n.key);
            }
        }
    }
}

/// Every child of `parent` on the chart has a parent line from `parent`.
fn assert_children_linked(l: &Layout, parent: Key, children: &[Key]) {
    let p = node(l, parent).id;
    for &c in children {
        let id = node(l, c).id;
        assert!(
            l.edges
                .iter()
                .any(|e| e.kind == EdgeKind::Parent && e.from == p && e.to == id),
            "child {c} has a line from {parent}"
        );
    }
}

fn distinct_ys(l: &Layout, keys: &[Key]) -> usize {
    let ys: HashSet<u32> = keys.iter().map(|&k| node(l, k).rect.y.to_bits()).collect();
    ys.len()
}

/// Ivan (1) with partner 20 and `n` children 100.. (no partners, no children), parents 2 and 3.
fn with_children(n: u32) -> (MapSource, Vec<Key>) {
    let mut s = MapSource::default();
    s.family(Some(2), Some(3), &[1]);
    let kids: Vec<Key> = (100..100 + n).collect();
    s.family(Some(1), Some(20), &kids);
    for (i, &k) in kids.iter().enumerate() {
        s.births.insert(k, i as i32);
    }
    (s, kids)
}

#[test]
fn fold_off_is_today() {
    let m = Metrics::default();
    for s in [around_family(), open_family()] {
        // Nothing opened: exactly the charts of the unchanged functions, folded or not (nothing here is long enough).
        let old = [
            (AroundBase::Line, around(&s, 1, ALL, &m, 100_000)),
            (
                AroundBase::Siblings,
                around_with_siblings(&s, 1, ALL, &m, 100_000),
            ),
            (AroundBase::All, around_all_kin(&s, 1, ALL, &m, 100_000)),
        ];
        for (base, old) in &old {
            for fold in [false, true] {
                let new = around_open(&s, 1, ALL, *base, &[], fold, &m, 100_000);
                assert_same(&new, old);
            }
        }
        // Openings: the fold makes no difference where no run is long enough.
        let opens: [&[OpenSpec]; 4] = [
            &[sib(6, BranchDepth::All)],
            &[sib(2, BranchDepth::Children), kids(12, BranchDepth::All)],
            &[
                sib(1, BranchDepth::All),
                sib(2, BranchDepth::All),
                sib(6, BranchDepth::All),
            ],
            &[sib(6, BranchDepth::Children)],
        ];
        for base in [AroundBase::Line, AroundBase::Siblings, AroundBase::All] {
            for open in opens {
                let off = around_open(&s, 1, ALL, base, open, false, &m, 100_000);
                let on = around_open(&s, 1, ALL, base, open, true, &m, 100_000);
                assert_same(&on, &off);
            }
        }
        for kin in [ManyKin::Line, ManyKin::Siblings, ManyKin::All] {
            for chosen in [&[1][..], &[1, 20][..], &[1, 4][..]] {
                let off = around_many_kin(&s, chosen, ALL, kin, false, &m, 100_000);
                let on = around_many_kin(&s, chosen, ALL, kin, true, &m, 100_000);
                assert_same(&on, &off);
            }
        }
        let line = around_many(&s, &[1, 4], ALL, &m, 100_000);
        let off = around_many_kin(&s, &[1, 4], ALL, ManyKin::Line, false, &m, 100_000);
        assert_same(&line, &off);
    }
    // The wide families: unfolded, exactly today's charts; folded, the twelve children fold and the chart stays clean.
    for wide in [&[4][..], &[5][..], &[1][..], &[4, 5][..], &[4, 1, 5][..]] {
        let s = wide_siblings_family(wide);
        let old = [
            (AroundBase::Line, around(&s, 1, ALL, &m, 100_000)),
            (
                AroundBase::Siblings,
                around_with_siblings(&s, 1, ALL, &m, 100_000),
            ),
            (AroundBase::All, around_all_kin(&s, 1, ALL, &m, 100_000)),
        ];
        for (base, old) in &old {
            assert_same(
                &around_open(&s, 1, ALL, *base, &[], false, &m, 100_000),
                old,
            );
            let on = around_open(&s, 1, ALL, *base, &[], true, &m, 100_000);
            assert_eq!(on.nodes.len(), old.nodes.len());
            assert_no_overlap(&on);
            assert_lines_clear_of_cards(&on);
            common::assert_connectors_readable(&on);
            for &w in wide {
                if on.nodes.iter().any(|n| n.key == w) {
                    let i = wide.iter().position(|&x| x == w).unwrap() as Key;
                    let kids: Vec<Key> = (1000 + 12 * i..1012 + 12 * i).collect();
                    assert!(distinct_ys(&on, &kids) >= 2, "children of {w} fold");
                    assert_children_linked(&on, w, &kids);
                }
            }
        }
    }
}

#[test]
fn small_runs_do_not_fold() {
    let (s, kids) = with_children(5);
    let m = Metrics::default();
    for base in [AroundBase::Line, AroundBase::Siblings, AroundBase::All] {
        let off = around_open(&s, 1, ALL, base, &[], false, &m, 100_000);
        let on = around_open(&s, 1, ALL, base, &[], true, &m, 100_000);
        assert_same(&on, &off);
        assert_eq!(distinct_ys(&on, &kids), 1);
    }
    let off = around_many_kin(&s, &[1], ALL, ManyKin::Siblings, false, &m, 100_000);
    let on = around_many_kin(&s, &[1], ALL, ManyKin::Siblings, true, &m, 100_000);
    assert_same(&on, &off);
}

#[test]
fn big_leaf_run_folds() {
    let (s, kids) = with_children(18);
    let m = Metrics::default();
    for base in [AroundBase::Line, AroundBase::Siblings, AroundBase::All] {
        let off = around_open(&s, 1, ALL, base, &[], false, &m, 100_000);
        let l = around_open(&s, 1, ALL, base, &[], true, &m, 100_000);
        assert_eq!(distinct_ys(&off, &kids), 1);
        assert!(
            distinct_ys(&l, &kids) >= 2,
            "the 18 children fold into sub-rows"
        );
        assert!(l.bounds.w < off.bounds.w, "the folded chart is narrower");
        assert_no_overlap(&l);
        assert_lines_clear_of_cards(&l);
        assert_children_linked(&l, 1, &kids);
        assert_children_linked(&l, 20, &kids);
        common::assert_connectors_readable(&l);
        // The children keep their order: sub-row by sub-row, left to right.
        let mut order: Vec<Key> = kids.clone();
        order.sort_by(|&a, &b| {
            let (ra, rb) = (node(&l, a).rect, node(&l, b).rect);
            ra.y.total_cmp(&rb.y).then(ra.x.total_cmp(&rb.x))
        });
        assert_eq!(order, kids);
    }
    // The same run under several chosen people.
    let off = around_many_kin(&s, &[1], ALL, ManyKin::Line, false, &m, 100_000);
    let l = around_many_kin(&s, &[1], ALL, ManyKin::Line, true, &m, 100_000);
    assert_eq!(distinct_ys(&off, &kids), 1);
    assert!(distinct_ys(&l, &kids) >= 2);
    assert_no_overlap(&l);
    assert_lines_clear_of_cards(&l);
    assert_children_linked(&l, 1, &kids);
}

/// Ivan (1) with partner 20 and `n` children 100..; the children at `big` have a partner (200 + i) and a child
/// (300 + i).
fn mixed_children(n: u32, big: &[usize]) -> (MapSource, Vec<Key>) {
    let (mut s, kids) = with_children(n);
    for &i in big {
        let k = kids[i];
        s.family(Some(k), Some(200 + i as Key), &[300 + i as Key]);
    }
    (s, kids)
}

#[test]
fn non_leaves_keep_place() {
    let m = Metrics::default();
    for n in [12, 20] {
        let (s, kids) = mixed_children(n, &[3, 9]);
        let (a, b) = (kids[3], kids[9]);
        for base in [AroundBase::Line, AroundBase::Siblings, AroundBase::All] {
            let off = around_open(&s, 1, ALL, base, &[], false, &m, 100_000);
            let l = around_open(&s, 1, ALL, base, &[], true, &m, 100_000);
            assert!(node(&off, a).rect.x < node(&off, b).rect.x);
            assert!(node(&l, a).rect.x < node(&l, b).rect.x);
            // The non-leaves stay in the children's own row.
            assert_eq!(node(&l, a).rect.y, node(&l, kids[0]).rect.y);
            assert_eq!(node(&l, b).rect.y, node(&l, kids[0]).rect.y);
            assert_no_overlap(&l);
            assert_lines_clear_of_cards(&l);
            assert_children_linked(&l, 1, &kids);
            common::assert_connectors_readable(&l);
            let leaves: Vec<Key> = kids[10..].to_vec();
            if n == 12 {
                // Runs of 3, 5 and 2: none is long enough.
                assert_same(&l, &off);
            } else {
                // The run of 10 after the second non-leaf folds; the shorter ones do not.
                assert!(distinct_ys(&l, &leaves) >= 2);
                assert_eq!(distinct_ys(&l, &kids[4..9]), 1);
            }
        }
    }
}

/// Ivan (1) with 4 generations of ancestors (Ahnentafel keys 2..31); every ancestor below the top row and Ivan have
/// 12 to 20 brothers and sisters without partners or children.
fn big_chart() -> (MapSource, Vec<OpenSpec>) {
    let mut s = MapSource::default();
    let mut open = Vec::new();
    for n in 1..16u32 {
        let count = 12 + (n * 7) % 9;
        let mut kids: Vec<Key> = (0..count).map(|i| 1000 + n * 100 + i).collect();
        kids.insert(kids.len() / 2, n);
        s.family(Some(2 * n), Some(2 * n + 1), &kids);
        open.push(sib(n, BranchDepth::All));
    }
    (s, open)
}

fn aspect(l: &Layout) -> f32 {
    let (w, h) = (l.bounds.w, l.bounds.h);
    w.max(h) / w.min(h)
}

#[test]
fn big_chart_closer_to_a3() {
    let (s, open) = big_chart();
    let m = Metrics::default();
    let off = around_open(&s, 1, 4, AroundBase::Line, &open, false, &m, 100_000);
    let on = around_open(&s, 1, 4, AroundBase::Line, &open, true, &m, 100_000);
    assert!(off.nodes.len() >= 250, "{} people", off.nodes.len());
    assert_eq!(on.nodes.len(), off.nodes.len());
    let a3 = std::f32::consts::SQRT_2;
    let (a_off, a_on) = (aspect(&off), aspect(&on));
    assert!(a_on < a_off, "aspect folded {a_on} unfolded {a_off}");
    assert!(
        (a_on - a3).abs() < (a_off - a3).abs(),
        "aspect folded {a_on} unfolded {a_off}"
    );
    eprintln!("big chart: aspect {a_off} -> {a_on}");
    assert_no_overlap(&on);
    assert_lines_clear_of_cards(&on);
    common::assert_connectors_readable(&on);
    for n in 1..16u32 {
        let sibs = big_sibs(n);
        assert_children_linked(&on, 2 * n, &sibs);
        assert_children_linked(&on, 2 * n + 1, &sibs);
    }
}

/// The siblings of ancestor `n` in [`big_chart`].
fn big_sibs(n: u32) -> Vec<Key> {
    let count = 12 + (n * 7) % 9;
    (0..count).map(|i| 1000 + n * 100 + i).collect()
}

fn right(l: &Layout, keys: &[Key]) -> f32 {
    keys.iter()
        .map(|&k| node(l, k).rect.x + node(l, k).rect.w)
        .fold(f32::NEG_INFINITY, f32::max)
}

fn left(l: &Layout, keys: &[Key]) -> f32 {
    keys.iter()
        .map(|&k| node(l, k).rect.x)
        .fold(f32::INFINITY, f32::min)
}

#[test]
fn which_groups_fold() {
    let (s, open) = big_chart();
    let m = Metrics::default();
    let l = around_open(&s, 1, 4, AroundBase::Line, &open, true, &m, 100_000);
    // Block 0: the centred person's own brothers and sisters.
    assert!(
        distinct_ys(&l, &big_sibs(1)) >= 2,
        "the person's siblings fold"
    );
    // Outer groups (on the side away from the middle): the father's (2) siblings left of him, the mother's (3) right
    // of her, and so on up the line.
    assert!(right(&l, &big_sibs(2)) < node(&l, 2).rect.x);
    assert!(left(&l, &big_sibs(3)) > node(&l, 3).rect.x);
    for n in [2, 3, 4, 7, 8, 10, 13, 15] {
        assert!(distinct_ys(&l, &big_sibs(n)) >= 2, "siblings of {n} fold");
    }
    // Inner groups stand between their ancestor's couple and the couple's child: folded, their grid would stand in the
    // way of the line to the child, so they stay in one row, in their place (the father's mother 5 has hers on her
    // right, the mother's father 6 on his left).
    for n in [5, 6, 9, 11, 12, 14] {
        assert_eq!(
            distinct_ys(&l, &big_sibs(n)),
            1,
            "siblings of {n} stay in one row"
        );
    }
    assert!(left(&l, &big_sibs(5)) > node(&l, 5).rect.x);
    assert!(right(&l, &big_sibs(6)) < node(&l, 6).rect.x);
}

#[test]
fn several_people_fold_too() {
    let (s, _) = big_chart();
    let m = Metrics::default();
    // Ivan (1) and his first brother (1100) chosen, with all relatives: the chosen row's other brothers and sisters
    // and every ancestor's brothers and sisters fold.
    let chosen = [1, 1100];
    for kin in [ManyKin::Siblings, ManyKin::All] {
        let off = around_many_kin(&s, &chosen, 4, kin, false, &m, 100_000);
        let on = around_many_kin(&s, &chosen, 4, kin, true, &m, 100_000);
        assert_eq!(on.nodes.len(), off.nodes.len());
        assert_no_overlap(&on);
        assert_lines_clear_of_cards(&on);
        common::assert_connectors_readable(&on);
        let rest: Vec<Key> = big_sibs(1)[1..].to_vec();
        assert!(
            distinct_ys(&on, &rest) >= 2,
            "the chosen row's siblings fold"
        );
        assert_children_linked(&on, 2, &big_sibs(1));
        assert_children_linked(&on, 3, &big_sibs(1));
        let a3 = std::f32::consts::SQRT_2;
        assert!(
            (aspect(&on) - a3).abs() < (aspect(&off) - a3).abs(),
            "{kin:?}: aspect folded {} unfolded {}",
            aspect(&on),
            aspect(&off)
        );
        if kin == ManyKin::All {
            // Some of the ancestors' brothers and sisters fold (the others would add crossing lines).
            let folded = (2..16u32)
                .filter(|&n| distinct_ys(&on, &big_sibs(n)) >= 2)
                .count();
            assert!(folded > 0, "some ancestors' siblings fold");
            for n in 2..16u32 {
                assert_children_linked(&on, 2 * n, &big_sibs(n));
                assert_children_linked(&on, 2 * n + 1, &big_sibs(n));
            }
            let (q0, q1) = (gt_layout::quality(&off), gt_layout::quality(&on));
            assert!(q1.line_crossings <= q0.line_crossings);
            eprintln!(
                "several people, all: aspect {} -> {}, {folded} of 14 ancestor groups fold",
                aspect(&off),
                aspect(&on)
            );
        }
    }
}

/// Ivan (1) with 3 generations of ancestors (7 couples, Ahnentafel keys 2..15); Ivan and the ancestors below the top
/// row have 14 brothers and sisters without partners or children.
fn three_generations() -> (MapSource, Vec<OpenSpec>) {
    let mut s = MapSource::default();
    let mut open = Vec::new();
    for n in 1..8u32 {
        let mut kids: Vec<Key> = (0..14).map(|i| 1000 + n * 100 + i).collect();
        kids.insert(7, n);
        s.family(Some(2 * n), Some(2 * n + 1), &kids);
        open.push(sib(n, BranchDepth::All));
    }
    (s, open)
}

/// Folding never adds a crossing line, a line through a card or an overlap, for the chart around one person and for
/// the chart of several people.
#[test]
fn folding_adds_no_crossing() {
    let m = Metrics::default();
    let check = |what: &str, off: &Layout, on: &Layout| {
        let (q0, q1) = (gt_layout::quality(off), gt_layout::quality(on));
        assert!(
            q1.line_crossings <= q0.line_crossings,
            "{what}: crossings folded {} unfolded {}",
            q1.line_crossings,
            q0.line_crossings
        );
        assert!(
            q1.lines_through_boxes <= q0.lines_through_boxes,
            "{what}: {q1:?}"
        );
        assert_eq!(q1.box_overlaps, 0, "{what}");
    };
    let (big, big_open) = big_chart();
    let (three, three_open) = three_generations();
    let mut cases: Vec<(String, MapSource, Vec<OpenSpec>, u8)> = vec![
        (
            "around".into(),
            around_family(),
            vec![sib(2, BranchDepth::All)],
            ALL,
        ),
        (
            "open".into(),
            open_family(),
            vec![sib(6, BranchDepth::All)],
            ALL,
        ),
        ("5 children".into(), with_children(5).0, Vec::new(), ALL),
        ("18 children".into(), with_children(18).0, Vec::new(), ALL),
        (
            "12 mixed".into(),
            mixed_children(12, &[3, 9]).0,
            Vec::new(),
            ALL,
        ),
        (
            "20 mixed".into(),
            mixed_children(20, &[3, 9]).0,
            Vec::new(),
            ALL,
        ),
        ("three generations".into(), three, three_open, 3),
        ("big chart".into(), big, big_open, 4),
    ];
    for wide in [&[4][..], &[5][..], &[1][..], &[4, 5][..], &[4, 1, 5][..]] {
        cases.push((
            format!("wide {wide:?}"),
            wide_siblings_family(wide),
            Vec::new(),
            ALL,
        ));
    }
    for (name, s, open, gens) in &cases {
        for base in [AroundBase::Line, AroundBase::Siblings, AroundBase::All] {
            for open in [&[][..], &open[..]] {
                let off = around_open(s, 1, *gens, base, open, false, &m, 100_000);
                let on = around_open(s, 1, *gens, base, open, true, &m, 100_000);
                check(&format!("{name} {base:?} open {}", open.len()), &off, &on);
            }
        }
        for kin in [ManyKin::Siblings, ManyKin::All] {
            let off = around_many_kin(s, &[1], *gens, kin, false, &m, 100_000);
            let on = around_many_kin(s, &[1], *gens, kin, true, &m, 100_000);
            check(&format!("{name} {kin:?} [1]"), &off, &on);
        }
    }
    // Two chosen people: Ivan and his first brother.
    for (name, s, gens) in [
        ("three generations", three_generations().0, 3),
        ("big chart", big_chart().0, 4),
    ] {
        let off = around_many_kin(&s, &[1, 1100], gens, ManyKin::All, false, &m, 100_000);
        let on = around_many_kin(&s, &[1, 1100], gens, ManyKin::All, true, &m, 100_000);
        check(&format!("{name} All [1, 1100]"), &off, &on);
        let a3 = std::f32::consts::SQRT_2;
        eprintln!(
            "{name} several people: aspect {} -> {}",
            aspect(&off),
            aspect(&on)
        );
        assert!((aspect(&on) - a3).abs() <= (aspect(&off) - a3).abs());
    }
}
