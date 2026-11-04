mod common;

use common::MapSource;
use gt_layout::{
    EdgeKind, Key, Layout, Metrics, Node, PartnerOf, Rel, RelativesOptions, relatives,
};

fn opts(generations: u8, aunts: bool) -> RelativesOptions {
    RelativesOptions { generations, aunts }
}

fn run(s: &MapSource, root: Key, generations: u8, aunts: bool) -> Layout {
    relatives(
        s,
        root,
        &opts(generations, aunts),
        &Metrics::default(),
        100_000,
    )
}

fn node_of(l: &Layout, key: Key) -> &Node {
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
            assert!(apart, "nodes {} and {} overlap", a.id, b.id);
        }
    }
}

/// Parents 1 and 2 with children 10, 11, 12 (birth order 11, 10, 12).
fn trio() -> MapSource {
    let mut s = MapSource::default();
    s.family(Some(1), Some(2), &[10, 11, 12]);
    s.births.extend([(11, 100), (10, 200), (12, 300)]);
    s
}

#[test]
fn siblings_are_included_on_the_focus_row() {
    let l = run(&trio(), 10, 2, true);
    assert_eq!(l.root, 0);
    assert_eq!(l.nodes[0].key, 10);
    assert_eq!(l.nodes[0].rel, Some(Rel::Focus));
    for k in [11, 12] {
        let n = node_of(&l, k);
        assert_eq!(n.rel, Some(Rel::Sibling { half: false }));
        assert_eq!(n.rect.y, l.nodes[0].rect.y);
    }
    // The parents sit one row above.
    assert!(node_of(&l, 1).rect.y < l.nodes[0].rect.y);
    assert_eq!(node_of(&l, 1).rel, Some(Rel::Ancestor(1)));
}

#[test]
fn siblings_are_ordered_by_birth_with_the_focus_in_its_place() {
    let l = run(&trio(), 10, 2, true);
    let x = |k| node_of(&l, k).rect.x;
    assert!(x(11) < x(10) && x(10) < x(12));
}

#[test]
fn half_siblings_are_flagged() {
    let mut s = MapSource::default();
    s.family(Some(1), Some(2), &[10, 11]);
    s.family(Some(1), Some(3), &[12]);
    s.family(Some(4), Some(2), &[13]);
    let l = run(&s, 10, 2, true);
    assert_eq!(
        node_of(&l, 11).rel,
        Some(Rel::Sibling { half: false }),
        "full sibling"
    );
    assert_eq!(node_of(&l, 12).rel, Some(Rel::Sibling { half: true }));
    assert_eq!(node_of(&l, 13).rel, Some(Rel::Sibling { half: true }));
    // A half sibling hangs from one parent only; a full sibling from both.
    let into = |k: Key| {
        let to = node_of(&l, k).id;
        l.edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Parent && e.to == to)
            .map(|e| l.nodes[e.from as usize].key)
            .collect::<Vec<_>>()
    };
    assert_eq!(into(11).len(), 2);
    assert_eq!(into(12), vec![1]);
    assert_eq!(into(13), vec![2]);
}

#[test]
fn partners_sit_next_to_their_person_and_nieces_hang_below() {
    let mut s = trio();
    s.family(Some(11), Some(50), &[60, 61]);
    s.family(Some(10), Some(51), &[]);
    let l = run(&s, 10, 2, true);
    let m = Metrics::default();
    let a = node_of(&l, 11);
    let p = node_of(&l, 50);
    assert_eq!(p.rel, Some(Rel::Partner(PartnerOf::Sibling)));
    assert_eq!(p.rect.x, a.rect.x + m.node_w + m.couple_gap);
    assert_eq!(p.rect.y, a.rect.y);
    assert_eq!(node_of(&l, 60).rel, Some(Rel::NieceNephew));
    assert!(node_of(&l, 60).rect.y > a.rect.y);
    let mine = node_of(&l, 51);
    assert_eq!(mine.rel, Some(Rel::Partner(PartnerOf::Focus)));
    assert_eq!(mine.rect.y, l.nodes[0].rect.y);
}

