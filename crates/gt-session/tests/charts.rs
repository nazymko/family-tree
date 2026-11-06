#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use gt_core::date::calendar::{Calendar, first_jdn};
use gt_gedcom::{ImportOptions, import};
use gt_session::chart::ahnen_label;
use gt_session::*;
use gt_store::Project;
use std::fmt::Write as _;
use tempfile::TempDir;
use uuid::Uuid;

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

const HEAD: &str = "0 HEAD\n1 SOUR TEST\n2 VERS 1.0\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n";

fn indi(n: u32, name: &str, sex: &str, extra: &str) -> String {
    format!("0 @I{n}@ INDI\n1 NAME {name}\n1 SEX {sex}\n{extra}")
}

fn fam(n: u32, husb: u32, wife: u32, kids: &[u32]) -> String {
    let mut s = format!("0 @F{n}@ FAM\n1 HUSB @I{husb}@\n1 WIFE @I{wife}@\n");
    for k in kids {
        writeln!(s, "1 CHIL @I{k}@").unwrap();
    }
    s
}

fn ged(records: &[String]) -> String {
    format!("{HEAD}{}0 TRLR\n", records.concat())
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

/// Me (1), Father Fred (2), Mother Mary (3), Fred's parents Gus (4) and Gina (5), Mary's parents
/// Max (6) and Mia (7).
fn pedigree_ged() -> String {
    ged(&[
        indi(1, "Me /X/", "M", ""),
        indi(2, "Fred /X/", "M", ""),
        indi(3, "Mary /Y/", "F", ""),
        indi(4, "Gus /X/", "M", ""),
        indi(5, "Gina /X/", "F", ""),
        indi(6, "Max /Y/", "M", ""),
        indi(7, "Mia /Y/", "F", ""),
        fam(1, 2, 3, &[1]),
        fam(2, 4, 5, &[2]),
        fam(3, 6, 7, &[3]),
    ])
}

fn pedigree_req(s: &Session, name: &str, generations: u32) -> ChartRequest {
    ChartRequest {
        kind: ChartKind::Pedigree,
        root: id_of(s, name),
        generations,
        aunts_uncles: None,
        siblings: None,
    }
}

fn descendants_req(s: &Session, name: &str, generations: u32) -> ChartRequest {
    ChartRequest {
        kind: ChartKind::Descendants,
        root: id_of(s, name),
        generations,
        aunts_uncles: None,
        siblings: None,
    }
}

fn everything() -> Rect {
    Rect {
        x: -1e6,
        y: -1e6,
        w: 2e6,
        h: 2e6,
    }
}

#[test]
fn open_pedigree_meta() {
    let (_d, mut s) = open_ged(&pedigree_ged());
    let m = s.chart_open(&pedigree_req(&s, "Me X", 3)).unwrap();
    assert_eq!(m.node_count, 7);
    assert_eq!(m.root_node, 0);
    assert!(!m.truncated);
    assert_eq!(m.kind, ChartKind::Pedigree);
    assert_eq!(m.version, s.revision());
    assert!(m.bounds.w > 0.0 && m.bounds.h > 0.0);
    let v = serde_json::to_value(&m).unwrap();
    assert!(v["chartId"].is_u64() && v["rootNode"] == 0 && v["nodeCount"] == 7);
}

#[test]
fn window_returns_only_intersecting_nodes_and_their_edges() {
    let (_d, mut s) = open_ged(&pedigree_ged());
    let m = s.chart_open(&pedigree_req(&s, "Me X", 3)).unwrap();
    let all = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    assert_eq!(all.nodes.len(), 7);
    assert_eq!(all.edges.len(), 6);
    // The first column only (x 0..176), plus the gap where the root's two edges start.
    let r = Rect {
        x: 0.0,
        y: -1000.0,
        w: 200.0,
        h: 5000.0,
    };
    let w = s.chart_window(m.chart_id, &r, 1.0).unwrap();
    assert_eq!(w.nodes.len(), 1);
    assert_eq!(w.nodes[0].id, 0);
    assert_eq!(w.nodes[0].ahnen, Some(1));
    assert_eq!(w.nodes[0].person.as_ref().unwrap().name, "Me X");
    assert_eq!(w.edges.len(), 2);
    assert!(w.edges.iter().all(|e| e.from == 0));
    let far = Rect {
        x: 1e5,
        y: 1e5,
        w: 10.0,
        h: 10.0,
    };
    let none = s.chart_window(m.chart_id, &far, 1.0).unwrap();
    assert!(none.nodes.is_empty() && none.edges.is_empty());
}

#[test]
fn window_wire_shape_matches_types_ts() {
    let (_d, mut s) = open_ged(&pedigree_ged());
    let m = s.chart_open(&pedigree_req(&s, "Me X", 3)).unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    let v = serde_json::to_value(&w).unwrap();
    let n = &v["nodes"][0];
    for key in [
        "id", "person", "x", "y", "w", "h", "gen", "ahnen", "dupOf", "moreUp", "moreDown", "nav",
        "label",
    ] {
        assert!(n.get(key).is_some(), "missing {key}");
    }
    assert!(n["label"].as_str().unwrap().contains("Me X"));
    assert_eq!(v["edges"][0]["kind"], "parent");
    assert!(v["edges"][0]["pts"].is_array());
}

#[test]
fn low_zoom_strips_text() {
    let (_d, mut s) = open_ged(&pedigree_ged());
    let m = s.chart_open(&pedigree_req(&s, "Me X", 3)).unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 0.2).unwrap();
    assert_eq!(w.nodes.len(), 7);
    assert!(
        w.nodes
            .iter()
            .all(|n| n.person.is_none() && n.label.is_empty())
    );
    let w = s.chart_window(m.chart_id, &everything(), 0.25).unwrap();
    assert!(
        w.nodes
            .iter()
            .all(|n| n.person.is_some() && !n.label.is_empty())
    );
}

