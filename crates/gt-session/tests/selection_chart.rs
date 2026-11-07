#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The chart of chosen people: them, their ancestors and their partners (and optionally all descendants).

mod common;

use common::*;
use gt_session::*;
use std::collections::HashSet;
use uuid::Uuid;

/// Ivan + Olena have Taras and Maria; Taras + Oksana have Andriy. Ivan's parents are Petro + Halyna,
/// Olena's are Stepan + Nina. Pavlo + Iryna have Dmytro (a second, unconnected family). Zed is alone.
const KOVAL: &str = "\
0 @I1@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n1 FAMS @F1@\n1 FAMC @F2@\n\
0 @I2@ INDI\n1 NAME Olena /Koval/\n1 SEX F\n1 FAMS @F1@\n1 FAMC @F3@\n\
0 @I3@ INDI\n1 NAME Taras /Koval/\n1 SEX M\n1 FAMC @F1@\n1 FAMS @F4@\n\
0 @I4@ INDI\n1 NAME Maria /Koval/\n1 SEX F\n1 FAMC @F1@\n\
0 @I5@ INDI\n1 NAME Oksana /Bondar/\n1 SEX F\n1 FAMS @F4@\n\
0 @I6@ INDI\n1 NAME Andriy /Koval/\n1 SEX M\n1 FAMC @F4@\n\
0 @I7@ INDI\n1 NAME Petro /Koval/\n1 SEX M\n1 FAMS @F2@\n\
0 @I8@ INDI\n1 NAME Halyna /Koval/\n1 SEX F\n1 FAMS @F2@\n\
0 @I9@ INDI\n1 NAME Stepan /Moroz/\n1 SEX M\n1 FAMS @F3@\n\
0 @I10@ INDI\n1 NAME Nina /Moroz/\n1 SEX F\n1 FAMS @F3@\n\
0 @I11@ INDI\n1 NAME Zed /Solo/\n1 SEX M\n\
0 @I12@ INDI\n1 NAME Pavlo /Shevchenko/\n1 SEX M\n1 FAMS @F5@\n\
0 @I13@ INDI\n1 NAME Iryna /Shevchenko/\n1 SEX F\n1 FAMS @F5@\n\
0 @I14@ INDI\n1 NAME Dmytro /Shevchenko/\n1 SEX M\n1 FAMC @F5@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n1 CHIL @I4@\n\
0 @F2@ FAM\n1 HUSB @I7@\n1 WIFE @I8@\n1 CHIL @I1@\n\
0 @F3@ FAM\n1 HUSB @I9@\n1 WIFE @I10@\n1 CHIL @I2@\n\
0 @F4@ FAM\n1 HUSB @I3@\n1 WIFE @I5@\n1 CHIL @I6@\n\
0 @F5@ FAM\n1 HUSB @I12@\n1 WIFE @I13@\n1 CHIL @I14@\n";

fn everything() -> Rect {
    Rect {
        x: -1e6,
        y: -1e6,
        w: 2e6,
        h: 2e6,
    }
}

fn open() -> (tempfile::TempDir, Session) {
    open_tree(&tree_of(KOVAL))
}

fn ids(s: &Session, given: &[&str]) -> Vec<Uuid> {
    given.iter().map(|g| find(s.tree(), g)).collect()
}

/// Given names of every drawn person, sorted.
fn shown(s: &Session, m: &WholeMeta) -> Vec<String> {
    let w = s.chart_window(m.meta.chart_id, &everything(), 1.0).unwrap();
    let mut names: Vec<String> = w
        .nodes
        .iter()
        .filter_map(|n| n.person.as_ref())
        .map(|p| p.name.split(' ').next().unwrap().to_string())
        .collect();
    names.sort();
    names
}

fn sorted(v: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = v.iter().map(ToString::to_string).collect();
    v.sort();
    v
}

#[test]
fn subset_is_the_chosen_person_ancestors_and_partners() {
    let (_d, mut s) = open();
    let m = s.chart_open_selection(&ids(&s, &["Taras"]), false).unwrap();
    assert_eq!(
        shown(&s, &m),
        sorted(&[
            "Taras", "Oksana", "Ivan", "Olena", "Petro", "Halyna", "Stepan", "Nina"
        ])
    );
    assert_eq!(m.meta.node_count, 8);
    assert_eq!(m.chosen, 1);
    assert_eq!(m.blocks.len(), 1);
    assert!(!m.blocks[0].unconnected);
    assert_eq!(m.blocks[0].people, 8);
    assert_eq!(m.meta.kind, ChartKind::Whole);
}

#[test]
fn a_shared_ancestor_appears_once_and_connected_people_make_one_group() {
    let (_d, mut s) = open();
    let m = s
        .chart_open_selection(&ids(&s, &["Taras", "Maria"]), false)
        .unwrap();
    let names = shown(&s, &m);
    assert_eq!(names.iter().filter(|n| *n == "Ivan").count(), 1);
    assert_eq!(names.len(), 9, "{names:?}");
    assert_eq!(m.blocks.len(), 1);
    assert_eq!(m.chosen, 2);
    // The same person chosen twice counts once.
    let again = s
        .chart_open_selection(&ids(&s, &["Maria", "Taras", "Maria"]), false)
        .unwrap();
    assert_eq!(again.chosen, 2);
    assert_eq!(again.meta.chart_id, m.meta.chart_id);
}

