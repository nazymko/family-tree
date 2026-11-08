//! Every chart in the four directions (ancestors on top, at the bottom, on the left, on the right): cards keep their
//! real size, nothing overlaps, no line runs through a card, the crossings are the ones of the native drawing, and
//! the oldest generation is on the asked side of the person the chart starts from.

mod common;

use common::{MapSource, assert_connectors_readable};
use gt_layout::{
    Direction, Key, Layout, ManyKin, Metrics, RelativesOptions, around, around_all_kin,
    around_many_kin, around_with_siblings, descendants, pedigree, quality, relatives,
};

type Maker<'a> = Box<dyn Fn(&Metrics) -> Layout + 'a>;

const ALL: [Direction; 4] = [
    Direction::Top,
    Direction::Bottom,
    Direction::Left,
    Direction::Right,
];

// The chosen woman F, her husband and daughters, her brother, her parents and their families.
const F: Key = 1;
const H: Key = 2;
const D1: Key = 3;
const D2: Key = 4;
const B: Key = 5;
const P: Key = 6;
const M: Key = 7;
const U: Key = 8;
const GP1: Key = 9;
const GP2: Key = 10;
const GM1: Key = 11;
const GM2: Key = 12;
const A: Key = 13;
const AH: Key = 14;
const C1: Key = 15;
const C2: Key = 16;

fn fixture() -> MapSource {
    let mut s = MapSource::default();
    s.family(Some(GP1), Some(GP2), &[P, U]);
    s.family(Some(GM1), Some(GM2), &[M, A]);
    s.family(Some(P), Some(M), &[F, B]);
    s.family(Some(F), Some(H), &[D1, D2]);
    s.family(Some(A), Some(AH), &[C1, C2]);
    s
}

fn centre(l: &Layout, key: Key) -> (f32, f32) {
    let n = l
        .nodes
        .iter()
        .find(|n| n.key == key && n.dup_of.is_none())
        .expect("placed");
    (n.rect.x + n.rect.w / 2.0, n.rect.y + n.rect.h / 2.0)
}

/// `older` lies on the side `dir` of `younger`, clearly (a whole card away, not only offset).
fn on_side(l: &Layout, older: Key, younger: Key, dir: Direction) -> bool {
    let (ox, oy) = centre(l, older);
    let (yx, yy) = centre(l, younger);
    match dir {
        Direction::Top => oy < yy - 1.0,
        Direction::Bottom => oy > yy + 1.0,
        Direction::Left => ox < yx - 1.0,
        Direction::Right => ox > yx + 1.0,
    }
}

fn assert_real_cards(l: &Layout, m: &Metrics, what: &str) {
    for n in &l.nodes {
        assert_eq!(n.rect.w, m.node_w, "{what}: card width");
        assert_eq!(n.rect.h, m.node_h, "{what}: card height");
    }
}

/// Lays out natively with the metrics for `to`, checks the native drawing, then turns it and checks it again.
fn check(
    what: &str,
    native: Direction,
    to: Direction,
    make: &dyn Fn(&Metrics) -> Layout,
    check_buses: bool,
) -> Layout {
    let real = Metrics::default();
    let lm = real.for_direction(native, to);
    let mut l = make(&lm);
    let before = quality(&l);
    if check_buses {
        assert_connectors_readable(&l);
    } else {
        assert_eq!(before.box_overlaps, 0, "{what} {to:?}: {before:?}");
        assert_eq!(before.lines_through_boxes, 0, "{what} {to:?}: {before:?}");
        assert_eq!(before.line_overlaps, 0, "{what} {to:?}: {before:?}");
    }
    let (w0, h0) = (l.bounds.w, l.bounds.h);
    l.orient(native, to);
    let q = quality(&l);
    assert_eq!(q.box_overlaps, 0, "{what} {to:?}: {q:?}");
    assert_eq!(q.lines_through_boxes, 0, "{what} {to:?}: {q:?}");
    assert_eq!(q.line_overlaps, 0, "{what} {to:?}: {q:?}");
    assert_eq!(q.line_crossings, before.line_crossings, "{what} {to:?}");
    assert_real_cards(&l, &real, &format!("{what} {to:?}"));
    // The bounds are the turned ones when the axes swapped.
    if native.is_horizontal() != to.is_horizontal() {
        assert_eq!((l.bounds.w, l.bounds.h), (h0, w0), "{what} {to:?}: bounds");
    } else {
        assert_eq!((l.bounds.w, l.bounds.h), (w0, h0), "{what} {to:?}: bounds");
    }
    l
}

