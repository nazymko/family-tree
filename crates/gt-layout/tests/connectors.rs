//! The connector lines of the "built around" charts and the relatives chart stay readable: every family has its own
//! bus, buses near each other get different lanes, no vertical line of one family lines up with another's, and a
//! couple's children hang from the middle of a partner connector long enough to be seen.

mod common;

use common::{MapSource, assert_connectors_readable};
use gt_layout::{
    Edge, EdgeKind, Key, Layout, ManyKin, Metrics, RelativesOptions, around, around_all_kin,
    around_many_kin, around_with_siblings, relatives,
};

// The chosen woman, her husband and daughters, her brother, her parents and their families.
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
const GG1: Key = 17;
const GG2: Key = 18;
const GU: Key = 19;
const GG3: Key = 20;

/// The shape of a real chart that read badly: three families of great-grandparents in one row, two couples of
/// grandparents, an aunt with her own family beside the parents' children, a husband and two daughters.
fn fixture() -> MapSource {
    let mut s = MapSource::default();
    s.family(Some(GG1), Some(GG2), &[GM1, GU]);
    s.family(Some(GG3), None, &[GM2]);
    s.family(Some(GP1), Some(GP2), &[P, U]);
    s.family(Some(GM1), Some(GM2), &[M, A]);
    s.family(Some(P), Some(M), &[F, B]);
    s.family(Some(F), Some(H), &[D1, D2]);
    s.family(Some(A), Some(AH), &[C1, C2]);
    s
}

fn id(l: &Layout, key: Key) -> u32 {
    l.nodes
        .iter()
        .find(|n| n.key == key && n.dup_of.is_none())
        .unwrap_or_else(|| panic!("{key} placed"))
        .id
}

/// The parent-to-child edges that end at any of `keys`.
fn into<'a>(l: &'a Layout, keys: &[Key]) -> Vec<&'a Edge> {
    let ids: Vec<u32> = keys.iter().map(|&k| id(l, k)).collect();
    l.edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Parent && ids.contains(&e.to))
        .collect()
}

/// The bus of the family whose children are `keys`: its height and the span along x.
fn bus(l: &Layout, keys: &[Key]) -> (f32, f32, f32) {
    let mut found: Option<(f32, f32, f32)> = None;
    for e in into(l, keys).into_iter().filter(|e| e.pts.len() == 8) {
        let (y, x0, x1) = (e.pts[3], e.pts[2].min(e.pts[4]), e.pts[2].max(e.pts[4]));
        let f = found.get_or_insert((y, x0, x1));
        assert!((f.0 - y).abs() < 0.01, "one family, one bus");
        f.1 = f.1.min(x0);
        f.2 = f.2.max(x1);
    }
    found.unwrap_or_else(|| panic!("a bus into {keys:?}"))
}

/// Two buses read as separate: on different lanes, or ending a gap apart.
fn separate(a: (f32, f32, f32), b: (f32, f32, f32), m: &Metrics) -> bool {
    (a.0 - b.0).abs() >= 6.0 || a.1 - b.2 >= m.gap_x || b.1 - a.2 >= m.gap_x
}

fn modes(s: &MapSource, chosen: &[Key]) -> Vec<(String, Layout)> {
    let m = Metrics::default();
    let root = chosen[0];
    let mut v = vec![
        ("line".to_string(), around(s, root, 255, &m, 10_000)),
        (
            "siblings".to_string(),
            around_with_siblings(s, root, 255, &m, 10_000),
        ),
        ("all".to_string(), around_all_kin(s, root, 255, &m, 10_000)),
        (
            "relatives".to_string(),
            relatives(
                s,
                root,
                &RelativesOptions {
                    generations: 3,
                    aunts: true,
                },
                &m,
                10_000,
            ),
        ),
    ];
    for (name, kin) in [
        ("many-line", ManyKin::Line),
        ("many-siblings", ManyKin::Siblings),
        ("many-all", ManyKin::All),
    ] {
        v.push((
            name.to_string(),
            around_many_kin(s, chosen, 255, kin, &m, 10_000),
        ));
    }
    v
}

