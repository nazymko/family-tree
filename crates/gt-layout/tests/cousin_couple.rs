//! Two chosen first cousins through their mothers: the partner of one cousin has parents of their own on the chart.
//! Each partner of a couple must stand on the side of their own parents so no parent-to-child connector crosses.

mod common;

use common::MapSource;
use gt_layout::{Key, Layout, ManyKin, Metrics, around_many_kin, quality};

const FEDIR: Key = 1;
const FENNA: Key = 2;
const MARA: Key = 3;
const KATA: Key = 4;
const LEV: Key = 5;
const MYKOLA: Key = 6;
const NINA: Key = 7;
const HANNA: Key = 8;
const IVAN: Key = 9;
const DOMNA: Key = 10;
const HUSBAND: Key = 11;

fn shape(with_lev: bool, wide: bool) -> MapSource {
    let mut s = MapSource::default();
    s.family(Some(FEDIR), Some(FENNA), &[MARA, KATA]);
    s.family(Some(IVAN), Some(DOMNA), &[MYKOLA]);
    s.family(with_lev.then_some(LEV), Some(MARA), &[NINA]);
    s.family(Some(MYKOLA), Some(KATA), &[HANNA]);
    if wide {
        s.family(Some(HANNA), Some(HUSBAND), &[20, 21]);
        s.family(Some(20), Some(22), &[24, 25]);
        s.family(Some(21), Some(23), &[26]);
        s.family(Some(24), Some(27), &[28]);
    }
    s
}

fn check(name: &str, l: &Layout) {
    let q = quality(l);
    assert_eq!(q.line_crossings, 0, "{name}: {q:?}");
    assert_eq!(q.line_overlaps, 0, "{name}: {q:?}");
    assert_eq!(q.bus_clashes, 0, "{name}: {q:?}");
}

#[test]
fn partners_with_parents_on_the_chart_stand_on_their_parents_side() {
    let m = Metrics::default();
    for (lev, wide) in [(true, true), (true, false), (false, true), (false, false)] {
        for kin in [ManyKin::Line, ManyKin::Siblings, ManyKin::All] {
            let s = shape(lev, wide);
            for chosen in [
                vec![NINA, HANNA],
                vec![HANNA, NINA],
                vec![HANNA],
                vec![NINA],
            ] {
                let l = around_many_kin(&s, &chosen, 255, kin, false, &m, 10_000);
                check(&format!("lev={lev} wide={wide} {kin:?} {chosen:?}"), &l);
            }
        }
    }
}
