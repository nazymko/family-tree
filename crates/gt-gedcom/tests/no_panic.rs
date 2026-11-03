#![allow(clippy::unwrap_used)]
//! Stable-toolchain stand-in for the cargo-fuzz targets: untrusted input must never panic.
//! `PROPTEST_CASES=100000 cargo test -p gt-gedcom --test no_panic --release` runs it much longer.

use gt_core::date::{DayOrder, GedcomVersion, GenDate};
use gt_gedcom::{ExportOptions, ImportOptions, export, import, tokenize};
use proptest::prelude::*;

const SEEDS: [&[u8]; 4] = [
    include_bytes!("../../../tests/fixtures/gedcom/basic551.ged"),
    include_bytes!("../../../tests/fixtures/gedcom/family7.ged"),
    include_bytes!("../../../tests/fixtures/gedcom/broken.ged"),
    include_bytes!("../../../tests/fixtures/gedcom/ansel.ged"),
];

fn cases() -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(256)
}

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: cases(),
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// import, then export in both versions, then import the export again.
fn pipeline(bytes: &[u8]) {
    let out = import(bytes, &ImportOptions::default());
    let _ = out.report.to_text();
    for version in [GedcomVersion::V551, GedcomVersion::V70] {
        let exported = export(&out.tree, &ExportOptions { version });
        let again = import(&exported, &ImportOptions::default());
        let _ = again.report.to_text();
    }
}

fn mutate(seed: &[u8], ops: &[(u8, usize, u8)]) -> Vec<u8> {
    let mut v = seed.to_vec();
    for &(kind, pos, byte) in ops {
        if v.is_empty() {
            v.push(byte);
            continue;
        }
        let p = pos % v.len();
        match kind % 5 {
            0 => v[p] = byte,
            1 => v.insert(p, byte),
            2 => {
                v.remove(p);
            }
            3 => v.truncate(p),
            _ => {
                let end = (p + 1 + byte as usize).min(v.len());
                let chunk: Vec<u8> = v[p..end].to_vec();
                v.splice(p..p, chunk);
            }
        }
    }
    v
}

fn lit(items: &[&str]) -> impl Strategy<Value = String> + use<> {
    proptest::sample::select(items.iter().map(|s| s.to_string()).collect::<Vec<_>>())
}

fn value() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => lit(&[
            "", "@I1@", "@F1@", "@N1@", "@VOID@", "@@", "@", "Anna /Mueller/", "//", "/",
            "ABT 1850", "BET 1 JAN 1900 AND 3 MAR 1901", "Y", "M", "7.0", "5.5.1", "UTF-8",
            "ANSEL", "../../etc/passwd", "C:\\x\\y.jpg", "file:///C:/a%20b.jpg",
            "https://example.com/a.jpg", "0.5 0.5 2 -1", "N52.5",
            "0123456789abcdef0123456789abcdef", "018f3a52-7c3e-7b4f-8a2d-1c2d3e4f5a6b",
        ]),
        1 => "\\PC{0,40}",
        1 => "[ -~]{0,300}",
    ]
}

fn tag() -> impl Strategy<Value = String> {
    prop_oneof![
        8 => lit(&[
            "HEAD", "INDI", "FAM", "NAME", "GIVN", "SURN", "SEX", "BIRT", "DEAT", "MARR",
            "DATE", "PHRASE", "PLAC", "MAP", "LATI", "LONG", "HUSB", "WIFE", "CHIL", "FAMC",
            "FAMS", "PEDI", "NOTE", "SNOTE", "CONT", "CONC", "SOUR", "REPO", "OBJE", "FILE",
            "FORM", "ASSO", "ROLE", "EVEN", "FACT", "TYPE", "NO", "_NO", "_GTID", "_EVID",
            "_PREF", "_PROOF", "_REGION", "_FREL", "_MREL", "SCHMA", "TAG", "GEDC", "VERS",
            "CHAR", "CHAN", "RESN", "TRLR", "SUBM", "SUBN", "ADOP", "BAPM", "DIV", "OCCU",
            "TITL", "PAGE", "QUAY", "DATA", "TEXT", "EVEN", "TIME", "_MARNM",
        ]),
        1 => "[A-Z_]{1,8}",
    ]
}

