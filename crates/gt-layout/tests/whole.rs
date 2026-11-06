#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::collections::HashSet;
use std::time::Instant;

use gt_layout::{
    BlockKind, EdgeKind, Metrics, Rect, WholeFamily, WholeInput, WholeLayout, quality, whole,
};

fn input(count: usize, fams: &[(Option<u32>, Option<u32>, &[u32])]) -> WholeInput {
    WholeInput {
        count,
        families: fams
            .iter()
            .map(|(a, b, kids)| WholeFamily {
                partners: [*a, *b],
                children: kids.to_vec(),
            })
            .collect(),
        birth: vec![None; count],
        surname: vec![String::new(); count],
        include: None,
    }
}

fn run(i: &WholeInput) -> WholeLayout {
    whole(i, &Metrics::default())
}

fn node(w: &WholeLayout, key: u32) -> gt_layout::Node {
    let id = w.extras.node_of(key).unwrap();
    w.layout.nodes[id as usize].clone()
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

fn assert_no_overlap(w: &WholeLayout) {
    let mut idx: Vec<usize> = (0..w.layout.nodes.len()).collect();
    idx.sort_by(|&a, &b| {
        w.layout.nodes[a]
            .rect
            .x
            .total_cmp(&w.layout.nodes[b].rect.x)
    });
    for (i, &a) in idx.iter().enumerate() {
        let ra = w.layout.nodes[a].rect;
        for &b in &idx[i + 1..] {
            let rb = w.layout.nodes[b].rect;
            if rb.x >= ra.x + ra.w {
                break;
            }
            assert!(
                !overlaps(&ra, &rb),
                "nodes {a} and {b} overlap: {ra:?} {rb:?}"
            );
        }
    }
    for b in &w.extras.blocks {
        for n in &w.layout.nodes {
            assert!(!overlaps(&b.rect, &n.rect), "heading over node {}", n.id);
        }
    }
}

fn assert_everyone_once(w: &WholeLayout, count: usize) {
    assert_eq!(w.layout.nodes.len(), count);
    let keys: HashSet<u32> = w.layout.nodes.iter().map(|n| n.key).collect();
    assert_eq!(keys.len(), count);
    assert!(w.layout.nodes.iter().all(|n| n.dup_of.is_none()));
}

/// 0+1 -> 2; 2+3 -> 4,5.
fn three_generations() -> WholeInput {
    input(6, &[(Some(0), Some(1), &[2]), (Some(2), Some(3), &[4, 5])])
}

#[test]
fn generations_are_rows_for_a_three_generation_family() {
    let w = run(&three_generations());
    assert_everyone_once(&w, 6);
    let g = |k| node(&w, k).r#gen;
    assert_eq!([g(0), g(1), g(2), g(3), g(4), g(5)], [0, 0, 1, 1, 2, 2]);
    let y = |k| node(&w, k).rect.y;
    assert!(y(0) < y(2) && y(2) < y(4));
    assert_eq!(y(4), y(5));
    assert_no_overlap(&w);
}

#[test]
fn partners_share_a_row_side_by_side_with_a_partner_line() {
    let w = run(&three_generations());
    let (a, b) = (node(&w, 0), node(&w, 1));
    assert_eq!(a.rect.y, b.rect.y);
    let m = Metrics::default();
    assert!((b.rect.x - (a.rect.x + a.rect.w + m.couple_gap)).abs() < 0.01);
    assert!(
        w.layout
            .edges
            .iter()
            .any(|e| e.kind == EdgeKind::Partner && e.from == a.id && e.to == b.id)
    );
}

#[test]
fn each_child_gets_one_line_from_the_couple_and_parents_are_centred() {
    let w = run(&three_generations());
    let kids: Vec<_> = w
        .layout
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Parent && e.from == node(&w, 2).id)
        .collect();
    assert_eq!(kids.len(), 2);
    // The couple 2+3 sits centred over its two children.
    let (c, d) = (node(&w, 2), node(&w, 3));
    let couple_mid = (c.rect.x + d.rect.x + d.rect.w) / 2.0;
    let (k1, k2) = (node(&w, 4), node(&w, 5));
    let kids_mid = (k1.rect.x + k2.rect.x + k2.rect.w) / 2.0;
    assert!(
        (couple_mid - kids_mid).abs() < 1.0,
        "{couple_mid} vs {kids_mid}"
    );
    // Lines end on the top of the children.
    for e in kids {
        let end_y = e.pts[e.pts.len() - 1];
        assert!((end_y - k1.rect.y).abs() < 0.01);
    }
}

