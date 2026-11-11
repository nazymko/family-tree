//! A person with children by several partners, drawn as part of a collateral branch (a descendant of a brother of
//! an ancestor) or as the centre of a chart: the person stands among their partners, every couple connector joins
//! the person to that partner (never one partner to the next), and each child hangs from its own parents' connector.
//!
//! Invented people: GP/GM (100, 101) have X (1) and S (2); S's child Z (3) is the centre of the cousin charts; X's
//! child A (10) is the person with several partners: partners 11, 12, 13 with children 21.., 22.., 23..

mod common;

use common::MapSource;
use gt_layout::{
    EdgeKind, Key, Layout, Metrics, Node, RelativesOptions, around, around_all_kin,
    around_with_siblings, descendants, relatives,
};

const A: Key = 10;
const SIBLING_BEFORE: Key = 9;
const SIBLING_AFTER: Key = 19;

/// `partners[i]` = (partner key, children by that partner).
fn branch(partners: &[(Key, &[Key])], siblings: bool) -> MapSource {
    let mut s = MapSource::default();
    let mut kids_of_x = vec![A];
    if siblings {
        kids_of_x = vec![SIBLING_BEFORE, A, SIBLING_AFTER];
    }
    s.family(Some(100), Some(101), &[1, 2]);
    s.family(Some(2), Some(50), &[3]);
    s.family(Some(1), Some(60), &kids_of_x);
    for (i, &k) in kids_of_x.iter().enumerate() {
        s.births.insert(k, i as i32);
    }
    for &(p, kids) in partners {
        s.family(Some(A), Some(p), kids);
    }
    if siblings {
        // The brothers and sisters have a family of their own, so the row is as wide as in a real tree.
        s.family(Some(SIBLING_BEFORE), Some(70), &[80, 81]);
        s.family(Some(SIBLING_AFTER), Some(71), &[82]);
    }
    s
}

fn node(l: &Layout, key: Key) -> &Node {
    l.nodes
        .iter()
        .find(|n| n.key == key && n.dup_of.is_none())
        .unwrap_or_else(|| panic!("key {key} placed"))
}

fn cx(l: &Layout, key: Key) -> f32 {
    node(l, key).rect.cx_pub()
}

trait Cx {
    fn cx_pub(&self) -> f32;
}
impl Cx for gt_layout::Rect {
    fn cx_pub(&self) -> f32 {
        self.x + self.w / 2.0
    }
}

fn key_of(l: &Layout, id: u32) -> Key {
    l.nodes[id as usize].key
}

/// Every Partner connector has the person as one end, and the other end is one of the person's partners.
fn assert_connectors_join_person(l: &Layout, partners: &[Key]) {
    let a = node(l, A).id;
    let pids: Vec<u32> = partners.iter().map(|&p| node(l, p).id).collect();
    for e in l.edges.iter().filter(|e| e.kind == EdgeKind::Partner) {
        let (f, t) = (key_of(l, e.from), key_of(l, e.to));
        if partners.contains(&f) || partners.contains(&t) {
            assert!(
                e.from == a || e.to == a,
                "partner connector {f} - {t} does not join the person {A}"
            );
            assert!(
                !(pids.contains(&e.from) && pids.contains(&e.to)),
                "partner connector joins two partners {f} - {t}"
            );
        }
    }
    for &p in partners {
        let n = l
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Partner)
            .filter(|e| {
                let (f, t) = (key_of(l, e.from), key_of(l, e.to));
                (f == A && t == p) || (f == p && t == A)
            })
            .count();
        assert_eq!(n, 1, "one connector between {A} and partner {p}");
    }
}

