//! The chart built around one person with all relatives: the brothers and sisters of the person and of every
//! ancestor on the line, with partners and all their descendants.

mod common;

use std::collections::HashSet;
use std::time::Instant;

use common::MapSource;
use gt_layout::{EdgeKind, Key, Layout, Metrics, Node, PartnerOf, Rel, around, around_all_kin};

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

#[test]
fn the_person_stays_in_the_middle_with_every_ancestor_over_their_own_child() {
    let l = run(&family(), 1);
    let mid = |a: Key, b: Key| (cx(node_of(&l, a)) + cx(node_of(&l, b))) / 2.0;
    assert!(
        (mid(2, 3) - cx(node_of(&l, 1))).abs() < 0.01,
        "parents over the person"
    );
    assert!(
        (mid(6, 7) - cx(node_of(&l, 2))).abs() < 0.01,
        "father's parents over the father"
    );
    assert!(
        (mid(8, 9) - cx(node_of(&l, 3))).abs() < 0.01,
        "mother's parents over the mother"
    );
    assert!(
        (mid(60, 61) - cx(node_of(&l, 6))).abs() < 0.01,
        "great-grandparents over 6"
    );
    assert!(
        (mid(80, 81) - cx(node_of(&l, 9))).abs() < 0.01,
        "9's parents over 9"
    );
    assert!(y(&l, 2) < y(&l, 1));
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
