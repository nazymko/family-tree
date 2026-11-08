#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The chart built around people reaches brothers and sisters through a family that has children only (no father,
//! no mother entered), and draws the unnamed-siblings marker of every family whose children it shows.

mod common;

use common::*;
use gt_session::*;
use uuid::Uuid;

/// Petro, Pavlo and Polina are children of F0, a family with no partners at all; Pavlo has a son Pylyp. Petro + Maria
/// have Mykola; Mykola + Hanna have Olha and Dana; Olha + Taras have Yarema. Taras is the son of Ihor + Nadia, Ihor the
/// son of Vasyl + Zoya. Vasyl is the son of Gleb + Lida (F6); Zoya and her sister Zina are children of F7, which has no
/// partners either.
const FAMILY: &str = "\
0 @I1@ INDI\n1 NAME Petro /Koval/\n1 SEX M\n1 FAMC @F0@\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Pavlo /Koval/\n1 SEX M\n1 FAMC @F0@\n1 FAMS @F8@\n\
0 @I3@ INDI\n1 NAME Polina /Koval/\n1 SEX F\n1 FAMC @F0@\n\
0 @I4@ INDI\n1 NAME Maria /Lis/\n1 SEX F\n1 FAMS @F1@\n\
0 @I5@ INDI\n1 NAME Mykola /Koval/\n1 SEX M\n1 FAMC @F1@\n1 FAMS @F2@\n\
0 @I6@ INDI\n1 NAME Hanna /Moroz/\n1 SEX F\n1 FAMS @F2@\n\
0 @I7@ INDI\n1 NAME Olha /Koval/\n1 SEX F\n1 BIRT\n2 DATE 1980\n1 FAMC @F2@\n1 FAMS @F3@\n\
0 @I8@ INDI\n1 NAME Taras /Bondar/\n1 SEX M\n1 BIRT\n2 DATE 1979\n1 FAMC @F4@\n1 FAMS @F3@\n\
0 @I9@ INDI\n1 NAME Ihor /Bondar/\n1 SEX M\n1 FAMC @F5@\n1 FAMS @F4@\n\
0 @I10@ INDI\n1 NAME Nadia /Tkach/\n1 SEX F\n1 FAMS @F4@\n\
0 @I11@ INDI\n1 NAME Vasyl /Bondar/\n1 SEX M\n1 FAMC @F6@\n1 FAMS @F5@\n\
0 @I12@ INDI\n1 NAME Zoya /Shevchuk/\n1 SEX F\n1 FAMC @F7@\n1 FAMS @F5@\n\
0 @I13@ INDI\n1 NAME Pylyp /Koval/\n1 SEX M\n1 FAMC @F8@\n\
0 @I14@ INDI\n1 NAME Dana /Koval/\n1 SEX F\n1 BIRT\n2 DATE 1983\n1 FAMC @F2@\n\
0 @I15@ INDI\n1 NAME Yarema /Bondar/\n1 SEX M\n1 FAMC @F3@\n\
0 @I16@ INDI\n1 NAME Gleb /Bondar/\n1 SEX M\n1 FAMS @F6@\n\
0 @I17@ INDI\n1 NAME Lida /Sydor/\n1 SEX F\n1 FAMS @F6@\n\
0 @I19@ INDI\n1 NAME Zina /Shevchuk/\n1 SEX F\n1 FAMC @F7@\n\
0 @F0@ FAM\n1 CHIL @I1@\n1 CHIL @I2@\n1 CHIL @I3@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I4@\n1 CHIL @I5@\n\
0 @F2@ FAM\n1 HUSB @I5@\n1 WIFE @I6@\n1 CHIL @I7@\n1 CHIL @I14@\n\
0 @F3@ FAM\n1 HUSB @I8@\n1 WIFE @I7@\n1 CHIL @I15@\n\
0 @F4@ FAM\n1 HUSB @I9@\n1 WIFE @I10@\n1 CHIL @I8@\n\
0 @F5@ FAM\n1 HUSB @I11@\n1 WIFE @I12@\n1 CHIL @I9@\n\
0 @F6@ FAM\n1 HUSB @I16@\n1 WIFE @I17@\n1 CHIL @I11@\n\
0 @F7@ FAM\n1 CHIL @I12@\n1 CHIL @I19@\n\
0 @F8@ FAM\n1 HUSB @I2@\n1 CHIL @I13@\n";

