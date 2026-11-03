use gt_gedcom::{TokenIssue, tokenize};
use proptest::prelude::*;

#[test]
fn nested_structure() {
    let t = tokenize(b"0 @I1@ INDI\r\n1 NAME John /Smith/\r\n2 GIVN John\r\n");
    assert_eq!(t.records.len(), 1);
    let rec = &t.records[0];
    assert_eq!(rec.xref.as_deref(), Some("@I1@"));
    assert_eq!(rec.tag, "INDI");
    assert_eq!(rec.children.len(), 1);
    let name = &rec.children[0];
    assert_eq!(name.tag, "NAME");
    assert_eq!(name.value.as_deref(), Some("John /Smith/"));
    assert_eq!(name.children.len(), 1);
    assert_eq!(name.children[0].tag, "GIVN");
    assert_eq!(name.children[0].value.as_deref(), Some("John"));
    assert!(t.issues.is_empty());
}

#[test]
fn conc_splits_multibyte_char() {
    let mut f = vec![0xEF, 0xBB, 0xBF];
    f.extend_from_slice(b"0 HEAD\r\n0 @N1@ NOTE\r\n1 NOTE caf\xC3\r\n2 CONC \xA9 ok\r\n");
    let t = tokenize(&f);
    let note = &t.records[1].children[0];
    assert_eq!(note.value.as_deref(), Some("caf\u{e9} ok"));
}

#[test]
fn cont_inserts_newline() {
    let t = tokenize(b"0 @N1@ NOTE\r\n1 NOTE a\r\n2 CONT b\r\n");
    assert_eq!(t.records[0].children[0].value.as_deref(), Some("a\nb"));
}

#[test]
fn ansel_file_decodes() {
    let bytes = include_bytes!("../../../tests/fixtures/gedcom/ansel.ged");
    let t = tokenize(bytes);
    let indi = &t.records[1];
    assert_eq!(indi.children[0].value.as_deref(), Some("M\u{fc}ller"));
}

#[test]
fn level_jump_reported() {
    let t = tokenize(b"0 HEAD\r\n2 X y\r\n");
    assert_eq!(t.issues, vec![TokenIssue::LevelJump { line: 2 }]);
    assert_eq!(t.records[0].children.len(), 1);
    assert_eq!(t.records[0].children[0].tag, "X");
}

#[test]
fn bad_line_skipped_and_at_unescaped() {
    let t = tokenize(b"0 HEAD\r\nnonsense\r\n1 NOTE a@@b\r\n\r\n  1  SOUR x\r\n");
    assert_eq!(t.issues.len(), 1);
    assert!(matches!(t.issues[0], TokenIssue::BadLine { line: 2, .. }));
    assert_eq!(t.records[0].children[0].value.as_deref(), Some("a@b"));
    assert_eq!(t.records[0].children[1].tag, "SOUR");
}

#[test]
fn orphan_conc_reported() {
    let t = tokenize(b"0 CONC x\r\n");
    assert_eq!(t.issues, vec![TokenIssue::OrphanConc { line: 1 }]);
}

#[test]
fn utf16_converted() {
    let mut f = vec![0xFF, 0xFE];
    f.extend(
        "0 HEAD\r\n1 NOTE \u{e9}\r\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes),
    );
    let t = tokenize(&f);
    assert_eq!(t.records[0].children[0].value.as_deref(), Some("\u{e9}"));
}

#[test]
fn to_raw_maps_positions() {
    let t = tokenize(b"0 @I1@ INDI\r\n1 NAME A\r\n1 SEX M\r\n");
    let raw = t.records[0].to_raw(3);
    assert_eq!(raw.pos, 3);
    assert_eq!(raw.xref.as_deref(), Some("@I1@"));
    assert_eq!(raw.children[1].pos, 1);
    assert_eq!(raw.children[1].tag, "SEX");
}

proptest! {
    #[test]
    fn garbage_does_not_panic(bytes in proptest::collection::vec(any::<u8>(), 0..=4096)) {
        let _ = tokenize(&bytes);
    }
}
