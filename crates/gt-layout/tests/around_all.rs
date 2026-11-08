//! The chart built around one person with all relatives: the brothers and sisters of the person and of every
//! ancestor on the line, with partners and all their descendants.

mod common;

use std::collections::HashSet;
use std::time::Instant;

use common::MapSource;
use gt_layout::{
    EdgeKind, Key, Layout, Metrics, Node, PartnerOf, Rel, around, around_all_kin,
    around_with_siblings,
};

const ALL: u8 = 255;

fn run(s: &MapSource, root: Key) -> Layout {
    around_all_kin(s, root, ALL, &Metrics::default(), 100_000)
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

fn y(l: &Layout, k: Key) -> f32 {
    node_of(l, k).rect.y
}

fn assert_no_overlap(l: &Layout) {
    for (i, a) in l.nodes.iter().enumerate() {
        for b in &l.nodes[i + 1..] {
            let apart = a.rect.x + a.rect.w <= b.rect.x
                || b.rect.x + b.rect.w <= a.rect.x
                || a.rect.y + a.rect.h <= b.rect.y
                || b.rect.y + b.rect.h <= a.rect.y;
            assert!(
                apart,
                "nodes {} ({}) and {} ({}) overlap",
                a.id, a.key, b.id, b.key
            );
        }
    }
}

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
                assert!(
                    !hit,
                    "edge {}->{} crosses card {} ({})",
                    e.from, e.to, n.id, n.key
                );
            }
        }
    }
}

fn assert_clean(l: &Layout) {
    assert_no_overlap(l);
    assert_lines_clear_of_cards(l);
}

/// Person 1 with partner 20 and children 30, 31 (30 has partner 40 and a child 50); parents 2 and 3; father's
/// parents 6 and 7; mother's parents 8 and 9; 6's parents 60 and 61; 9's parents 80 and 81.
/// Collaterals: sister 4 (partner 70, child 71, grandchild 72, great-grandchild 73); the father's sister 10
/// (partner 11, children 12 and 13; 12 has partner 14 and child 15, who has child 16); the mother's brother 17
/// (child 18, who has child 19); 6's sister 62 (partner 63, child 64, who has child 65); 9's brother 66.
fn family() -> MapSource {
    let mut s = MapSource::default();
    s.family(Some(60), Some(61), &[6, 62]);
    s.family(Some(6), Some(7), &[2, 10]);
    s.family(Some(8), Some(9), &[3, 17]);
    s.family(Some(2), Some(3), &[1, 4]);
    s.family(Some(1), Some(20), &[30, 31]);
    s.family(Some(30), Some(40), &[50]);
    s.family(Some(4), Some(70), &[71]);
    s.family(Some(71), None, &[72]);
    s.family(Some(72), None, &[73]);
    s.family(Some(10), Some(11), &[12, 13]);
    s.family(Some(12), Some(14), &[15]);
    s.family(Some(15), None, &[16]);
    s.family(Some(17), None, &[18]);
    s.family(Some(18), None, &[19]);
    s.family(Some(62), Some(63), &[64]);
    s.family(Some(64), None, &[65]);
    s.family(Some(80), Some(81), &[9, 66]);
    s.births.extend([(30, 1), (31, 2), (4, 5), (1, 3)]);
    s
}

#[test]
fn shows_every_collateral_relative_and_all_their_descendants() {
    let l = run(&family(), 1);
    let want: HashSet<Key> = [
        1, 2, 3, 6, 7, 8, 9, 60, 61, 80, 81, 20, 30, 31, 40, 50, // the line
        4, 70, 71, 72, 73, // the sister's family, all generations
        10, 11, 12, 13, 14, 15, 16, // the father's sister's family
        17, 18, 19, // the mother's brother's family
        62, 63, 64, 65, 66, // the great-aunt and great-uncle
    ]
    .into();
    assert_eq!(keys(&l), want);
    assert_eq!(l.nodes.len(), want.len());
    assert!(!l.truncated);
    assert_clean(&l);
}

#[test]
fn the_line_only_chart_does_not_show_the_wider_family() {
    let l = around(&family(), 1, ALL, &Metrics::default(), 100_000);
    for k in [4, 10, 12, 17, 62, 66] {
        assert!(!keys(&l).contains(&k), "{k} is not on the line");
    }
}