#[test]
fn siblings_stay_together_in_birth_order() {
    let mut i = input(
        11,
        &[
            (Some(9), Some(10), &[0, 5]),
            (Some(0), Some(1), &[2, 3, 4]),
            (Some(5), Some(6), &[7, 8]),
        ],
    );
    // Listed 2, 3, 4 but born 4, 2, 3.
    i.birth[2] = Some(1902);
    i.birth[3] = Some(1903);
    i.birth[4] = Some(1900);
    let w = run(&i);
    let x = |k| node(&w, k).rect.x;
    assert!(x(4) < x(2) && x(2) < x(3));
    // Contiguous: the other family's children are not in between.
    let (lo, hi) = (x(4).min(x(2)).min(x(3)), x(4).max(x(2)).max(x(3)));
    for k in [7, 8] {
        assert!(x(k) < lo || x(k) > hi);
    }
    assert_no_overlap(&w);
}

#[test]
fn contradictory_data_terminates_and_places_everyone_once() {
    // A parent of B, B parent of A, a person who is their own child, and a uncle-niece marriage.
    let i = input(
        8,
        &[
            (Some(0), None, &[1]),
            (Some(1), None, &[0]),
            (Some(2), Some(3), &[2, 4]),
            (Some(3), Some(4), &[5]),
            (Some(4), Some(5), &[6]),
            (Some(6), Some(0), &[7, 7]),
        ],
    );
    let w = run(&i);
    assert_everyone_once(&w, 8);
    assert_no_overlap(&w);
}

#[test]
fn cousin_marriage_shows_each_person_once() {
    // 0+1 -> 2,3; 2+4 -> 5; 3+6 -> 7; 5+7 -> 8 (first cousins).
    let i = input(
        9,
        &[
            (Some(0), Some(1), &[2, 3]),
            (Some(2), Some(4), &[5]),
            (Some(3), Some(6), &[7]),
            (Some(5), Some(7), &[8]),
        ],
    );
    let w = run(&i);
    assert_everyone_once(&w, 9);
    assert_no_overlap(&w);
    assert_eq!(node(&w, 5).rect.y, node(&w, 7).rect.y);
    assert!(node(&w, 8).rect.y > node(&w, 5).rect.y);
}

#[test]
fn a_person_with_two_partners_sits_between_them() {
    let i = input(5, &[(Some(0), Some(1), &[3]), (Some(1), Some(2), &[4])]);
    let w = run(&i);
    let x = |k| node(&w, k).rect.x;
    assert!((x(0) < x(1) && x(1) < x(2)) || (x(2) < x(1) && x(1) < x(0)));
    assert_no_overlap(&w);
}

#[test]
fn disconnected_components_are_separate_blocks_largest_first() {
    let mut i = input(
        9,
        &[
            (Some(0), Some(1), &[2]),
            (Some(3), Some(4), &[5, 6, 7]),
            (Some(8), None, &[]),
        ],
    );
    i.surname = [
        "Koval",
        "Koval",
        "Koval",
        "Bondar",
        "Bondar",
        "Bondar",
        "Hrytsenko",
        "Bondar",
        "Lone",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    let w = run(&i);
    let fam: Vec<_> = w
        .extras
        .blocks
        .iter()
        .filter(|b| b.kind == BlockKind::Family)
        .collect();
    assert_eq!(fam.len(), 2);
    assert_eq!((fam[0].people, fam[0].surname.as_str()), (5, "Bondar"));
    assert_eq!((fam[1].people, fam[1].surname.as_str()), (3, "Koval"));
    // The two families do not share any space.
    let members = |keys: &[u32]| -> Rect {
        let rs: Vec<Rect> = keys.iter().map(|&k| node(&w, k).rect).collect();
        let x0 = rs.iter().map(|r| r.x).fold(f32::MAX, f32::min);
        let y0 = rs.iter().map(|r| r.y).fold(f32::MAX, f32::min);
        let x1 = rs.iter().map(|r| r.x + r.w).fold(f32::MIN, f32::max);
        let y1 = rs.iter().map(|r| r.y + r.h).fold(f32::MIN, f32::max);
        Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        }
    };
    assert!(!overlaps(&members(&[0, 1, 2]), &members(&[3, 4, 5, 6, 7])));
    assert_no_overlap(&w);
}

