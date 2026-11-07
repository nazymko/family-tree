#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The chart built around several people with brothers and sisters, and with all relatives.

mod common;

use common::*;
use gt_session::*;

/// Hryhorii + Halyna have Petro, Stepan and Zinaida. Petro + Olena have Iryna and Ivan; Stepan + Nina have Andriy and
/// Bohdan. Ivan + Maria have Taras; Andriy + Sofia have Dmytro; Bohdan + Kateryna have Yurii. Zinaida has Vira.
/// Ivan and Andriy are cousins; Iryna and Ivan are sister and brother.
const FAMILY: &str = "\
0 @I1@ INDI\n1 NAME Hryhorii /Koval/\n1 SEX M\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Halyna /Koval/\n1 SEX F\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Petro /Koval/\n1 SEX M\n1 FAMC @F1@\n1 FAMS @F2@\n\
0 @I4@ INDI\n1 NAME Stepan /Koval/\n1 SEX M\n1 FAMC @F1@\n1 FAMS @F3@\n\
0 @I5@ INDI\n1 NAME Zinaida /Koval/\n1 SEX F\n1 FAMC @F1@\n1 FAMS @F6@\n\
0 @I6@ INDI\n1 NAME Olena /Bondar/\n1 SEX F\n1 FAMS @F2@\n\
0 @I7@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n1 BIRT\n2 DATE 1980\n1 FAMC @F2@\n1 FAMS @F4@\n\
0 @I8@ INDI\n1 NAME Nina /Moroz/\n1 SEX F\n1 FAMS @F3@\n\
0 @I9@ INDI\n1 NAME Andriy /Koval/\n1 SEX M\n1 BIRT\n2 DATE 1981\n1 FAMC @F3@\n1 FAMS @F5@\n\
0 @I10@ INDI\n1 NAME Maria /Shevchuk/\n1 SEX F\n1 FAMS @F4@\n\
0 @I11@ INDI\n1 NAME Taras /Koval/\n1 SEX M\n1 FAMC @F4@\n\
0 @I12@ INDI\n1 NAME Sofia /Tkach/\n1 SEX F\n1 FAMS @F5@\n\
0 @I13@ INDI\n1 NAME Dmytro /Koval/\n1 SEX M\n1 FAMC @F5@\n\
0 @I14@ INDI\n1 NAME Iryna /Koval/\n1 SEX F\n1 BIRT\n2 DATE 1978\n1 FAMC @F2@\n\
0 @I15@ INDI\n1 NAME Bohdan /Koval/\n1 SEX M\n1 BIRT\n2 DATE 1983\n1 FAMC @F3@\n1 FAMS @F7@\n\
0 @I16@ INDI\n1 NAME Kateryna /Lis/\n1 SEX F\n1 FAMS @F7@\n\
0 @I17@ INDI\n1 NAME Yurii /Koval/\n1 SEX M\n1 FAMC @F7@\n\
0 @I18@ INDI\n1 NAME Vira /Koval/\n1 SEX F\n1 FAMC @F6@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n1 CHIL @I4@\n1 CHIL @I5@\n\
0 @F2@ FAM\n1 HUSB @I3@\n1 WIFE @I6@\n1 CHIL @I14@\n1 CHIL @I7@\n\
0 @F3@ FAM\n1 HUSB @I4@\n1 WIFE @I8@\n1 CHIL @I9@\n1 CHIL @I15@\n\
0 @F4@ FAM\n1 HUSB @I7@\n1 WIFE @I10@\n1 CHIL @I11@\n\
0 @F5@ FAM\n1 HUSB @I9@\n1 WIFE @I12@\n1 CHIL @I13@\n\
0 @F6@ FAM\n1 WIFE @I5@\n1 CHIL @I18@\n\
0 @F7@ FAM\n1 HUSB @I15@\n1 WIFE @I16@\n1 CHIL @I17@\n";

const CARD: f32 = 176.0;
const COUPLE_GAP: f32 = 8.0;
const ROW: f32 = 114.0;

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

