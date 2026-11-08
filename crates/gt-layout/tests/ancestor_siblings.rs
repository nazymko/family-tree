//! Several chosen people with the wider family (`ManyKin::All`): every ancestor's brothers and sisters stand on the
//! outer side of the ancestor's couple (a left partner's to the left, a right partner's to the right), the lines make
//! room for them, and no family's connector crosses another's. Synthetic families only.

mod common;

use common::MapSource;
use gt_layout::{Key, Layout, ManyKin, Metrics, around_many_kin, quality};

// The wife's side (the shape of the `connectors` fixture): her husband and daughters, her brother, her parents, an
// aunt with a family, two couples of grandparents, great-grandparents and a great-uncle beside GM1.
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
// The husband's side: parents, an uncle with a family, grandparents and a great-uncle.
const HP: Key = 30;
const HM: Key = 31;
const HGP1: Key = 32;
const HGP2: Key = 33;
const HGM1: Key = 34;
const HGM2: Key = 35;
const HU: Key = 36;
const HC1: Key = 37;
const HC2: Key = 38;
const HUW: Key = 39;
const HGU: Key = 40;
const HGG1: Key = 41;
const HGG2: Key = 42;

/// Every person of the world.
const ALL: [Key; 33] = [
    F, H, D1, D2, B, P, M, U, GP1, GP2, GM1, GM2, A, AH, C1, C2, GG1, GG2, GU, GG3, HP, HM, HGP1,
    HGP2, HGM1, HGM2, HU, HC1, HC2, HUW, HGU, HGG1, HGG2,
];

/// The world, or its mirror image (every couple with father and mother swapped). A brother or sister with a family
/// can stand on the outer side of the whole chart only: the first world has them on the right of the wife's tree and
/// the left of the husband's, so it suits the husband first; the mirror suits the wife first.
fn world(mirror: bool) -> MapSource {
    let mut s = MapSource::default();
    let mut fam = |a: Option<Key>, b: Option<Key>, kids: &[Key]| {
        if mirror {
            s.family(b, a, kids);
        } else {
            s.family(a, b, kids);
        }
    };
    fam(Some(GG1), Some(GG2), &[GM1, GU]);
    fam(Some(GG3), None, &[GM2]);
    fam(Some(GP1), Some(GP2), &[P, U]);
    fam(Some(GM1), Some(GM2), &[M, A]);
    fam(Some(P), Some(M), &[F, B]);
    fam(Some(F), Some(H), &[D1, D2]);
    fam(Some(A), Some(AH), &[C1, C2]);
    fam(Some(HGG1), Some(HGG2), &[HGP1, HGU]);
    fam(Some(HGP1), Some(HGP2), &[HP, HU]);
    fam(Some(HGM1), Some(HGM2), &[HM]);
    fam(Some(HP), Some(HM), &[H]);
    fam(Some(HU), Some(HUW), &[HC1, HC2]);
    s
}

fn node(l: &Layout, key: Key) -> &gt_layout::Node {
    l.nodes
        .iter()
        .find(|n| n.key == key)
        .unwrap_or_else(|| panic!("{key} placed"))
}

fn cx(l: &Layout, key: Key) -> f32 {
    {
        let r = node(l, key).rect;
        r.x + r.w / 2.0
    }
}

/// (brother or sister, ancestor, the ancestor's partner on the chart)
const SIBS: [(Key, Key, Key); 6] = [
    (U, P, M),
    (A, M, P),
    (GU, GM1, GM2),
    (HU, HP, HM),
    (HGU, HGP1, HGP2),
    (B, F, H),
];