#[test]
fn stale_after_edit() {
    let (_d, mut s) = open_ged(&pedigree_ged());
    let m = s.chart_open(&pedigree_req(&s, "Me X", 3)).unwrap();
    s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    // Any change that rebuilds the session data (an edit does the same) invalidates the chart.
    s.set_settings(SettingsPatch {
        living_max_age: Some(90),
        ..SettingsPatch::default()
    })
    .unwrap();
    let e = s.chart_window(m.chart_id, &everything(), 1.0).unwrap_err();
    assert_eq!(e.code, ErrorCode::Stale);
    let e = s.chart_outline(m.chart_id, None, 10).unwrap_err();
    assert_eq!(e.code, ErrorCode::Stale);
    // Reopening works and records the new revision.
    let m2 = s.chart_open(&pedigree_req(&s, "Me X", 3)).unwrap();
    assert_eq!(m2.version, s.revision());
    assert!(s.chart_window(m2.chart_id, &everything(), 1.0).is_ok());
}

#[test]
fn lru_keeps_two_charts() {
    let (_d, mut s) = open_ged(&pedigree_ged());
    let a = s.chart_open(&pedigree_req(&s, "Me X", 3)).unwrap().chart_id;
    let b = s.chart_open(&pedigree_req(&s, "Me X", 2)).unwrap().chart_id;
    let c = s.chart_open(&pedigree_req(&s, "Me X", 1)).unwrap().chart_id;
    assert_eq!(
        s.chart_window(a, &everything(), 1.0).unwrap_err().code,
        ErrorCode::NotFound
    );
    assert!(s.chart_window(b, &everything(), 1.0).is_ok());
    assert!(s.chart_window(c, &everything(), 1.0).is_ok());
    // Using a chart keeps it: b is touched, so the next open evicts c.
    s.chart_window(b, &everything(), 1.0).unwrap();
    let d = s.chart_open(&pedigree_req(&s, "Me X", 3)).unwrap().chart_id;
    assert!(s.chart_window(b, &everything(), 1.0).is_ok());
    assert_eq!(
        s.chart_window(c, &everything(), 1.0).unwrap_err().code,
        ErrorCode::NotFound
    );
    s.chart_close(d);
    assert_eq!(
        s.chart_window(d, &everything(), 1.0).unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[test]
fn open_unknown_person_is_not_found() {
    let (_d, mut s) = open_ged(&pedigree_ged());
    let req = ChartRequest {
        kind: ChartKind::Pedigree,
        root: Uuid::nil(),
        generations: 3,
        aunts_uncles: None,
        siblings: None,
    };
    assert_eq!(s.chart_open(&req).unwrap_err().code, ErrorCode::NotFound);
}

/// Anna is the father's parent and also the child of her own child (a loop in the data).
fn cycle_ged() -> String {
    ged(&[
        indi(1, "Anna /C/", "F", ""),
        indi(2, "Ben /C/", "M", ""),
        indi(3, "Cora /C/", "F", ""),
        fam(1, 2, 3, &[1]), // Anna's parents: Ben, Cora
        fam(2, 1, 3, &[2]), // Ben's parents: Anna (as husb), Cora
    ])
}

#[test]
fn cycle_chart_terminates_with_dup() {
    let (_d, mut s) = open_ged(&cycle_ged());
    let m = s.chart_open(&pedigree_req(&s, "Anna C", 12)).unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    assert!(w.nodes.iter().any(|n| n.dup_of.is_some()));
    assert!(w.edges.iter().any(|e| e.kind == EdgeKindDto::Dup));
    let d = s.chart_open(&descendants_req(&s, "Anna C", 8)).unwrap();
    assert!(d.node_count < 20);
    let w = s.chart_window(d.chart_id, &everything(), 1.0).unwrap();
    assert!(w.nodes.iter().any(|n| n.dup_of.is_some()));
    // Dup items say so in the outline.
    let dup = w
        .nodes
        .iter()
        .find(|n| n.dup_of.is_some() && n.nav.parent.is_some())
        .unwrap();
    let parent = dup.nav.parent.unwrap();
    let items = s.chart_outline(d.chart_id, Some(parent), 100).unwrap();
    let item = items.iter().find(|i| i.node_id == dup.id).unwrap();
    assert!(item.label.ends_with("(appears earlier)"), "{}", item.label);
}

/// A father with `n` children by one wife.
fn wide_ged(n: u32) -> String {
    let mut s = String::from(HEAD);
    s.push_str(&indi(1, "Dad /W/", "M", ""));
    s.push_str(&indi(2, "Mum /W/", "F", ""));
    for i in 0..n {
        s.push_str(&indi(10 + i, &format!("Kid{i} /W/"), "U", ""));
    }
    s.push_str("0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n");
    for i in 0..n {
        writeln!(s, "1 CHIL @I{}@", 10 + i).unwrap();
    }
    s.push_str("0 TRLR\n");
    s
}

#[test]
#[ignore = "heavy: runs before merge"]
fn heavy_truncation_at_20000() {
    let (_d, mut s) = open_ged(&wide_ged(30_000));
    let m = s.chart_open(&descendants_req(&s, "Dad W", 3)).unwrap();
    assert!(m.truncated);
    assert_eq!(m.node_count, 20_000);
    let m = s.chart_open(&descendants_req(&s, "Dad W", 3)).unwrap();
    let huge = Rect {
        x: -1e9,
        y: -1e9,
        w: 2e9,
        h: 2e9,
    };
    let w = s.chart_window(m.chart_id, &huge, 0.1).unwrap();
    assert_eq!(w.nodes.len(), 20_000);
}

#[test]
fn outline_labels_ahnentafel() {
    let (_d, mut s) = open_ged(&pedigree_ged());
    let m = s.chart_open(&pedigree_req(&s, "Me X", 3)).unwrap();
    let top = s.chart_outline(m.chart_id, None, 10).unwrap();
    assert_eq!(top.len(), 1);
    assert_eq!(top[0].label, "1 Self");
    assert_eq!(top[0].node_id, m.root_node);
    assert!(top[0].has_children);
    assert_eq!(top[0].person.as_ref().unwrap().name, "Me X");

    let l1 = s.chart_outline(m.chart_id, Some(m.root_node), 10).unwrap();
    let labels: Vec<_> = l1.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, ["2 Father", "3 Mother"]);
    assert_eq!(l1[0].person.as_ref().unwrap().name, "Fred X");

    let mother = l1[1].node_id;
    let l2 = s.chart_outline(m.chart_id, Some(mother), 10).unwrap();
    let labels: Vec<_> = l2.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, ["6 Mother's father", "7 Mother's mother"]);
    let father = l1[0].node_id;
    let l2 = s.chart_outline(m.chart_id, Some(father), 10).unwrap();
    assert_eq!(l2[0].label, "4 Father's father");
    assert_eq!(l2[1].label, "5 Father's mother");
    assert!(!l2[0].has_children);
}