#[test]
fn people_without_links_go_in_the_unconnected_block_at_the_end() {
    let i = input(
        7,
        &[
            (Some(0), Some(1), &[2]),
            (Some(3), None, &[]),
            (None, None, &[4]),
        ],
    );
    let w = run(&i);
    assert_everyone_once(&w, 7);
    let last = w.extras.blocks.last().unwrap();
    assert_eq!(last.kind, BlockKind::Unconnected);
    // 3, 4, 5, 6 have no link that counts.
    assert_eq!(last.people, 4);
    let family_bottom = [0, 1, 2]
        .iter()
        .map(|&k| node(&w, k).rect.y + node(&w, k).rect.h)
        .fold(f32::MIN, f32::max);
    for k in [3, 4, 5, 6] {
        let r = node(&w, k).rect;
        assert!(r.y > family_bottom, "person {k} is not below the families");
        assert!(r.y > last.rect.y);
    }
    assert_no_overlap(&w);
}

#[test]
fn empty_input_is_an_empty_layout() {
    let w = run(&WholeInput::default());
    assert!(w.layout.nodes.is_empty());
    assert!(w.extras.blocks.is_empty());
}

#[test]
fn lineage_is_ancestors_and_descendants_only() {
    let w = run(&three_generations());
    let id = |k| w.extras.node_of(k).unwrap();
    let mut want = vec![id(0), id(1), id(2), id(4)];
    want.sort_unstable();
    assert_eq!(w.extras.lineage(id(2)), {
        let mut v = want.clone();
        v.push(id(5));
        v.sort_unstable();
        v
    });
    // 3 married in: their line is themselves and their children.
    let mut v = vec![id(3), id(4), id(5)];
    v.sort_unstable();
    assert_eq!(w.extras.lineage(id(3)), v);
}

#[test]
fn navigation_links_follow_the_first_family() {
    let w = run(&three_generations());
    let c = node(&w, 2);
    let k1 = node(&w, 4);
    let k2 = node(&w, 5);
    assert_eq!(c.nav.child, Some(k1.id));
    assert_eq!(k1.nav.parent, Some(c.id));
    assert_eq!(k1.nav.next, Some(k2.id));
    assert_eq!(k2.nav.prev, Some(k1.id));
}

/// Deterministic generator.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }
}

/// About `target` people over several generations: founders, 1-4 children per couple, children
/// marry someone from outside or, now and then, a cousin; some people have a second partner.
fn synthetic(target: usize, seed: u64) -> WholeInput {
    let mut rng = Lcg(seed);
    let mut count = 0usize;
    let mut fams: Vec<WholeFamily> = Vec::new();
    let mut birth: Vec<Option<i32>> = Vec::new();
    let mut surname: Vec<String> = Vec::new();
    let mut new_person = |year: i32, name: usize, count: &mut usize| -> u32 {
        birth.push(if year % 7 == 0 { None } else { Some(year) });
        surname.push(format!("Surname{}", name % 40));
        *count += 1;
        (*count - 1) as u32
    };
    let mut gen_pairs: Vec<(u32, u32)> = Vec::new();
    for f in 0..30 {
        let a = new_person(1800, f, &mut count);
        let b = new_person(1802, f, &mut count);
        gen_pairs.push((a, b));
    }
    let mut year = 1825;
    let mut all_kids: Vec<u32> = Vec::new();
    while count < target {
        let mut kids_now: Vec<u32> = Vec::new();
        for &(a, b) in &gen_pairs {
            if count >= target {
                break;
            }
            let n = 1 + rng.next(4) as usize;
            let mut ks = Vec::new();
            for _ in 0..n {
                let k = new_person(year + rng.next(8) as i32, a as usize, &mut count);
                ks.push(k);
                kids_now.push(k);
                all_kids.push(k);
            }
            fams.push(WholeFamily {
                partners: [Some(a), Some(b)],
                children: ks,
            });
        }
        // Pair the children up: outsiders, with a cousin marriage every so often.
        let mut next: Vec<(u32, u32)> = Vec::new();
        let mut i = 0;
        while i < kids_now.len() {
            let a = kids_now[i];
            let partner = if rng.next(25) == 0 && i + 5 < kids_now.len() {
                kids_now[i + 5]
            } else {
                new_person(year + rng.next(8) as i32, i, &mut count)
            };
            if partner != a {
                next.push((a, partner));
                if rng.next(12) == 0 {
                    // A second marriage with no children of its own here.
                    let c = new_person(year + 3, i, &mut count);
                    fams.push(WholeFamily {
                        partners: [Some(a), Some(c)],
                        children: Vec::new(),
                    });
                }
            }
            i += 1 + rng.next(2) as usize;
        }
        gen_pairs = next;
        year += 25;
        if gen_pairs.is_empty() {
            break;
        }
    }
    // A few strangers.
    for _ in 0..40 {
        new_person(1900, 99, &mut count);
    }
    WholeInput {
        count,
        families: fams,
        birth,
        surname,
        include: None,
    }
}