fn check(name: &str, s: &MapSource, chosen: &[Key]) {
    let m = Metrics::default();
    let l = around_many_kin(s, chosen, 255, ManyKin::All, &m, 10_000);
    let q = quality(&l);
    assert_eq!(q.box_overlaps, 0, "{name}: {q:?}");
    assert_eq!(q.lines_through_boxes, 0, "{name}: {q:?}");
    assert_eq!(q.line_overlaps, 0, "{name}: {q:?}");
    assert_eq!(q.line_crossings, 0, "{name}: {q:?}");
    assert_eq!(q.bus_clashes, 0, "{name}: {q:?}");
    assert_eq!(q.aligned_verticals, 0, "{name}: {q:?}");
    // Everyone is drawn, once.
    for k in ALL {
        let n = l.nodes.iter().filter(|n| n.key == k).count();
        assert_eq!(n, 1, "{name}: {k} drawn {n} times");
    }
    assert!(l.nodes.iter().all(|n| n.dup_of.is_none()), "{name}");
    // Partners stand next to each other: same row, nobody between them, the same distance everywhere.
    let couples = [
        (F, H),
        (P, M),
        (GP1, GP2),
        (GM1, GM2),
        (A, AH),
        (HP, HM),
        (HGP1, HGP2),
        (HGM1, HGM2),
        (HU, HUW),
        (GG1, GG2),
        (HGG1, HGG2),
    ];
    let mut dist = None;
    for (a, b) in couples {
        let (na, nb) = (node(&l, a), node(&l, b));
        assert_eq!(na.rect.y, nb.rect.y, "{name}: {a} and {b} in one row");
        let (lo, hi) = (cx(&l, a).min(cx(&l, b)), cx(&l, a).max(cx(&l, b)));
        let between = l
            .nodes
            .iter()
            .filter(|n| {
                n.rect.y == na.rect.y
                    && n.rect.x + n.rect.w / 2.0 > lo + 1.0
                    && n.rect.x + n.rect.w / 2.0 < hi - 1.0
            })
            .count();
        assert_eq!(between, 0, "{name}: {a} and {b} are not next to each other");
        let d = hi - lo;
        assert!(
            (*dist.get_or_insert(d) - d).abs() < 0.01,
            "{name}: partner distance {d}"
        );
    }
    // Brothers and sisters of an ancestor stand on the outer side of the ancestor's couple.
    for (sib, anc, partner) in SIBS {
        if sib == B && !chosen.contains(&B) {
            continue;
        }
        let outer_left = cx(&l, anc) < cx(&l, partner);
        assert_eq!(
            cx(&l, sib) < cx(&l, anc),
            outer_left,
            "{name}: {sib} (sibling of {anc}) on the wrong side of the couple"
        );
    }
    // Parents hang over their children: the couple's middle is not far from the middle of its children.
    for ((a, b), kids) in [
        ((P, M), vec![F, B]),
        ((GM1, GM2), vec![M, A]),
        ((HP, HM), vec![H]),
    ] {
        let mid = (cx(&l, a) + cx(&l, b)) / 2.0;
        let k: Vec<f32> = kids.iter().map(|&k| cx(&l, k)).collect();
        let kmid = (k.iter().copied().fold(f32::INFINITY, f32::min)
            + k.iter().copied().fold(f32::NEG_INFINITY, f32::max))
            / 2.0;
        // The couples make room for the brothers and sisters beside them, but stay within two cards of the middle.
        let off = (mid - kmid).abs();
        assert!(
            off <= 2.0 * (m.node_w + m.gap_x),
            "{name}: couple {a},{b} is {off} off the middle of its children"
        );
    }
}

/// Every order of `chosen` in which H and F stand next to each other.
fn orders(chosen: &[Key]) -> Vec<Vec<Key>> {
    let mut out = Vec::new();
    let n = chosen.len();
    let mut idx: Vec<usize> = (0..n).collect();
    fn go(v: &mut Vec<usize>, k: usize, f: &mut dyn FnMut(&[usize])) {
        if k == v.len() {
            f(v);
            return;
        }
        for i in k..v.len() {
            v.swap(k, i);
            go(v, k + 1, f);
            v.swap(k, i);
        }
    }
    go(&mut idx, 0, &mut |p| {
        let o: Vec<Key> = p.iter().map(|&i| chosen[i]).collect();
        let (a, b) = (
            o.iter().position(|&k| k == H).unwrap(),
            o.iter().position(|&k| k == F).unwrap(),
        );
        if a.abs_diff(b) == 1 {
            out.push(o);
        }
    });
    out
}

#[test]
fn a_married_couple_whose_both_sides_have_families_in_every_order() {
    for mirror in [false, true] {
        for o in orders(&[H, F]) {
            check(&format!("{o:?} mirror={mirror}"), &world(mirror), &o);
        }
    }
}

#[test]
fn a_couple_and_a_sibling_of_one_of_them_in_every_order() {
    for mirror in [false, true] {
        for o in orders(&[H, F, B]) {
            check(&format!("{o:?} mirror={mirror}"), &world(mirror), &o);
        }
    }
}

#[test]
fn two_deep_pedigrees_with_all_kin_lay_out_fast_without_overlaps() {
    let s = MapSource::ahnen_tree(1 << 13);
    let t = std::time::Instant::now();
    let l = around_many_kin(&s, &[4, 5], 255, ManyKin::All, &Metrics::default(), 100_000);
    let q = quality(&l);
    assert_eq!(q.box_overlaps, 0, "{q:?}");
    assert!(l.nodes.len() > 1000);
    assert!(t.elapsed().as_secs() < 20, "{:?}", t.elapsed());
}