fn count(ns: &[ChartNode], name: &str) -> usize {
    ns.iter()
        .filter(|n| n.person.as_ref().is_some_and(|p| p.name == name))
        .count()
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

/// Father and mother stand side by side one row above `child`; returns the middle of the couple.
fn couple_above(ns: &[ChartNode], father: &str, mother: &str, child: &ChartNode) -> f32 {
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
    (f.x + mo.x + mo.w) / 2.0
}

fn check_cousins(ns: &[ChartNode]) {
    let (ivan, andriy) = (at(ns, "Ivan Koval"), at(ns, "Andriy Koval"));
    let c1 = couple_above(ns, "Petro Koval", "Olena Bondar", ivan);
    let c2 = couple_above(ns, "Stepan Koval", "Nina Moroz", andriy);
    assert!((c1 - cx(ivan)).abs() < 1.0, "Ivan's parents over him");
    assert!((c2 - cx(andriy)).abs() < 1.0, "Andriy's parents over him");
    let (petro, stepan) = (at(ns, "Petro Koval"), at(ns, "Stepan Koval"));
    let c3 = couple_above(ns, "Hryhorii Koval", "Halyna Koval", petro);
    assert!((c3 - (cx(petro) + cx(stepan)) / 2.0).abs() < 1.0);
    assert_eq!(count(ns, "Hryhorii Koval"), 1);
}

#[test]
fn two_cousins_with_their_brothers_and_sisters() {
    let (_d, mut s) = open_tree(&tree_of(FAMILY));
    let (ml, _) = open(&mut s, &["Ivan", "Andriy"], AroundKin::Line);
    let (m, w) = open(&mut s, &["Ivan", "Andriy"], AroundKin::Siblings);
    let ns = &w.nodes;
    for want in ["Iryna Koval", "Bohdan Koval", "Kateryna Lis", "Yurii Koval"] {
        assert!(has(ns, want), "{want}");
    }
    // Aunts and their families stay out in this mode.
    assert!(!has(ns, "Zinaida Koval") && !has(ns, "Vira Koval"));
    assert!(m.node_count > ml.node_count);
    assert_no_overlaps(ns);
    assert_each_person_once(ns);
    assert_no_connector_crosses_a_card(&w);
    check_cousins(ns);
    // Brothers and sisters stand in the chosen row.
    let ivan = at(ns, "Ivan Koval");
    assert!((at(ns, "Iryna Koval").y - ivan.y).abs() < 0.5);
    assert!((at(ns, "Bohdan Koval").y - ivan.y).abs() < 0.5);
    assert!(at(ns, "Yurii Koval").y > ivan.y);
}

#[test]
fn two_cousins_with_all_relatives() {
    let (_d, mut s) = open_tree(&tree_of(FAMILY));
    let (ms, _) = open(&mut s, &["Ivan", "Andriy"], AroundKin::Siblings);
    let (m, w) = open(&mut s, &["Ivan", "Andriy"], AroundKin::All);
    let ns = &w.nodes;
    assert!(has(ns, "Zinaida Koval") && has(ns, "Vira Koval"));
    assert!(m.node_count > ms.node_count);
    assert!(!m.truncated);
    assert_no_overlaps(ns);
    assert_each_person_once(ns);
    assert_no_connector_crosses_a_card(&w);
    check_cousins(ns);
    // The cousin Vira stands in the chosen row, her mother in the row of the grandparents' sons.
    assert!((at(ns, "Vira Koval").y - at(ns, "Ivan Koval").y).abs() < 0.5);
    assert!((at(ns, "Zinaida Koval").y - at(ns, "Petro Koval").y).abs() < 0.5);
}

#[test]
fn two_chosen_siblings_share_one_group_and_one_pair_of_parents() {
    let (_d, mut s) = open_tree(&tree_of(FAMILY));
    for kin in [AroundKin::Siblings, AroundKin::All] {
        let (_m, w) = open(&mut s, &["Ivan", "Iryna"], kin);
        let ns = &w.nodes;
        assert_no_overlaps(ns);
        assert_each_person_once(ns);
        assert_no_connector_crosses_a_card(&w);
        let (ivan, iryna) = (at(ns, "Ivan Koval"), at(ns, "Iryna Koval"));
        let c = couple_above(ns, "Petro Koval", "Olena Bondar", ivan);
        assert!(
            (c - (cx(ivan) + cx(iryna)) / 2.0).abs() < 1.0,
            "over the midpoint"
        );
        assert_eq!(count(ns, "Petro Koval"), 1);
    }
}