#[test]
fn ahnen_labels_deeper() {
    assert_eq!(ahnen_label(1), "Self");
    assert_eq!(ahnen_label(8), "Father's father's father");
    assert_eq!(ahnen_label(15), "Mother's mother's mother");
    assert_eq!(ahnen_label(16), "2x great-grandfather");
    assert_eq!(ahnen_label(31), "2x great-grandmother");
    assert_eq!(ahnen_label(32), "3x great-grandfather");
}

#[test]
fn male_parent_first_whatever_the_family_order() {
    // The family lists the wife-slot person (male) second only by sex: here HUSB is the woman.
    let g = ged(&[
        indi(1, "Kid /Z/", "M", ""),
        indi(2, "Joan /Z/", "F", ""),
        indi(3, "John /Z/", "M", ""),
        fam(1, 2, 3, &[1]),
    ]);
    let (_d, mut s) = open_ged(&g);
    let m = s.chart_open(&pedigree_req(&s, "Kid Z", 2)).unwrap();
    let l = s.chart_outline(m.chart_id, Some(m.root_node), 10).unwrap();
    assert_eq!(l[0].label, "2 Father");
    assert_eq!(l[0].person.as_ref().unwrap().name, "John Z");
    assert_eq!(l[1].person.as_ref().unwrap().name, "Joan Z");
}

#[test]
fn outline_limit_clamped() {
    let (_d, mut s) = open_ged(&wide_ged(150));
    let m = s.chart_open(&descendants_req(&s, "Dad W", 3)).unwrap();
    let items = s
        .chart_outline(m.chart_id, Some(m.root_node), 1000)
        .unwrap();
    assert_eq!(items.len(), 100);
    let items = s.chart_outline(m.chart_id, Some(m.root_node), 5).unwrap();
    assert_eq!(items.len(), 5);
    assert_eq!(s.chart_outline(m.chart_id, None, 0).unwrap().len(), 0);
}