#[test]
fn every_mode_is_readable_for_one_chosen_person_and_for_a_couple() {
    let s = fixture();
    for chosen in [vec![F], vec![F, H], vec![F, B], vec![D1]] {
        for (name, l) in modes(&s, &chosen) {
            let q = gt_layout::quality(&l);
            assert_eq!(q.box_overlaps, 0, "{name} {chosen:?}: {q:?}");
            assert_eq!(q.lines_through_boxes, 0, "{name} {chosen:?}: {q:?}");
            assert_eq!(q.line_overlaps, 0, "{name} {chosen:?}: {q:?}");
            assert_eq!(q.bus_clashes, 0, "{name} {chosen:?}: {q:?}");
            assert_eq!(q.aligned_verticals, 0, "{name} {chosen:?}: {q:?}");
            assert_connectors_readable(&l);
        }
    }
}

#[test]
fn defect_1_great_grandparents_of_two_families_do_not_share_a_bus() {
    let m = Metrics::default();
    for (name, l) in [
        ("all", around_all_kin(&fixture(), F, 255, &m, 10_000)),
        (
            "many-all",
            around_many_kin(&fixture(), &[F, H], 255, ManyKin::All, &m, 10_000),
        ),
    ] {
        let a = bus(&l, &[GM1, GU]);
        let b = bus(&l, &[GM2]);
        assert!(separate(a, b, &m), "{name}: {a:?} vs {b:?}");
        // GG1 and GG2 drop to the bus of their own children only.
        for e in into(&l, &[GM2]) {
            assert!(
                [id(&l, GG3)].contains(&e.from),
                "{name}: only the lone parent drops to GM2"
            );
        }
        for e in into(&l, &[GM1, GU]) {
            assert!(
                [id(&l, GG1), id(&l, GG2)].contains(&e.from),
                "{name}: only GG1 and GG2 drop to GM1 and GU"
            );
        }
    }
}

#[test]
fn defect_2_the_two_couples_of_grandparents_have_buses_apart() {
    let m = Metrics::default();
    let l = around_all_kin(&fixture(), F, 255, &m, 10_000);
    let a = bus(&l, &[P, U]);
    let b = bus(&l, &[M, A]);
    assert!(separate(a, b, &m), "{a:?} vs {b:?}");
}

#[test]
fn defect_3_the_mothers_line_is_not_the_line_from_the_couple_to_the_daughters() {
    let m = Metrics::default();
    for l in [
        around_all_kin(&fixture(), F, 255, &m, 10_000),
        around_many_kin(&fixture(), &[F, H], 255, ManyKin::All, &m, 10_000),
        around_with_siblings(&fixture(), F, 255, &m, 10_000),
    ] {
        let trunk = into(&l, &[D1, D2])[0].pts[0];
        // Every vertical line of the families on the rows above, near the couple's line, keeps its distance.
        for e in into(&l, &[F, B]) {
            for x in [e.pts[0], e.pts[e.pts.len() - 2]] {
                assert!(
                    (x - trunk).abs() >= m.gap_x / 2.0,
                    "a line at x={x} lines up with the couple's line at x={trunk}"
                );
            }
        }
    }
}

#[test]
fn defect_4_the_children_of_the_parents_and_of_the_aunt_do_not_read_as_one_bar() {
    let m = Metrics::default();
    let l = around_all_kin(&fixture(), F, 255, &m, 10_000);
    let a = bus(&l, &[F, B]);
    let b = bus(&l, &[C1, C2]);
    assert!(separate(a, b, &m), "{a:?} vs {b:?}");
}