#[test]
fn everyone_stands_in_their_own_generation_row() {
    let l = run(&family(), 1);
    let me = y(&l, 1);
    // Same row as the person: the sister, the cousins, the great-aunt's grandchild.
    for k in [4, 12, 13, 18, 20, 65] {
        assert_eq!(y(&l, k), me, "{k} in the person's row");
    }
    // The parents' row: parents, aunts, uncles, the great-aunt's child.
    for k in [2, 3, 10, 17, 64, 11] {
        assert_eq!(y(&l, k), y(&l, 2), "{k} in the parents' row");
    }
    // The grandparents' row.
    for k in [6, 7, 8, 9, 62, 66] {
        assert_eq!(y(&l, k), y(&l, 6), "{k} in the grandparents' row");
    }
    assert_eq!(y(&l, 60), y(&l, 80));
    assert!(y(&l, 60) < y(&l, 6));
    // Below: nephew, cousin's child, grandchild.
    for k in [71, 15, 19, 30, 31] {
        assert_eq!(y(&l, k), y(&l, 30), "{k} one row below");
    }
    for k in [72, 16, 50] {
        assert_eq!(y(&l, k), y(&l, 30) + (y(&l, 30) - me), "{k} two rows below");
    }
    assert!(y(&l, 73) > y(&l, 72));
}

/// Every couple of parents of the line: the two stand one card width plus the partner gap apart, in one row.
fn assert_couple_adjacent(l: &Layout, a: Key, b: Key) {
    let m = Metrics::default();
    let (na, nb) = (node_of(l, a), node_of(l, b));
    assert_eq!(na.rect.y, nb.rect.y, "{a} and {b} in one row");
    assert!(
        ((cx(nb) - cx(na)) - (m.node_w + m.couple_gap)).abs() < 0.01,
        "{a} and {b} are next to each other, not {} apart",
        cx(nb) - cx(na)
    );
}

#[test]
fn the_person_stays_in_the_middle_with_each_couple_of_parents_next_to_each_other() {
    let l = run(&family(), 1);
    let mid = |a: Key, b: Key| (cx(node_of(&l, a)) + cx(node_of(&l, b))) / 2.0;
    for (a, b) in [(2, 3), (6, 7), (8, 9), (60, 61), (80, 81)] {
        assert_couple_adjacent(&l, a, b);
    }
    assert!(
        (mid(2, 3) - cx(node_of(&l, 1))).abs() < 0.01,
        "parents over the person"
    );
    // Higher up the couples keep to their side of the person and every ancestor is above their child.
    for (a, b, child) in [(6, 7, 2), (8, 9, 3), (60, 61, 6), (80, 81, 9)] {
        assert!(y(&l, a) < y(&l, child) && y(&l, b) < y(&l, child));
    }
    assert!(
        mid(6, 7) < cx(node_of(&l, 1)),
        "the father's line on the left"
    );
    assert!(
        mid(8, 9) > cx(node_of(&l, 1)),
        "the mother's line on the right"
    );
    assert!(y(&l, 2) < y(&l, 1));
    assert_clean(&l);
}

/// A full pedigree of `depth` generations above person 1 (key 2n and 2n+1 are the parents of n), every ancestor
/// with a brother who has `kids` children, each with `kids` children, and a wide family below the person.
fn wide_pedigree(depth: u32, kids: u32) -> (MapSource, Vec<(Key, Key)>) {
    let mut s = MapSource::default();
    let mut couples = Vec::new();
    let mut next: Key = 10_000;
    let mut fresh = || {
        next += 1;
        next
    };
    let mut line: Vec<Key> = vec![1];
    for _ in 0..depth {
        let mut up = Vec::new();
        for &c in &line {
            let (f, m) = (c * 2, c * 2 + 1);
            let uncle = fresh();
            if c == 1 {
                let sister = fresh();
                s.family(Some(f), Some(m), &[c, uncle, sister]);
            } else {
                s.family(Some(f), Some(m), &[c, uncle]);
            }
            couples.push((f, m));
            // The uncle's family, two generations of it, hangs off him.
            let mut layer = vec![uncle];
            for _ in 0..2 {
                let mut below = Vec::new();
                for &p in &layer {
                    let v: Vec<Key> = (0..kids).map(|_| fresh()).collect();
                    s.family(Some(p), Some(fresh()), &v);
                    below.extend(v);
                }
                layer = below;
            }
            up.push(f);
            up.push(m);
        }
        line = up;
    }
    // The person: a wide line of their own.
    let own: Vec<Key> = (0..kids + 3).map(|_| fresh()).collect();
    s.family(Some(1), Some(fresh()), &own);
    for &c in &own {
        let v: Vec<Key> = (0..kids).map(|_| fresh()).collect();
        s.family(Some(c), Some(fresh()), &v);
    }
    (s, couples)
}

