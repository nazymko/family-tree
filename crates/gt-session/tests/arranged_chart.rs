#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The chart arranged around one person: same people, another order, cached beside the plain chart.

mod common;

use common::*;
use gt_session::*;
use uuid::Uuid;

/// Ivan + Olena have Taras and Maria; Taras + Oksana have Andriy. Ivan's parents are Petro + Halyna,
/// Olena's are Stepan + Nina. Zed is alone.
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
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n1 CHIL @I4@\n\
0 @F2@ FAM\n1 HUSB @I7@\n1 WIFE @I8@\n1 CHIL @I1@\n\
0 @F3@ FAM\n1 HUSB @I9@\n1 WIFE @I10@\n1 CHIL @I2@\n\
0 @F4@ FAM\n1 HUSB @I3@\n1 WIFE @I5@\n1 CHIL @I6@\n";

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

fn id(s: &Session, given: &str) -> Uuid {
    find(s.tree(), given)
}

fn names(s: &Session, m: &WholeMeta) -> Vec<String> {
    let w = s.chart_window(m.meta.chart_id, &everything(), 1.0).unwrap();
    let mut v: Vec<String> = w
        .nodes
        .iter()
        .filter_map(|n| n.person.as_ref())
        .map(|p| p.name.clone())
        .collect();
    v.sort();
    v
}

#[test]
fn the_same_people_are_drawn_and_the_plain_chart_stays_the_plain_chart() {
    let (_d, mut s) = open();
    let plain = s.chart_open_whole().unwrap();
    let a = s.chart_open_arranged(id(&s, "Taras"), None).unwrap();
    assert_ne!(a.meta.chart_id, plain.meta.chart_id);
    assert_eq!(a.meta.node_count, plain.meta.node_count);
    assert_eq!(names(&s, &a), names(&s, &plain));
    // The plain chart is still the one served (not replaced, not evicted).
    assert_eq!(s.chart_open_whole().unwrap(), plain);
    // Asking again gives the held chart; another person replaces it, still beside the plain one.
    assert_eq!(s.chart_open_arranged(id(&s, "Taras"), None).unwrap(), a);
    let b = s.chart_open_arranged(id(&s, "Maria"), None).unwrap();
    assert_ne!(b.meta.chart_id, a.meta.chart_id);
    assert_eq!(s.chart_open_whole().unwrap(), plain);
    assert!(s.chart_window(a.meta.chart_id, &everything(), 1.0).is_err());
}

#[test]
fn it_works_on_the_chart_of_chosen_people_and_leaves_that_one_alone() {
    let (_d, mut s) = open();
    let sel = vec![id(&s, "Taras")];
    let chosen = s.chart_open_selection(&sel, false).unwrap();
    let a = s
        .chart_open_arranged(id(&s, "Taras"), Some((&sel, false)))
        .unwrap();
    assert_eq!(a.chosen, 1);
    assert_eq!(a.meta.node_count, chosen.meta.node_count);
    assert_eq!(s.chart_open_selection(&sel, false).unwrap(), chosen);
    // Someone the selection leaves out cannot be the centre.
    let e = s
        .chart_open_arranged(id(&s, "Zed"), Some((&sel, false)))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn edits_make_it_stale_and_a_deleted_person_is_not_found() {
    let (_d, mut s) = open();
    let taras = id(&s, "Taras");
    let a = s.chart_open_arranged(taras, None).unwrap();
    s.apply(EditCommand::DeletePerson { id: id(&s, "Zed") })
        .unwrap();
    let e = s
        .chart_window(a.meta.chart_id, &everything(), 1.0)
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Stale);
    let again = s.chart_open_arranged(taras, None).unwrap();
    assert_eq!(again.meta.node_count, a.meta.node_count - 1);
    s.apply(EditCommand::DeletePerson { id: taras }).unwrap();
    assert_eq!(
        s.chart_open_arranged(taras, None).unwrap_err().code,
        ErrorCode::NotFound
    );
    assert_eq!(
        s.chart_open_arranged(Uuid::from_u128(7), None)
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
}