fn everything() -> Rect {
    Rect {
        x: -1e6,
        y: -1e6,
        w: 2e6,
        h: 2e6,
    }
}

fn family_of(s: &Session, child: Uuid) -> Uuid {
    s.tree()
        .families
        .values()
        .find(|f| f.children.iter().any(|c| c.person == child))
        .unwrap()
        .common
        .id
}

/// One unnamed-siblings marker in the family `child` is a child of.
fn mark(s: &mut Session, child: &str) {
    let family = family_of(s, find(s.tree(), child));
    s.apply(EditCommand::AddSiblingMarker { family }).unwrap();
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
            roots: if ids.len() > 1 { Some(ids) } else { None },
        })
        .unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    (m, w)
}

fn named<'a>(w: &'a ChartWindow, name: &str) -> Vec<&'a ChartNode> {
    w.nodes
        .iter()
        .filter(|n| n.person.as_ref().is_some_and(|p| p.name == name))
        .collect()
}

fn markers(w: &ChartWindow) -> usize {
    w.nodes
        .iter()
        .filter(|n| n.person.as_ref().is_some_and(|p| p.sibling_marker))
        .count()
}

fn assert_clean(w: &ChartWindow) {
    let ns = &w.nodes;
    let mut seen = std::collections::HashSet::new();
    for (i, a) in ns.iter().enumerate() {
        assert!(a.dup_of.is_none(), "no repeated card");
        assert!(
            seen.insert(a.person.as_ref().unwrap().id),
            "{} drawn twice",
            a.id
        );
        for b in &ns[i + 1..] {
            let apart =
                a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
            assert!(apart, "boxes {} and {} overlap", a.id, b.id);
        }
    }
    for e in &w.edges {
        for seg in e.pts.windows(4).step_by(2) {
            let (x0, x1) = (seg[0].min(seg[2]), seg[0].max(seg[2]));
            let (y0, y1) = (seg[1].min(seg[3]), seg[1].max(seg[3]));
            for n in ns {
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

fn one<'a>(w: &'a ChartWindow, name: &str) -> &'a ChartNode {
    let v = named(w, name);
    assert_eq!(v.len(), 1, "{name} drawn once");
    v[0]
}

/// The great-uncle and great-aunt (and the great-uncle's son) are present, in the grandparent's row, joined by a bar.
fn assert_great_family(w: &ChartWindow) {
    let petro = one(w, "Petro Koval");
    for name in ["Pavlo Koval", "Polina Koval"] {
        let n = one(w, name);
        assert!((n.y - petro.y).abs() < 0.5, "{name} stands in Petro's row");
    }
    assert!(one(w, "Pylyp Koval").y > petro.y, "the great-uncle's son");
    let bar = |to: &ChartNode| {
        w.edges
            .iter()
            .any(|e| e.kind == EdgeKindDto::Sibling && (e.to == to.id || e.from == to.id))
    };
    assert!(
        bar(one(w, "Pavlo Koval")) && bar(one(w, "Polina Koval")),
        "sibling bar"
    );
    // The bar is not a parent line.
    assert!(
        !w.edges
            .iter()
            .any(|e| e.kind == EdgeKindDto::Parent && e.to == petro.id)
    );
}

/// Parents of each chosen person stand side by side over them.
fn assert_parents_over(w: &ChartWindow, father: &str, mother: &str, child: &str) {
    let (f, m, c) = (one(w, father), one(w, mother), one(w, child));
    assert!((f.y - (c.y - 114.0)).abs() < 0.5 && (m.y - f.y).abs() < 0.5);
    assert!(
        (m.x - f.x - (176.0 + 8.0)).abs() < 0.5,
        "{father} and {mother} adjacent"
    );
}

#[test]
fn the_wifes_great_uncle_and_great_aunt_come_through_a_family_without_partners() {
    for order in [["Olha", "Taras"], ["Taras", "Olha"]] {
        let (_d, mut s) = open_tree(&tree_of(FAMILY));
        let (_m, w) = open(&mut s, &order, AroundKin::All);
        assert_great_family(&w);
        assert_clean(&w);
        assert_parents_over(&w, "Mykola Koval", "Hanna Moroz", "Olha Koval");
        assert_parents_over(&w, "Ihor Bondar", "Nadia Tkach", "Taras Bondar");
        // The husband's grandmother's sister, through the other family without partners.
        assert_eq!(named(&w, "Zina Shevchuk").len(), 1);
    }
}

#[test]
fn one_chosen_person_gets_them_in_every_mode() {
    let (_d, mut s) = open_tree(&tree_of(FAMILY));
    let (_m, w) = open(&mut s, &["Olha"], AroundKin::All);
    assert_great_family(&w);
    assert_clean(&w);
    assert_parents_over(&w, "Mykola Koval", "Hanna Moroz", "Olha Koval");
    let (_m, w) = open(&mut s, &["Olha"], AroundKin::Line);
    assert!(named(&w, "Pavlo Koval").is_empty());
}

#[test]
fn own_brothers_and_sisters_in_a_family_without_partners() {
    let (_d, mut s) = open_tree(&tree_of(FAMILY));
    for who in [
        vec!["Petro"],
        vec!["Petro", "Maria"],
        vec!["Maria", "Petro"],
    ] {
        let (_m, w) = open(&mut s, &who, AroundKin::Siblings);
        let petro = one(&w, "Petro Koval");
        for name in ["Pavlo Koval", "Polina Koval"] {
            assert!((one(&w, name).y - petro.y).abs() < 0.5, "{name}");
        }
        // Siblings mode does not go on to the siblings' relatives beyond their own children.
        assert_clean(&w);
        assert!(
            w.edges.iter().any(|e| e.kind == EdgeKindDto::Sibling),
            "sibling bar"
        );
    }
    // All relatives from the same place.
    let (_m, w) = open(&mut s, &["Petro"], AroundKin::All);
    assert_eq!(named(&w, "Pylyp Koval").len(), 1);
    assert_clean(&w);
    // Relatives chart of one person.
    let id = find(s.tree(), "Petro");
    let m = s
        .chart_open(&ChartRequest {
            kind: ChartKind::Relatives,
            root: id,
            generations: 2,
            aunts_uncles: Some(true),
            kin: None,
            roots: None,
        })
        .unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    assert_eq!(named(&w, "Pavlo Koval").len(), 1);
    assert_eq!(named(&w, "Polina Koval").len(), 1);
    assert_clean(&w);
}

#[test]
fn unnamed_sibling_markers_at_every_generation_are_drawn_once() {
    let (_d, mut s) = open_tree(&tree_of(FAMILY));
    // F2 (the wife's own), F4 (the husband's own), F1 (the wife's parents' generation), F5 (the husband's), F0 and F7
    // (family without partners) and F6 at the grandparents' generation.
    for child in ["Olha", "Taras", "Mykola", "Ihor", "Petro", "Zoya", "Vasyl"] {
        mark(&mut s, child);
    }
    let (_m, w) = open(&mut s, &["Olha", "Taras"], AroundKin::All);
    assert_eq!(markers(&w), 7);
    assert_clean(&w);
    let (_m, w) = open(&mut s, &["Taras", "Olha"], AroundKin::All);
    assert_eq!(markers(&w), 7);
    assert_clean(&w);
    // One chosen person: the markers of her line only (F2, F1, F0).
    let (_m, w) = open(&mut s, &["Olha"], AroundKin::All);
    assert_eq!(markers(&w), 3);
    assert_clean(&w);
    // Her own brothers and sisters only: the marker beside her.
    let (_m, w) = open(&mut s, &["Olha"], AroundKin::Siblings);
    assert_eq!(markers(&w), 1);
    let (_m, w) = open(&mut s, &["Olha", "Taras"], AroundKin::Siblings);
    assert_eq!(markers(&w), 2);
    assert_clean(&w);
}