#[test]
fn the_line_is_a_compact_spine_whatever_the_width_of_the_families_around_it() {
    let (s, couples) = wide_pedigree(3, 3);
    let l = run(&s, 1);
    assert!(!l.truncated);
    // Every couple on the line stands side by side, however wide the families below and beside are.
    for &(f, m) in &couples {
        if keys(&l).contains(&f) && keys(&l).contains(&m) {
            assert_couple_adjacent(&l, f, m);
        }
    }
    assert_couple_adjacent(&l, 2, 3);
    let me = node_of(&l, 1);
    assert!((((cx(node_of(&l, 2)) + cx(node_of(&l, 3))) / 2.0) - cx(me)).abs() < 0.01);
    // The father's line is left of the person and the mother's right, every ancestor above their child.
    for &(f, m) in &couples {
        let child = f / 2;
        assert!(y(&l, f) < y(&l, child) && y(&l, m) < y(&l, child));
        let on_left = {
            let mut a = f;
            while a > 3 {
                a /= 2;
            }
            a == 2
        };
        let c = (cx(node_of(&l, f)) + cx(node_of(&l, m))) / 2.0;
        if child != 1 {
            assert_eq!(c < cx(me), on_left, "couple {f}/{m} on its own side");
        }
    }
    assert_clean(&l);
}

#[test]
fn the_families_hang_on_the_outer_side_of_their_ancestor() {
    let l = run(&family(), 1);
    // Father's side to the left, mother's side to the right.
    for k in [10, 12, 15] {
        assert!(
            cx(node_of(&l, k)) < cx(node_of(&l, 2)),
            "{k} left of the father"
        );
    }
    assert!(
        cx(node_of(&l, 62)) < cx(node_of(&l, 6)),
        "the great-aunt is left of her sibling"
    );
    for k in [17, 18, 19] {
        assert!(
            cx(node_of(&l, k)) > cx(node_of(&l, 3)),
            "{k} right of the mother"
        );
    }
    assert!(
        cx(node_of(&l, 66)) > cx(node_of(&l, 9)),
        "the great-uncle is right of his sister"
    );
    assert!(cx(node_of(&l, 10)) < cx(node_of(&l, 1)));
    assert!(cx(node_of(&l, 17)) > cx(node_of(&l, 1)));
}

#[test]
fn relationships_are_named_by_generations_up_and_down() {
    let l = run(&family(), 1);
    let rel = |k| node_of(&l, k).rel.expect("a relation");
    assert_eq!(rel(1), Rel::Focus);
    assert_eq!(rel(4), Rel::Sibling { half: false });
    assert_eq!(rel(71), Rel::NieceNephew);
    assert_eq!(rel(72), Rel::Kin { up: 1, down: 3 });
    assert_eq!(rel(73), Rel::Kin { up: 1, down: 4 });
    assert_eq!(rel(10), Rel::AuntUncle { half: false });
    assert_eq!(rel(12), Rel::Kin { up: 2, down: 2 });
    assert_eq!(rel(15), Rel::Kin { up: 2, down: 3 });
    assert_eq!(rel(62), Rel::Kin { up: 3, down: 1 });
    assert_eq!(rel(64), Rel::Kin { up: 3, down: 2 });
    assert_eq!(rel(65), Rel::Kin { up: 3, down: 3 });
    assert_eq!(rel(66), Rel::Kin { up: 3, down: 1 });
    assert_eq!(rel(70), Rel::Partner(PartnerOf::Sibling));
    assert_eq!(rel(11), Rel::Partner(PartnerOf::AuntUncle));
    assert_eq!(rel(14), Rel::Partner(PartnerOf::Kin { up: 2, down: 2 }));
    assert_eq!(rel(63), Rel::Partner(PartnerOf::Kin { up: 3, down: 1 }));
}

#[test]
fn the_cousins_families_do_not_collide_with_the_persons_descendants() {
    // Wide families on every side and a wide line below the person.
    let mut s = family();
    let mut next: Key = 1000;
    let mut kids = |parent: Key, n: u32, s: &mut MapSource| -> Vec<Key> {
        let v: Vec<Key> = (0..n)
            .map(|_| {
                next += 1;
                next
            })
            .collect();
        s.family(Some(parent), None, &v);
        v
    };
    for p in [12, 13, 18, 64, 71] {
        for c in kids(p, 4, &mut s) {
            kids(c, 3, &mut s);
        }
    }
    for p in [30, 31] {
        for c in kids(p, 5, &mut s) {
            kids(c, 2, &mut s);
        }
    }
    let l = run(&s, 1);
    assert_clean(&l);
    let mid = (cx(node_of(&l, 2)) + cx(node_of(&l, 3))) / 2.0;
    assert!(
        (mid - cx(node_of(&l, 1))).abs() < 0.01,
        "parents still over the person"
    );
    assert!(((cx(node_of(&l, 60)) + cx(node_of(&l, 61))) / 2.0 - cx(node_of(&l, 6))).abs() < 0.01);
}