#[test]
fn five_thousand_people_terminate_without_overlap_and_are_stable() {
    let i = synthetic(5000, 7);
    assert!(i.count >= 5000, "{} people", i.count);
    let t = Instant::now();
    let w = run(&i);
    let took = t.elapsed();
    println!(
        "whole-tree layout of {} people: {:?} ({} edges, bounds {:.0} x {:.0}, {} blocks)",
        i.count,
        took,
        w.layout.edges.len(),
        w.layout.bounds.w,
        w.layout.bounds.h,
        w.extras.blocks.len()
    );
    assert_everyone_once(&w, i.count);
    assert_no_overlap(&w);
    // Same data, same picture.
    let again = run(&i);
    assert_eq!(w.layout.nodes, again.layout.nodes);
    assert_eq!(w.layout.edges, again.layout.edges);
    assert_eq!(w.extras.blocks, again.extras.blocks);
    // Every viewport query agrees with a full scan.
    let r = Rect {
        x: 0.0,
        y: 0.0,
        w: 1500.0,
        h: 900.0,
    };
    let (n, _) = w.layout.query(&r);
    let scan = w
        .layout
        .nodes
        .iter()
        .filter(|n| overlaps(&n.rect, &r))
        .count();
    assert_eq!(n.len(), scan);
}

#[test]
fn the_output_is_deterministic_for_small_trees_too() {
    let i = synthetic(300, 3);
    let a = run(&i);
    let b = run(&i);
    assert_eq!(a.layout.nodes, b.layout.nodes);
    assert_eq!(a.layout.edges, b.layout.edges);
    assert_no_overlap(&a);
}

/// No segment of any partner line may run through a box other than the two it joins.
fn assert_partner_lines_clear_of_boxes(w: &WholeLayout) {
    for e in w
        .layout
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Partner)
    {
        for seg in e.pts.windows(4).step_by(2) {
            let (x0, x1) = (seg[0].min(seg[2]), seg[0].max(seg[2]));
            let (y0, y1) = (seg[1].min(seg[3]), seg[1].max(seg[3]));
            for n in &w.layout.nodes {
                if n.id == e.from || n.id == e.to {
                    continue;
                }
                let r = n.rect;
                let hit = x1 > r.x + 0.5
                    && x0 < r.x + r.w - 0.5
                    && y1 > r.y + 0.5
                    && y0 < r.y + r.h - 0.5;
                assert!(
                    !hit,
                    "partner line {}-{} runs through box {}",
                    e.from, e.to, n.id
                );
            }
        }
    }
}