#[test]
fn unconnected_people_make_separate_groups() {
    let (_d, mut s) = open();
    let m = s
        .chart_open_selection(&ids(&s, &["Taras", "Dmytro", "Zed"]), false)
        .unwrap();
    assert_eq!(m.chosen, 3);
    assert_eq!(m.blocks.len(), 3);
    let titles: Vec<&str> = m.blocks.iter().map(|b| b.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Koval family (8 people)",
            "Shevchenko family (3 people)",
            "Unconnected people (1)"
        ]
    );
    assert_eq!(m.meta.node_count, 12);
}

#[test]
fn the_descendants_flag_adds_everyone_below_and_their_partners() {
    let (_d, mut s) = open();
    let ivan = ids(&s, &["Ivan"]);
    let without = s.chart_open_selection(&ivan, false).unwrap();
    assert_eq!(
        shown(&s, &without),
        sorted(&["Ivan", "Olena", "Petro", "Halyna"])
    );
    let with = s.chart_open_selection(&ivan, true).unwrap();
    assert_ne!(with.meta.chart_id, without.meta.chart_id);
    // Olena's own parents are not below or beside Ivan: they stay off.
    assert_eq!(
        shown(&s, &with),
        sorted(&[
            "Ivan", "Olena", "Petro", "Halyna", "Taras", "Maria", "Oksana", "Andriy"
        ])
    );
    assert_eq!(with.chosen, 1);
    // Maria has no partner or children: her ancestors and herself.
    let t = s.chart_open_selection(&ids(&s, &["Maria"]), true).unwrap();
    assert_eq!(shown(&s, &t).len(), 7, "{:?}", shown(&s, &t));
}

#[test]
fn loops_in_the_data_end() {
    // Ivan is his own child, and his grandchild is his parent.
    let looped = "\
0 @I1@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n1 FAMS @F1@\n1 FAMC @F1@\n1 FAMC @F2@\n\
0 @I2@ INDI\n1 NAME Olena /Koval/\n1 SEX F\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Taras /Koval/\n1 SEX M\n1 FAMS @F2@\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I1@\n1 CHIL @I3@\n\
0 @F2@ FAM\n1 HUSB @I3@\n1 CHIL @I1@\n";
    let (_d, mut s) = open_tree(&tree_of(looped));
    let ivan = ids(&s, &["Ivan"]);
    for flag in [false, true] {
        let m = s.chart_open_selection(&ivan, flag).unwrap();
        let names = shown(&s, &m);
        let unique: HashSet<&String> = names.iter().collect();
        assert_eq!(names.len(), unique.len(), "{names:?}");
        assert!(names.contains(&"Ivan".to_string()));
    }
}

#[test]
fn a_sibling_marker_is_drawn_with_its_family_but_not_counted() {
    let (_d, mut s) = open();
    let taras = find(s.tree(), "Taras");
    let fam = s
        .tree()
        .families
        .values()
        .find(|f| f.children.iter().any(|c| c.person == taras))
        .unwrap()
        .common
        .id;
    let marker = s
        .apply(EditCommand::AddSiblingMarker { family: fam })
        .unwrap()
        .focus
        .unwrap();
    let m = s.chart_open_selection(&[taras], false).unwrap();
    assert_eq!(m.meta.node_count, 8);
    assert_eq!(m.blocks[0].people, 8);
    let w = s.chart_window(m.meta.chart_id, &everything(), 1.0).unwrap();
    assert_eq!(w.nodes.len(), 9);
    assert!(
        w.nodes
            .iter()
            .any(|n| n.person.as_ref().is_some_and(|p| p.sibling_marker))
    );
    // The marker cannot be chosen: with nobody else chosen the chart is refused.
    let e = s.chart_open_selection(&[marker], false).unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
    // A marker of a family that is not on the chart stays off.
    let zed_only = s.chart_open_selection(&ids(&s, &["Zed"]), false).unwrap();
    assert_eq!(zed_only.meta.node_count, 1);
    let w = s
        .chart_window(zed_only.meta.chart_id, &everything(), 1.0)
        .unwrap();
    assert_eq!(w.nodes.len(), 1);
}