/// The start of the line that leads to `child` lies on the connector (A, partner) of its own parents: for a straight
/// connector between neighbouring cards on the middle of it, for a dipping one on its horizontal part.
fn assert_child_hangs_from(l: &Layout, child: Key, partner: Key) {
    let cid = node(l, child).id;
    let (aid, pid) = (node(l, A).id, node(l, partner).id);
    let conn = l
        .edges
        .iter()
        .find(|e| {
            e.kind == EdgeKind::Partner
                && ((e.from == aid && e.to == pid) || (e.from == pid && e.to == aid))
        })
        .unwrap_or_else(|| panic!("connector {A} - {partner}"));
    // Edges that end in the child, from either parent of the family.
    let ins: Vec<_> = l
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Parent && e.to == cid)
        .collect();
    assert!(!ins.is_empty(), "child {child} has a line from its parents");
    let seg: Vec<(f32, f32, f32, f32)> = conn
        .pts
        .windows(4)
        .step_by(2)
        .map(|w| (w[0], w[1], w[2], w[3]))
        .collect();
    let on = |x: f32, y: f32| {
        seg.iter().any(|&(x0, y0, x1, y1)| {
            x >= x0.min(x1) - 0.6
                && x <= x0.max(x1) + 0.6
                && y >= y0.min(y1) - 0.6
                && y <= y0.max(y1) + 0.6
        })
    };
    assert!(
        ins.iter().any(|e| on(e.pts[0], e.pts[1])),
        "child {child} does not hang from the connector {A} - {partner}: starts {:?}, connector {:?}",
        ins.iter().map(|e| (e.pts[0], e.pts[1])).collect::<Vec<_>>(),
        conn.pts
    );
    // And it hangs from no other partner's connector of this person.
    for other in l.edges.iter().filter(|e| {
        e.kind == EdgeKind::Partner && (e.from == aid || e.to == aid) && !std::ptr::eq(*e, conn)
    }) {
        let seg: Vec<(f32, f32, f32, f32)> = other
            .pts
            .windows(4)
            .step_by(2)
            .map(|w| (w[0], w[1], w[2], w[3]))
            .collect();
        let on_other = |x: f32, y: f32| {
            seg.iter().any(|&(x0, y0, x1, y1)| {
                x >= x0.min(x1) - 0.6
                    && x <= x0.max(x1) + 0.6
                    && y >= y0.min(y1) - 0.6
                    && y <= y0.max(y1) + 0.6
            })
        };
        assert!(
            ins.iter().all(|e| !on_other(e.pts[0], e.pts[1])),
            "child {child} also starts on another connector of {A}"
        );
    }
}

fn assert_no_overlap(l: &Layout) {
    for (i, a) in l.nodes.iter().enumerate() {
        for b in &l.nodes[i + 1..] {
            let apart = a.rect.x + a.rect.w <= b.rect.x
                || b.rect.x + b.rect.w <= a.rect.x
                || a.rect.y + a.rect.h <= b.rect.y
                || b.rect.y + b.rect.h <= a.rect.y;
            assert!(apart, "cards {} and {} overlap", a.key, b.key);
        }
    }
}

