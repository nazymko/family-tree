#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use gt_core::date::calendar::{Calendar, first_jdn};
use gt_gedcom::{ImportOptions, import};
use gt_session::KinshipKind::{Blood, Partner, PartnerOfRelative, RelativeOfPartner, SelfPerson};
use gt_session::SexKind::{Female, Male, Unknown};
use gt_session::*;
use gt_store::Project;
use std::fmt::Write as _;
use tempfile::TempDir;
use uuid::Uuid;

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
    let s = Session::open(&proj, SessionClock::fixed(today(), 1_793_664_000_000)).unwrap();
    (dir, s)
}

fn indi(n: u32, name: &str, sex: &str) -> String {
    format!("0 @I{n}@ INDI\n1 NAME {name}\n1 SEX {sex}\n")
}

fn fam(n: u32, husb: u32, wife: u32, married: bool, kids: &[u32]) -> String {
    let mut s = format!("0 @F{n}@ FAM\n1 HUSB @I{husb}@\n1 WIFE @I{wife}@\n");
    if married {
        s.push_str("1 MARR\n");
    }
    for k in kids {
        writeln!(s, "1 CHIL @I{k}@").unwrap();
    }
    s
}

/// Invented family. Gil+Gwen -> Gus, Gabe. Gus+Gina -> Dan, Ada, Uri. Dan+Mia -> Max, Sia; Dan+Sue -> Hal.
/// Abe+Ada -> Cora -> Cleo. Max+Wendy -> Kay; Wendy has parents Wes+Wanda and a brother Will.
/// Hugo+Sia -> Nell. Uri+Ulla. Max+Cora (cousins) -> Cass. Zed is alone.
const NAMES: [(&str, &str); 26] = [
    ("Gil Test", "M"),
    ("Gwen Test", "F"),
    ("Gus Test", "M"),
    ("Gabe Test", "M"),
    ("Gina Other", "F"),
    ("Dan Test", "M"),
    ("Ada Test", "F"),
    ("Uri Test", "M"),
    ("Mia Other", "F"),
    ("Max Test", "M"),
    ("Sia Test", "F"),
    ("Sue Other", "F"),
    ("Hal Test", "M"),
    ("Abe Other", "M"),
    ("Cora Other", "F"),
    ("Cleo Other", "F"),
    ("Wendy Other", "F"),
    ("Kay Test", "F"),
    ("Wes Other", "M"),
    ("Wanda Other", "F"),
    ("Will Other", "M"),
    ("Hugo Other", "M"),
    ("Nell Other", "F"),
    ("Ulla Other", "F"),
    ("Cass Test", "F"),
    ("Zed Alone", "M"),
];

fn setup() -> (TempDir, Session) {
    let n = |name: &str| NAMES.iter().position(|(x, _)| *x == name).unwrap() as u32 + 1;
    let mut recs: Vec<String> = NAMES
        .iter()
        .enumerate()
        .map(|(i, (name, sex))| indi(i as u32 + 1, name, sex))
        .collect();
    let f = |i, h: &str, w: &str, m, kids: &[&str]| {
        let k: Vec<u32> = kids.iter().map(|x| n(x)).collect();
        fam(i, n(h), n(w), m, &k)
    };
    recs.extend([
        f(1, "Gil Test", "Gwen Test", true, &["Gus Test", "Gabe Test"]),
        f(
            2,
            "Gus Test",
            "Gina Other",
            true,
            &["Dan Test", "Ada Test", "Uri Test"],
        ),
        f(3, "Dan Test", "Mia Other", true, &["Max Test", "Sia Test"]),
        f(4, "Dan Test", "Sue Other", false, &["Hal Test"]),
        f(5, "Abe Other", "Ada Test", true, &["Cora Other"]),
        f(
            6,
            "Wes Other",
            "Wanda Other",
            true,
            &["Wendy Other", "Will Other"],
        ),
        f(7, "Max Test", "Wendy Other", true, &["Kay Test"]),
        f(8, "Hugo Other", "Sia Test", true, &["Nell Other"]),
        f(9, "Uri Test", "Ulla Other", true, &[]),
        f(10, "Max Test", "Cora Other", false, &["Cass Test"]),
        f(11, "Abe Other", "Cora Other", true, &["Cleo Other"]),
    ]);
    let ged = format!(
        "0 HEAD\n1 SOUR TEST\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n{}0 TRLR\n",
        recs.concat()
    );
    open_ged(&ged)
}