#[test]
fn a_deleted_person_is_dropped_and_nobody_left_is_not_found() {
    let (_d, mut s) = open();
    let both = ids(&s, &["Taras", "Dmytro"]);
    let before = s.chart_open_selection(&both, false).unwrap();
    assert_eq!(before.chosen, 2);
    s.apply(EditCommand::DeletePerson { id: both[1] }).unwrap();
    // The old chart is stale; the same request now lays out what is left.
    let e = s
        .chart_window(before.meta.chart_id, &everything(), 1.0)
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Stale);
    let after = s.chart_open_selection(&both, false).unwrap();
    assert_eq!(after.chosen, 1);
    assert_eq!(after.meta.node_count, 8);
    s.apply(EditCommand::DeletePerson { id: both[0] }).unwrap();
    assert_eq!(
        s.chart_open_selection(&both, false).unwrap_err().code,
        ErrorCode::NotFound
    );
    assert_eq!(
        s.chart_open_selection(&[], false).unwrap_err().code,
        ErrorCode::NotFound
    );
    // Undo brings the people back.
    s.undo().unwrap();
    s.undo().unwrap();
    assert_eq!(s.chart_open_selection(&both, false).unwrap().chosen, 2);
}

#[test]
fn the_chart_of_everyone_is_not_touched_by_a_selection_chart() {
    let (_d, mut s) = open();
    let all = s.chart_open_whole().unwrap();
    assert_eq!(all.chosen, 0);
    let sel = s.chart_open_selection(&ids(&s, &["Taras"]), false).unwrap();
    assert_ne!(sel.meta.chart_id, all.meta.chart_id);
    // Still the same chart, still alive, still everyone.
    let again = s.chart_open_whole().unwrap();
    assert_eq!(again, all);
    assert_eq!(again.meta.node_count, 14);
    assert_eq!(
        s.chart_window(all.meta.chart_id, &everything(), 1.0)
            .unwrap()
            .nodes
            .len(),
        14
    );
    // And the selection chart is not served for the plain one (nor the other way round).
    let sel2 = s.chart_open_selection(&ids(&s, &["Taras"]), false).unwrap();
    assert_eq!(sel2.meta.chart_id, sel.meta.chart_id);
    // A different request replaces the selection chart only.
    let other = s.chart_open_selection(&ids(&s, &["Taras"]), true).unwrap();
    assert_ne!(other.meta.chart_id, sel.meta.chart_id);
    assert!(
        s.chart_window(sel.meta.chart_id, &everything(), 1.0)
            .is_err()
    );
    assert!(
        s.chart_window(all.meta.chart_id, &everything(), 1.0)
            .is_ok()
    );
    assert_eq!(s.chart_open_whole().unwrap(), all);
    // Opening other kinds of chart does not push the selection chart out either.
    let taras = find(s.tree(), "Taras");
    for _ in 0..4 {
        s.chart_open(&ChartRequest {
            kind: ChartKind::Pedigree,
            root: taras,
            generations: 3,
            aunts_uncles: None,
            kin: None,
        })
        .unwrap();
    }
    assert!(
        s.chart_window(other.meta.chart_id, &everything(), 1.0)
            .is_ok()
    );
}

#[test]
fn lineage_locate_and_node_keys_work_on_the_selection_chart() {
    let (_d, mut s) = open();
    let m = s.chart_open_selection(&ids(&s, &["Taras"]), false).unwrap();
    let id = m.meta.chart_id;
    let taras = s
        .chart_locate(id, find(s.tree(), "Taras"))
        .unwrap()
        .unwrap();
    // Taras, his parents and grandparents (and Oksana is only a partner).
    assert_eq!(s.chart_lineage(id, taras.node_id).unwrap().len(), 7);
    assert!(
        s.chart_locate(id, find(s.tree(), "Maria"))
            .unwrap()
            .is_none()
    );
    assert!(s.chart_locate(id, Uuid::nil()).unwrap().is_none());
    s.chart_set_reference(id, Some(find(s.tree(), "Taras")))
        .unwrap();
    let w = s.chart_window(id, &everything(), 1.0).unwrap();
    let ivan = w
        .nodes
        .iter()
        .find(|n| {
            n.person
                .as_ref()
                .is_some_and(|p| p.name.starts_with("Ivan"))
        })
        .unwrap();
    assert_eq!(ivan.person.as_ref().unwrap().id, find(s.tree(), "Ivan"));
    assert!(ivan.kinship.is_some());
    assert_eq!(
        s.chart_outline(id, None, 10).unwrap_err().code,
        ErrorCode::Invalid
    );
}

#[test]
fn reversed_direction_puts_the_ancestors_below() {
    let (_d, mut s) = open();
    let taras = ids(&s, &["Taras"]);
    let m = s.chart_open_selection(&taras, false).unwrap();
    let y = |s: &Session, m: &WholeMeta, name: &str| {
        s.chart_window(m.meta.chart_id, &everything(), 1.0)
            .unwrap()
            .nodes
            .iter()
            .find(|n| n.person.as_ref().is_some_and(|p| p.name.starts_with(name)))
            .unwrap()
            .y
    };
    assert!(y(&s, &m, "Ivan") < y(&s, &m, "Taras"));
    s.set_settings(SettingsPatch {
        tree_direction: Some(TreeDirection::Reversed),
        ..SettingsPatch::default()
    })
    .unwrap();
    let r = s.chart_open_selection(&taras, false).unwrap();
    assert_ne!(r.meta.chart_id, m.meta.chart_id);
    assert!(y(&s, &r, "Ivan") > y(&s, &r, "Taras"));
}