fn level() -> impl Strategy<Value = String> {
    prop_oneof![
        16 => (0u8..5).prop_map(|l| l.to_string()),
        1 => lit(&["99999999999999999999", "-1", "x", ""]),
    ]
}

fn line() -> impl Strategy<Value = String> {
    (
        level(),
        proptest::option::of(prop_oneof![
            3 => lit(&["@I1@", "@F1@", "@N1@", "@I2@", "@S1@"]),
            1 => "@[A-Z0-9]{1,5}@",
        ]),
        tag(),
        value(),
    )
        .prop_map(|(l, x, t, v)| match x {
            Some(x) => format!("{l} {x} {t} {v}"),
            None => format!("{l} {t} {v}"),
        })
}

fn document() -> impl Strategy<Value = Vec<u8>> {
    (
        lit(&["\n", "\r\n", "\r"]),
        any::<bool>(),
        lit(&["5.5.1", "7.0", "5.5"]),
        proptest::collection::vec(line(), 0..120),
    )
        .prop_map(|(eol, bom, vers, lines)| {
            let mut s = String::new();
            if bom {
                s.push('\u{feff}');
            }
            s.push_str(&format!("0 HEAD{eol}1 GEDC{eol}2 VERS {vers}{eol}"));
            for l in lines {
                s.push_str(&l);
                s.push_str(&eol);
            }
            s.push_str(&format!("0 TRLR{eol}"));
            s.into_bytes()
        })
}

fn date_text() -> impl Strategy<Value = String> {
    let tok = prop_oneof![
        6 => lit(&[
            "ABT", "BEF", "AFT", "BET", "AND", "FROM", "TO", "CAL", "EST", "INT", "@#DJULIAN@",
            "@#DHEBREW@", "@#DFRENCH R@", "@#DUNKNOWN@", "JAN", "FEB", "TSH", "ADR", "VEND",
            "COMP", "B.C.", "BCE", "1850/51", "0", "00", "31", "32", "1", "12", "1900",
            "99999", "2147483647", "-5", "99999999999999999999", "/", "(phrase)", "-", ".",
        ]),
        1 => "\\PC{1,6}",
    ];
    proptest::collection::vec(tok, 0..10).prop_map(|v| v.join(" "))
}

