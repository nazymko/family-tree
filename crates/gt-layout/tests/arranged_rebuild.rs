#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! "Arrange around this person" rebuilds the chart with that person as its structural centre: it is not the usual
//! order with the same people merely scrolled to. Synthetic people only.

use std::collections::HashSet;

use gt_layout::{Metrics, Rect, WholeFamily, WholeInput, WholeLayout, whole, whole_around};

/// A deterministic world of about `target` people from one pair of founders: every couple has two to four children,
/// every fifth person also has a second family, and every child brings a partner who has no parents of their own.
/// Returns the input and the people of the youngest generation.
fn world(target: usize) -> (WholeInput, Vec<u32>) {
    let mut fams: Vec<WholeFamily> = Vec::new();
    let mut count: u32 = 0;
    let mut seed: u64 = 12345;
    let mut rnd = move |m: u64| {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (seed >> 33) % m
    };
    let next = |c: &mut u32| {
        let v = *c;
        *c += 1;
        v
    };
    let a = next(&mut count);
    let b = next(&mut count);
    let mut level: Vec<(u32, u32)> = vec![(a, b)];
    let mut youngest: Vec<u32> = Vec::new();
    while (count as usize) < target {
        let mut kids_all: Vec<u32> = Vec::new();
        for (p, q) in level.clone() {
            let k = 2 + rnd(3) as usize;
            let kids: Vec<u32> = (0..k).map(|_| next(&mut count)).collect();
            fams.push(WholeFamily {
                partners: [Some(p), Some(q)],
                children: kids.clone(),
            });
            if rnd(5) == 0 {
                let r = next(&mut count);
                let k2: Vec<u32> = (0..2).map(|_| next(&mut count)).collect();
                fams.push(WholeFamily {
                    partners: [Some(p), Some(r)],
                    children: k2.clone(),
                });
                kids_all.extend(k2);
            }
            kids_all.extend(kids);
        }
        youngest = kids_all.clone();
        level = kids_all.iter().map(|&c| (c, next(&mut count))).collect();
    }
    let n = count as usize;
    (
        WholeInput {
            count: n,
            families: fams,
            birth: vec![None; n],
            surname: vec![String::new(); n],
            include: None,
        },
        youngest,
    )
}