#[test]
fn aunts_and_uncles_are_an_option() {
    let mut s = trio();
    // Parent 1 has a brother 20 (with a partner 21) through grandparents 3 and 4.
    s.family(Some(3), Some(4), &[1, 20]);
    s.family(Some(20), Some(21), &[]);
    let on = run(&s, 10, 3, true);
    let off = run(&s, 10, 3, false);
    let uncle = node_of(&on, 20);
    assert_eq!(uncle.rel, Some(Rel::AuntUncle { half: false }));
    assert_eq!(uncle.rect.y, node_of(&on, 1).rect.y);
    assert_eq!(
        node_of(&on, 21).rel,
        Some(Rel::Partner(PartnerOf::AuntUncle))
    );
    assert!(off.nodes.iter().all(|n| n.key != 20 && n.key != 21));
    assert_no_overlap(&on);
}

#[test]
fn siblings_are_not_called_half_when_one_parent_is_unknown() {
    let mut s = MapSource::default();
    s.family(Some(1), None, &[10, 11]);
    s.family(Some(1), Some(3), &[12]);
    let l = run(&s, 10, 2, true);
    for k in [11, 12] {
        assert_eq!(node_of(&l, k).rel, Some(Rel::Sibling { half: false }));
    }
}

#[test]
fn cousins_hang_below_aunts_and_uncles_one_generation_only() {
    let mut s = trio();
    // Parent 1 has a brother 20 with a child 22, who has a child 23.
    s.family(Some(3), Some(4), &[1, 20]);
    s.family(Some(20), Some(21), &[22]);
    s.family(Some(22), None, &[23]);
    let on = run(&s, 10, 3, true);
    let cousin = node_of(&on, 22);
    assert_eq!(cousin.rel, Some(Rel::Cousin));
    assert!(cousin.rect.y > node_of(&on, 20).rect.y);
    assert!(
        cousin.rect.y < on.nodes[0].rect.y,
        "cousins have their own row above the centred person's row"
    );
    assert!(cousin.more_down, "the cousin's own children are not drawn");
    assert!(on.nodes.iter().all(|n| n.key != 23));
    assert_no_overlap(&on);
    let off = run(&s, 10, 3, false);
    assert!(off.nodes.iter().all(|n| n.key != 22));
}

