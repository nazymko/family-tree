#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The chart built around one person with side branches opened per person.

mod common;

use common::*;
use gt_session::*;

/// Invented family. Ivan's father Petro has parents Vasyl and Hanna; Vasyl's parents are Mykola and Marta.
/// Vasyl's brother Ostap has a son Pavlo, whose daughter is Lesia. Petro's sister Daryna has a son Yurii.
/// Ivan's brother Hryhorii has a daughter Sofia, who has a son Danylo. Ivan's wife Olena has a brother Stepan.
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

fn open_with(
    s: &mut Session,
    kin: AroundKin,
    roots: Option<Vec<uuid::Uuid>>,
    open: Option<Vec<OpenBranch>>,
) -> (ChartMeta, ChartWindow) {
    let req = ChartRequest {
        kind: ChartKind::Around,
        root: find(s.tree(), "Ivan Petrovych"),
        generations: 255,
        aunts_uncles: None,
        kin: Some(kin),
        roots,
        open,
        direction: None,
        print: None,
    };
    let m = s.chart_open(&req).unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    (m, w)
}

fn branch(
    s: &Session,
    who: &str,
    siblings: Option<BranchDepth>,
    children: Option<BranchDepth>,
) -> OpenBranch {
    OpenBranch {
        person: find(s.tree(), who),
        siblings,
        children,
    }
}

fn names(w: &ChartWindow) -> Vec<String> {
    w.nodes
        .iter()
        .filter_map(|n| n.person.as_ref().map(|p| p.name.clone()))
        .collect()
}

fn has(w: &ChartWindow, given: &str) -> bool {
    names(w).iter().any(|n| n.starts_with(given))
}

fn node<'a>(w: &'a ChartWindow, given: &str) -> &'a ChartNode {
    w.nodes
        .iter()
        .find(|n| n.person.as_ref().is_some_and(|p| p.name.starts_with(given)))
        .unwrap_or_else(|| panic!("{given} is shown"))
}

type Shape = Vec<(u32, f32, f32, Option<String>)>;

fn shape(w: &ChartWindow) -> Shape {
    w.nodes
        .iter()
        .map(|n| (n.id, n.x, n.y, n.person.as_ref().map(|p| p.name.clone())))
        .collect()
}

#[test]
fn absent_and_empty_openings_give_the_same_chart() {
    let (_d, mut s) = open_tree(&tree_of(FAMILY));
    for kin in [AroundKin::Line, AroundKin::Siblings, AroundKin::All] {
        let (m0, a) = open_with(&mut s, kin, None, None);
        let (m1, b) = open_with(&mut s, kin, None, Some(vec![]));
        assert_eq!(shape(&a), shape(&b));
        assert_eq!(m0.node_count, m1.node_count);
        assert_eq!(a.edges.len(), b.edges.len());
    }
}

#[test]
fn flags_and_marks_without_openings() {
    let (_d, mut s) = open_tree(&tree_of(FAMILY));
    let (_, w) = open_with(&mut s, AroundKin::Line, None, None);
    assert_eq!(node(&w, "Vasyl").can_open_siblings, Some(true));
    assert_eq!(node(&w, "Ivan").can_open_siblings, Some(true));
    assert_eq!(node(&w, "Olena").can_open_siblings, None, "married in");
    assert_eq!(node(&w, "Olena").can_open_children, None);
    assert_eq!(node(&w, "Taras").can_open_siblings, None);
    assert!(w.nodes.iter().all(|n| n.opened.is_none()));
}

#[test]
fn siblings_of_an_ancestor_with_children_then_all_descendants() {
    let (_d, mut s) = open_tree(&tree_of(FAMILY));
    let (_, plain) = open_with(&mut s, AroundKin::Line, None, None);
    assert!(!has(&plain, "Ostap"));
    let open = vec![branch(&s, "Vasyl", Some(BranchDepth::Children), None)];
    let (_, w) = open_with(&mut s, AroundKin::Line, None, Some(open));
    assert!(has(&w, "Ostap") && has(&w, "Pavlo"));
    assert!(!has(&w, "Lesia"), "one more row only");
    assert_eq!(node(&w, "Vasyl").opened, Some(true));
    assert_eq!(node(&w, "Pavlo").can_open_children, Some(true));
    let open = vec![branch(&s, "Vasyl", Some(BranchDepth::All), None)];
    let (_, w) = open_with(&mut s, AroundKin::Line, None, Some(open));
    assert!(has(&w, "Lesia"));
    // The line is still there and nothing from the wider family came with it.
    assert!(has(&w, "Mykola") && !has(&w, "Daryna") && !has(&w, "Hryhorii"));
}

#[test]
fn children_of_an_opened_relative_step_by_step_and_closed_branches_have_no_effect() {
    let (_d, mut s) = open_tree(&tree_of(FAMILY));
    let only_kids = vec![branch(&s, "Pavlo", None, Some(BranchDepth::All))];
    let (_, w) = open_with(&mut s, AroundKin::Line, None, Some(only_kids));
    assert!(!has(&w, "Pavlo") && !has(&w, "Lesia"), "Pavlo is not shown");
    let open = vec![
        branch(&s, "Vasyl", Some(BranchDepth::Children), None),
        branch(&s, "Pavlo", None, Some(BranchDepth::Children)),
    ];
    let (_, w) = open_with(&mut s, AroundKin::Line, None, Some(open));
    assert!(has(&w, "Lesia"));
    assert_eq!(node(&w, "Pavlo").opened, Some(true));
}

#[test]
fn several_chosen_people_ignore_the_openings() {
    let (_d, mut s) = open_tree(&tree_of(FAMILY));
    let roots = vec![find(s.tree(), "Ivan Petrovych"), find(s.tree(), "Hryhorii")];
    let open = vec![branch(&s, "Vasyl", Some(BranchDepth::All), None)];
    let (_, a) = open_with(&mut s, AroundKin::Line, Some(roots.clone()), None);
    let (_, b) = open_with(&mut s, AroundKin::Line, Some(roots), Some(open));
    assert_eq!(shape(&a), shape(&b));
    assert!(!has(&b, "Ostap"));
}
