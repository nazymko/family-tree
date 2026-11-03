#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use gt_core::date::calendar::{Calendar, first_jdn};
use gt_gedcom::{ImportOptions, import};
use gt_session::*;
use gt_store::Project;
use proptest::prelude::*;
use tempfile::TempDir;

const NOW_MS: i64 = 1_793_664_000_000; // 2026-11-03

fn today() -> i32 {
    first_jdn(Calendar::Gregorian, 2026, Some(11), Some(3)).unwrap()
}

fn open_ged(ged: &str) -> (TempDir, Session) {
    let out = import(ged.as_bytes(), &ImportOptions::default());
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("project");
    let mut p = Project::create(&proj).unwrap();
    p.save_tree(&out.tree, today()).unwrap();
    drop(p);
    let s = Session::open(&proj, SessionClock::fixed(today(), NOW_MS)).unwrap();
    (dir, s)
}

fn ged(records: &str) -> String {
    format!("0 HEAD\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n{records}0 TRLR\n")
}

fn birth_at(n: u32, place: &str) -> String {
    format!("0 @I{n}@ INDI\n1 NAME P{n} /X/\n1 BIRT\n2 DATE 1900\n2 PLAC {place}\n")
}

#[test]
fn preview_about_has_range() {
    let p = parse_date_preview("abt 1850", DayOrder::Dmy);
    assert_eq!(p.understood_as, "about 1850");
    assert_eq!(p.range.as_deref(), Some("1845 to 1855"));
    assert_eq!(p.status, DateStatus::Ok);
    assert!(p.warnings.is_empty());
    assert_eq!(p.raw, "abt 1850");
}

#[test]
fn preview_ranges_and_exact() {
    let exact = parse_date_preview("3 Apr 1850", DayOrder::Dmy);
    assert_eq!(exact.range, None);
    let year = parse_date_preview("1900", DayOrder::Dmy);
    assert_eq!(year.range.as_deref(), Some("1 Jan 1900 to 31 Dec 1900"));
    let bet = parse_date_preview("bet 1850 and 1860", DayOrder::Dmy);
    assert_eq!(bet.range.as_deref(), Some("1850 to 1860"));
    assert_eq!(parse_date_preview("bef 1850", DayOrder::Dmy).range, None);
}

#[test]
fn preview_dmy_mdy_ambiguity_warns() {
    let d = parse_date_preview("03/04/1900", DayOrder::Dmy);
    assert_eq!(d.understood_as, "3 Apr 1900");
    assert_eq!(d.warnings, ["Day and month are ambiguous; read as DMY"]);
    let m = parse_date_preview("03/04/1900", DayOrder::Mdy);
    assert_eq!(m.understood_as, "4 Mar 1900");
    assert_eq!(m.warnings, ["Day and month are ambiguous; read as MDY"]);
    assert!(
        parse_date_preview("25/04/1900", DayOrder::Dmy)
            .warnings
            .is_empty()
    );
}

#[test]
fn preview_unparsed_status_and_raw() {
    let p = parse_date_preview("sometime soon", DayOrder::Dmy);
    assert_eq!(p.status, DateStatus::Unparsed);
    assert_eq!(p.raw, "sometime soon");
    assert_eq!(p.understood_as, "sometime soon");
    assert_eq!(p.range, None);
    assert_eq!(
        p.warnings,
        ["Not understood as a date. It will be saved exactly as you typed it."]
    );
}

#[test]
fn preview_phrase() {
    let p = parse_date_preview("(in the spring)", DayOrder::Dmy);
    assert_eq!(p.status, DateStatus::Phrase);
    assert_eq!(p.range, None);
    assert!(p.warnings.is_empty());
}

#[test]
fn preview_bce() {
    let p = parse_date_preview("44 BCE", DayOrder::Dmy);
    assert_eq!(p.status, DateStatus::Ok);
    assert_eq!(p.understood_as, "44 BCE");
    assert_eq!(p.range.as_deref(), Some("1 Jan 44 BCE to 31 Dec 44 BCE"));
    let a = parse_date_preview("abt 44 BCE", DayOrder::Dmy);
    assert_eq!(a.range.as_deref(), Some("49 BCE to 39 BCE"));
}

#[test]
fn preview_dual_year_warns() {
    let p = parse_date_preview("1750/51", DayOrder::Dmy);
    assert_eq!(p.status, DateStatus::Ok);
    assert!(p.warnings.iter().any(|w| w.starts_with("Double year")));
}

proptest! {
    #[test]
    fn preview_never_panics(s in any::<String>()) {
        let p = parse_date_preview(&s, DayOrder::Dmy);
        prop_assert_eq!(p.raw, s);
    }
}

