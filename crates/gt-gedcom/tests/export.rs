#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use gt_core::DayOrder;
use gt_core::date::GedcomVersion;
use gt_core::model::*;
use gt_gedcom::{ExportOptions, ImportOptions, export, import, tokenize};

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

fn basic() -> Vec<u8> {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/gedcom/basic551.ged"
    );
    std::fs::read(p).unwrap()
}

#[test]
fn line_limit_551() {
    let long: String = "Grüße é \u{4e2d} @ ".repeat(80);
    let long = format!("{}\n{}", &long[..long.len() - 1], long);
    let src = format!(
        "0 HEAD\n0 @I1@ INDI\n1 NOTE {}\n0 TRLR\n",
        long.replace('@', "@@").replace('\n', "\n2 CONT ")
    );
    let a = import(src.as_bytes(), &opts());
    let out = export(
        &a.tree,
        &ExportOptions {
            version: GedcomVersion::V551,
        },
    );
    let text = String::from_utf8(out.clone()).unwrap();
    assert!(text.contains("CHAR UTF-8"));
    assert!(text.contains("\r\n"));
    for line in text.split("\r\n") {
        assert!(line.len() <= 255, "line too long: {}", line.len());
    }
    assert!(text.contains(" CONC "));
    let b = import(&out, &opts());
    let na = a.tree.notes.values().next().unwrap();
    let nb = b.tree.notes.values().next().unwrap();
    assert_eq!(na.text, long);
    assert_eq!(nb.text, na.text);
}

#[test]
fn v70_has_no_conc_and_uses_lf() {
    let long = "x".repeat(1000);
    let src = format!("0 HEAD\n0 @I1@ INDI\n1 NOTE {long}\n0 TRLR\n");
    let a = import(src.as_bytes(), &opts());
    let text = ex(&a.tree, GedcomVersion::V70);
    assert!(!text.contains("CONC"));
    assert!(!text.contains('\r'));
    assert!(text.contains("GEDC\n2 VERS 7.0"));
    assert!(!text.contains("CHAR"));
    assert!(text.lines().any(|l| l.len() > 1000));
}

#[test]
fn schma_declares_extensions_70() {
    let a = import(&basic(), &opts());
    let text = ex(&a.tree, GedcomVersion::V70);
    assert!(text.contains("2 TAG _GTID https://gene-tree.app/gedcom/ext/GTID"));
    assert!(text.contains("2 TAG _EVID https://gene-tree.app/gedcom/ext/EVID"));
    let t551 = ex(&a.tree, GedcomVersion::V551);
    assert!(!t551.contains("SCHMA"));
}

#[test]
fn every_underscore_tag_is_declared_in_70() {
    let a = import(&basic(), &opts());
    let text = ex(&a.tree, GedcomVersion::V70);
    let tok = tokenize(text.as_bytes());
    let head = &tok.records[0];
    let declared: Vec<String> = head
        .children
        .iter()
        .filter(|c| c.tag == "SCHMA")
        .flat_map(|c| c.children.iter())
        .filter_map(|c| c.value.as_deref())
        .map(|v| v.split(' ').next().unwrap().to_string())
        .collect();
    fn walk(n: &gt_gedcom::GedNode, declared: &[String], bad: &mut Vec<String>) {
        if n.tag.starts_with('_') && !declared.contains(&n.tag) {
            bad.push(n.tag.clone());
        }
        for c in &n.children {
            walk(c, declared, bad);
        }
    }
    let mut bad = vec![];
    for r in &tok.records[1..] {
        walk(r, &declared, &mut bad);
    }
    // _MTTAG is foreign data carried verbatim and is not ours to declare.
    bad.retain(|t| t != "_MTTAG");
    assert!(bad.is_empty(), "undeclared: {bad:?}");
}

#[test]
fn unknown_tag_position_preserved() {
    let a = import(&basic(), &opts());
    let text = ex(&a.tree, GedcomVersion::V551);
    let tok = tokenize(text.as_bytes());
    let indi = tok.records.iter().find(|r| r.tag == "INDI").unwrap();
    let tags: Vec<&str> = indi.children.iter().map(|c| c.tag.as_str()).collect();
    let name = tags.iter().position(|t| *t == "NAME").unwrap();
    let mt = tags.iter().position(|t| *t == "_MTTAG").unwrap();
    let birt = tags.iter().position(|t| *t == "BIRT").unwrap();
    assert!(name < mt && mt < birt, "{tags:?}");
    assert!(tags.contains(&"_UID"));
}