#[test]
fn defect_5_the_partner_connector_is_long_and_the_children_hang_from_its_middle() {
    let m = Metrics::default();
    for (name, l) in modes(&fixture(), &[F]) {
        for e in l.edges.iter().filter(|e| e.kind == EdgeKind::Partner) {
            let len = (e.pts[2] - e.pts[0]).abs();
            assert!(len >= 20.0, "{name}: partner connector {len} long");
        }
    }
    for (name, l) in [
        ("all", around_all_kin(&fixture(), F, 255, &m, 10_000)),
        (
            "many-all",
            around_many_kin(&fixture(), &[F, H], 255, ManyKin::All, &m, 10_000),
        ),
    ] {
        // Each couple's children start between the two cards, level with the partner connector.
        for (parents, kids) in [
            ([F, H], vec![D1, D2]),
            ([A, AH], vec![C1, C2]),
            ([P, M], vec![F, B]),
            ([GP1, GP2], vec![P, U]),
            ([GM1, GM2], vec![M, A]),
        ] {
            let (a, b) = (id(&l, parents[0]), id(&l, parents[1]));
            let conn = l
                .edges
                .iter()
                .find(|e| {
                    e.kind == EdgeKind::Partner
                        && [a, b].contains(&e.from)
                        && [a, b].contains(&e.to)
                })
                .unwrap_or_else(|| panic!("{name}: partner connector {parents:?}"));
            let (x0, x1) = (conn.pts[0].min(conn.pts[2]), conn.pts[0].max(conn.pts[2]));
            for e in into(&l, &kids) {
                assert!(
                    e.pts[0] >= x0 && e.pts[0] <= x1,
                    "{name}: {parents:?} drop at {} outside the connector {x0}..{x1}",
                    e.pts[0]
                );
                assert!((e.pts[1] - conn.pts[1]).abs() < 0.01);
            }
        }
    }
}

#[test]
fn a_lone_parents_children_hang_from_the_card_centre() {
    let m = Metrics::default();
    let l = around_all_kin(&fixture(), F, 255, &m, 10_000);
    let g = l.nodes[id(&l, GG3) as usize].rect;
    for e in into(&l, &[GM2]) {
        assert_eq!(e.pts[1], g.y + g.h, "starts at the card's bottom edge");
        assert!((e.pts[0] - (g.x + g.w / 2.0)).abs() <= g.w / 2.0 - 16.0 + 0.01);
    }
}

#[test]
fn rows_grow_when_a_gap_needs_more_lanes() {
    // Six families side by side under one row of parents: they need several lanes; no two buses clash and the
    // chart stays free of overlaps.
    let mut s = MapSource::default();
    let roots: Vec<Key> = (1..=6).collect();
    s.family(Some(100), Some(101), &roots);
    for (i, &r) in roots.iter().enumerate() {
        let i = i as Key;
        s.family(Some(r), Some(200 + i), &[300 + i * 2, 301 + i * 2]);
    }
    let l = around_many_kin(
        &s,
        &[100],
        255,
        ManyKin::Siblings,
        &Metrics::default(),
        10_000,
    );
    assert_connectors_readable(&l);
}

#[test]
fn an_ancestors_brother_stands_on_the_outer_side_of_the_couple_and_no_line_crosses_another_family()
{
    let m = Metrics::default();
    let s = fixture();
    for (name, l) in [
        ("all", around_all_kin(&s, F, 255, &m, 10_000)),
        (
            "many-all",
            around_many_kin(&s, &[F], 255, ManyKin::All, &m, 10_000),
        ),
        (
            "many-all-couple",
            around_many_kin(&s, &[F, H], 255, ManyKin::All, &m, 10_000),
        ),
        ("siblings", around_with_siblings(&s, F, 255, &m, 10_000)),
    ] {
        let x = |k: Key| l.nodes[id(&l, k) as usize].rect.x;
        let q = gt_layout::quality(&l);
        assert_eq!(q.line_crossings, 0, "{name}: {q:?}");
        if name != "siblings" {
            // GM1 is the left partner of GM1 + GM2: its brother GU stands left of it.
            assert!(
                x(GU) < x(GM1),
                "{name}: GU at {} left of GM1 at {}",
                x(GU),
                x(GM1)
            );
            assert!(x(GM1) < x(GM2), "{name}: parents keep their order");
        }
    }
}
