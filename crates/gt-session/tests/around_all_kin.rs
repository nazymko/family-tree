#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The chart built around one person with all relatives: great-uncles, aunts, cousins and their descendants.

mod common;

use common::*;
use gt_session::*;

/// Ivan Petrovych Koval's father Petro has parents Vasyl and Hanna; Vasyl's parents are Mykola and Marta. Vasyl's
/// brother Ostap (Ivan's great-uncle) has a son Pavlo, whose daughter Lesia is Ivan's second cousin. Petro's sister
/// Daryna (Ivan's aunt) has a son Yurii (a cousin). Ivan's brother Hryhorii has a daughter Sofia, who has a son
/// Danylo. Ivan's wife is Olena, whose brother Stepan is not a relative of the line and stays out.
const FAMILY: &str = "\
0 @I1@ INDI\n1 NAME Ivan Petrovych /Koval/\n1 SEX M\n1 FAMS @F1@\n1 FAMC @F2@\n\
0 @I2@ INDI\n1 NAME Olena /Koval/\n1 SEX F\n1 FAMS @F1@\n1 FAMC @F3@\n\
0 @I3@ INDI\n1 NAME Taras /Koval/\n1 SEX M\n1 FAMC @F1@\n\
0 @I7@ INDI\n1 NAME Petro /Koval/\n1 SEX M\n1 FAMS @F2@\n1 FAMC @F7@\n\
0 @I8@ INDI\n1 NAME Halyna /Koval/\n1 SEX F\n1 FAMS @F2@\n\
0 @I9@ INDI\n1 NAME Stepan /Moroz/\n1 SEX M\n1 FAMS @F3@\n\
0 @I10@ INDI\n1 NAME Nina /Moroz/\n1 SEX F\n1 FAMS @F3@\n\
0 @I11@ INDI\n1 NAME Stepan /Moroz/ Jr\n1 SEX M\n1 FAMC @F3@\n\
0 @I12@ INDI\n1 NAME Hryhorii /Koval/\n1 SEX M\n1 FAMC @F2@\n1 FAMS @F6@\n\
0 @I13@ INDI\n1 NAME Sofia /Koval/\n1 SEX F\n1 FAMC @F6@\n1 FAMS @F12@\n\
0 @I14@ INDI\n1 NAME Danylo /Koval/\n1 SEX M\n1 FAMC @F12@\n\
0 @I20@ INDI\n1 NAME Vasyl /Koval/\n1 SEX M\n1 FAMS @F7@\n1 FAMC @F8@\n\
0 @I21@ INDI\n1 NAME Hanna /Koval/\n1 SEX F\n1 FAMS @F7@\n\
0 @I22@ INDI\n1 NAME Ostap /Koval/\n1 SEX M\n1 FAMC @F8@\n1 FAMS @F10@\n\
0 @I23@ INDI\n1 NAME Mykola /Koval/\n1 SEX M\n1 FAMS @F8@\n\
0 @I24@ INDI\n1 NAME Marta /Koval/\n1 SEX F\n1 FAMS @F8@\n\
0 @I25@ INDI\n1 NAME Pavlo /Koval/\n1 SEX M\n1 FAMC @F10@\n1 FAMS @F11@\n\
0 @I26@ INDI\n1 NAME Lesia /Koval/\n1 SEX F\n1 FAMC @F11@\n\
0 @I27@ INDI\n1 NAME Daryna /Koval/\n1 SEX F\n1 FAMC @F7@\n1 FAMS @F9@\n\
0 @I28@ INDI\n1 NAME Yurii /Hrytsenko/\n1 SEX M\n1 FAMC @F9@\n\
0 @I29@ INDI\n1 NAME Zed /Solo/\n1 SEX M\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n\
0 @F2@ FAM\n1 HUSB @I7@\n1 WIFE @I8@\n1 CHIL @I1@\n1 CHIL @I12@\n\
0 @F3@ FAM\n1 HUSB @I9@\n1 WIFE @I10@\n1 CHIL @I2@\n1 CHIL @I11@\n\
0 @F6@ FAM\n1 HUSB @I12@\n1 CHIL @I13@\n\
0 @F7@ FAM\n1 HUSB @I20@\n1 WIFE @I21@\n1 CHIL @I7@\n1 CHIL @I27@\n\
0 @F8@ FAM\n1 HUSB @I23@\n1 WIFE @I24@\n1 CHIL @I20@\n1 CHIL @I22@\n\
0 @F9@ FAM\n1 WIFE @I27@\n1 CHIL @I28@\n\
0 @F10@ FAM\n1 HUSB @I22@\n1 CHIL @I25@\n\
0 @F11@ FAM\n1 HUSB @I25@\n1 CHIL @I26@\n\
0 @F12@ FAM\n1 WIFE @I13@\n1 CHIL @I14@\n";

fn everything() -> Rect {
    Rect {
        x: -1e6,
        y: -1e6,
        w: 2e6,
        h: 2e6,
    }
}

fn open(s: &mut Session, kin: Option<AroundKin>, generations: u32) -> (ChartMeta, ChartWindow) {
    let req = ChartRequest {
        kind: ChartKind::Around,
        root: find(s.tree(), "Ivan Petrovych"),
        generations,
        aunts_uncles: None,
        kin,
    };
    let m = s.chart_open(&req).unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    (m, w)
}

fn node<'a>(w: &'a ChartWindow, given: &str) -> &'a ChartNode {
    w.nodes
        .iter()
        .find(|n| n.person.as_ref().is_some_and(|p| p.name.starts_with(given)))
        .unwrap_or_else(|| panic!("{given} is shown"))
}