type Chart = (&'static str, fn(&MapSource) -> Layout);

fn charts() -> Vec<Chart> {
    let m = Metrics::default;
    let _ = m;
    vec![
        // Descendants of the grandparent: A is deep in X's subtree.
        ("descendants", |s| {
            descendants(s, 100, 20, &Metrics::default(), 100_000)
        }),
        // Around the cousin: all kin, A is a first cousin once removed.
        ("around_all_kin", |s| {
            around_all_kin(s, 3, 255, &Metrics::default(), 100_000)
        }),
        // Relatives chart centred on A: A is the focus.
        ("relatives(focus)", |s| {
            relatives(
                s,
                A,
                &RelativesOptions {
                    generations: 4,
                    aunts: false,
                },
                &Metrics::default(),
                100_000,
            )
        }),
        ("around_with_siblings(focus)", |s| {
            around_with_siblings(s, A, 255, &Metrics::default(), 100_000)
        }),
        ("around(focus)", |s| {
            around(s, A, 255, &Metrics::default(), 100_000)
        }),
    ]
}

/// The charts in which A is drawn with all partners and their children.
fn check_all(partners: &[(Key, &[Key])], siblings: bool, expect_order: &[Key]) {
    let s = branch(partners, siblings);
    let keys: Vec<Key> = partners.iter().map(|p| p.0).collect();
    for (name, run) in charts() {
        let l = run(&s);
        let ctx = |m: &str| format!("{name}: {m}");
        assert_no_overlap(&l);
        assert_connectors_join_person(&l, &keys);
        // The order along the row: the person among their partners.
        let xs: Vec<f32> = expect_order.iter().map(|&k| cx(&l, k)).collect();
        assert!(
            xs.windows(2).all(|w| w[0] < w[1]),
            "{}",
            ctx(&format!("row order {expect_order:?} has x {xs:?}"))
        );
        let ys: Vec<f32> = expect_order.iter().map(|&k| node(&l, k).rect.y).collect();
        assert!(ys.iter().all(|&y| y == ys[0]), "{}", ctx("one row"));
        for &(p, kids) in partners {
            for &k in kids {
                assert_child_hangs_from(&l, k, p);
            }
        }
    }
}

#[test]
fn two_partners_with_children_stand_either_side_of_the_person() {
    check_all(&[(11, &[21]), (12, &[22, 23])], false, &[11, A, 12]);
}

#[test]
fn two_partners_in_a_row_of_brothers_and_sisters() {
    // A is neither first nor last: neighbours with families of their own must not overlap the wider row.
    check_all(&[(11, &[21]), (12, &[22, 23])], true, &[11, A, 12]);
}

#[test]
fn two_partners_one_without_children() {
    check_all(&[(11, &[]), (12, &[22, 23])], false, &[11, A, 12]);
    check_all(&[(11, &[21]), (12, &[])], false, &[11, A, 12]);
}

#[test]
fn three_partners_continue_to_the_right_each_joined_to_the_person() {
    // Rule: P1 on the left, P2, P3 ... to the right; the connector of P3 dips under P2 and still joins the person.
    check_all(
        &[(11, &[21]), (12, &[22, 23]), (13, &[24])],
        false,
        &[11, A, 12, 13],
    );
}

#[test]
fn three_partners_in_a_row_of_brothers_and_sisters() {
    check_all(
        &[(11, &[21]), (12, &[22, 23]), (13, &[24])],
        true,
        &[11, A, 12, 13],
    );
}

#[test]
fn one_partner_stays_on_the_right() {
    check_all(&[(11, &[21, 22])], true, &[A, 11]);
    check_all(&[(11, &[])], false, &[A, 11]);
}

#[test]
fn no_partner_has_no_connector() {
    let s = branch(&[], true);
    let l = descendants(&s, 100, 20, &Metrics::default(), 100_000);
    let a = node(&l, A).id;
    assert!(
        l.edges
            .iter()
            .all(|e| e.kind != EdgeKind::Partner || (e.from != a && e.to != a))
    );
}

#[test]
fn mirrored_for_right_to_left_the_person_stays_in_the_middle_and_connectors_hold() {
    let s = branch(&[(11, &[21]), (12, &[22, 23])], true);
    for (name, run) in charts() {
        let mut l = run(&s);
        l.mirror(gt_layout::Axis::Horizontal);
        assert_connectors_join_person(&l, &[11, 12]);
        let xs: Vec<f32> = [12, A, 11].iter().map(|&k| cx(&l, k)).collect();
        assert!(xs.windows(2).all(|w| w[0] < w[1]), "{name}: {xs:?}");
        for (p, kids) in [(11, vec![21]), (12, vec![22, 23])] {
            for k in kids {
                assert_child_hangs_from(&l, k, p);
            }
        }
    }
}

/// The whole-tree chart (its own layout) already draws the person in the middle; keep it so.
#[test]
fn whole_tree_draws_the_person_between_two_partners() {
    use gt_layout::{WholeFamily, WholeInput, whole};
    // 0 = X, 1 = A, 2 = P1, 3 = P2, 4 = C1, 5 = C2, 6 = C3.
    let fam = |a, b, kids: &[u32]| WholeFamily {
        partners: [Some(a), Some(b)],
        children: kids.to_vec(),
    };
    let input = WholeInput {
        count: 7,
        families: vec![
            WholeFamily {
                partners: [Some(0), None],
                children: vec![1],
            },
            fam(1, 2, &[4]),
            fam(1, 3, &[5, 6]),
        ],
        birth: vec![None; 7],
        surname: vec![String::new(); 7],
        include: None,
    };
    let w = whole(&input, &Metrics::default());
    let x = |k: Key| {
        let n = &w.layout.nodes[w.extras.node_of(k).expect("person placed") as usize];
        n.rect.x + n.rect.w / 2.0
    };
    let order_ok = (x(2) < x(1) && x(1) < x(3)) || (x(3) < x(1) && x(1) < x(2));
    assert!(order_ok, "A between P1 and P2: {} {} {}", x(2), x(1), x(3));
    let id = |k: Key| w.extras.node_of(k).expect("person placed");
    for e in w
        .layout
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Partner)
    {
        assert!(
            e.from == id(1) || e.to == id(1),
            "partner connector {} - {} does not join A",
            e.from,
            e.to
        );
    }
}
