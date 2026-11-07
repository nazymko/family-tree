#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The chart built around several people when two of them are a couple (or one descends from the other): every
//! chosen person brings everything the mode includes, however they relate to the others.

mod common;

use common::*;
use gt_session::*;

const CARD: f32 = 176.0;
const COUPLE_GAP: f32 = 8.0;
const ROW: f32 = 114.0;

/// People as (key, given, surname, sex, famc, fams...). Families as (id, husband, wife, children).
type P = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
    &'static [&'static str],
);
type F = (
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
    &'static [&'static str],
);

/// Ivan Koval (husband) and Maria Shevchuk (wife) have Dmytro. Ivan has a brother, Taras; his father Petro has four
/// brothers and sisters, each with a child; his mother Olena has a brother Vasyl with a daughter; his
/// grandfather Hryhorii has a brother Pavlo with a son. Maria has a sister Nadia with a daughter, and an aunt Yaryna
/// with a son.
const PEOPLE: &[P] = &[
    ("Fedir", "Fedir", "Koval", "M", None, &["F10"]),
    ("Hanna", "Hanna", "Rud", "F", None, &["F10"]),
    ("Hryhorii", "Hryhorii", "Koval", "M", Some("F10"), &["F1"]),
    ("Pavlo", "Pavlo", "Koval", "M", Some("F10"), &["F7"]),
    ("Halyna", "Halyna", "Kravets", "F", None, &["F1"]),
    ("Petro", "Petro", "Koval", "M", Some("F1"), &["F2"]),
    ("Stepan", "Stepan", "Koval", "M", Some("F1"), &["F3"]),
    ("Mykola", "Mykola", "Koval", "M", Some("F1"), &["F4"]),
    ("Hanna2", "Hanka", "Koval", "F", Some("F1"), &["F5"]),
    ("Oksana", "Oksana", "Koval", "F", Some("F1"), &["F6"]),
    ("Olena", "Olena", "Bondar", "F", Some("F11"), &["F2"]),
    ("Yosyp", "Yosyp", "Bondar", "M", None, &["F11"]),
    ("Vira", "Vira", "Bondar", "F", None, &["F11"]),
    ("Vasyl", "Vasyl", "Bondar", "M", Some("F11"), &["F12"]),
    ("Zoya", "Zoya", "Bondar", "F", Some("F12"), &[]),
    ("Ivan", "Ivan", "Koval", "M", Some("F2"), &["F8"]),
    ("Taras", "Taras", "Koval", "M", Some("F2"), &[]),
    ("Andriy", "Andriy", "Koval", "M", Some("F3"), &[]),
    ("Bohdan", "Bohdan", "Koval", "M", Some("F4"), &[]),
    ("Nina", "Nina", "Hryhor", "F", Some("F5"), &[]),
    ("Lesia", "Lesia", "Koval", "F", Some("F6"), &[]),
    ("Marko", "Marko", "Koval", "M", Some("F7"), &[]),
    ("Maria", "Maria", "Shevchuk", "F", Some("F9"), &["F8"]),
    ("Dmytro", "Dmytro", "Koval", "M", Some("F8"), &[]),
    ("Oleh", "Oleh", "Shevchuk", "M", Some("F13"), &["F9"]),
    ("Halyna2", "Halia", "Lis", "F", None, &["F9"]),
    ("Nadia", "Nadia", "Shevchuk", "F", Some("F9"), &["F15"]),
    ("Sofiia", "Sofiia", "Shevchuk", "F", Some("F15"), &[]),
    ("Yaryna", "Yaryna", "Shevchuk", "F", Some("F13"), &["F14"]),
    ("Roman", "Roman", "Shevchuk", "M", Some("F14"), &[]),
    ("Danylo", "Danylo", "Shevchuk", "M", None, &["F13"]),
    ("Marta", "Marta", "Pavlenko", "F", None, &["F13"]),
];
const FAMS: &[F] = &[
    ("F10", Some("Fedir"), Some("Hanna"), &["Hryhorii", "Pavlo"]),
    (
        "F1",
        Some("Hryhorii"),
        Some("Halyna"),
        &["Petro", "Stepan", "Mykola", "Hanna2", "Oksana"],
    ),
    ("F2", Some("Petro"), Some("Olena"), &["Ivan", "Taras"]),
    ("F3", Some("Stepan"), None, &["Andriy"]),
    ("F4", Some("Mykola"), None, &["Bohdan"]),
    ("F5", None, Some("Hanna2"), &["Nina"]),
    ("F6", None, Some("Oksana"), &["Lesia"]),
    ("F7", Some("Pavlo"), None, &["Marko"]),
    ("F11", Some("Yosyp"), Some("Vira"), &["Olena", "Vasyl"]),
    ("F12", Some("Vasyl"), None, &["Zoya"]),
    ("F8", Some("Ivan"), Some("Maria"), &["Dmytro"]),
    ("F9", Some("Oleh"), Some("Halyna2"), &["Maria", "Nadia"]),
    ("F13", Some("Danylo"), Some("Marta"), &["Oleh", "Yaryna"]),
    ("F14", None, Some("Yaryna"), &["Roman"]),
    ("F15", None, Some("Nadia"), &["Sofiia"]),
];

