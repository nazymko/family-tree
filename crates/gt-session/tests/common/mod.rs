#![allow(dead_code, clippy::unwrap_used)]

use gt_core::date::calendar::{Calendar, first_jdn};
use gt_core::tree::Tree;
use gt_gedcom::{ImportOptions, import};
use gt_session::*;
use gt_store::Project;
use tempfile::TempDir;
use uuid::Uuid;

pub const NOW_MS: i64 = 1_793_664_000_000;

pub fn today() -> i32 {
    first_jdn(Calendar::Gregorian, 2026, Some(11), Some(3)).unwrap()
}

const HEAD: &str = "0 HEAD\n1 SOUR TEST\n2 VERS 1.0\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n";

pub fn tree_of(records: &str) -> Tree {
    let ged = format!("{HEAD}{records}0 TRLR\n");
    import(ged.as_bytes(), &ImportOptions::default()).tree
}

pub fn open_tree(tree: &Tree) -> (TempDir, Session) {
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("project");
    let mut p = Project::create(&proj).unwrap();
    p.save_tree(tree, today()).unwrap();
    drop(p);
    let s = Session::open(&proj, SessionClock::fixed(today(), NOW_MS)).unwrap();
    (dir, s)
}

/// Anna (1), Karl (2), married with child Hans (3); Eva (4) has no relatives.
pub const SAMPLE: &str = "0 @I1@ INDI\n1 NAME Anna /Schmidt/\n1 SEX F\n1 BIRT\n2 DATE 1850\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Karl /Schmidt/\n1 SEX M\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Hans /Schmidt/\n1 SEX M\n1 FAMC @F1@\n\
0 @I4@ INDI\n1 NAME Eva /Meyer/\n1 SEX F\n\
0 @F1@ FAM\n1 HUSB @I2@\n1 WIFE @I1@\n1 CHIL @I3@\n";

pub fn find(tree: &Tree, given: &str) -> Uuid {
    tree.persons
        .values()
        .find(|p| p.names.iter().any(|n| n.given.as_deref() == Some(given)))
        .map(|p| p.common.id)
        .unwrap()
}

pub fn draft(given: &str, sex: SexKind) -> PersonDraft {
    PersonDraft {
        given: Some(given.into()),
        surnames: vec!["Schmidt".into()],
        sex,
        birth_date_raw: None,
        birth_place_text: None,
    }
}