#[test]
fn outline_children_lazy() {
    // Gus has wife Gina; children Hal (1900) and Ida (1902); Hal has child Jon.
    let g = ged(&[
        indi(1, "Gus /D/", "M", ""),
        indi(2, "Gina /D/", "F", ""),
        indi(3, "Ida /D/", "F", "1 BIRT\n2 DATE 1902\n"),
        indi(4, "Hal /D/", "M", "1 BIRT\n2 DATE 1900\n"),
        indi(5, "Jon /D/", "M", ""),
        indi(6, "Kay /D/", "F", ""),
        fam(1, 1, 2, &[3, 4]),
        fam(2, 4, 6, &[5]),
    ]);
    let (_d, mut s) = open_ged(&g);
    let m = s.chart_open(&descendants_req(&s, "Gus D", 4)).unwrap();
    let top = s.chart_outline(m.chart_id, None, 100).unwrap();
    assert_eq!(top.len(), 1);
    assert_eq!(top[0].label, "Self");
    assert!(top[0].has_children);

    let l1 = s
        .chart_outline(m.chart_id, Some(top[0].node_id), 100)
        .unwrap();
    let labels: Vec<_> = l1.iter().map(|i| i.label.as_str()).collect();
    // Partner first, then the children in birth order (Hal 1900 before Ida 1902).
    assert_eq!(labels, ["Partner", "Child 1", "Child 2"]);
    assert_eq!(l1[0].person.as_ref().unwrap().name, "Gina D");
    assert_eq!(l1[1].person.as_ref().unwrap().name, "Hal D");
    assert!(!l1[0].has_children);
    assert!(l1[1].has_children);
    assert!(!l1[2].has_children);

    let l2 = s
        .chart_outline(m.chart_id, Some(l1[1].node_id), 100)
        .unwrap();
    let labels: Vec<_> = l2.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, ["Partner", "Grandchild 1"]);
    assert_eq!(l2[1].person.as_ref().unwrap().name, "Jon D");

    let e = s.chart_outline(m.chart_id, Some(9999), 10).unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
    let e = s.chart_outline(9999, None, 10).unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn descendants_nav_matches_keyboard_semantics() {
    let (_d, mut s) = open_ged(&wide_ged(3));
    let m = s.chart_open(&descendants_req(&s, "Dad W", 3)).unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    let root = w.nodes.iter().find(|n| n.id == m.root_node).unwrap();
    let first = w
        .nodes
        .iter()
        .find(|n| Some(n.id) == root.nav.child)
        .unwrap();
    assert_eq!(first.nav.parent, Some(root.id));
    assert!(first.nav.prev.is_none() && first.nav.next.is_some());
}

/// A straight line of `n` people, each the child of the next (pedigree) / previous (descendants).
fn chain_ged(n: u32) -> String {
    let mut recs = Vec::new();
    for i in 1..=n {
        recs.push(indi(i, &format!("P{i:02} /L/"), "M", ""));
    }
    for i in 1..n {
        // I{i+1} is the father of I{i}; wife slot left to a placeholder-free single parent.
        recs.push(format!(
            "0 @F{i}@ FAM\n1 HUSB @I{}@\n1 CHIL @I{i}@\n",
            i + 1
        ));
    }
    ged(&recs)
}

#[test]
fn generations_clamped() {
    let (_d, mut s) = open_ged(&chain_ged(20));
    let ped = |s: &mut Session, g: u32| {
        s.chart_open(&pedigree_req(s, "P01 L", g))
            .unwrap()
            .node_count
    };
    assert_eq!(ped(&mut s, 12), 12);
    assert_eq!(ped(&mut s, 0), 1);
    // "Show all" asks for the most there is: every ancestor, whatever the number.
    assert_eq!(ped(&mut s, 255), 20);
    assert_eq!(ped(&mut s, 100_000), 20);
    // Descendants of the oldest person: down the chain.
    let desc = |s: &mut Session, g: u32| {
        s.chart_open(&descendants_req(s, "P20 L", g))
            .unwrap()
            .node_count
    };
    assert_eq!(desc(&mut s, 99), 8);
    assert_eq!(desc(&mut s, 0), 1);
}

#[test]
fn living_people_are_not_masked_like_the_read_side() {
    // The read side shows names of living people to the owner; charts use the same PersonRef.
    let (_d, mut s) = open_ged(&pedigree_ged());
    let m = s.chart_open(&pedigree_req(&s, "Me X", 3)).unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    for n in &w.nodes {
        let p = n.person.as_ref().unwrap();
        assert_eq!(Some(p.clone()), s.person_ref(p.id));
    }
}

#[test]
#[ignore = "release-only budget check"]
fn chart_window_under_8ms_10k() {
    // 100 children with 99 children each: about 10k nodes.
    let mut recs = vec![indi(1, "Root /R/", "M", "")];
    let mut n = 1000u32;
    let mut fams = String::from("0 @F0@ FAM\n1 HUSB @I1@\n");
    let mut kids = Vec::new();
    for i in 0..100u32 {
        let c = 2 + i;
        recs.push(indi(c, &format!("C{i} /R/"), "M", ""));
        fams.push_str(&format!("1 CHIL @I{c}@\n"));
        let mut f = format!("0 @F{}@ FAM\n1 HUSB @I{c}@\n", 100 + i);
        for _ in 0..99 {
            recs.push(indi(n, &format!("G{n} /R/"), "U", ""));
            f.push_str(&format!("1 CHIL @I{n}@\n"));
            n += 1;
        }
        kids.push(f);
    }
    recs.push(fams);
    recs.extend(kids);
    let (_d, mut s) = open_ged(&ged(&recs));
    let m = s.chart_open(&descendants_req(&s, "Root R", 3)).unwrap();
    assert!(m.node_count >= 10_000, "{}", m.node_count);
    let r = Rect {
        x: 0.0,
        y: 0.0,
        w: 1920.0,
        h: 1080.0,
    };
    let mut worst = std::time::Duration::ZERO;
    for i in 0..50 {
        let r = Rect {
            x: r.x + f32::from(i as u8) * 300.0,
            ..r
        };
        let t = std::time::Instant::now();
        let w = s.chart_window(m.chart_id, &r, 1.0).unwrap();
        worst = worst.max(t.elapsed());
        std::hint::black_box(w);
    }
    assert!(
        worst < std::time::Duration::from_millis(8),
        "worst window {worst:?}"
    );
}

