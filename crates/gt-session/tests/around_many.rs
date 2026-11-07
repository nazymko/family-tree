#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The chart built around several people: the chosen people in the middle, every one's ancestors above (once),
//! every one's descendants below, no collateral relatives.

mod common;

use common::*;
use gt_gedcom::synthetic::synthetic_tree;
use gt_session::*;
use gt_store::Project;
use std::time::{Duration, Instant};

/// Hryhorii and Halyna Koval (the shared grandparents) have Petro and Stepan, and a daughter Zinaida (an aunt, not on
/// any line). Petro + Olena have Ivan; Ivan + Maria have Taras. Stepan + Nina have Andriy; Andriy + Sofia have Dmytro.
/// Ivan and Andriy are the cousins. Zed is alone.
const KOVAL: &str = "\
0 @I1@ INDI\n1 NAME Hryhorii /Koval/\n1 SEX M\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Halyna /Koval/\n1 SEX F\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Petro /Koval/\n1 SEX M\n1 FAMC @F1@\n1 FAMS @F2@\n\
0 @I4@ INDI\n1 NAME Stepan /Koval/\n1 SEX M\n1 FAMC @F1@\n1 FAMS @F3@\n\
0 @I5@ INDI\n1 NAME Zinaida /Koval/\n1 SEX F\n1 FAMC @F1@\n\
0 @I6@ INDI\n1 NAME Olena /Bondar/\n1 SEX F\n1 FAMS @F2@\n\
0 @I7@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n1 FAMC @F2@\n1 FAMS @F4@\n\
0 @I8@ INDI\n1 NAME Nina /Moroz/\n1 SEX F\n1 FAMS @F3@\n\
0 @I9@ INDI\n1 NAME Andriy /Koval/\n1 SEX M\n1 FAMC @F3@\n1 FAMS @F5@\n\
0 @I10@ INDI\n1 NAME Maria /Shevchuk/\n1 SEX F\n1 FAMS @F4@\n\
0 @I11@ INDI\n1 NAME Taras /Koval/\n1 SEX M\n1 FAMC @F4@\n\
0 @I12@ INDI\n1 NAME Sofia /Tkach/\n1 SEX F\n1 FAMS @F5@\n\
0 @I13@ INDI\n1 NAME Dmytro /Koval/\n1 SEX M\n1 FAMC @F5@\n\
0 @I14@ INDI\n1 NAME Zed /Solo/\n1 SEX M\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n1 CHIL @I4@\n1 CHIL @I5@\n\
0 @F2@ FAM\n1 HUSB @I3@\n1 WIFE @I6@\n1 CHIL @I7@\n\
0 @F3@ FAM\n1 HUSB @I4@\n1 WIFE @I8@\n1 CHIL @I9@\n\
0 @F4@ FAM\n1 HUSB @I7@\n1 WIFE @I10@\n1 CHIL @I11@\n\
0 @F5@ FAM\n1 HUSB @I9@\n1 WIFE @I12@\n1 CHIL @I13@\n";

fn everything() -> Rect {
    Rect {
        x: -1e6,
        y: -1e6,
        w: 2e6,
        h: 2e6,
    }
}

fn request(s: &Session, given: &[&str], generations: u32, kin: Option<AroundKin>) -> ChartRequest {
    let ids: Vec<_> = given.iter().map(|g| find(s.tree(), g)).collect();
    ChartRequest {
        kind: ChartKind::Around,
        root: ids[0],
        generations,
        aunts_uncles: None,
        kin,
        roots: Some(ids),
    }
}

fn nodes(s: &Session, m: &ChartMeta) -> Vec<ChartNode> {
    s.chart_window(m.chart_id, &everything(), 1.0)
        .unwrap()
        .nodes
}