fn id_of(s: &Session, name: &str) -> Uuid {
    let q = PersonQuery {
        text: name.to_string(),
        ..PersonQuery::default()
    };
    s.list_persons(&q, 0, 200)
        .items
        .iter()
        .find(|r| r.person.name == name)
        .unwrap_or_else(|| panic!("no person {name}"))
        .person
        .id
}

type Got = Option<(KinshipKind, u8, u8, bool, bool, SexKind)>;

fn rel(s: &Session, from: &str, to: &str) -> Got {
    s.kinship_between(id_of(s, from), id_of(s, to))
        .map(|k| (k.kind, k.up, k.down, k.half, k.married, k.via))
}

fn blood(up: u8, down: u8, half: bool) -> Got {
    Some((Blood, up, down, half, false, Unknown))
}

#[test]
fn self_and_unrelated() {
    let (_d, s) = setup();
    assert_eq!(rel(&s, "Max Test", "Max Test").unwrap().0, SelfPerson);
    assert_eq!(rel(&s, "Max Test", "Zed Alone"), None);
    assert_eq!(rel(&s, "Zed Alone", "Max Test"), None);
}

#[test]
fn direct_line() {
    let (_d, s) = setup();
    assert_eq!(rel(&s, "Max Test", "Dan Test"), blood(1, 0, false));
    assert_eq!(rel(&s, "Max Test", "Gus Test"), blood(2, 0, false));
    assert_eq!(rel(&s, "Max Test", "Gil Test"), blood(3, 0, false));
    assert_eq!(rel(&s, "Gus Test", "Max Test"), blood(0, 2, false));
    assert_eq!(rel(&s, "Dan Test", "Kay Test"), blood(0, 2, false));
    assert_eq!(rel(&s, "Kay Test", "Gil Test"), blood(4, 0, false));
}

#[test]
fn siblings_and_halves() {
    let (_d, s) = setup();
    assert_eq!(rel(&s, "Max Test", "Sia Test"), blood(1, 1, false));
    assert_eq!(rel(&s, "Max Test", "Hal Test"), blood(1, 1, true));
    assert_eq!(rel(&s, "Hal Test", "Sia Test"), blood(1, 1, true));
}

#[test]
fn aunts_uncles_nieces_cousins() {
    let (_d, s) = setup();
    assert_eq!(rel(&s, "Max Test", "Uri Test"), blood(2, 1, false));
    assert_eq!(rel(&s, "Max Test", "Ada Test"), blood(2, 1, false));
    assert_eq!(rel(&s, "Max Test", "Gabe Test"), blood(3, 1, false));
    assert_eq!(rel(&s, "Max Test", "Nell Other"), blood(1, 2, false));
    assert_eq!(rel(&s, "Max Test", "Cleo Other"), blood(2, 3, false));
    assert_eq!(rel(&s, "Kay Test", "Will Other"), blood(2, 1, false));
    // Max and Cora are cousins who also have a child: the closer link, the partner, wins.
    assert_eq!(rel(&s, "Max Test", "Cora Other").unwrap().0, Partner);
    assert_eq!(rel(&s, "Sia Test", "Cora Other"), blood(2, 2, false));
}

#[test]
fn partners() {
    let (_d, s) = setup();
    let w = rel(&s, "Max Test", "Wendy Other").unwrap();
    assert_eq!((w.0, w.4), (Partner, true));
    // No marriage record (only an explicitly unmarried couple is not): still husband and wife.
    let p = rel(&s, "Dan Test", "Sue Other").unwrap();
    assert_eq!((p.0, p.4), (Partner, true));
}

