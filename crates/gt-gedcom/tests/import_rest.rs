#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use gt_core::DayOrder;
use gt_core::model::*;
use gt_gedcom::{ImportOptions, ImportOutcome, MissingReason, Repair, import};
use std::path::PathBuf;

fn imp_with(s: &str, roots: Vec<PathBuf>) -> ImportOutcome {
    import(
        s.as_bytes(),
        &ImportOptions {
            day_order: DayOrder::Dmy,
            media_roots: roots,
            ..Default::default()
        },
    )
}

fn imp(s: &str) -> ImportOutcome {
    imp_with(s, vec![])
}

fn fixture(name: &str) -> ImportOutcome {
    let p = format!(
        "{}/../../tests/fixtures/gedcom/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    import(&std::fs::read(p).unwrap(), &ImportOptions::default())
}

#[test]
fn same_sex_couple() {
    let o = imp(
        "0 HEAD\n0 @I1@ INDI\n1 SEX M\n1 FAMS @F1@\n0 @I2@ INDI\n1 SEX M\n1 FAMS @F1@\n0 @F1@ FAM\n1 HUSB @I1@\n1 HUSB @I2@\n0 TRLR\n",
    );
    let f = o.tree.families.values().next().unwrap();
    assert!(f.partner1.is_some() && f.partner2.is_some());
    assert_ne!(f.partner1, f.partner2);
    assert!(o.report.repairs.is_empty());
}

#[test]
fn per_parent_relationship() {
    let o = imp(
        "0 HEAD\n0 @I1@ INDI\n1 FAMS @F1@\n0 @I3@ INDI\n1 FAMC @F1@\n0 @F1@ FAM\n1 HUSB @I1@\n1 CHIL @I3@\n2 _FREL Natural\n2 _MREL Adopted\n0 TRLR\n",
    );
    let l = &o.tree.families.values().next().unwrap().children[0];
    assert_eq!(l.rel1, ParentRel::Birth);
    assert_eq!(l.rel2, ParentRel::Adopted);
    assert!(l.preferred);
}

#[test]
fn pedi_applies_to_both() {
    let o = fixture("family7.ged");
    let l = &o.tree.families.values().next().unwrap().children[0];
    assert_eq!(
        (l.rel1.clone(), l.rel2.clone()),
        (ParentRel::Adopted, ParentRel::Adopted)
    );
    let f = o.tree.families.values().next().unwrap();
    assert_eq!(f.kind, UnionKind::Married);
    assert_eq!(o.tree.events.len(), 1);
    assert!(matches!(
        o.tree.events.values().next().unwrap().participants[0].target,
        ParticipantRef::Family(_)
    ));
}

#[test]
fn citation_on_name() {
    let o = imp(
        "0 HEAD\n0 @S1@ SOUR\n1 TITL Parish book\n0 @I1@ INDI\n1 NAME A /B/\n2 SOUR @S1@\n3 PAGE p. 12\n3 QUAY 3\n3 DATA\n4 DATE 1 JAN 1900\n4 TEXT quoted\n0 TRLR\n",
    );
    let p = o.tree.persons.values().next().unwrap();
    let cid = p.names[0].citations[0];
    let c = &o.tree.citations[&cid];
    assert_eq!(c.page.as_deref(), Some("p. 12"));
    assert_eq!(c.quality, Some(3));
    assert_eq!(c.text.as_deref(), Some("quoted"));
    assert!(c.date.is_some());
    let s = &o.tree.sources[&c.source.unwrap()];
    assert_eq!(s.title.as_deref(), Some("Parish book"));
}

#[test]
fn inline_source_creates_untitled_source() {
    let o = imp("0 HEAD\n0 @I1@ INDI\n1 BIRT\n2 SOUR heard from aunt\n0 TRLR\n");
    let c = o.tree.citations.values().next().unwrap();
    let s = &o.tree.sources[&c.source.unwrap()];
    assert_eq!(s.title, None);
    assert_eq!(s.text.as_deref(), Some("heard from aunt"));
}

#[test]
fn snote_shared() {
    let o = fixture("family7.ged");
    let n = o.tree.notes.values().next().unwrap();
    assert!(n.shared);
    assert_eq!(n.text, "Shared note line one\nline two");
    let p = o.tree.persons.values().next().unwrap();
    assert_eq!(p.common.notes, vec![n.common.id]);
}

#[test]
fn inline_note_not_shared() {
    let o = imp("0 HEAD\n0 @I1@ INDI\n1 NOTE hello\n2 CONT world\n0 TRLR\n");
    let n = o.tree.notes.values().next().unwrap();
    assert!(!n.shared);
    assert_eq!(n.text, "hello\nworld");
}

#[test]
fn media_region_and_primary() {
    let o = imp(
        "0 HEAD\n0 @I1@ INDI\n1 OBJE @O1@\n2 _PRIM Y\n2 _REGION 0.1 0.2 0.3 0.4\n0 @O1@ OBJE\n1 FILE pics\\a.jpg\n2 FORM jpeg\n1 TITL Portrait\n0 TRLR\n",
    );
    let p = o.tree.persons.values().next().unwrap();
    let l = &p.common.media[0];
    assert!(l.primary);
    assert_eq!(
        l.region,
        Some(Region {
            x: 0.1,
            y: 0.2,
            w: 0.3,
            h: 0.4
        })
    );
    let m = &o.tree.media[&l.media];
    assert_eq!(m.path, "pics/a.jpg");
    assert_eq!(m.mime.as_deref(), Some("jpeg"));
    assert_eq!(m.title.as_deref(), Some("Portrait"));
}

#[test]
fn broken_file_is_repaired() {
    let o = fixture("broken.ged");
    let r = &o.report.repairs;
    assert!(
        r.iter()
            .any(|x| matches!(x, Repair::DanglingPointer { xref, .. } if xref == "@F9@"))
    );
    assert!(
        r.iter()
            .any(|x| matches!(x, Repair::DuplicateXref { xref, .. } if xref == "@I1@"))
    );
    assert!(r.iter().any(|x| matches!(x, Repair::OneWayLink { .. })));
    assert!(r.contains(&Repair::MissingTrailer));
    assert_eq!(o.tree.persons.len(), 4);
    let f = o.tree.families.values().next().unwrap();
    assert_eq!(f.children.len(), 1);
}

#[test]
fn missing_header_reported() {
    let o = imp("0 @I1@ INDI\n0 TRLR\n");
    assert!(o.report.repairs.contains(&Repair::MissingHeader));
}

#[test]
fn missing_media_reported() {
    let o = imp("0 HEAD\n0 @O1@ OBJE\n1 FILE C:\\Users\\x\\photo.jpg\n0 TRLR\n");
    assert_eq!(o.report.missing_media.len(), 1);
    assert_eq!(o.report.missing_media[0].path, "C:/Users/x/photo.jpg");
    assert_eq!(o.report.missing_media[0].reason, MissingReason::NotFound);
    assert_eq!(
        o.tree.media.values().next().unwrap().path,
        "C:/Users/x/photo.jpg"
    );
}

#[test]
fn media_found_by_basename() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub/photo.jpg"), b"x").unwrap();
    let o = imp_with(
        "0 HEAD\n0 @O1@ OBJE\n1 FILE C:\\Users\\x\\photo.jpg\n0 TRLR\n",
        vec![dir.path().to_path_buf()],
    );
    assert!(o.report.missing_media.is_empty());
    assert_eq!(o.resolved_media.len(), 1);
    assert!(o.resolved_media[0].1.ends_with("photo.jpg"));
    let m = o.tree.media.values().next().unwrap();
    assert_eq!(m.original_name.as_deref(), Some("photo.jpg"));
    assert_eq!(o.resolved_media[0].0, m.common.id);
}

#[test]
fn relative_file_resolves_and_rejects_parent_dirs() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.jpg"), b"x").unwrap();
    let o = imp_with(
        "0 HEAD\n0 @O1@ OBJE\n1 FILE a.jpg\n0 TRLR\n",
        vec![dir.path().to_path_buf()],
    );
    assert_eq!(o.resolved_media.len(), 1);
    let o = imp_with(
        "0 HEAD\n0 @O1@ OBJE\n1 FILE ../a.jpg\n0 TRLR\n",
        vec![dir.path().join("nowhere")],
    );
    assert_eq!(o.report.missing_media.len(), 1);
}