fn names(w: &ChartWindow) -> Vec<String> {
    w.nodes
        .iter()
        .filter_map(|n| n.person.as_ref().map(|p| p.name.clone()))
        .collect()
}

#[test]
fn all_relatives_adds_every_ancestors_family_and_only_the_line_and_siblings_modes_do_not() {
    let (_d, mut s) = open_tree(&tree_of(FAMILY));
    let far = ["Ostap", "Pavlo", "Lesia", "Daryna", "Yurii", "Danylo"];
    let (_, line) = open(&mut s, Some(AroundKin::Line), 255);
    let (_, sibs) = open(&mut s, Some(AroundKin::Siblings), 255);
    let (m, all) = open(&mut s, Some(AroundKin::All), 255);
    assert!(!m.truncated);
    let line_names = names(&line);
    for f in far {
        assert!(
            !line_names.iter().any(|n| n.starts_with(f)),
            "{f} is not on the line"
        );
    }
    let sib_names = names(&sibs);
    assert!(sib_names.iter().any(|n| n.starts_with("Hryhorii")));
    assert!(sib_names.iter().any(|n| n.starts_with("Sofia")));
    for f in ["Ostap", "Pavlo", "Lesia", "Daryna", "Yurii", "Danylo"] {
        assert!(
            !sib_names.iter().any(|n| n.starts_with(f)),
            "{f} is not in the siblings mode"
        );
    }
    let all_names = names(&all);
    for f in far {
        assert!(all_names.iter().any(|n| n.starts_with(f)), "{f} is shown");
    }
    assert!(
        !all_names
            .iter()
            .any(|n| n.starts_with("Zed") || n.starts_with("Stepan"))
    );
    // No kin given is the line only.
    let (_, none) = open(&mut s, None, 255);
    assert_eq!(names(&none), line_names);
}

#[test]
fn everyone_is_in_their_generation_row_and_named_by_generations_up_and_down() {
    let (_d, mut s) = open_tree(&tree_of(FAMILY));
    let (_, w) = open(&mut s, Some(AroundKin::All), 255);
    let y = |given: &str| node(&w, given).y;
    assert_eq!(
        y("Lesia"),
        y("Ivan Petrovych"),
        "a second cousin is in the person's row"
    );
    assert_eq!(y("Pavlo"), y("Petro"));
    assert_eq!(y("Ostap"), y("Vasyl"));
    assert_eq!(y("Yurii"), y("Ivan Petrovych"));
    assert_eq!(y("Daryna"), y("Petro"));
    assert!(y("Danylo") > y("Sofia") && y("Sofia") > y("Hryhorii"));
    let role = |given: &str| node(&w, given).role.unwrap();
    let kin = |given: &str, up: i32, down: u32| {
        let r = role(given);
        assert_eq!(
            (r.kind, r.n, r.k),
            (RoleKind::Kin, up, down),
            "{given}: {r:?}"
        );
    };
    kin("Ostap", 3, 1); // the grandfather's brother
    kin("Pavlo", 3, 2); // the father's first cousin
    kin("Lesia", 3, 3); // a second cousin
    kin("Yurii", 2, 2); // a first cousin
    kin("Danylo", 1, 3);
    assert_eq!(role("Daryna").kind, RoleKind::AuntUncle);
    assert_eq!(role("Sofia").kind, RoleKind::NieceNephew);
    assert_eq!(node(&w, "Ostap").relation.as_deref(), Some("great-uncle"));
}

#[test]
fn the_parents_stay_over_the_person() {
    let (_d, mut s) = open_tree(&tree_of(FAMILY));
    let (_, w) = open(&mut s, Some(AroundKin::All), 255);
    let cx = |given: &str| {
        let n = node(&w, given);
        n.x + n.w / 2.0
    };
    assert!(((cx("Petro") + cx("Halyna")) / 2.0 - cx("Ivan Petrovych")).abs() < 0.5);
    // The parents stand side by side, and so do the grandparents, on the father's side of the person.
    let step = node(&w, "Petro").w + 8.0;
    assert!((cx("Halyna") - cx("Petro") - step).abs() < 0.5);
    assert!((cx("Hanna") - cx("Vasyl") - step).abs() < 0.5);
    assert!(cx("Hanna") < cx("Ivan Petrovych"));
    assert!(
        cx("Ostap") < cx("Vasyl"),
        "the father's side is on the left"
    );
}

#[test]
fn the_outline_reaches_the_relatives() {
    let (_d, mut s) = open_tree(&tree_of(FAMILY));
    let (m, _) = open(&mut s, Some(AroundKin::All), 255);
    let mut seen = Vec::new();
    let mut todo = vec![None];
    while let Some(parent) = todo.pop() {
        for it in s.chart_outline(m.chart_id, parent, 100).unwrap() {
            if let Some(p) = &it.person {
                seen.push(p.name.clone());
            }
            if it.has_children && seen.len() < 500 {
                todo.push(Some(it.node_id));
            }
        }
    }
    for f in ["Ostap", "Lesia", "Yurii", "Danylo", "Hryhorii"] {
        assert!(
            seen.iter().any(|n| n.starts_with(f)),
            "{f} is in the outline: {seen:?}"
        );
    }
}

#[test]
fn the_generation_choice_limits_the_line_and_the_cap_is_reported() {
    let (_d, mut s) = open_tree(&tree_of(FAMILY));
    let (_, w) = open(&mut s, Some(AroundKin::All), 1);
    let n = names(&w);
    assert!(
        n.iter().any(|x| x.starts_with("Daryna")) || n.iter().any(|x| x.starts_with("Hryhorii"))
    );
    for f in ["Vasyl", "Ostap", "Lesia"] {
        assert!(
            !n.iter().any(|x| x.starts_with(f)),
            "{f} is beyond one generation"
        );
    }
}
