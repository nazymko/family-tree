#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The chart arranged around one person: the same people, in an order that brings their closest relatives near.

use std::collections::HashSet;

use gt_layout::{Metrics, Rect, WholeFamily, WholeInput, WholeLayout, whole, whole_around};

type Fam = (Option<u32>, Option<u32>, Vec<u32>);

fn input(count: usize, fams: &[Fam]) -> WholeInput {
    WholeInput {
        count,
        families: fams
            .iter()
            .map(|(a, b, kids)| WholeFamily {
                partners: [*a, *b],
                children: kids.clone(),
            })
            .collect(),
        birth: (0..count).map(|k| Some(k as i32)).collect(),
        surname: vec![String::new(); count],
        include: None,
    }
}

/// Ivan Petrovych Koval (3) married Olena (5), who comes from a big family; her brothers and sisters have children.
/// His own brothers have children too. Two of their children married in, with parents of their own.
fn big_family() -> WholeInput {
    let f = |a: u32, b: u32, kids: &[u32]| (Some(a), Some(b), kids.to_vec());
    input(
        47,
        &[
            f(0, 1, &[2, 3, 4]),
            f(2, 13, &[26, 27, 28]),
            f(4, 14, &[29, 30]),
            f(3, 5, &[15, 16, 17]),
            f(6, 7, &[8, 9, 5, 10, 11, 12]),
            f(8, 31, &[32, 33]),
            f(9, 34, &[35, 36]),
            f(10, 37, &[38, 39]),
            f(11, 40, &[41, 42]),
            f(12, 43, &[44, 45]),
            f(16, 18, &[21, 22]),
            f(19, 20, &[18]),
            f(15, 23, &[46]),
            f(24, 25, &[23]),
        ],
    )
}

fn arranged(i: &WholeInput, around: u32) -> WholeLayout {
    whole_around(i, &Metrics::default(), false, Some(around))
}

fn plain(i: &WholeInput) -> WholeLayout {
    whole(i, &Metrics::default())
}

fn same(a: &WholeLayout, b: &WholeLayout) -> bool {
    format!("{:?}", a.layout.nodes) == format!("{:?}", b.layout.nodes)
        && format!("{:?}", a.layout.edges) == format!("{:?}", b.layout.edges)
}

fn cx(w: &WholeLayout, key: u32) -> f32 {
    let n = &w.layout.nodes[w.extras.node_of(key).unwrap() as usize];
    n.rect.x + n.rect.w / 2.0
}

fn cy(w: &WholeLayout, key: u32) -> f32 {
    w.layout.nodes[w.extras.node_of(key).unwrap() as usize]
        .rect
        .y
}

