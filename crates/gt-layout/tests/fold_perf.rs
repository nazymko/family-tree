#![allow(clippy::unwrap_used)]
//! How long the compact print layout (fold) takes on a big "all kin" chart. A fold that tried every grid shape against
//! every run, each time measuring the whole chart, took a minute at 3,000 people and held the app's one session lock.

mod common;

use std::time::Instant;

use common::MapSource;
use gt_layout::{AroundBase, Key, Layout, ManyKin, Metrics, around_many_kin, around_open};

/// Person 1 with `gens` generations of ancestors (Ahnentafel keys); every ancestor below the top row and person 1
/// have 12 to 20 brothers and sisters, and every fifth of them is married with eight children.
fn all_kin(gens: u32) -> MapSource {
    let mut s = MapSource::default();
    let mut next: Key = 1_000_000;
    for n in 1..(1u32 << gens) {
        let count = 12 + (n * 7) % 9;
        let mut kids: Vec<Key> = Vec::new();
        for i in 0..count {
            let k = next;
            next += 1;
            kids.push(k);
            if i % 5 == 2 {
                let partner = next;
                let children: Vec<Key> = (next + 1..next + 9).collect();
                next += 9;
                s.family(Some(k), Some(partner), &children);
            }
        }
        kids.insert(kids.len() / 2, n);
        s.family(Some(2 * n), Some(2 * n + 1), &kids);
    }
    s
}

fn time(f: impl Fn() -> Layout) -> (Layout, f64) {
    let t = Instant::now();
    let l = f();
    (l, t.elapsed().as_secs_f64() * 1000.0)
}

/// Prints the timings with fold off and on for growing charts (`cargo test --release --test fold_perf -- --ignored
/// --nocapture`).
#[test]
#[ignore = "timing test: run with --release -- --ignored"]
fn fold_timings() {
    let m = Metrics::default();
    for gens in 3..=6u32 {
        let s = all_kin(gens);
        let g = gens as u8;
        let (off, t_off) = time(|| around_open(&s, 1, g, AroundBase::All, &[], false, &m, 100_000));
        let (on, t_on) = time(|| around_open(&s, 1, g, AroundBase::All, &[], true, &m, 100_000));
        let (q0, q1) = (gt_layout::quality(&off), gt_layout::quality(&on));
        eprintln!(
            "{} people: fold off {t_off:.1} ms, on {t_on:.1} ms; crossings {} -> {}, aspect {:.2} -> {:.2}",
            off.nodes.len(),
            q0.line_crossings,
            q1.line_crossings,
            off.bounds.w / off.bounds.h,
            on.bounds.w / on.bounds.h,
        );
        let (_, t_many) = time(|| around_many_kin(&s, &[1], g, ManyKin::All, true, &m, 100_000));
        eprintln!("  several people, all, fold on: {t_many:.1} ms");
    }
}

/// About 3,000 people with all kin fold in under a second (release build).
#[test]
#[ignore = "timing test: run with --release -- --ignored"]
fn fold_3k_under_1s_release() {
    let s = all_kin(6);
    let m = Metrics::default();
    let (on, ms) = time(|| around_open(&s, 1, 6, AroundBase::All, &[], true, &m, 100_000));
    assert!(on.nodes.len() >= 2_500, "{} people", on.nodes.len());
    assert!(
        ms < 1_000.0,
        "took {ms:.0} ms for {} people",
        on.nodes.len()
    );
    let off = around_open(&s, 1, 6, AroundBase::All, &[], false, &m, 100_000);
    let (q0, q1) = (gt_layout::quality(&off), gt_layout::quality(&on));
    assert!(q1.line_crossings <= q0.line_crossings, "{q0:?} {q1:?}");
    assert!(q1.lines_through_boxes <= q0.lines_through_boxes);
    assert_eq!(q1.box_overlaps, 0);
}

/// A debug build is far slower, so the bound is loose there; it still catches the minute-long search.
#[test]
fn fold_1k_finishes_in_debug() {
    let s = all_kin(5);
    let m = Metrics::default();
    let (on, ms) = time(|| around_open(&s, 1, 5, AroundBase::All, &[], true, &m, 100_000));
    assert!(on.nodes.len() >= 1_000, "{} people", on.nodes.len());
    let bound = if cfg!(debug_assertions) {
        20_000.0
    } else {
        1_000.0
    };
    assert!(ms < bound, "took {ms:.0} ms for {} people", on.nodes.len());
}
