//! A person with a partnered family and a family without a second parent: every family's children hang below that
//! family's own anchor (the couple connector, or the person's own card for the partnerless family), the groups stand
//! in the order of the anchors and no child line crosses another family's line or a couple connector.
//!
//! Invented people: GP/GM (100, 101) have M (10). M has P (11) with child C1 (21), and a family alone with C2 (22).

mod common;

use common::MapSource;
use gt_layout::{
    EdgeKind, Key, Layout, Metrics, Node, RelativesOptions, around, around_all_kin,
    around_with_siblings, descendants, relatives,
};

const M: Key = 10;

fn node(l: &Layout, key: Key) -> Option<&Node> {
    l.nodes.iter().find(|n| n.key == key && n.dup_of.is_none())
}

fn cx(l: &Layout, key: Key) -> f32 {
    let n = node(l, key).unwrap_or_else(|| panic!("key {key} placed"));
    n.rect.x + n.rect.w / 2.0
}

type Seg = (f32, f32, f32, f32);

fn segs(pts: &[f32]) -> Vec<Seg> {
    pts.windows(4)
        .step_by(2)
        .map(|w| (w[0], w[1], w[2], w[3]))
        .collect()
}

fn side(a: (f32, f32), b: (f32, f32), p: (f32, f32)) -> f32 {
    (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
}

/// Two segments cross in the interior of both (touching at an end or running along each other is fine).
fn cross(a: Seg, b: Seg) -> bool {
    let (a0, a1, b0, b1) = ((a.0, a.1), (a.2, a.3), (b.0, b.1), (b.2, b.3));
    let e = 0.01;
    let (d1, d2, d3, d4) = (
        side(a0, a1, b0),
        side(a0, a1, b1),
        side(b0, b1, a0),
        side(b0, b1, a1),
    );
    d1.abs() > e && d2.abs() > e && d3.abs() > e && d4.abs() > e && d1 * d2 < 0.0 && d3 * d4 < 0.0
}

fn on(seg: &[Seg], x: f32, y: f32) -> bool {
    seg.iter().any(|&(x0, y0, x1, y1)| {
        x >= x0.min(x1) - 0.6
            && x <= x0.max(x1) + 0.6
            && y >= y0.min(y1) - 0.6
            && y <= y0.max(y1) + 0.6
    })
}

/// The Parent line(s) that end in `child`.
fn lines_of(l: &Layout, child: Key) -> Vec<Vec<Seg>> {
    let id = node(l, child).expect("child placed").id;
    l.edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Parent && e.to == id)
        .map(|e| segs(&e.pts))
        .collect()
}

/// The connector between M and the partner, if both are drawn.
fn connector(l: &Layout, partner: Key) -> Option<Vec<Seg>> {
    let (m, p) = (node(l, M)?.id, node(l, partner)?.id);
    l.edges
        .iter()
        .find(|e| {
            e.kind == EdgeKind::Partner
                && ((e.from == m && e.to == p) || (e.from == p && e.to == m))
        })
        .map(|e| segs(&e.pts))
}

type Group<'a> = (Option<Key>, &'a [Key]);

/// `groups[i]` = (partner of that family or None, its children).
fn source(groups: &[Group]) -> MapSource {
    let mut s = MapSource::default();
    s.family(Some(100), Some(101), &[M]);
    for &(p, kids) in groups {
        s.family(Some(M), p, kids);
    }
    s
}