#[test]
fn places_prefix_matches_leaf_and_chain() {
    let (_d, s) = open_ged(&ged(&format!(
        "{}{}{}",
        birth_at(1, "Hamburg, Germany"),
        birth_at(2, "Germantown, Pennsylvania, USA"),
        birth_at(3, "Berlin, Germany"),
    )));
    let ham = s.suggest_places("ham", 10);
    assert_eq!(ham.len(), 1);
    assert_eq!(ham[0].full, "Hamburg, Germany");
    assert_eq!(ham[0].name, "Hamburg");
    let ger: Vec<String> = s
        .suggest_places("ger", 20)
        .into_iter()
        .map(|p| p.full)
        .collect();
    // Leaf matches (Germany itself, Germantown) first, then places under Germany.
    assert!(ger[..2].contains(&"Germany".to_string()));
    assert!(ger[..2].contains(&"Germantown, Pennsylvania, USA".to_string()));
    assert!(ger[2..].contains(&"Hamburg, Germany".to_string()));
    assert!(ger[2..].contains(&"Berlin, Germany".to_string()));
    // Case does not matter; a full chain prefix works.
    assert_eq!(s.suggest_places("HAMBURG, ger", 5).len(), 1);
}

#[test]
fn places_use_count_ranking() {
    let (_d, s) = open_ged(&ged(&format!(
        "{}{}{}{}",
        birth_at(1, "Alpha, Land"),
        birth_at(2, "Alpine, Land"),
        birth_at(3, "Alpine, Land"),
        birth_at(4, "Alpen, Land"),
    )));
    let names: Vec<String> = s
        .suggest_places("alp", 20)
        .into_iter()
        .map(|p| p.name)
        .collect();
    // Alpine has 2 uses; Alpen and Alpha 1 each (ICU order breaks the tie).
    assert_eq!(names, ["Alpine", "Alpen", "Alpha"]);
}

#[test]
fn places_limit_clamped_20() {
    let recs: String = (0..30)
        .map(|i| birth_at(i, &format!("Town{i:02}")))
        .collect();
    let (_d, s) = open_ged(&ged(&recs));
    assert_eq!(s.suggest_places("town", 500).len(), 20);
    assert_eq!(s.suggest_places("town", 5).len(), 5);
    assert_eq!(s.suggest_places("town", 0).len(), 0);
    assert_eq!(s.suggest_places("", 20).len(), 20);
}

#[test]
fn sources_prefix_and_counts() {
    let recs = "0 @S1@ SOUR\n1 TITL Hamburg parish register\n\
                0 @S2@ SOUR\n1 TITL Census 1871\n1 AUTH Prussian office\n\
                0 @S3@ SOUR\n1 TITL Parish of Altona\n\
                0 @I1@ INDI\n1 NAME A /B/\n1 BIRT\n2 DATE 1900\n2 SOUR @S3@\n3 PAGE 1\n\
                1 DEAT\n2 DATE 1950\n2 SOUR @S3@\n3 PAGE 2\n";
    let (_d, s) = open_ged(&ged(recs));
    let by_par = s.suggest_sources("par", 10);
    let titles: Vec<_> = by_par.iter().map(|x| x.title.clone().unwrap()).collect();
    // Title prefix first, then a later word of a title.
    assert_eq!(titles, ["Parish of Altona", "Hamburg parish register"]);
    assert_eq!(by_par[0].citation_count, 2);
    assert_eq!(by_par[1].citation_count, 0);
    assert_eq!(s.suggest_sources("prus", 10).len(), 1);
    assert_eq!(s.suggest_sources("", 10).len(), 3);
    assert_eq!(s.suggest_sources("", 1).len(), 1);
    assert!(s.suggest_sources("zzz", 10).is_empty());
}

#[test]
fn last_citation_none_then_some() {
    let (_d, none) = open_ged(&ged("0 @I1@ INDI\n1 NAME A /B/\n"));
    assert!(none.last_citation().is_none());
    let recs = "0 @S1@ SOUR\n1 TITL Reg\n\
                0 @I1@ INDI\n1 NAME A /B/\n1 BIRT\n2 DATE 1900\n2 SOUR @S1@\n3 PAGE first\n\
                1 DEAT\n2 DATE 1950\n2 SOUR @S1@\n3 PAGE second\n";
    let (_d2, some) = open_ged(&ged(recs));
    let c = some.last_citation().unwrap();
    assert_eq!(c.page.as_deref(), Some("second"));
    assert_eq!(c.source.unwrap().title.as_deref(), Some("Reg"));
}

#[cfg(not(debug_assertions))]
#[test]
fn parse_date_under_5ms() {
    let t = std::time::Instant::now();
    for _ in 0..100 {
        let _ = parse_date_preview("bet 3 Apr 1850 and 12/05/1860", DayOrder::Dmy);
    }
    assert!(t.elapsed() < std::time::Duration::from_millis(500));
}