fn exercise_date(s: &str) {
    for order in [DayOrder::Dmy, DayOrder::Mdy] {
        let d = GenDate::parse(s, order);
        let _ = d.display();
        let _ = d.sort_key();
        let _ = d.to_gedcom(GedcomVersion::V551);
        let _ = d.to_gedcom(GedcomVersion::V70);
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn tokenize_arbitrary_bytes(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let _ = tokenize(&bytes);
    }

    #[test]
    fn pipeline_arbitrary_bytes(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        pipeline(&bytes);
    }

    #[test]
    fn pipeline_structured_documents(doc in document()) {
        let _ = tokenize(&doc);
        pipeline(&doc);
    }

    #[test]
    fn pipeline_mutated_fixtures(
        seed in 0usize..SEEDS.len(),
        ops in proptest::collection::vec((any::<u8>(), any::<usize>(), any::<u8>()), 1..24),
    ) {
        pipeline(&mutate(SEEDS[seed], &ops));
    }

    #[test]
    fn date_parse_arbitrary_text(s in "\\PC{0,60}") {
        exercise_date(&s);
    }

    #[test]
    fn date_parse_token_soup(s in date_text()) {
        exercise_date(&s);
    }
}

// Deterministic edge cases: each must import without panicking and yield the expected tree.

fn utf16(text: &str, big_endian: bool) -> Vec<u8> {
    let mut out = if big_endian {
        vec![0xFE, 0xFF]
    } else {
        vec![0xFF, 0xFE]
    };
    for u in text.encode_utf16() {
        out.extend_from_slice(&if big_endian {
            u.to_be_bytes()
        } else {
            u.to_le_bytes()
        });
    }
    out
}

const SMALL: &str = "0 HEAD
1 GEDC
2 VERS 5.5.1
0 @I1@ INDI
1 NAME Anna /Müller 王/
0 TRLR
";

fn only_name(out: &gt_gedcom::ImportOutcome) -> &gt_core::model::Name {
    assert_eq!(out.tree.persons.len(), 1);
    let p = out.tree.persons.values().next().unwrap();
    assert_eq!(p.names.len(), 1);
    &p.names[0]
}

fn assert_small_decodes(bytes: &[u8]) {
    let out = import(bytes, &ImportOptions::default());
    let n = only_name(&out);
    assert_eq!(n.given.as_deref(), Some("Anna"));
    assert_eq!(n.surnames, vec!["Müller 王".to_string()]);
    pipeline(bytes);
}

#[test]
fn utf16le_with_bom_decodes_names() {
    assert_small_decodes(&utf16(SMALL, false));
}

#[test]
fn utf16be_with_bom_decodes_names() {
    assert_small_decodes(&utf16(SMALL, true));
}

#[test]
fn utf16_odd_length_does_not_panic() {
    let mut bytes = utf16(SMALL, false);
    bytes.push(0x41);
    let out = import(&bytes, &ImportOptions::default());
    assert_eq!(out.tree.persons.len(), 1);
    pipeline(&bytes);
}

#[test]
fn deep_level_chain_is_imported() {
    let mut s = String::from(
        "0 HEAD
1 GEDC
2 VERS 5.5.1
0 @I1@ INDI
1 NAME Anna /Mueller/
",
    );
    for level in 2..200 {
        s.push_str(&format!(
            "{level} _DEEP n{level}
"
        ));
    }
    s.push_str(
        "0 TRLR
",
    );
    let out = import(s.as_bytes(), &ImportOptions::default());
    assert_eq!(only_name(&out).given.as_deref(), Some("Anna"));
    pipeline(s.as_bytes());

    // Levels 0..199 in one chain: only one level-0 record, no person, no panic.
    let mut chain = String::new();
    for level in 0..200 {
        chain.push_str(&format!(
            "{level} TAG x
"
        ));
    }
    let out = import(chain.as_bytes(), &ImportOptions::default());
    assert!(out.tree.persons.is_empty());
    let tokens = tokenize(chain.as_bytes());
    assert_eq!(tokens.records.len(), 1);
    pipeline(chain.as_bytes());
}

#[test]
fn hundred_thousand_lines_import_all_persons() {
    let mut s = String::from(
        "0 HEAD
1 GEDC
2 VERS 5.5.1
",
    );
    for i in 0..100_000 {
        s.push_str(&format!(
            "0 @I{i}@ INDI
"
        ));
    }
    s.push_str(
        "0 TRLR
",
    );
    let out = import(s.as_bytes(), &ImportOptions::default());
    assert_eq!(out.tree.persons.len(), 100_000);
    pipeline(s.as_bytes());
}

#[test]
fn single_one_megabyte_line_is_kept_whole() {
    let mut s = String::from(
        "0 HEAD
1 GEDC
2 VERS 5.5.1
0 @I1@ INDI
1 NOTE ",
    );
    s.push_str(&"x".repeat(1024 * 1024));
    s.push_str(
        "
0 TRLR
",
    );
    let out = import(s.as_bytes(), &ImportOptions::default());
    assert_eq!(out.tree.persons.len(), 1);
    assert_eq!(out.tree.notes.len(), 1);
    assert_eq!(
        out.tree.notes.values().next().unwrap().text.len(),
        1024 * 1024
    );
    pipeline(s.as_bytes());

    let mut t = String::from("0 ");
    t.push_str(&"A".repeat(1024 * 1024));
    let out = import(t.as_bytes(), &ImportOptions::default());
    assert!(out.tree.persons.is_empty());
    pipeline(t.as_bytes());
}