fn names(ns: &[ChartNode]) -> Vec<String> {
    let mut v: Vec<String> = ns
        .iter()
        .filter_map(|n| n.person.as_ref().map(|p| p.name.clone()))
        .collect();
    v.sort();
    v
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

#[test]
fn two_cousins_share_their_grandparents_which_are_drawn_once() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    let m = s
        .chart_open(&request(&s, &["Ivan", "Andriy"], 255, None))
        .unwrap();
    assert_eq!(m.kind, ChartKind::Around);
    assert!(!m.truncated);
    let ns = nodes(&s, &m);
    let v = names(&ns);
    for want in [
        "Hryhorii Koval",
        "Halyna Koval",
        "Petro Koval",
        "Olena Bondar",
        "Stepan Koval",
        "Nina Moroz",
        "Ivan Koval",
        "Maria Shevchuk",
        "Andriy Koval",
        "Sofia Tkach",
        "Taras Koval",
        "Dmytro Koval",
    ] {
        assert!(
            v.contains(&want.to_string()),
            "{want} is on the chart: {v:?}"
        );
    }
    // The shared grandparents once; the aunt and the stranger are not on any line.
    assert_eq!(v.iter().filter(|n| *n == "Hryhorii Koval").count(), 1);
    assert_eq!(v.iter().filter(|n| *n == "Halyna Koval").count(), 1);
    assert!(!v.contains(&"Zinaida Koval".to_string()));
    assert!(!v.contains(&"Zed Solo".to_string()));
    assert_eq!(v.len(), 12);
    assert!(ns.iter().all(|n| n.dup_of.is_none()));
    assert_no_overlaps(&ns);
    // Both cousins are marked, the first is the root.
    let chosen = m.chosen_nodes.clone().unwrap();
    assert_eq!(chosen.len(), 2);
    assert_eq!(chosen[0], m.root_node);
    let by_id = |id: u32| ns.iter().find(|n| n.id == id).unwrap();
    assert_eq!(by_id(chosen[0]).person.as_ref().unwrap().name, "Ivan Koval");
    assert_eq!(
        by_id(chosen[1]).person.as_ref().unwrap().name,
        "Andriy Koval"
    );
    // Ancestors above, descendants below.
    let y = |name: &str| {
        ns.iter()
            .find(|n| n.person.as_ref().is_some_and(|p| p.name == name))
            .unwrap()
            .y
    };
    assert!(y("Hryhorii Koval") < y("Petro Koval"));
    assert!(y("Petro Koval") < y("Ivan Koval"));
    assert!(y("Ivan Koval") < y("Taras Koval"));
    assert!(y("Andriy Koval") < y("Dmytro Koval"));
    assert_eq!(y("Ivan Koval"), y("Andriy Koval"));
}

#[test]
fn labels_are_relative_to_the_nearest_chosen_person() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    let m = s
        .chart_open(&request(&s, &["Ivan", "Andriy"], 255, None))
        .unwrap();
    let ns = nodes(&s, &m);
    let rel = |name: &str| {
        ns.iter()
            .find(|n| n.person.as_ref().is_some_and(|p| p.name == name))
            .unwrap()
            .relation
            .clone()
    };
    // Every chosen person is worded like the single centre person.
    assert_eq!(rel("Ivan Koval"), rel("Andriy Koval"));
    assert_ne!(rel("Ivan Koval"), rel("Petro Koval"));
    assert!(rel("Petro Koval").is_some());
    assert!(rel("Taras Koval").is_some());
    assert_ne!(rel("Petro Koval"), rel("Hryhorii Koval"));
}

#[test]
fn the_outline_lists_every_chosen_person_at_the_top() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    let m = s
        .chart_open(&request(&s, &["Ivan", "Andriy"], 255, None))
        .unwrap();
    let top = s.chart_outline(m.chart_id, None, 10).unwrap();
    let top_names: Vec<&str> = top
        .iter()
        .map(|i| i.person.as_ref().unwrap().name.as_str())
        .collect();
    assert_eq!(top_names, ["Ivan Koval", "Andriy Koval"]);
    assert!(top.iter().all(|i| i.has_children));
}