// ---- relatives ----

/// Ivan Koval (1) and Olena Koval (2) with sons Petro (3), Andriy (4) and daughter Maria (5); Ivan's
/// earlier marriage to Halyna (6) gave Taras (7); Petro and Sofia (8) have Danylo (9).
fn relatives_ged() -> String {
    ged(&[
        indi(1, "Ivan /Koval/", "M", ""),
        indi(2, "Olena /Koval/", "F", ""),
        indi(3, "Petro /Koval/", "M", "1 BIRT\n2 DATE 1980\n"),
        indi(4, "Andriy /Koval/", "M", "1 BIRT\n2 DATE 1975\n"),
        indi(5, "Maria /Koval/", "F", "1 BIRT\n2 DATE 1985\n"),
        indi(6, "Halyna /Bondar/", "F", ""),
        indi(7, "Taras /Koval/", "M", "1 BIRT\n2 DATE 1970\n"),
        indi(8, "Sofia /Koval/", "F", ""),
        indi(9, "Danylo /Koval/", "M", ""),
        fam(1, 1, 2, &[3, 4, 5]),
        fam(2, 1, 6, &[7]),
        fam(3, 3, 8, &[9]),
    ])
}

fn relatives_req(s: &Session, name: &str, generations: u32) -> ChartRequest {
    ChartRequest {
        kind: ChartKind::Relatives,
        root: id_of(s, name),
        generations,
        aunts_uncles: None,
        siblings: None,
    }
}

#[test]
fn relatives_window_labels_each_person() {
    let (_d, mut s) = open_ged(&relatives_ged());
    let m = s.chart_open(&relatives_req(&s, "Petro Koval", 2)).unwrap();
    assert_eq!(m.kind, ChartKind::Relatives);
    assert_eq!(m.root_node, 0);
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    let rel = |name: &str| {
        w.nodes
            .iter()
            .find(|n| n.person.as_ref().is_some_and(|p| p.name == name))
            .unwrap_or_else(|| panic!("no {name}"))
            .relation
            .clone()
    };
    assert_eq!(rel("Petro Koval").as_deref(), Some("starting person"));
    assert_eq!(rel("Andriy Koval").as_deref(), Some("brother"));
    assert_eq!(rel("Maria Koval").as_deref(), Some("sister"));
    assert_eq!(rel("Taras Koval").as_deref(), Some("half brother"));
    assert_eq!(rel("Ivan Koval").as_deref(), Some("father"));
    assert_eq!(rel("Olena Koval").as_deref(), Some("mother"));
    assert_eq!(rel("Sofia Koval").as_deref(), Some("partner"));
    assert_eq!(rel("Danylo Koval").as_deref(), Some("son"));
    // Brothers and sisters are ordered by birth: Taras 1970, Andriy 1975, Petro 1980, Maria 1985.
    let x = |name: &str| {
        w.nodes
            .iter()
            .find(|n| n.person.as_ref().is_some_and(|p| p.name == name))
            .unwrap()
            .x
    };
    assert!(x("Taras Koval") < x("Andriy Koval"));
    assert!(x("Andriy Koval") < x("Petro Koval"));
    assert!(x("Petro Koval") < x("Maria Koval"));
    let focus = w.nodes.iter().find(|n| n.id == 0).unwrap();
    assert!(focus.label.contains("Petro Koval") && focus.label.contains("starting person"));
    // Dots-only zoom carries no text.
    let low = s.chart_window(m.chart_id, &everything(), 0.2).unwrap();
    assert!(low.nodes.iter().all(|n| n.relation.is_none()));
}

#[test]
fn relatives_can_hide_aunts_and_wire_shape_has_relation() {
    let (_d, mut s) = open_ged(&relatives_ged());
    let mut req = relatives_req(&s, "Petro Koval", 2);
    req.aunts_uncles = Some(false);
    let m = s.chart_open(&req).unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    let v = serde_json::to_value(&w).unwrap();
    assert!(v["nodes"][0]["relation"].is_string());
    let r = serde_json::to_value(&req).unwrap();
    assert_eq!(r["auntsUncles"], false);
    assert_eq!(r["kind"], "relatives");
}

#[test]
fn relatives_outline_groups_the_related_people() {
    let (_d, mut s) = open_ged(&relatives_ged());
    let m = s.chart_open(&relatives_req(&s, "Petro Koval", 2)).unwrap();
    let root = s.chart_outline(m.chart_id, None, 10).unwrap();
    assert_eq!(root.len(), 1);
    assert_eq!(root[0].label, "Starting person");
    assert!(root[0].has_children);
    let groups = s
        .chart_outline(m.chart_id, Some(root[0].node_id), 50)
        .unwrap();
    let titles: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
    assert_eq!(titles, ["Parents", "Siblings", "Partners", "Children"]);
    assert!(groups.iter().all(|g| g.person.is_none() && g.has_children));
    let names = |g: &OutlineItem| -> Vec<String> {
        s.chart_outline(m.chart_id, Some(g.node_id), 50)
            .unwrap()
            .into_iter()
            .map(|i| format!("{} {}", i.label, i.person.unwrap().name))
            .collect()
    };
    assert_eq!(
        names(&groups[0]),
        ["Father Ivan Koval", "Mother Olena Koval"]
    );
    assert_eq!(
        names(&groups[1]),
        [
            "Half brother Taras Koval",
            "Brother Andriy Koval",
            "Sister Maria Koval"
        ]
    );
    assert_eq!(names(&groups[2]), ["Partner Sofia Koval"]);
    assert_eq!(names(&groups[3]), ["Son Danylo Koval"]);
    // Unknown group rows are refused.
    assert!(
        s.chart_outline(
            m.chart_id,
            Some(gt_session::chart::GROUP_BASE | (9999 << 3)),
            5
        )
        .is_err()
    );
}