type Chart = (&'static str, Box<dyn Fn(&MapSource) -> Layout>);

fn charts(centre: Key) -> Vec<Chart> {
    let m = Metrics::default;
    vec![
        (
            "descendants(M)",
            Box::new(move |s| descendants(s, M, 20, &m(), 100_000)),
        ),
        (
            "relatives(M)",
            Box::new(move |s| {
                relatives(
                    s,
                    M,
                    &RelativesOptions {
                        generations: 4,
                        aunts: false,
                    },
                    &m(),
                    100_000,
                )
            }),
        ),
        (
            "around(centre)",
            Box::new(move |s| around(s, centre, 255, &m(), 100_000)),
        ),
        (
            "around_with_siblings(centre)",
            Box::new(move |s| around_with_siblings(s, centre, 255, &m(), 100_000)),
        ),
        (
            "around_all_kin(M)",
            Box::new(move |s| around_all_kin(s, M, 255, &m(), 100_000)),
        ),
        (
            "around_all_kin(GP)",
            Box::new(move |s| around_all_kin(s, 100, 255, &m(), 100_000)),
        ),
        (
            "descendants(GP)",
            Box::new(move |s| descendants(s, 100, 20, &m(), 100_000)),
        ),
    ]
}

/// `expected`: (partner, children) in the expected left-to-right order of the anchors.
fn check(groups: &[Group], expected: &[Group], centre: Key) {
    let s = source(groups);
    for (name, run) in charts(centre) {
        let l = run(&s);
        let ctx = |m: String| format!("{name}: {m}");
        // The order and the hanging place are only defined where the couple connector is drawn: a chart that shows
        // the parents of a child draws two lines into the parents' row and no connector, or no partner at all.
        let drawn = expected
            .iter()
            .filter_map(|g| g.0)
            .all(|p| connector(&l, p).is_some());
        // Child groups in the order of the anchors (children drawn in this chart only).
        let mut last: Option<(Key, f32)> = None;
        for &(_, kids) in expected.iter().filter(|_| drawn) {
            let xs: Vec<(Key, f32)> = kids
                .iter()
                .filter(|&&k| node(&l, k).is_some())
                .map(|&k| (k, cx(&l, k)))
                .collect();
            if let (Some(&(pk, px)), Some(lo)) =
                (last.as_ref(), xs.iter().map(|c| c.1).reduce(f32::min))
            {
                assert!(
                    px < lo,
                    "{}",
                    ctx(format!("group of {pk} (x {px}) is not left of {xs:?}"))
                );
            }
            if let Some(hi) = xs.iter().max_by(|a, b| a.1.total_cmp(&b.1)) {
                last = Some(*hi);
            }
        }
        // Every child hangs from its own family's anchor.
        for &(partner, kids) in expected.iter().filter(|_| drawn) {
            for &k in kids {
                if node(&l, k).is_none() {
                    continue;
                }
                let lines = lines_of(&l, k);
                assert!(
                    !lines.is_empty(),
                    "{}",
                    ctx(format!("child {k} has a line"))
                );
                let starts: Vec<(f32, f32)> = lines.iter().map(|s| (s[0].0, s[0].1)).collect();
                match partner {
                    // This chart does not draw that partner (a chart centred on a child shows only its parents).
                    Some(p) if node(&l, p).is_none() => {}
                    Some(p) => {
                        let c =
                            connector(&l, p).unwrap_or_else(|| panic!("{name}: connector to {p}"));
                        assert!(
                            starts.iter().any(|&(x, y)| on(&c, x, y)),
                            "{}",
                            ctx(format!(
                                "child {k} does not hang from the connector to {p}: {starts:?}"
                            ))
                        );
                    }
                    None => {
                        for p2 in groups.iter().filter_map(|g| g.0) {
                            if let Some(c) = connector(&l, p2) {
                                assert!(
                                    starts.iter().all(|&(x, y)| !on(&c, x, y)),
                                    "{}",
                                    ctx(format!(
                                        "partnerless child {k} hangs from the connector to {p2}"
                                    ))
                                );
                            }
                        }
                    }
                }
            }
        }
        // No two lines of different children cross, and no child line crosses a couple connector.
        let placed: Vec<Key> = expected
            .iter()
            .flat_map(|g| g.1.iter().copied())
            .filter(|&k| node(&l, k).is_some())
            .collect();
        for (i, &a) in placed.iter().enumerate() {
            for la in lines_of(&l, a) {
                for &b in &placed[i + 1..] {
                    for lb in lines_of(&l, b) {
                        for &sa in &la {
                            for &sb in &lb {
                                assert!(
                                    !cross(sa, sb),
                                    "{}",
                                    ctx(format!("lines of {a} and {b} cross"))
                                );
                            }
                        }
                    }
                }
                for p in expected.iter().filter_map(|g| g.0) {
                    if let Some(c) = connector(&l, p) {
                        for &sa in &la {
                            for &sc in &c {
                                assert!(
                                    !cross(sa, sc),
                                    "{}",
                                    ctx(format!("line of {a} crosses the connector to {p}"))
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

const P: Key = 11;
const C1: Key = 21;
const C2: Key = 22;

#[test]
fn centre_is_the_child_of_the_partnerless_family() {
    // The couple family is listed first: its child used to take the left place and cross the other line.
    check(
        &[(Some(P), &[C1]), (None, &[C2])],
        &[(None, &[C2]), (Some(P), &[C1])],
        C2,
    );
}

#[test]
fn centre_is_the_child_of_the_couple_family() {
    check(
        &[(Some(P), &[C1]), (None, &[C2])],
        &[(None, &[C2]), (Some(P), &[C1])],
        C1,
    );
}

#[test]
fn partnerless_family_listed_first_gives_the_same_picture() {
    check(
        &[(None, &[C2]), (Some(P), &[C1])],
        &[(None, &[C2]), (Some(P), &[C1])],
        C2,
    );
}

#[test]
fn partnerless_children_of_several_kids() {
    check(
        &[(Some(P), &[21, 23]), (None, &[22, 24])],
        &[(None, &[22, 24]), (Some(P), &[21, 23])],
        22,
    );
}

#[test]
fn two_partners_and_a_partnerless_family_the_partnerless_stands_in_the_middle() {
    // Row: P1 - M - P2; anchors left to right: connector to P1, M, connector to P2.
    check(
        &[(Some(11), &[21]), (Some(12), &[23]), (None, &[22])],
        &[(Some(11), &[21]), (None, &[22]), (Some(12), &[23])],
        22,
    );
    check(
        &[(None, &[22]), (Some(11), &[21]), (Some(12), &[23])],
        &[(Some(11), &[21]), (None, &[22]), (Some(12), &[23])],
        21,
    );
}

#[test]
fn without_a_partnerless_family_nothing_changes() {
    check(
        &[(Some(11), &[21]), (Some(12), &[23])],
        &[(Some(11), &[21]), (Some(12), &[23])],
        21,
    );
}

/// The whole-tree chart: 0 = GP, 1 = M, 2 = P, 3 = C1 (couple), 4 = C2 (M alone). Lines of different families must
/// not cross, and C2 does not stand on P's side of C1.
#[test]
fn whole_tree_children_of_the_partnerless_family_do_not_cross_the_couple_family() {
    use gt_layout::{WholeFamily, WholeInput, whole};
    let fam = |a, b, kids: &[u32]| WholeFamily {
        partners: [Some(a), b],
        children: kids.to_vec(),
    };
    let input = WholeInput {
        count: 5,
        families: vec![
            fam(0, None, &[1]),
            fam(1, Some(2), &[3]),
            fam(1, None, &[4]),
        ],
        birth: vec![None; 5],
        surname: vec![String::new(); 5],
        include: None,
    };
    let w = whole(&input, &Metrics::default());
    let id = |k: Key| w.extras.node_of(k).expect("person placed");
    let x = |k: Key| {
        let n = &w.layout.nodes[id(k) as usize];
        n.rect.x + n.rect.w / 2.0
    };
    let lines = |k: Key| -> Vec<Vec<Seg>> {
        w.layout
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Parent && e.to == id(k))
            .map(|e| segs(&e.pts))
            .collect()
    };
    for a in lines(3) {
        for b in lines(4) {
            for &sa in &a {
                for &sb in &b {
                    assert!(!cross(sa, sb), "the lines of C1 and C2 cross");
                }
            }
        }
    }
    for e in w
        .layout
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Partner)
    {
        for k in [3, 4] {
            for l in lines(k) {
                for &s in &l {
                    assert!(
                        !cross(s, segs(&e.pts)[0]),
                        "a child line crosses a couple connector"
                    );
                }
            }
        }
    }
    // C2 (M alone) is not under P: P stands on the other side of M.
    assert!((x(4) - x(2)).abs() > 1.0 || (x(4) - x(1)).abs() < (x(2) - x(1)).abs());
}