#[test]
fn one_person_in_roots_is_the_ordinary_chart() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    let m = s.chart_open(&request(&s, &["Ivan"], 255, None)).unwrap();
    assert!(m.chosen_nodes.is_none());
    let v = names(&nodes(&s, &m));
    assert!(!v.contains(&"Stepan Koval".to_string()));
    assert!(v.contains(&"Taras Koval".to_string()));
}

#[test]
fn several_people_ignore_the_wider_kin_choice_and_show_the_line_only() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    let line = s
        .chart_open(&request(&s, &["Ivan", "Andriy"], 255, None))
        .unwrap();
    let a = names(&nodes(&s, &line));
    let all = s
        .chart_open(&request(&s, &["Ivan", "Andriy"], 255, Some(AroundKin::All)))
        .unwrap();
    assert_eq!(names(&nodes(&s, &all)), a);
}

#[test]
fn generations_limit_the_rows_above_and_below() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    let m = s
        .chart_open(&request(&s, &["Ivan", "Andriy"], 1, None))
        .unwrap();
    let v = names(&nodes(&s, &m));
    assert!(v.contains(&"Petro Koval".to_string()));
    assert!(v.contains(&"Taras Koval".to_string()));
    assert!(!v.contains(&"Hryhorii Koval".to_string()));
}

#[test]
fn people_who_left_the_project_or_repeat_are_dropped() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    let ivan = find(s.tree(), "Ivan");
    let req = ChartRequest {
        kind: ChartKind::Around,
        root: ivan,
        generations: 255,
        aunts_uncles: None,
        kin: None,
        roots: Some(vec![ivan, ivan, uuid::Uuid::from_u128(42)]),
    };
    let m = s.chart_open(&req).unwrap();
    assert!(m.chosen_nodes.is_none());
}

#[test]
fn reversed_direction_puts_the_ancestors_below() {
    let (_d, mut s) = open_tree(&tree_of(KOVAL));
    s.set_settings(SettingsPatch {
        tree_direction: Some(TreeDirection::Reversed),
        ..SettingsPatch::default()
    })
    .unwrap();
    let m = s
        .chart_open(&request(&s, &["Ivan", "Andriy"], 255, None))
        .unwrap();
    let ns = nodes(&s, &m);
    let y = |name: &str| {
        ns.iter()
            .find(|n| n.person.as_ref().is_some_and(|p| p.name == name))
            .unwrap()
            .y
    };
    assert!(y("Hryhorii Koval") > y("Ivan Koval"));
    assert!(y("Taras Koval") < y("Ivan Koval"));
    assert_no_overlaps(&ns);
}

#[test]
#[ignore = "heavy: runs before merge"]
fn heavy_twenty_people_of_a_5000_person_tree_open_in_reasonable_time() {
    let tree = synthetic_tree(5_000, 1_500);
    let mut ids: Vec<_> = tree.persons.keys().copied().collect();
    ids.sort();
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("project");
    let mut p = Project::create(&proj).unwrap();
    p.save_tree(&tree, today()).unwrap();
    drop(p);
    let mut s = Session::open(&proj, SessionClock::fixed(today(), NOW_MS)).unwrap();
    let chosen: Vec<_> = ids.iter().step_by(250).take(20).copied().collect();
    assert_eq!(chosen.len(), 20);
    let t = Instant::now();
    let m = s
        .chart_open(&ChartRequest {
            kind: ChartKind::Around,
            root: chosen[0],
            generations: 255,
            aunts_uncles: None,
            kin: None,
            roots: Some(chosen.clone()),
        })
        .unwrap();
    let took = t.elapsed();
    assert!(took < Duration::from_secs(30), "took {took:?}");
    assert!(!m.truncated);
    assert!(m.node_count >= 20);
    assert_eq!(m.chosen_nodes.as_ref().map(Vec::len), Some(20));
    let ns = nodes(&s, &m);
    assert_eq!(ns.len(), m.node_count as usize);
    let outline = s.chart_outline(m.chart_id, None, 100).unwrap();
    assert_eq!(outline.len(), 20);
}