#[test]
fn a_cousin_marriage_uses_the_appears_earlier_mechanism() {
    // The sister 4 has a child with the cousin 12: 12 is placed twice, the second box points at the first.
    let mut s = family();
    s.family(Some(4), Some(12), &[75]);
    let l = run(&s, 1);
    let twelve: Vec<&Node> = l.nodes.iter().filter(|n| n.key == 12).collect();
    assert_eq!(twelve.len(), 2);
    assert_eq!(twelve.iter().filter(|n| n.dup_of.is_some()).count(), 1);
    assert!(l.edges.iter().any(|e| e.kind == EdgeKind::Dup));
    assert_no_overlap(&l);
}

#[test]
fn a_loop_in_the_data_does_not_hang() {
    // 60 is also a child of 30: the person's own descendant is their ancestor.
    let mut s = family();
    s.family(Some(30), None, &[60]);
    let l = run(&s, 1);
    // The ancestor met again as a descendant is drawn as a repeat of the first box.
    assert!(l.nodes.iter().any(|n| n.dup_of.is_some()));
    assert!(l.edges.iter().any(|e| e.kind == EdgeKind::Dup));
    assert_no_overlap(&l);
    // A sister who is her own great-grandniece's child.
    let mut s = family();
    s.family(Some(73), None, &[4]);
    let l = run(&s, 1);
    assert_no_overlap(&l);
}

#[test]
fn the_node_cap_truncates_and_says_so() {
    let s = family();
    let full = run(&s, 1);
    let l = around_all_kin(&s, 1, ALL, &Metrics::default(), 30);
    assert!(l.truncated);
    assert!(l.nodes.len() <= 30);
    assert!(l.nodes.len() < full.nodes.len());
    // The line itself comes first: parents and grandparents are there.
    for k in [1, 2, 3, 6, 7, 8, 9] {
        assert!(keys(&l).contains(&k), "{k} kept");
    }
    assert_no_overlap(&l);
}

#[test]
fn the_generation_choice_limits_the_line_and_the_families_on_it() {
    let l = around_all_kin(&family(), 1, 1, &Metrics::default(), 100_000);
    for k in [2, 3, 10, 12, 17, 18, 4] {
        assert!(keys(&l).contains(&k), "{k} shown");
    }
    for k in [6, 7, 62, 64, 66] {
        assert!(
            !keys(&l).contains(&k),
            "{k} is beyond the chosen generations"
        );
    }
    assert_clean(&l);
}

#[test]
fn a_lone_parent_and_unknown_parents_still_lay_out() {
    let mut s = MapSource::default();
    s.family(Some(6), None, &[2, 10]);
    s.family(Some(2), None, &[1, 4]);
    s.family(Some(10), None, &[12]);
    s.family(Some(4), None, &[5]);
    let l = run(&s, 1);
    assert_clean(&l);
    for k in [1, 2, 4, 5, 6, 10, 12] {
        assert!(keys(&l).contains(&k));
    }
    // Alone: nothing but the person.
    assert_eq!(run(&MapSource::default(), 1).nodes.len(), 1);
}

/// A deterministic synthetic population of about `n` people in one family: a founding couple, every couple
/// has three children and every child brings an outsider partner. Returns the tree and a person from the
/// middle generation (a child of a couple with grandchildren and great-grandchildren of its own).
fn big_tree(n: u32) -> (MapSource, Key) {
    let mut s = MapSource::default();
    let mut next: Key = 3;
    let mut frontier: Vec<Key> = vec![1, 2];
    let mut generations: Vec<Vec<Key>> = vec![frontier.clone()];
    while next < n {
        let mut grown = Vec::new();
        for pair in frontier.chunks(2) {
            if next >= n {
                break;
            }
            let kids: Vec<Key> = (0..3).map(|i| next + i).collect();
            next += 3;
            s.family(Some(pair[0]), pair.get(1).copied(), &kids);
            for &k in &kids {
                grown.push(k);
                grown.push(next);
                next += 1;
            }
        }
        if grown.is_empty() {
            break;
        }
        generations.push(grown.clone());
        frontier = grown;
    }
    let mid = &generations[generations.len() / 2];
    let root = mid[0];
    (s, root)
}