#[test]
fn relatives_generations_are_clamped() {
    let (_d, mut s) = open_ged(&relatives_ged());
    let m = s.chart_open(&relatives_req(&s, "Petro Koval", 99)).unwrap();
    assert_eq!(m.node_count, 8);
    let m0 = s.chart_open(&relatives_req(&s, "Petro Koval", 0)).unwrap();
    assert!(m0.node_count >= 1);
}

/// Petro's father Ivan has a sister Zoya (11) with a daughter Nina (12); their parents are Mykola and Hanna.
fn relatives_cousins_ged() -> String {
    ged(&[
        indi(1, "Ivan /Koval/", "M", ""),
        indi(2, "Olena /Koval/", "F", ""),
        indi(3, "Petro /Koval/", "M", "1 BIRT\n2 DATE 1980\n"),
        indi(10, "Mykola /Koval/", "M", ""),
        indi(13, "Hanna /Koval/", "F", ""),
        indi(11, "Zoya /Koval/", "F", ""),
        indi(12, "Nina /Shevchuk/", "F", ""),
        fam(1, 1, 2, &[3]),
        fam(2, 10, 13, &[1, 11]),
        fam(3, 11, 0, &[12]),
    ])
}

#[test]
fn relatives_label_cousins() {
    let (_d, mut s) = open_ged(&relatives_cousins_ged());
    let m = s.chart_open(&relatives_req(&s, "Petro Koval", 3)).unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    let rel = |name: &str| {
        w.nodes
            .iter()
            .find(|n| n.person.as_ref().is_some_and(|p| p.name == name))
            .and_then(|n| n.relation.clone())
    };
    assert_eq!(rel("Zoya Koval").as_deref(), Some("aunt"));
    assert_eq!(rel("Nina Shevchuk").as_deref(), Some("cousin"));
}

/// Expanding every row of the outline, with a path guard, must end and never list a person twice as expandable.
#[test]
fn relatives_outline_expands_fully_and_terminates() {
    let (_d, mut s) = open_ged(&relatives_cousins_ged());
    let m = s.chart_open(&relatives_req(&s, "Petro Koval", 3)).unwrap();
    let mut stack: Vec<(u32, usize)> = s
        .chart_outline(m.chart_id, None, 50)
        .unwrap()
        .into_iter()
        .map(|i| (i.node_id, 0))
        .collect();
    let mut expandable_people = std::collections::HashSet::new();
    let mut rows = 0;
    while let Some((id, depth)) = stack.pop() {
        rows += 1;
        assert!(rows < 500 && depth < 40, "the outline does not end");
        for it in s.chart_outline(m.chart_id, Some(id), 50).unwrap() {
            if it.person.is_some() && it.has_children {
                assert!(
                    expandable_people.insert(it.node_id),
                    "node {} is expandable in two places",
                    it.node_id
                );
            }
            if it.has_children {
                stack.push((it.node_id, depth + 1));
            }
        }
    }
}

/// Focus > Parents > Father > Children lists the focus again, as a reference that cannot be expanded.
#[test]
fn relatives_outline_father_children_is_a_reference() {
    let (_d, mut s) = open_ged(&relatives_cousins_ged());
    let m = s.chart_open(&relatives_req(&s, "Petro Koval", 3)).unwrap();
    let root = s.chart_outline(m.chart_id, None, 50).unwrap().remove(0);
    let groups = s.chart_outline(m.chart_id, Some(root.node_id), 50).unwrap();
    let parents = s
        .chart_outline(m.chart_id, Some(groups[0].node_id), 50)
        .unwrap();
    let father = parents
        .iter()
        .find(|p| p.label.starts_with("Father"))
        .unwrap();
    assert!(father.has_children);
    let fg = s
        .chart_outline(m.chart_id, Some(father.node_id), 50)
        .unwrap();
    let children = fg.iter().find(|g| g.label == "Children").unwrap();
    let kids = s
        .chart_outline(m.chart_id, Some(children.node_id), 50)
        .unwrap();
    assert_eq!(kids.len(), 1);
    assert_eq!(kids[0].node_id, root.node_id);
    assert!(!kids[0].has_children);
    assert!(kids[0].label.contains("also shown above"));
}