#[test]
fn in_laws_through_a_partner() {
    let (_d, s) = setup();
    // Wife's parents and brother.
    assert_eq!(
        rel(&s, "Max Test", "Wanda Other"),
        Some((RelativeOfPartner, 1, 0, false, true, Female))
    );
    assert_eq!(
        rel(&s, "Max Test", "Will Other"),
        Some((RelativeOfPartner, 1, 1, false, true, Female))
    );
    // Husband's side, seen from Wendy.
    assert_eq!(
        rel(&s, "Wendy Other", "Dan Test"),
        Some((RelativeOfPartner, 1, 0, false, true, Male))
    );
    assert_eq!(
        rel(&s, "Wendy Other", "Sia Test"),
        Some((RelativeOfPartner, 1, 1, false, true, Male))
    );
}

#[test]
fn partner_of_a_relative() {
    let (_d, s) = setup();
    assert_eq!(
        rel(&s, "Max Test", "Hugo Other"),
        Some((PartnerOfRelative, 1, 1, false, true, Female))
    );
    assert_eq!(
        rel(&s, "Max Test", "Ulla Other"),
        Some((PartnerOfRelative, 2, 1, false, true, Male))
    );
    // The father's wife (no marriage record counts as married): the partner of a parent who is not the mother.
    assert_eq!(
        rel(&s, "Max Test", "Sue Other"),
        Some((PartnerOfRelative, 1, 0, false, true, Male))
    );
    // A daughter-in-law.
    assert_eq!(
        rel(&s, "Dan Test", "Wendy Other"),
        Some((PartnerOfRelative, 0, 1, false, true, Male))
    );
}

#[test]
fn pedigree_collapse_terminates_with_the_nearest_relation() {
    let (_d, s) = setup();
    // Cass's parents are cousins: Gus is three steps up by both lines.
    assert_eq!(rel(&s, "Cass Test", "Gus Test"), blood(3, 0, false));
    assert_eq!(rel(&s, "Cass Test", "Max Test"), blood(1, 0, false));
    assert_eq!(rel(&s, "Cass Test", "Cora Other"), blood(1, 0, false));
    // Kay and Cass share Max only, through different families: half sisters.
    assert_eq!(rel(&s, "Cass Test", "Kay Test"), blood(1, 1, true));
}

#[test]
fn chart_windows_carry_kinship_for_the_reference_person() {
    let (_d, mut s) = setup();
    let me = id_of(&s, "Max Test");
    let m = s
        .chart_open(&ChartRequest {
            kind: ChartKind::Pedigree,
            root: me,
            generations: 3,
            aunts_uncles: None,
            kin: None,
            roots: None,
        })
        .unwrap();
    let all = Rect {
        x: -1e6,
        y: -1e6,
        w: 2e6,
        h: 2e6,
    };
    let w = s.chart_window(m.chart_id, &all, 1.0).unwrap();
    let by = |name: &str| {
        w.nodes
            .iter()
            .find(|n| n.person.as_ref().is_some_and(|p| p.name == name))
            .unwrap()
    };
    assert!(by("Max Test").kinship.is_none());
    let dan = by("Dan Test").kinship.unwrap();
    assert_eq!((dan.kind, dan.up, dan.down, dan.sex), (Blood, 1, 0, Male));
    // Far out the boxes carry no text and no kinship.
    let far = s.chart_window(m.chart_id, &all, 0.1).unwrap();
    assert!(far.nodes.iter().all(|n| n.kinship.is_none()));
    // The whole-tree chart has no reference until one is named.
    let wm = s.chart_open_whole().unwrap();
    let ww = s.chart_window(wm.meta.chart_id, &all, 1.0).unwrap();
    assert!(ww.nodes.iter().all(|n| n.kinship.is_none()));
    s.chart_set_reference(wm.meta.chart_id, Some(me)).unwrap();
    let ww = s.chart_window(wm.meta.chart_id, &all, 1.0).unwrap();
    let hal = ww
        .nodes
        .iter()
        .find(|n| n.person.as_ref().is_some_and(|p| p.name == "Hal Test"))
        .unwrap();
    assert!(hal.kinship.unwrap().half);
    s.chart_set_reference(wm.meta.chart_id, None).unwrap();
    let ww = s.chart_window(wm.meta.chart_id, &all, 1.0).unwrap();
    assert!(ww.nodes.iter().all(|n| n.kinship.is_none()));
}