/// Parents, partners, brothers and sisters, children and grandparents of `p`.
fn close(i: &WholeInput, p: u32) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    let parents = |x: u32| -> Vec<u32> {
        i.families
            .iter()
            .filter(|f| f.children.contains(&x))
            .flat_map(|f| f.partners.iter().flatten().copied())
            .collect()
    };
    for f in &i.families {
        let partners: Vec<u32> = f.partners.iter().flatten().copied().collect();
        if partners.contains(&p) {
            out.extend(partners.iter().copied().filter(|&q| q != p));
            out.extend(f.children.iter().copied());
        }
        if f.children.contains(&p) {
            out.extend(f.children.iter().copied().filter(|&q| q != p));
        }
    }
    for q in parents(p) {
        out.push(q);
        out.extend(parents(q));
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn distance(w: &WholeLayout, i: &WholeInput, p: u32) -> f32 {
    close(i, p)
        .iter()
        .map(|&q| (cx(w, p) - cx(w, q)).abs())
        .sum()
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

fn assert_sound(w: &WholeLayout, i: &WholeInput, shown: usize) {
    assert_eq!(w.layout.nodes.len(), shown);
    let keys: HashSet<u32> = w.layout.nodes.iter().map(|n| n.key).collect();
    assert_eq!(keys.len(), shown, "everybody exactly once");
    for (a, na) in w.layout.nodes.iter().enumerate() {
        for nb in &w.layout.nodes[a + 1..] {
            assert!(!overlaps(&na.rect, &nb.rect), "{} over {}", na.key, nb.key);
        }
    }
    for f in &i.families {
        for c in &f.children {
            for p in f.partners.iter().flatten() {
                if w.extras.node_of(*c).is_some() && w.extras.node_of(*p).is_some() {
                    assert!(cy(w, *p) < cy(w, *c), "parent {p} not above child {c}");
                }
            }
        }
    }
}

#[test]
fn the_close_relatives_are_nearer_arranged_than_in_the_usual_order() {
    let i = big_family();
    let usual = plain(&i);
    let (mut before, mut after) = (0.0, 0.0);
    for p in [16u32, 3, 5, 21, 18, 15, 17] {
        let counted = [16u32, 3, 15, 17].contains(&p);
        let a = arranged(&i, p);
        assert_sound(&a, &i, 47);
        let (d0, d1) = (distance(&usual, &i, p), distance(&a, &i, p));
        println!("person {p}: usual {d0:.0}, arranged {d1:.0}");
        assert!(d1 <= d0 * 1.02, "person {p} got further: {d0} -> {d1}");
        if counted {
            before += d0;
            after += d1;
        }
    }
    println!("total: usual {before:.0}, arranged {after:.0}");
    // The four with brothers, sisters or a married-in parent; the others may only not get worse.
    assert!(
        after < before * 0.7,
        "not clearly nearer: {before} -> {after}"
    );
}

#[test]
fn the_arrangement_is_deterministic_and_none_changes_nothing() {
    let i = big_family();
    assert!(same(&arranged(&i, 16), &arranged(&i, 16)));
    let none = whole_around(&i, &Metrics::default(), false, None);
    assert!(same(&none, &plain(&i)));
}

#[test]
fn reversed_direction_and_a_mask_still_hold_together() {
    let i = big_family();
    let r = whole_around(&i, &Metrics::default(), true, Some(16));
    assert_eq!(r.layout.nodes.len(), 47);
    for f in &i.families {
        for c in &f.children {
            for p in f.partners.iter().flatten() {
                assert!(cy(&r, *p) > cy(&r, *c), "reversed: {p} below {c}");
            }
        }
    }
    // Only the family of Ivan and Olena: ancestors of 16 and their partners.
    let mut m = big_family();
    let on = [
        0u32, 1, 2, 3, 4, 5, 6, 7, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
    ];
    m.include = Some((0..47).map(|k| on.contains(&k)).collect());
    let a = arranged(&m, 16);
    assert_sound(&a, &m, 18);
    // A target the mask leaves out changes nothing.
    assert!(same(&arranged(&m, 40), &plain(&m)));
}

#[test]
fn loops_and_several_parent_sets_stay_safe() {
    let f = |a: u32, b: u32, kids: &[u32]| (Some(a), Some(b), kids.to_vec());
    // Cousins marry (6 and 7), a child with two sets of parents (8), a person who partners their own ancestor (9).
    let i = input(
        10,
        &[
            f(0, 1, &[2, 3]),
            f(2, 4, &[6]),
            f(3, 5, &[7]),
            f(6, 7, &[8]),
            f(0, 9, &[8]),
            f(9, 8, &[]),
            (Some(8), None, vec![9]),
        ],
    );
    for p in 0..10 {
        let a = arranged(&i, p);
        assert_eq!(a.layout.nodes.len(), 10);
        assert!(same(&a, &arranged(&i, p)));
        for (x, nx) in a.layout.nodes.iter().enumerate() {
            for ny in &a.layout.nodes[x + 1..] {
                assert!(!overlaps(&nx.rect, &ny.rect));
            }
        }
    }
}

#[test]
fn the_chosen_family_is_shelved_first() {
    let base = 47u32;
    let mut i = big_family();
    // A second, larger family.
    i.families.push(WholeFamily {
        partners: [Some(base), Some(base + 1)],
        children: (base + 2..base + 60).collect(),
    });
    i.count = 47 + 60;
    i.birth = vec![None; i.count];
    i.surname = vec![String::new(); i.count];
    let usual = plain(&i);
    let a = arranged(&i, 16);
    assert_eq!(usual.extras.blocks[0].people, 60);
    assert_eq!(a.extras.blocks[0].people, 47);
    assert_eq!(a.layout.nodes.len(), 107);
}
