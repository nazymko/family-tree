#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Godparents in GEDCOM: a person as `ASSO` with a godparent's role, a name only as a 7.0
//! `ASSO @VOID@` with a `PHRASE` or a 5.5.1 `NOTE Godfather: <name>`, both ways.

use gt_core::DayOrder;
use gt_core::date::GedcomVersion;
use gt_core::model::*;
use gt_gedcom::{ExportOptions, ImportOptions, export, import};

fn opts() -> ImportOptions {
    ImportOptions {
        day_order: DayOrder::Dmy,
        media_roots: vec![],
        ..Default::default()
    }
}

fn ex(t: &gt_core::tree::Tree, v: GedcomVersion) -> String {
    String::from_utf8(export(t, &ExportOptions { version: v })).unwrap()
}

const SRC: &str = "0 HEAD
0 @I1@ INDI
1 NAME Petro /Koval/
1 BAPM
2 DATE 6 FEB 1906
2 PLAC Ivanivka
2 ASSO @I2@
3 RELA Godfather
3 NOTE the uncle
2 ASSO @I3@
3 RELA Godparent
2 NOTE Godmother: Hanna Bondar, a widow of Ivanivka
3 CONT she could not sign
2 ASSO @I3@
3 RELA Witness
0 @I2@ INDI
1 NAME Ivan /Koval/
1 SEX M
0 @I3@ INDI
1 NAME Maria /Shevchuk/
1 SEX F
0 TRLR
";

fn baptism(t: &gt_core::tree::Tree) -> &Event {
    t.events
        .values()
        .find(|e| e.kind == EventKind::Baptism)
        .unwrap()
}

#[test]
fn godparents_are_read_from_asso_and_from_the_note() {
    let a = import(SRC.as_bytes(), &opts());
    let e = baptism(&a.tree);
    let gods: Vec<_> = e
        .godparents
        .iter()
        .map(|g| (g.person.is_some(), g.name.clone(), g.role, g.note.clone()))
        .collect();
    assert_eq!(
        gods,
        vec![
            (
                true,
                None,
                GodparentRole::Godfather,
                Some("the uncle".into())
            ),
            (true, None, GodparentRole::Godparent, None),
            (
                false,
                Some("Hanna Bondar, a widow of Ivanivka".into()),
                GodparentRole::Godmother,
                Some("she could not sign".into()),
            ),
        ]
    );
    // The witness stays a participant; no note is left over.
    assert!(e.participants.iter().any(|p| p.role == Role::Witness));
    assert!(e.common.notes.is_empty());
}

#[test]
fn godparents_round_trip_in_both_versions() {
    let a = import(SRC.as_bytes(), &opts());
    for v in [GedcomVersion::V551, GedcomVersion::V70] {
        let text = ex(&a.tree, v);
        if v == GedcomVersion::V70 {
            assert!(
                text.contains("2 ASSO @VOID@\n3 PHRASE Hanna Bondar, a widow of Ivanivka"),
                "{text}"
            );
            assert!(text.contains("3 ROLE GODP\n4 PHRASE Godfather"), "{text}");
        } else {
            assert!(text.contains("3 RELA Godfather"), "{text}");
            assert!(
                text.contains("2 NOTE Godmother: Hanna Bondar, a widow of Ivanivka"),
                "{text}"
            );
        }
        let b = import(text.as_bytes(), &opts());
        assert_eq!(
            a.tree.normalized_for_compare(),
            b.tree.normalized_for_compare(),
            "{v:?}\n{text}"
        );
    }
}

#[test]
fn a_shared_event_does_not_get_its_godparents_twice() {
    let src = "0 HEAD
0 @I1@ INDI
1 BAPM
2 _EVID abc
2 ASSO @I3@
3 RELA Godmother
0 @I2@ INDI
1 BAPM
2 _EVID abc
2 ASSO @I3@
3 RELA Godmother
0 @I3@ INDI
1 NAME Maria /Shevchuk/
0 TRLR
";
    let a = import(src.as_bytes(), &opts());
    assert_eq!(a.tree.events.len(), 1);
    assert_eq!(baptism(&a.tree).godparents.len(), 1);
}