fn assert_children_below_parents(w: &WholeLayout, i: &WholeInput) {
    for f in &i.families {
        let top = f.partners.iter().flatten().map(|&p| node(w, p).r#gen).max();
        let Some(top) = top else { continue };
        for &c in &f.children {
            assert!(node(w, c).r#gen > top, "child {c} is not below its parents");
        }
    }
}

#[test]
fn uncle_niece_marriage_keeps_the_child_below_both_parents() {
    // 0+1 -> 2,3; 2+4 -> 5; 3+5 -> 6: 3 marries his niece 5.
    let i = input(
        7,
        &[
            (Some(0), Some(1), &[2, 3]),
            (Some(2), Some(4), &[5]),
            (Some(3), Some(5), &[6]),
        ],
    );
    let w = run(&i);
    assert_everyone_once(&w, 7);
    assert_no_overlap(&w);
    assert!(node(&w, 6).r#gen > node(&w, 5).r#gen);
    assert!(node(&w, 6).r#gen > node(&w, 3).r#gen);
    assert_children_below_parents(&w, &i);
    assert_partner_lines_clear_of_boxes(&w);
    assert_eq!(run(&i).layout.nodes, w.layout.nodes);
}

#[test]
fn a_person_who_partners_their_own_ancestor_drops_the_partner_rule_not_the_parent_rule() {
    // 0+1 -> 2; 2+3 -> 4; 4+0 -> 5: 4 marries ancestor 0, so partners cannot share a row.
    let i = input(
        6,
        &[
            (Some(0), Some(1), &[2]),
            (Some(2), Some(3), &[4]),
            (Some(4), Some(0), &[5]),
        ],
    );
    let w = run(&i);
    assert_everyone_once(&w, 6);
    assert_no_overlap(&w);
    // Parents above children for the two honest families; the self-referential one cannot all hold.
    assert!(node(&w, 2).r#gen > node(&w, 0).r#gen);
    assert!(node(&w, 4).r#gen > node(&w, 2).r#gen);
    assert!(node(&w, 5).r#gen > node(&w, 4).r#gen);
    assert_partner_lines_clear_of_boxes(&w);
}

#[test]
fn consistent_trees_keep_partners_on_one_row_after_relaxing() {
    let i = three_generations();
    let w = run(&i);
    assert_eq!(node(&w, 2).r#gen, node(&w, 3).r#gen);
    assert_eq!(node(&w, 0).r#gen, node(&w, 1).r#gen);
    assert_children_below_parents(&w, &i);
}

#[test]
fn reversed_flips_each_family_but_keeps_headings_ids_and_blocks() {
    // 0 and 1 are the parents of 2; 3 is alone.
    let i = input(4, &[(Some(0), Some(1), &[2])]);
    let a = run(&i);
    let b = gt_layout::whole_directed(&i, &Metrics::default(), true);
    assert_eq!(a.layout.nodes.len(), b.layout.nodes.len());
    assert_eq!(a.extras.blocks.len(), b.extras.blocks.len());
    for (x, y) in a.extras.blocks.iter().zip(&b.extras.blocks) {
        assert_eq!(x.rect, y.rect);
        assert_eq!(x.body_h, y.body_h);
    }
    // Parent above child normally, below the child when reversed; headings stay above every person.
    assert!(node(&a, 0).rect.y < node(&a, 2).rect.y);
    assert!(node(&b, 0).rect.y > node(&b, 2).rect.y);
    for blk in &b.extras.blocks {
        for n in &b.layout.nodes {
            assert!(!overlaps(&blk.rect, &n.rect), "heading over node {}", n.id);
        }
    }
    let heading = &b.extras.blocks[0];
    assert!(node(&b, 2).rect.y >= heading.rect.y + heading.rect.h);
    assert!(
        node(&b, 0).rect.y + node(&b, 0).rect.h
            <= heading.rect.y + heading.rect.h + 12.0 + heading.body_h + 0.5
    );
    assert_eq!(a.extras.node_of(0), b.extras.node_of(0));
    // Every parent edge still ends on its boxes.
    for e in b.layout.edges.iter().filter(|e| e.kind == EdgeKind::Parent) {
        let child = &b.layout.nodes[e.to as usize].rect;
        let last = &e.pts[e.pts.len() - 2..];
        assert!(last[1] == child.y + child.h || last[1] == child.y);
    }
    assert_no_overlap(&b);
}

#[test]
fn nothing_lies_on_top_of_anything_in_big_trees() {
    for (size, seed) in [(300, 2), (1000, 3), (5000, 7)] {
        let q = quality(&run(&synthetic(size, seed)).layout);
        assert_eq!(q.box_overlaps, 0, "{size}: {q:?}");
        assert_eq!(q.lines_through_boxes, 0, "{size}: {q:?}");
        assert_eq!(q.line_overlaps, 0, "{size}: {q:?}");
    }
}

/// Grandparents (0, 1) with children 2, 3 and 4. Child 3 married 5, whose parents are 6 and 7 with another child 8.
/// Child 2 married 9, whose parents are 10 and 11. The couple 3 + 5 has children 12 and 13.
fn in_laws_on_both_sides() -> WholeInput {
    input(
        14,
        &[
            (Some(0), Some(1), &[2, 3, 4]),
            (Some(6), Some(7), &[5, 8]),
            (Some(10), Some(11), &[9]),
            (Some(2), Some(9), &[]),
            (Some(3), Some(5), &[12, 13]),
        ],
    )
}

#[test]
fn parents_in_law_hang_off_the_side_without_a_crossing() {
    let w = run(&in_laws_on_both_sides());
    let q = quality(&w.layout);
    assert_eq!(q.line_crossings, 0, "{q:?}");
    assert_eq!(q.line_overlaps, 0, "{q:?}");
    assert_eq!(q.lines_through_boxes, 0, "{q:?}");
    assert_no_overlap(&w);
}

#[test]
fn a_family_stays_together_under_its_parents() {
    let w = run(&in_laws_on_both_sides());
    let cx = |k| {
        let r = node(&w, k).rect;
        r.x + r.w / 2.0
    };
    // The children of the couple are centred under it.
    let couple = f32::midpoint(cx(3), cx(5));
    let kids = f32::midpoint(cx(12), cx(13));
    assert!((couple - kids).abs() < 1.0, "{couple} vs {kids}");
    // Nobody from another family sits between 12 and 13, or between the partner 5 and the sister 8.
    let row_of = |k: u32| node(&w, k).rect.y;
    let between = |a: f32, b: f32, k: u32| cx(k) > a.min(b) && cx(k) < a.max(b);
    for k in 0..14 {
        if ![12, 13].contains(&k) && row_of(k) == row_of(12) {
            assert!(!between(cx(12), cx(13), k), "{k}");
        }
        if ![3, 5, 8].contains(&k) && row_of(k) == row_of(5) {
            assert!(!between(cx(5), cx(8), k), "{k}");
        }
    }
}

#[test]
fn the_bars_of_two_families_do_not_read_as_one_line() {
    // Two sets of cousins side by side under one couple.
    let w = run(&input(
        14,
        &[
            (Some(0), Some(1), &[2, 4]),
            (Some(2), Some(3), &[6, 7, 8]),
            (Some(4), Some(5), &[9, 10, 11]),
        ],
    ));
    let bar = |parent: u32| {
        let from = node(&w, parent).id;
        let (mut lo, mut hi, mut y) = (f32::MAX, f32::MIN, 0.0);
        for e in w
            .layout
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Parent && e.from == from)
        {
            let n = e.pts.len();
            y = e.pts[n - 3];
            lo = lo.min(e.pts[n - 2]);
            hi = hi.max(e.pts[n - 2]);
        }
        (lo, hi, y)
    };
    let (a, b) = (bar(2), bar(4));
    let apart = a.0.max(b.0) - a.1.min(b.1);
    assert!(a.2 != b.2 || apart >= 28.0, "{a:?} {b:?}");
}

#[test]
fn a_child_with_two_sets_of_parents_gets_two_lines_that_do_not_coincide() {
    // Child 2 of 0 + 1, and also of 1 + 3 (a stepfather).
    let w = run(&input(
        4,
        &[(Some(0), Some(1), &[2]), (Some(1), Some(3), &[2])],
    ));
    let child = node(&w, 2).id;
    let ends: Vec<f32> = w
        .layout
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Parent && e.to == child)
        .map(|e| e.pts[e.pts.len() - 2])
        .collect();
    assert_eq!(ends.len(), 2);
    assert!((ends[0] - ends[1]).abs() > 10.0, "{ends:?}");
    assert_eq!(quality(&w.layout).line_overlaps, 0);
}

#[test]
fn a_line_past_a_row_goes_around_its_boxes() {
    // 0 + 1 have child 2, who is pushed two rows down by marrying 5, a grandchild of 3 + 4 through 6 + 7; 8 and 9 fill
    // the row in between.
    let w = run(&input(
        10,
        &[
            (Some(0), Some(1), &[2, 8, 9]),
            (Some(3), Some(4), &[6]),
            (Some(6), Some(7), &[5]),
            (Some(2), Some(5), &[]),
        ],
    ));
    assert_eq!(quality(&w.layout).lines_through_boxes, 0);
    assert_no_overlap(&w);
}

#[test]
fn cousins_who_marry_stay_readable() {
    // Brothers 2 and 4 have children 6 and 8, who marry; their other children and an uncle sit around them.
    let w = run(&input(
        11,
        &[
            (Some(0), Some(1), &[9, 2, 10, 4]),
            (Some(2), Some(3), &[6]),
            (Some(4), Some(5), &[8]),
            (Some(6), Some(8), &[]),
        ],
    ));
    let q = quality(&w.layout);
    assert_eq!(q.box_overlaps, 0);
    assert_eq!(q.lines_through_boxes, 0);
    assert!(q.line_crossings <= 2, "{q:?}");
}

#[test]
fn children_of_a_family_without_partners_sit_together_and_are_joined() {
    // Three sisters with no parents on record, plus an unrelated couple so the chart has company.
    let i = input(6, &[(None, None, &[0, 1, 2]), (Some(3), Some(4), &[5])]);
    let w = run(&i);
    let n: Vec<_> = (0..3).map(|k| node(&w, k)).collect();
    assert!(
        n.iter().all(|x| (x.rect.y - n[0].rect.y).abs() < 0.01),
        "one row"
    );
    let mut xs: Vec<f32> = n.iter().map(|x| x.rect.x).collect();
    xs.sort_by(f32::total_cmp);
    assert!(
        xs[2] - xs[0] < 2.0 * (176.0 + 40.0) + 1.0,
        "side by side: {xs:?}"
    );
    // Neighbouring sisters are joined by a bracket line, which is not a partner line.
    let ids: Vec<u32> = n.iter().map(|x| x.id).collect();
    let joins = w
        .layout
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Parent && ids.contains(&e.from) && ids.contains(&e.to))
        .count();
    assert_eq!(joins, 2, "two links join three siblings");
}

#[test]
fn one_child_and_a_marker_without_partners_are_joined() {
    let i = input(2, &[(None, None, &[0, 1])]);
    let w = run(&i);
    let joins = w
        .layout
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Parent)
        .count();
    assert_eq!(joins, 1);
}

#[test]
fn people_left_out_by_the_mask_are_not_drawn_or_linked() {
    // 0 + 1 have child 2 and child 3; 4 is alone; 5 + 2 are partners.
    let mut i = input(
        6,
        &[(Some(0), Some(1), &[2, 3]), (Some(2), Some(5), &[])],
    );
    let all = run(&i);
    assert_eq!(all.layout.nodes.len(), 6);
    i.include = Some(vec![true, true, true, false, false, true]);
    let w = run(&i);
    let mut keys: Vec<u32> = w.layout.nodes.iter().map(|n| n.key).collect();
    keys.sort_unstable();
    assert_eq!(keys, [0, 1, 2, 5]);
    // 3 and 4 are gone, and so is the block for them: one connected group, no unconnected one.
    assert_eq!(w.extras.blocks.len(), 1);
    assert_eq!(w.extras.blocks[0].kind, BlockKind::Family);
    assert!(w.extras.node_of(3).is_none() && w.extras.node_of(4).is_none());
    // A left-out partner leaves the other one alone in its family: still drawn, as unconnected.
    i.include = Some(vec![false, true, false, false, true, false]);
    let w = run(&i);
    assert_eq!(w.layout.nodes.len(), 2);
    assert_eq!(w.extras.blocks.len(), 1);
    assert_eq!(w.extras.blocks[0].kind, BlockKind::Unconnected);
}