#[test]
fn generations_limit_is_honoured() {
    // A straight line of ancestors 100.. and of descendants 200.. around focus 0.
    let mut s = MapSource::default();
    for i in 0..10u32 {
        let child = if i == 0 { 0 } else { 99 + i };
        s.family(Some(100 + i), None, &[child]);
    }
    let mut prev = 0;
    for i in 0..10u32 {
        s.family(Some(prev), None, &[200 + i]);
        prev = 200 + i;
    }
    let l = run(&s, 0, 3, true);
    let rows: Vec<i16> = l.nodes.iter().map(|n| n.r#gen).collect();
    assert_eq!(rows.iter().min(), Some(&-3));
    assert_eq!(rows.iter().max(), Some(&3));
    assert_eq!(l.nodes.len(), 7);
    assert!(node_of(&l, 102).more_up);
    assert!(node_of(&l, 202).more_down);
    assert!(!node_of(&l, 0).more_up);
}

#[test]
fn nothing_is_hidden_when_the_person_has_no_family() {
    let l = run(&MapSource::default(), 5, 3, true);
    assert_eq!(l.nodes.len(), 1);
    assert_eq!(l.bounds.x, 0.0);
    assert_eq!(l.bounds.y, 0.0);
}

#[test]
fn cousins_who_marry_are_drawn_once_and_the_layout_ends() {
    // 3 and 4 are the grandparents of both 1 and 2, who are cousins and parents of 10.
    // 30 is a loop: the child of 10 is also the parent of 3.
    let mut s = MapSource::default();
    s.family(Some(3), Some(4), &[5, 6]);
    s.family(Some(5), Some(7), &[1]);
    s.family(Some(6), Some(8), &[2]);
    s.family(Some(1), Some(2), &[10]);
    s.family(Some(10), None, &[30]);
    s.family(Some(30), None, &[3]);
    let l = run(&s, 10, 5, true);
    for k in [1, 2, 3, 4, 5, 6, 10, 30] {
        let placed = l.nodes.iter().filter(|n| n.key == k).count();
        let full = l
            .nodes
            .iter()
            .filter(|n| n.key == k && n.dup_of.is_none())
            .count();
        assert_eq!(full, 1, "key {k} placed in full once ({placed} boxes)");
    }
    assert!(l.nodes.iter().any(|n| n.dup_of.is_some()));
    assert!(l.edges.iter().any(|e| e.kind == EdgeKind::Dup));
    assert_no_overlap(&l);
}

#[test]
fn node_cap_truncates() {
    let l = relatives(&trio(), 10, &opts(3, true), &Metrics::default(), 3);
    assert!(l.truncated);
    assert!(l.nodes.len() <= 3);
}

#[test]
fn keyboard_links() {
    let mut s = trio();
    s.family(Some(10), Some(51), &[70]);
    let l = run(&s, 10, 2, true);
    let focus = &l.nodes[0];
    let father = node_of(&l, 1);
    assert_eq!(focus.nav.parent, Some(father.id));
    assert_eq!(father.nav.child, Some(focus.id));
    assert_eq!(focus.nav.child, Some(node_of(&l, 70).id));
    // Left and right walk the row in x order, partner included.
    let left = focus.nav.prev.map(|i| l.nodes[i as usize].key);
    assert_eq!(left, Some(11));
    let right = focus.nav.next.map(|i| l.nodes[i as usize].key);
    assert_eq!(right, Some(51));
}

/// 12 siblings with partners and children, 3 generations of ancestors with aunts, and 3 of descendants.
fn big_family() -> MapSource {
    let mut s = MapSource::default();
    let mut next = 1000u32;
    let mut fresh = |n: usize| -> Vec<u32> {
        let v: Vec<u32> = (0..n).map(|i| next + i as u32).collect();
        next += n as u32;
        v
    };
    // Focus 0 and 11 siblings, all children of 1 and 2.
    let sibs: Vec<u32> = std::iter::once(0).chain(fresh(11)).collect();
    s.family(Some(1), Some(2), &sibs);
    for (i, &k) in sibs.iter().enumerate() {
        let partner = fresh(1)[0];
        let kids = fresh(2);
        s.family(Some(k), Some(partner), &kids);
        s.births.insert(k, i as i32);
        if k == 0 {
            // Three generations below the focus.
            let mut level = kids;
            for _ in 0..2 {
                let mut nxt = Vec::new();
                for &c in &level {
                    let p = fresh(1)[0];
                    let ks = fresh(2);
                    s.family(Some(c), Some(p), &ks);
                    nxt.extend(ks);
                }
                level = nxt;
            }
        }
    }
    // Three generations above: every ancestor has two parents and one sibling with a partner.
    let mut level = vec![1u32, 2];
    for _ in 0..3 {
        let mut up = Vec::new();
        for &c in &level {
            let ps = fresh(2);
            let sib = fresh(2);
            s.family(Some(ps[0]), Some(ps[1]), &[c, sib[0]]);
            let cousins = fresh(2);
            s.family(Some(sib[0]), Some(sib[1]), &cousins);
            up.extend(ps);
        }
        level = up;
    }
    s
}

#[test]
fn twelve_siblings_three_generations_each_way_do_not_overlap() {
    let s = big_family();
    let l = run(&s, 0, 3, true);
    assert!(!l.truncated);
    assert!(l.nodes.len() > 70, "{} nodes", l.nodes.len());
    assert_no_overlap(&l);
    let sibs = l
        .nodes
        .iter()
        .filter(|n| matches!(n.rel, Some(Rel::Sibling { .. })))
        .count();
    assert_eq!(sibs, 11);
    assert!(
        l.nodes
            .iter()
            .filter(|n| n.rel == Some(Rel::Cousin))
            .count()
            == 4,
        "cousins are drawn"
    );
    let rows: std::collections::BTreeSet<i16> = l.nodes.iter().map(|n| n.r#gen).collect();
    assert_eq!(rows.first(), Some(&-3));
    assert_eq!(rows.last(), Some(&3));
    // Every box and edge lies inside the reported bounds.
    for n in &l.nodes {
        assert!(n.rect.x >= 0.0 && n.rect.y >= 0.0);
        assert!(n.rect.x + n.rect.w <= l.bounds.x + l.bounds.w + 0.01);
    }
}

#[test]
fn query_finds_the_focus() {
    let l = run(&big_family(), 0, 3, true);
    let (nodes, _) = l.query(&l.nodes[0].rect);
    assert!(nodes.iter().any(|n| n.id == 0));
}