#[test]
fn female_partner1_swapped() {
    let src = "0 HEAD\n0 @I1@ INDI\n1 SEX F\n1 FAMS @F1@\n0 @I2@ INDI\n1 SEX M\n1 FAMS @F1@\n0 @F1@ FAM\n1 HUSB @I2@\n1 WIFE @I1@\n0 TRLR\n";
    let mut a = import(src.as_bytes(), &opts());
    let (female, male) = {
        let mut it = a.tree.persons.keys();
        (*it.next().unwrap(), *it.next().unwrap())
    };
    let f = a.tree.families.values_mut().next().unwrap();
    f.partner1 = Some(female);
    f.partner2 = Some(male);
    let text = ex(&a.tree, GedcomVersion::V551);
    let tok = tokenize(text.as_bytes());
    let fam = tok.records.iter().find(|r| r.tag == "FAM").unwrap();
    let husb = fam.children.iter().find(|c| c.tag == "HUSB").unwrap();
    let man = tok
        .records
        .iter()
        .find(|r| {
            r.tag == "INDI"
                && r.children
                    .iter()
                    .any(|c| c.tag == "SEX" && c.value.as_deref() == Some("M"))
        })
        .unwrap();
    assert_eq!(husb.value, man.xref);
}

#[test]
fn shared_event_written_under_each_principal() {
    let src = "0 HEAD\n0 @I1@ INDI\n1 BAPM\n2 _EVID abc\n0 @I2@ INDI\n1 BAPM\n2 _EVID abc\n2 ASSO @I1@\n3 RELA Godfather\n0 TRLR\n";
    let a = import(src.as_bytes(), &opts());
    assert_eq!(a.tree.events.len(), 1);
    let text = ex(&a.tree, GedcomVersion::V551);
    assert_eq!(text.matches("1 BAPM").count(), 2);
    let b = import(text.as_bytes(), &opts());
    assert_eq!(b.tree.events.len(), 1);
    assert_eq!(
        a.tree.normalized_for_compare(),
        b.tree.normalized_for_compare()
    );
}

#[test]
fn unparsed_date_in_70_uses_phrase() {
    let src = "0 HEAD\n0 @I1@ INDI\n1 BIRT\n2 DATE sometime in spring\n0 TRLR\n";
    let a = import(src.as_bytes(), &opts());
    let t70 = ex(&a.tree, GedcomVersion::V70);
    assert!(t70.contains("2 DATE\n3 PHRASE sometime in spring"), "{t70}");
    let t551 = ex(&a.tree, GedcomVersion::V551);
    assert!(t551.contains("2 DATE sometime in spring"));
    for t in [t70, t551] {
        let b = import(t.as_bytes(), &opts());
        assert_eq!(
            a.tree.normalized_for_compare(),
            b.tree.normalized_for_compare()
        );
    }
}

#[test]
fn date_phrase_survives_551_via_note_or_int() {
    let src = "0 HEAD\n1 GEDC\n2 VERS 7.0\n0 @I1@ INDI\n1 BIRT\n2 DATE ABT 1850\n3 PHRASE family story\n1 DEAT\n2 DATE 1900\n3 PHRASE maybe\n0 TRLR\n";
    let a = import(src.as_bytes(), &opts());
    let t551 = ex(&a.tree, GedcomVersion::V551);
    assert!(t551.contains("NOTE Date phrase: family story"), "{t551}");
    assert!(t551.contains("2 DATE INT 1900 (maybe)"), "{t551}");
    for v in [GedcomVersion::V551, GedcomVersion::V70] {
        let b = import(ex(&a.tree, v).as_bytes(), &opts());
        assert_eq!(
            a.tree.normalized_for_compare(),
            b.tree.normalized_for_compare(),
            "{v:?}"
        );
    }
}

#[test]
fn places_names_and_ids_survive() {
    let a = import(&basic(), &opts());
    for v in [GedcomVersion::V551, GedcomVersion::V70] {
        let b = import(ex(&a.tree, v).as_bytes(), &opts());
        let pa = a.tree.persons.values().next().unwrap();
        let pb = b.tree.persons.values().next().unwrap();
        assert_eq!(pa.common.id, pb.common.id);
        assert_eq!(pa.names, pb.names);
        assert_eq!(a.tree.places, b.tree.places);
        assert_eq!(
            a.tree.normalized_for_compare(),
            b.tree.normalized_for_compare()
        );
        assert_eq!(b.tree.header.producer.as_deref(), Some("ANCESTRY"));
    }
    let _ = (Sex::Male, Role::Principal);
}