fn rect(w: &WholeLayout, k: u32) -> Rect {
    w.layout.nodes[w.extras.node_of(k).unwrap() as usize].rect
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

fn assert_sound(w: &WholeLayout, i: &WholeInput) {
    assert_eq!(w.layout.nodes.len(), i.count, "everybody is drawn");
    let keys: HashSet<u32> = w.layout.nodes.iter().map(|n| n.key).collect();
    assert_eq!(keys.len(), i.count, "everybody exactly once");
    // Overlaps can only happen inside a row: sort by row and sweep.
    let mut rows: std::collections::BTreeMap<i32, Vec<Rect>> = std::collections::BTreeMap::new();
    for n in &w.layout.nodes {
        rows.entry((n.rect.y * 10.0) as i32)
            .or_default()
            .push(n.rect);
    }
    for row in rows.values_mut() {
        row.sort_by(|a, b| a.x.total_cmp(&b.x));
        for pair in row.windows(2) {
            assert!(!overlaps(&pair[0], &pair[1]), "two cards overlap");
        }
    }
    for f in &i.families {
        for c in &f.children {
            for p in f.partners.iter().flatten() {
                assert!(
                    rect(w, *p).y < rect(w, *c).y,
                    "parent {p} not above child {c}"
                );
            }
        }
    }
    // Every family line is drawn: a parent edge for each parent and child.
    let drawn = w.layout.edges.len();
    let wanted: usize = i
        .families
        .iter()
        .map(|f| f.children.len() * f.partners.iter().flatten().count().max(1))
        .sum();
    assert!(
        drawn >= wanted / 2,
        "family lines are missing: {drawn} of {wanted}"
    );
}

fn moved(a: &WholeLayout, b: &WholeLayout, n: usize) -> usize {
    (0..n as u32)
        .filter(|&k| {
            let (p, q) = (rect(a, k), rect(b, k));
            (p.x - q.x).abs() > 1.0 || (p.y - q.y).abs() > 1.0
        })
        .count()
}

fn siblings(i: &WholeInput, t: u32) -> Vec<u32> {
    i.families
        .iter()
        .filter(|f| f.children.contains(&t))
        .flat_map(|f| f.children.iter().copied().filter(|&c| c != t))
        .collect()
}

fn parents(i: &WholeInput, t: u32) -> Vec<u32> {
    i.families
        .iter()
        .filter(|f| f.children.contains(&t))
        .flat_map(|f| f.partners.iter().flatten().copied())
        .collect()
}

#[test]
fn a_person_far_from_the_root_gets_a_rebuilt_chart_not_the_usual_one() {
    let (i, youngest) = world(400);
    let m = Metrics::default();
    let usual = whole(&i, &m);
    // Several far-away people: the first, the middle and the last of the youngest generation.
    for t in [
        youngest[0],
        youngest[youngest.len() / 2],
        youngest[youngest.len() - 1],
    ] {
        let a = whole_around(&i, &m, false, Some(t));
        assert_sound(&a, &i);
        assert!(
            moved(&usual, &a, i.count) >= 10,
            "person {t}: the chart is the usual one ({} cards moved)",
            moved(&usual, &a, i.count)
        );
        // Ancestors above, the person in the middle of their brothers and sisters, parents straight above them.
        let sibs = siblings(&i, t);
        if sibs.len() >= 2 {
            let lo = sibs
                .iter()
                .map(|&s| rect(&a, s).x)
                .fold(f32::INFINITY, f32::min);
            let hi = sibs
                .iter()
                .map(|&s| rect(&a, s).x)
                .fold(f32::NEG_INFINITY, f32::max);
            let x = rect(&a, t).x;
            assert!(
                lo < x && x < hi,
                "person {t} is not between their brothers and sisters"
            );
        }
        for p in parents(&i, t) {
            assert!(rect(&a, p).y < rect(&a, t).y);
        }
    }
    // Another person gives another chart.
    let one = whole_around(&i, &m, false, Some(youngest[0]));
    let two = whole_around(&i, &m, false, Some(youngest[youngest.len() - 1]));
    assert!(
        moved(&one, &two, i.count) >= 10,
        "a different person gives the same chart"
    );
}

#[test]
fn a_person_with_several_partners_stands_among_them_with_children_below() {
    let (mut i, youngest) = world(300);
    let t = youngest[youngest.len() / 3];
    let base = i.count as u32;
    // Three partners, with two, one and no children.
    let partners = [base, base + 1, base + 2];
    let kids = [vec![base + 3, base + 4], vec![base + 5], vec![]];
    for (p, k) in partners.iter().zip(kids) {
        i.families.push(WholeFamily {
            partners: [Some(t), Some(*p)],
            children: k,
        });
    }
    i.count += 6;
    i.birth = vec![None; i.count];
    i.surname = vec![String::new(); i.count];
    let m = Metrics::default();
    let usual = whole(&i, &m);
    let a = whole_around(&i, &m, false, Some(t));
    assert_sound(&a, &i);
    assert!(
        moved(&usual, &a, i.count) >= 10,
        "the chart is the usual one"
    );
    let xs: Vec<f32> = partners.iter().map(|&p| rect(&a, p).x).collect();
    let lo = xs.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let x = rect(&a, t).x;
    assert!(
        lo < x && x < hi,
        "{t} is not among their partners: {lo} {x} {hi}"
    );
    for c in [base + 3, base + 4, base + 5] {
        assert!(rect(&a, c).y > rect(&a, t).y, "child {c} not below");
    }
    // Same row as the person: the partners are beside them.
    for p in partners {
        assert!((rect(&a, p).y - rect(&a, t).y).abs() < 0.5);
    }
}

#[test]
#[ignore = "release-only budget check"]
fn five_thousand_people_are_rebuilt_in_a_blink() {
    let (i, youngest) = world(5000);
    let m = Metrics::default();
    let t0 = std::time::Instant::now();
    let _ = whole(&i, &m);
    let usual = t0.elapsed();
    let t = youngest[youngest.len() / 2];
    let t1 = std::time::Instant::now();
    let a = whole_around(&i, &m, false, Some(t));
    let took = t1.elapsed();
    println!("people {}: usual {usual:?}, around {took:?}", i.count);
    assert_sound(&a, &i);
    assert!(took.as_millis() < 500, "{took:?}");
}