fn ged() -> String {
    let id = |k: &str| format!("@I{}@", PEOPLE.iter().position(|p| p.0 == k).unwrap() + 1);
    let mut s = String::new();
    for (i, (_, given, sur, sex, famc, fams)) in PEOPLE.iter().enumerate() {
        s += &format!("0 @I{}@ INDI\n1 NAME {given} /{sur}/\n1 SEX {sex}\n", i + 1);
        // Birth years in list order keep brothers and sisters in a known order.
        s += &format!("1 BIRT\n2 DATE {}\n", 1900 + i);
        if let Some(c) = famc {
            s += &format!("1 FAMC @{c}@\n");
        }
        for f in *fams {
            s += &format!("1 FAMS @{f}@\n");
        }
    }
    for (f, h, w, kids) in FAMS {
        s += &format!("0 @{f}@ FAM\n");
        if let Some(h) = h {
            s += &format!("1 HUSB {}\n", id(h));
        }
        if let Some(w) = w {
            s += &format!("1 WIFE {}\n", id(w));
        }
        for k in *kids {
            s += &format!("1 CHIL {}\n", id(k));
        }
    }
    s
}

fn everything() -> Rect {
    Rect {
        x: -1e6,
        y: -1e6,
        w: 2e6,
        h: 2e6,
    }
}

fn open(s: &mut Session, given: &[&str], kin: AroundKin) -> (ChartMeta, ChartWindow) {
    let ids: Vec<_> = given.iter().map(|g| find(s.tree(), g)).collect();
    let m = s
        .chart_open(&ChartRequest {
            kind: ChartKind::Around,
            root: ids[0],
            generations: 255,
            aunts_uncles: None,
            kin: Some(kin),
            roots: Some(ids),
        })
        .unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    (m, w)
}

fn at<'a>(ns: &'a [ChartNode], name: &str) -> &'a ChartNode {
    ns.iter()
        .find(|n| n.person.as_ref().is_some_and(|p| p.name == name))
        .unwrap_or_else(|| panic!("{name} is not on the chart"))
}

fn has(ns: &[ChartNode], name: &str) -> bool {
    ns.iter()
        .any(|n| n.person.as_ref().is_some_and(|p| p.name == name))
}

fn cx(n: &ChartNode) -> f32 {
    n.x + n.w / 2.0
}

fn assert_no_overlaps(ns: &[ChartNode]) {
    for (i, a) in ns.iter().enumerate() {
        for b in &ns[i + 1..] {
            let apart =
                a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
            assert!(apart, "boxes {} and {} overlap", a.id, b.id);
        }
    }
}

fn assert_each_person_once(ns: &[ChartNode]) {
    let mut seen = std::collections::HashSet::new();
    for n in ns {
        assert!(n.dup_of.is_none(), "no repeated card");
        let id = n.person.as_ref().unwrap().id;
        assert!(seen.insert(id), "{} drawn twice", n.id);
    }
}

fn assert_no_connector_crosses_a_card(w: &ChartWindow) {
    for e in &w.edges {
        for seg in e.pts.windows(4).step_by(2) {
            let (x0, x1) = (seg[0].min(seg[2]), seg[0].max(seg[2]));
            let (y0, y1) = (seg[1].min(seg[3]), seg[1].max(seg[3]));
            for n in &w.nodes {
                let inside = x1 > n.x + 0.5
                    && x0 < n.x + n.w - 0.5
                    && y1 > n.y + 0.5
                    && y0 < n.y + n.h - 0.5;
                assert!(
                    !inside,
                    "a connector {}->{} crosses card {}",
                    e.from, e.to, n.id
                );
            }
        }
    }
}