#[test]
fn pedigree_in_all_four_directions() {
    let s = MapSource::ahnen_tree(31);
    for to in ALL {
        let l = check(
            "pedigree",
            Direction::Right,
            to,
            &|m| pedigree(&s, 1, 5, m, 10_000),
            false,
        );
        assert_eq!(l.nodes.len(), 31);
        for (older, younger) in [(2, 1), (3, 1), (4, 2), (13, 6), (31, 15)] {
            assert!(
                on_side(&l, older, younger, to),
                "{to:?}: {older} vs {younger}"
            );
        }
        // The two parents are centred on the child along the sibling axis.
        let (cx, cy) = centre(&l, 1);
        let (ax, ay) = centre(&l, 2);
        let (bx, by) = centre(&l, 3);
        if to.is_horizontal() {
            assert!(((ay + by) / 2.0 - cy).abs() < 1.0);
        } else {
            assert!(((ax + bx) / 2.0 - cx).abs() < 1.0);
        }
    }
}

#[test]
fn descendants_in_all_four_directions() {
    let s = fixture();
    for to in ALL {
        let l = check(
            "descendants",
            Direction::Top,
            to,
            &|m| descendants(&s, P, 4, m, 10_000),
            true,
        );
        // The root is the oldest generation: the children are on the far side.
        let far = match to {
            Direction::Top => Direction::Bottom,
            Direction::Bottom => Direction::Top,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        };
        assert!(on_side(&l, F, P, far), "{to:?}");
        assert!(on_side(&l, D1, F, far), "{to:?}");
    }
}

#[test]
fn relatives_and_around_modes_in_all_four_directions() {
    let s = fixture();
    for to in ALL {
        for chosen in [vec![F], vec![F, H], vec![F, B]] {
            let root = chosen[0];
            let makers: Vec<(&str, Maker)> = vec![
                ("line", Box::new(|m| around(&s, root, 255, m, 10_000))),
                (
                    "siblings",
                    Box::new(|m| around_with_siblings(&s, root, 255, m, 10_000)),
                ),
                (
                    "all",
                    Box::new(|m| around_all_kin(&s, root, 255, m, 10_000)),
                ),
                (
                    "relatives",
                    Box::new(|m| {
                        relatives(
                            &s,
                            root,
                            &RelativesOptions {
                                generations: 3,
                                aunts: true,
                            },
                            m,
                            10_000,
                        )
                    }),
                ),
            ];
            for (name, make) in &makers {
                let what = format!("{name} {chosen:?}");
                let l = check(&what, Direction::Top, to, make.as_ref(), true);
                assert!(on_side(&l, P, F, to), "{what} {to:?}");
                assert!(on_side(&l, GP1, P, to), "{what} {to:?}");
            }
            for (name, kin) in [
                ("many-line", ManyKin::Line),
                ("many-siblings", ManyKin::Siblings),
                ("many-all", ManyKin::All),
            ] {
                let what = format!("{name} {chosen:?}");
                let l = check(
                    &what,
                    Direction::Top,
                    to,
                    &|m| around_many_kin(&s, &chosen, 255, kin, m, 10_000),
                    true,
                );
                assert!(on_side(&l, P, F, to), "{what} {to:?}");
            }
        }
    }
}

#[test]
fn turning_twice_is_the_identity_up_to_the_native_side() {
    let s = fixture();
    let m = Metrics::default();
    let base = descendants(&s, P, 4, &m, 10_000);
    let mut l = base.clone();
    l.orient(Direction::Top, Direction::Bottom);
    l.orient(Direction::Bottom, Direction::Top);
    for (a, b) in base.nodes.iter().zip(&l.nodes) {
        assert_eq!(a.rect, b.rect);
    }
    let mut l = base.clone();
    l.orient(Direction::Top, Direction::Left);
    l.orient(Direction::Left, Direction::Top);
    for (a, b) in base.nodes.iter().zip(&l.nodes) {
        assert_eq!(a.rect, b.rect);
    }
}

#[test]
fn turned_metrics_swap_the_card_sides() {
    let m = Metrics::default();
    let t = m.for_direction(Direction::Top, Direction::Left);
    assert_eq!((t.node_w, t.node_h), (m.node_h, m.node_w));
    assert_eq!(m.for_direction(Direction::Top, Direction::Bottom), m);
    assert_eq!(m.for_direction(Direction::Right, Direction::Left), m);
    let v = m.for_direction(Direction::Right, Direction::Top);
    assert_eq!((v.node_w, v.node_h), (m.node_h, m.node_w));
}
