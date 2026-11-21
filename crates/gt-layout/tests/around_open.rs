#![allow(clippy::unwrap_used)]
//! The chart built around one person with side branches opened per person.

mod common;

use std::collections::HashSet;

use common::MapSource;
use gt_layout::{
    AroundBase, BranchDepth, Key, Layout, Metrics, OpenSpec, around, around_all_kin, around_open,
    around_with_siblings, open_flags,
};

const ALL: u8 = 255;

/// Person 1 with partner 20 and children 30 (partner 40, child 50) and 31; parents 2 and 3; father's parents
/// 6 and 7; mother's parents 8 and 9; 6's parents 60 and 61. Sister 4 (partner 70, child 71, grandchild 72). The
/// father's sister 10 (partner 11, children 12 and 13; 12 has partner 14 and child 15, who has child 16). 6's
/// sister 62 (partner 63, child 64, who has child 65).
fn family() -> MapSource {
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

fn keys(l: &Layout) -> HashSet<Key> {
    l.nodes.iter().map(|n| n.key).collect()
}

fn set(v: &[Key]) -> HashSet<Key> {
    v.iter().copied().collect()
}

fn line() -> HashSet<Key> {
    set(&[1, 2, 3, 6, 7, 8, 9, 60, 61, 20, 30, 31, 40, 50])
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

fn run(base: AroundBase, open: &[OpenSpec]) -> Layout {
    let l = around_open(&family(), 1, ALL, base, open, &Metrics::default(), 100_000);
    assert_clean(&l);
    l
}

fn assert_clean(l: &Layout) {
    for (i, a) in l.nodes.iter().enumerate() {
        for b in &l.nodes[i + 1..] {
            let apart = a.rect.x + a.rect.w <= b.rect.x
                || b.rect.x + b.rect.w <= a.rect.x
                || a.rect.y + a.rect.h <= b.rect.y
                || b.rect.y + b.rect.h <= a.rect.y;
            assert!(apart, "nodes {} and {} overlap", a.key, b.key);
        }
    }
    common::assert_connectors_readable(l);
}

fn shape(l: &Layout) -> Vec<(Key, f32, f32, Option<u32>)> {
    l.nodes
        .iter()
        .map(|n| (n.key, n.rect.x, n.rect.y, n.dup_of))
        .collect()
}

#[test]
fn no_openings_is_the_old_chart_for_every_base() {
    let s = family();
    let m = Metrics::default();
    let cases = [
        (AroundBase::Line, around(&s, 1, ALL, &m, 100_000)),
        (
            AroundBase::Siblings,
            around_with_siblings(&s, 1, ALL, &m, 100_000),
        ),
        (AroundBase::All, around_all_kin(&s, 1, ALL, &m, 100_000)),
    ];
    for (base, old) in cases {
        let new = around_open(&s, 1, ALL, base, &[], &m, 100_000);
        assert_eq!(shape(&new), shape(&old));
        assert_eq!(new.edges.len(), old.edges.len());
    }
}

#[test]
fn siblings_of_an_ancestor_with_children() {
    let l = run(AroundBase::Line, &[sib(6, BranchDepth::Children)]);
    let mut want = line();
    want.extend([62, 63, 64]);
    assert_eq!(keys(&l), want);
}

#[test]
fn siblings_of_an_ancestor_with_all_descendants() {
    let l = run(AroundBase::Line, &[sib(6, BranchDepth::All)]);
    let mut want = line();
    want.extend([62, 63, 64, 65]);
    assert_eq!(keys(&l), want);
}

#[test]
fn rows_of_the_opened_family() {
    let l = run(AroundBase::Line, &[sib(2, BranchDepth::Children)]);
    let y = |k: Key| l.nodes.iter().find(|n| n.key == k).unwrap().rect.y;
    assert_eq!(y(10), y(2));
    assert_eq!(y(11), y(2));
    assert_eq!(y(12), y(1));
    assert_eq!(y(13), y(1));
    assert!(!keys(&l).contains(&15));
}

#[test]
fn an_opening_under_a_closed_one_has_no_effect() {
    let l = run(AroundBase::Line, &[kids(12, BranchDepth::All)]);
    assert_eq!(keys(&l), line());
}

#[test]
fn children_of_an_opened_relative_go_one_step_at_a_time() {
    let open = [
        sib(2, BranchDepth::Children),
        kids(12, BranchDepth::Children),
    ];
    let l = run(AroundBase::Line, &open);
    assert!(keys(&l).is_superset(&set(&[10, 11, 12, 13, 14, 15])));
    assert!(!keys(&l).contains(&16));
    let open = [sib(2, BranchDepth::Children), kids(12, BranchDepth::All)];
    assert!(keys(&run(AroundBase::Line, &open)).contains(&16));
}

#[test]
fn the_siblings_base_keeps_the_persons_own_brothers_and_sisters() {
    let l = run(AroundBase::Siblings, &[sib(6, BranchDepth::Children)]);
    let k = keys(&l);
    assert!(k.is_superset(&set(&[4, 70, 71, 62, 63, 64])));
    assert!(!k.contains(&72) && !k.contains(&65) && !k.contains(&10));
}

#[test]
fn with_the_all_base_children_can_still_be_opened() {
    // Everything is shown already, so nothing new appears and the chart is clean.
    let all = run(AroundBase::All, &[]);
    let opened = run(AroundBase::All, &[sib(6, BranchDepth::Children)]);
    assert_eq!(keys(&all), keys(&opened));
}

#[test]
fn several_openings_do_not_overlap() {
    let open = [
        sib(1, BranchDepth::All),
        sib(2, BranchDepth::All),
        sib(6, BranchDepth::All),
    ];
    let l = run(AroundBase::Line, &open);
    assert!(keys(&l).len() > line().len() + 10);
}

#[test]
fn flags_say_who_can_be_opened() {
    let s = family();
    let m = Metrics::default();
    let flags = |open: &[OpenSpec]| {
        let l = around_open(&s, 1, ALL, AroundBase::Line, open, &m, 100_000);
        let f = open_flags(&s, &l, AroundBase::Line);
        move |k: Key| {
            let i = l.nodes.iter().position(|n| n.key == k).unwrap();
            f[i]
        }
    };
    let f = flags(&[]);
    assert!(f(1).0, "the person has a sister");
    assert!(f(2).0);
    assert!(f(6).0);
    assert!(!f(7).0, "no brothers or sisters");
    assert_eq!(f(20), (false, false), "a partner");
    assert_eq!(f(30), (false, false), "a descendant on the line");
    assert!(!f(1).1, "children are shown");
    let f = flags(&[sib(2, BranchDepth::Children)]);
    assert!(f(12).1, "a cousin with hidden children");
    assert!(!f(13).1, "no children");
    assert_eq!(f(11), (false, false), "an in-law");
    let all = around_open(&s, 1, ALL, AroundBase::All, &[], &m, 100_000);
    let f = open_flags(&s, &all, AroundBase::All);
    assert!(f.iter().all(|x| !x.0), "all relatives are shown");
}