#[test]
#[ignore = "heavy tier"]
fn heavy_five_thousand_people_with_all_relatives() {
    let (s, root) = big_tree(5_000);
    let t = Instant::now();
    let l = around_all_kin(&s, root, ALL, &Metrics::default(), 20_000);
    let took = t.elapsed();
    eprintln!(
        "all-relatives chart of a 5000-person tree: {} nodes, truncated={}, {:?}",
        l.nodes.len(),
        l.truncated,
        took
    );
    assert!(l.nodes.len() > 1_000, "most of the family is drawn");
    assert!(l.nodes.len() <= 20_000);
    assert!(took.as_secs_f32() < 10.0, "took {took:?}");
    assert_no_overlap(&l);
}

/// Person 1 + 20 have child 30; 1's parents 2 + 3; 2 is a child of family 100 with no partners, whose other children
/// are 90 and 91 (90 has partner 92 and child 93); 3's parents 4 + 5. Person 7 is a child of a family with no partners
/// with 9 and 10, and is the partner of 8.
fn parentless() -> MapSource {
    let mut s = MapSource::default();
    s.family(None, None, &[2, 90, 91]);
    s.family(Some(2), Some(3), &[1]);
    s.family(Some(4), Some(5), &[3]);
    s.family(Some(1), Some(20), &[30]);
    s.family(Some(90), Some(92), &[93]);
    s.family(None, None, &[7, 9, 10]);
    s.family(Some(7), Some(8), &[11]);
    s.births.extend([(2, 1), (90, 2), (91, 3)]);
    s
}

fn bars(l: &Layout) -> usize {
    l.edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Sibling)
        .count()
}

#[test]
fn brothers_and_sisters_in_a_family_without_partners_are_shown_beside_the_person() {
    let s = parentless();
    let l = around_all_kin(&s, 1, ALL, &Metrics::default(), 100_000);
    for k in [90, 91, 92, 93] {
        assert!(keys(&l).contains(&k), "{k} shown");
    }
    assert_eq!(y(&l, 90), y(&l, 2));
    assert_eq!(y(&l, 91), y(&l, 2));
    assert_eq!(bars(&l), 2, "a bar to each brother");
    assert_clean(&l);
    // Their own children are below, linked as parent edges.
    assert!(l.edges.iter().any(|e| e.kind == EdgeKind::Parent
        && l.nodes[e.from as usize].key == 90
        && l.nodes[e.to as usize].key == 93));
    // The chart of the line alone does not show them.
    let line = around(&s, 1, ALL, &Metrics::default(), 100_000);
    assert!(!keys(&line).contains(&90));
}

#[test]
fn own_brothers_and_sisters_through_a_family_without_partners() {
    let s = parentless();
    let l = around_with_siblings(&s, 7, ALL, &Metrics::default(), 100_000);
    for k in [9, 10] {
        assert_eq!(y(&l, k), y(&l, 7), "{k} beside 7");
    }
    assert_eq!(bars(&l), 2);
    assert_clean(&l);
    let o = gt_layout::RelativesOptions {
        generations: 3,
        aunts: true,
    };
    let l = gt_layout::relatives(&s, 7, &o, &Metrics::default(), 100_000);
    assert!(keys(&l).contains(&9) && keys(&l).contains(&10));
    assert_clean(&l);
}

#[test]
fn several_chosen_people_get_the_siblings_of_a_family_without_partners() {
    use gt_layout::{ManyKin, around_many_kin};
    let s = parentless();
    for chosen in [[1, 7], [7, 1], [7, 8], [8, 7]] {
        for kin in [ManyKin::Siblings, ManyKin::All] {
            let l = around_many_kin(&s, &chosen, ALL, kin, &Metrics::default(), 100_000);
            assert_eq!(l.nodes.iter().filter(|n| n.key == 9).count(), 1);
            assert_eq!(l.nodes.iter().filter(|n| n.key == 10).count(), 1);
            if kin == ManyKin::All && chosen.contains(&1) {
                for k in [90, 91, 92, 93] {
                    assert_eq!(l.nodes.iter().filter(|n| n.key == k).count(), 1, "{k}");
                }
                assert_eq!(y(&l, 90), y(&l, 2));
            }
            assert!(bars(&l) >= 2);
            assert_clean(&l);
        }
    }
}