/// Father and mother stand side by side one row above `child`, centred over it.
fn parents_over(ns: &[ChartNode], father: &str, mother: &str, child: &ChartNode, tol: f32) {
    let (f, mo) = (at(ns, father), at(ns, mother));
    assert!(
        (f.y - (child.y - ROW)).abs() < 0.5,
        "{father} one row above"
    );
    assert!((mo.y - f.y).abs() < 0.5);
    assert!(
        (mo.x - f.x - (CARD + COUPLE_GAP)).abs() < 0.5,
        "{father} and {mother} are adjacent"
    );
    let mid = (f.x + mo.x + mo.w) / 2.0;
    assert!(
        (mid - cx(child)).abs() < tol,
        "{father} and {mother} are over their child: off by {}",
        mid - cx(child)
    );
}

const HIS_SIDE: &[&str] = &[
    "Taras Koval",
    "Stepan Koval",
    "Mykola Koval",
    "Hanka Koval",
    "Oksana Koval",
    "Andriy Koval",
    "Bohdan Koval",
    "Nina Hryhor",
    "Lesia Koval",
    "Vasyl Bondar",
    "Zoya Bondar",
    "Pavlo Koval",
    "Marko Koval",
];
const HER_SIDE: &[&str] = &[
    "Nadia Shevchuk",
    "Sofiia Shevchuk",
    "Yaryna Shevchuk",
    "Roman Shevchuk",
];

fn check_couple(w: &ChartWindow, all: bool) {
    let ns = &w.nodes;
    assert_no_overlaps(ns);
    assert_each_person_once(ns);
    assert_no_connector_crosses_a_card(w);
    for want in ["Nadia Shevchuk", "Sofiia Shevchuk", "Taras Koval"] {
        assert!(has(ns, want), "{want}");
    }
    let (ivan, maria) = (at(ns, "Ivan Koval"), at(ns, "Maria Shevchuk"));
    parents_over(ns, "Petro Koval", "Olena Bondar", ivan, 120.0);
    parents_over(ns, "Oleh Shevchuk", "Halia Lis", maria, 120.0);
    parents_over(
        ns,
        "Hryhorii Koval",
        "Halyna Kravets",
        at(ns, "Petro Koval"),
        450.0,
    );
    assert!((at(ns, "Taras Koval").y - ivan.y).abs() < 0.5);
    // His brother stands on his outer side (away from his wife), her sister on hers.
    let sign = if ivan.x > maria.x { 1.0 } else { -1.0 };
    assert!(
        (at(ns, "Taras Koval").x - ivan.x) * sign > 0.0,
        "his brother outside"
    );
    assert!(
        (at(ns, "Nadia Shevchuk").x - maria.x) * sign < 0.0,
        "her sister outside"
    );
    if all {
        for want in HIS_SIDE.iter().chain(HER_SIDE) {
            assert!(has(ns, want), "{want}");
        }
        parents_over(
            ns,
            "Fedir Koval",
            "Hanna Rud",
            at(ns, "Hryhorii Koval"),
            1.0,
        );
    } else {
        for gone in ["Stepan Koval", "Vasyl Bondar", "Yaryna Shevchuk"] {
            assert!(!has(ns, gone), "{gone} belongs to All only");
        }
    }
}

#[test]
fn a_chosen_couple_keeps_both_sides_in_either_order() {
    let (_d, mut s) = open_tree(&tree_of(&ged()));
    for names in [["Maria", "Ivan"], ["Ivan", "Maria"]] {
        for kin in [AroundKin::Siblings, AroundKin::All] {
            let (_m, w) = open(&mut s, &names, kin);
            check_couple(&w, kin == AroundKin::All);
        }
    }
}

#[test]
fn a_chosen_descendant_of_another_chosen_person_keeps_each_person_once() {
    let (_d, mut s) = open_tree(&tree_of(&ged()));
    for kin in [AroundKin::Siblings, AroundKin::All] {
        let (_m, w) = open(&mut s, &["Petro", "Dmytro"], kin);
        let ns = &w.nodes;
        assert_no_overlaps(ns);
        assert_each_person_once(ns);
        assert_no_connector_crosses_a_card(&w);
        // Petro's own brothers and sisters, his sons and the grandson chosen second.
        for want in ["Ivan Koval", "Taras Koval", "Dmytro Koval", "Stepan Koval"] {
            assert!(has(ns, want), "{want}");
        }
        assert_eq!(has(ns, "Pavlo Koval"), kin == AroundKin::All);
    }
}
