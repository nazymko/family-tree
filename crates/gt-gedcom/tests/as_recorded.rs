#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! A name as a metric book writes it (old spelling) keeps its kind and its citation through GEDCOM.

use gt_core::DayOrder;
use gt_core::date::GedcomVersion;
use gt_core::model::*;
use gt_core::tree::Tree;
use gt_gedcom::{ExportOptions, ImportOptions, export, import};

fn opts() -> ImportOptions {
    ImportOptions {
        day_order: DayOrder::Dmy,
        media_roots: vec![],
        ..Default::default()
    }
}

const SRC: &str = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n\
0 @I1@ INDI\n\
1 NAME Федір Петрович /Вигаданенко/\n\
1 NAME Ѳеодоръ Петровъ /Вигаданенко/\n\
2 TYPE as recorded\n\
2 SOUR @S1@\n\
3 PAGE арк. 12 зв., № 34\n\
0 @S1@ SOUR\n\
1 TITL Метрична книга вигаданої церкви, 1885\n\
0 TRLR\n";

fn recorded(t: &Tree) -> &Name {
    let p = t.persons.values().next().unwrap();
    p.names
        .iter()
        .find(|n| n.kind == NameKind::AsRecorded)
        .expect("a name as recorded")
}

#[test]
fn as_recorded_name_imports_with_its_citation() {
    let t = import(SRC.as_bytes(), &opts()).tree;
    let n = recorded(&t);
    assert_eq!(n.given.as_deref(), Some("Ѳеодоръ Петровъ"));
    assert_eq!(n.surnames, vec!["Вигаданенко".to_string()]);
    assert_eq!(n.citations.len(), 1);
    let c = &t.citations[&n.citations[0]];
    assert_eq!(c.page.as_deref(), Some("арк. 12 зв., № 34"));
    let p = t.persons.values().next().unwrap();
    assert_eq!(p.names[0].kind, NameKind::Birth);
}

#[test]
fn as_recorded_round_trips_in_both_versions() {
    let a = import(SRC.as_bytes(), &opts()).tree;
    for (v, line) in [
        (GedcomVersion::V551, "2 TYPE as recorded"),
        (GedcomVersion::V70, "3 PHRASE as recorded"),
    ] {
        let out = export(&a, &ExportOptions { version: v });
        let text = String::from_utf8(out.clone()).unwrap();
        assert!(text.contains(line), "{v:?}:\n{text}");
        assert!(text.contains("Ѳеодоръ"), "{v:?}: the old letters stay");
        if v == GedcomVersion::V70 {
            assert!(text.contains("2 TYPE OTHER"));
        }
        let b = import(&out, &opts()).tree;
        let (na, nb) = (recorded(&a), recorded(&b));
        assert_eq!(nb.given, na.given);
        assert_eq!(nb.surnames, na.surnames);
        assert_eq!(nb.citations.len(), 1, "{v:?}");
        assert_eq!(
            b.citations[&nb.citations[0]].page.as_deref(),
            Some("арк. 12 зв., № 34")
        );
    }
}

#[test]
fn other_writings_of_the_type_are_read_too() {
    for ty in ["As Recorded", "as_recorded", "AS-RECORDED"] {
        let src = SRC.replace("2 TYPE as recorded", &format!("2 TYPE {ty}"));
        let t = import(src.as_bytes(), &opts()).tree;
        recorded(&t);
    }
}