/// A person whose only relatives are listed elsewhere has no expander; the relation is also carried as data.
#[test]
fn relatives_outline_has_no_expander_for_references_only_and_carries_relation() {
    let (_d, mut s) = open_ged(&relatives_cousins_ged());
    let m = s.chart_open(&relatives_req(&s, "Petro Koval", 3)).unwrap();
    let root = s.chart_outline(m.chart_id, None, 50).unwrap().remove(0);
    assert_eq!(root.relation, None);
    let groups = s.chart_outline(m.chart_id, Some(root.node_id), 50).unwrap();
    let parents = s
        .chart_outline(m.chart_id, Some(groups[0].node_id), 50)
        .unwrap();
    let father = parents
        .iter()
        .find(|p| p.label.starts_with("Father"))
        .unwrap();
    let mother = parents
        .iter()
        .find(|p| p.label.starts_with("Mother"))
        .unwrap();
    assert_eq!(father.relation.as_deref(), Some("father"));
    assert!(father.has_children, "his parents are not listed elsewhere");
    assert_eq!(mother.relation.as_deref(), Some("mother"));
    assert!(
        !mother.has_children,
        "her partner and child are listed above"
    );
    let v = serde_json::to_value(mother).unwrap();
    assert_eq!(v["relation"], "mother");
}

// ---- whole tree ----

/// The pedigree family (7 people) plus a lone Zed.
fn whole_ged() -> String {
    let mut g = pedigree_ged();
    g.truncate(g.len() - "0 TRLR\n".len());
    g.push_str(&indi(8, "Zed /Solo/", "M", ""));
    g.push_str("0 TRLR\n");
    g
}

#[test]
fn whole_tree_has_everyone_once_headings_and_an_unconnected_block() {
    let (_d, mut s) = open_ged(&whole_ged());
    let w = s.chart_open_whole().unwrap();
    assert_eq!(w.meta.kind, ChartKind::Whole);
    assert_eq!(w.meta.node_count, 8);
    assert_eq!(w.meta.version, s.revision());
    let titles: Vec<&str> = w.blocks.iter().map(|b| b.title.as_str()).collect();
    assert_eq!(titles, ["X family (7 people)", "Unconnected people (1)"]);
    assert_eq!(w.blocks[0].surname, "X");
    assert_eq!(w.blocks[1].surname, "");
    assert!(w.blocks[1].unconnected && !w.blocks[0].unconnected);
    let all = s.chart_window(w.meta.chart_id, &everything(), 1.0).unwrap();
    assert_eq!(all.nodes.len(), 8);
    assert_eq!(
        all.edges
            .iter()
            .filter(|e| e.kind == EdgeKindDto::Parent)
            .count(),
        3
    );
    assert!(
        all.nodes
            .iter()
            .all(|n| n.dup_of.is_none() && !n.label.is_empty())
    );
    let me = all
        .nodes
        .iter()
        .find(|n| n.person.as_ref().is_some_and(|p| p.name == "Me X"))
        .unwrap();
    assert_eq!(me.generation, 2);
    assert!(me.label.ends_with("generation 3"), "{}", me.label);
    let v = serde_json::to_value(&w).unwrap();
    assert!(v["meta"]["chartId"].is_u64() && v["blocks"][0]["rect"]["w"].is_number());
}

#[test]
fn whole_tree_is_cached_per_revision() {
    let (_d, mut s) = open_ged(&whole_ged());
    let a = s.chart_open_whole().unwrap();
    let b = s.chart_open_whole().unwrap();
    assert_eq!(a, b);
    // Other charts do not push it out.
    for name in ["Me X", "Fred X", "Mary Y"] {
        s.chart_open(&pedigree_req(&s, name, 3)).unwrap();
    }
    assert!(s.chart_window(a.meta.chart_id, &everything(), 1.0).is_ok());
    // A change makes it stale; asking again lays it out again.
    s.set_settings(SettingsPatch {
        living_max_age: Some(90),
        ..SettingsPatch::default()
    })
    .unwrap();
    let e = s
        .chart_window(a.meta.chart_id, &everything(), 1.0)
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Stale);
    let c = s.chart_open_whole().unwrap();
    assert_ne!(c.meta.chart_id, a.meta.chart_id);
    assert_eq!(c.meta.version, s.revision());
}

#[test]
fn whole_tree_far_out_sends_surnames_and_sex_only() {
    let (_d, mut s) = open_ged(&whole_ged());
    let w = s.chart_open_whole().unwrap();
    let all = s
        .chart_window(w.meta.chart_id, &everything(), 0.05)
        .unwrap();
    assert_eq!(all.nodes.len(), 8);
    for n in &all.nodes {
        assert!(n.label.is_empty());
        let p = n.person.as_ref().unwrap();
        assert!(p.lifespan.is_empty());
        assert!(["X", "Y", "Solo"].contains(&p.name.as_str()), "{}", p.name);
    }
    assert!(
        all.nodes
            .iter()
            .any(|n| n.person.as_ref().unwrap().sex == SexKind::Female)
    );
}

#[test]
fn whole_tree_lineage_and_locate() {
    let (_d, mut s) = open_ged(&whole_ged());
    let w = s.chart_open_whole().unwrap();
    let id = w.meta.chart_id;
    let me = s.chart_locate(id, id_of(&s, "Me X")).unwrap().unwrap();
    let fred = s.chart_locate(id, id_of(&s, "Fred X")).unwrap().unwrap();
    assert_ne!(me.node_id, fred.node_id);
    assert!(me.rect.y > fred.rect.y);
    // Me: self, two parents, four grandparents. Fred's line: Fred, Me, his parents.
    assert_eq!(s.chart_lineage(id, me.node_id).unwrap().len(), 7);
    assert_eq!(s.chart_lineage(id, fred.node_id).unwrap().len(), 4);
    assert!(s.chart_locate(id, Uuid::nil()).unwrap().is_none());
    assert_eq!(
        s.chart_lineage(id, 999).unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[test]
fn whole_tree_commands_refuse_other_charts_and_other_charts_refuse_whole() {
    let (_d, mut s) = open_ged(&whole_ged());
    let m = s.chart_open(&pedigree_req(&s, "Me X", 3)).unwrap();
    assert_eq!(
        s.chart_lineage(m.chart_id, 0).unwrap_err().code,
        ErrorCode::Invalid
    );
    assert_eq!(
        s.chart_locate(m.chart_id, id_of(&s, "Me X"))
            .unwrap_err()
            .code,
        ErrorCode::Invalid
    );
    let req = ChartRequest {
        kind: ChartKind::Whole,
        root: id_of(&s, "Me X"),
        generations: 3,
        aunts_uncles: None,
        siblings: None,
    };
    assert_eq!(s.chart_open(&req).unwrap_err().code, ErrorCode::Invalid);
    let w = s.chart_open_whole().unwrap();
    assert_eq!(
        s.chart_outline(w.meta.chart_id, None, 5).unwrap_err().code,
        ErrorCode::Invalid
    );
}

#[test]
fn whole_tree_of_an_empty_project_is_empty() {
    let (_d, mut s) = open_ged(&ged(&[]));
    let w = s.chart_open_whole().unwrap();
    assert_eq!(w.meta.node_count, 0);
    assert!(w.blocks.is_empty());
}

fn set_direction(s: &mut Session, d: TreeDirection) {
    s.set_settings(SettingsPatch {
        tree_direction: Some(d),
        ..SettingsPatch::default()
    })
    .unwrap();
}

fn node_named(w: &ChartWindow, name: &str) -> ChartNode {
    w.nodes
        .iter()
        .find(|n| n.person.as_ref().is_some_and(|p| p.name == name))
        .unwrap_or_else(|| panic!("no node {name}"))
        .clone()
}

#[test]
fn reversed_pedigree_puts_the_parents_left_of_the_root() {
    let (_d, mut s) = open_ged(&pedigree_ged());
    let m = s.chart_open(&pedigree_req(&s, "Me X", 3)).unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    assert!(node_named(&w, "Fred X").x > node_named(&w, "Me X").x);
    set_direction(&mut s, TreeDirection::Reversed);
    let m = s.chart_open(&pedigree_req(&s, "Me X", 3)).unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    assert!(node_named(&w, "Fred X").x < node_named(&w, "Me X").x);
    assert!(node_named(&w, "Gus X").x < node_named(&w, "Fred X").x);
    assert_eq!(w.edges.len(), 6);
}

#[test]
fn reversed_descendants_put_the_child_above_the_parent() {
    let (_d, mut s) = open_ged(&pedigree_ged());
    let m = s.chart_open(&descendants_req(&s, "Gus X", 3)).unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    assert!(node_named(&w, "Fred X").y > node_named(&w, "Gus X").y);
    set_direction(&mut s, TreeDirection::Reversed);
    let m = s.chart_open(&descendants_req(&s, "Gus X", 3)).unwrap();
    let w = s.chart_window(m.chart_id, &everything(), 1.0).unwrap();
    assert!(node_named(&w, "Fred X").y < node_named(&w, "Gus X").y);
    assert!(node_named(&w, "Me X").y < node_named(&w, "Fred X").y);
}

#[test]
fn reversed_whole_tree_is_reopened_and_keeps_headings_above_their_people() {
    let (_d, mut s) = open_ged(&whole_ged());
    let a = s.chart_open_whole().unwrap();
    let w = s.chart_window(a.meta.chart_id, &everything(), 1.0).unwrap();
    assert!(node_named(&w, "Mary Y").y < node_named(&w, "Me X").y);
    set_direction(&mut s, TreeDirection::Reversed);
    let b = s.chart_open_whole().unwrap();
    assert_ne!(a.meta.chart_id, b.meta.chart_id);
    let w = s.chart_window(b.meta.chart_id, &everything(), 1.0).unwrap();
    assert!(node_named(&w, "Mary Y").y > node_named(&w, "Me X").y);
    assert_eq!(a.blocks[0].rect, b.blocks[0].rect);
    let h = &b.blocks[0].rect;
    for n in w
        .nodes
        .iter()
        .filter(|n| n.person.as_ref().is_some_and(|p| p.name != "Zed Solo"))
    {
        assert!(n.y >= h.y + h.h, "{} above its heading", n.id);
    }
    // Locating a person follows the flip.
    let me = id_of(&s, "Me X");
    let loc = s.chart_locate(b.meta.chart_id, me).unwrap().unwrap();
    assert_eq!(loc.rect.y, node_named(&w, "Me X").y);
    // Asking again with the same direction reuses the chart.
    assert_eq!(s.chart_open_whole().unwrap(), b);
}
