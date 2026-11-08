#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! `get_around_chart`: the people of the "Built around" chart, as rows.

mod common;

use common::*;
use gt_gedcom::synthetic::synthetic_tree;
use gt_session::*;
use gt_store::Project;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use uuid::Uuid;

fn named(s: &mut Session, given: &str, sex: SexKind) -> PersonDraft {
    let _ = s;
    PersonDraft {
        sex,
        ..draft(given, "Koval", None, None)
    }
}

struct Family {
    ivan: Uuid,
    maria: Uuid,
}

/// Ivan Koval and Maria (wife) as partners; Ivan's brother Taras, his father Petro with a brother Stepan, his mother
/// Olena with a brother Vasyl; Maria's sister Nadia.
fn family(host: &TestHost) -> Family {
    with(host, |s| {
        let d = named(s, "Ivan", SexKind::Male);
        let ivan = add_person(s, d);
        let d = named(s, "Maria", SexKind::Female);
        let maria = add_relative(s, ivan, Relation::Partner, d);
        let d = named(s, "Petro", SexKind::Male);
        let petro = add_relative(s, ivan, Relation::Father, d);
        let d = named(s, "Olena", SexKind::Female);
        let olena = add_relative(s, ivan, Relation::Mother, d);
        let d = named(s, "Taras", SexKind::Male);
        add_relative(s, ivan, Relation::Sibling, d);
        let d = named(s, "Hryhorii", SexKind::Male);
        add_relative(s, petro, Relation::Father, d);
        let d = named(s, "Stepan", SexKind::Male);
        add_relative(s, petro, Relation::Sibling, d);
        let d = named(s, "Yosyp", SexKind::Male);
        add_relative(s, olena, Relation::Father, d);
        let d = named(s, "Vasyl", SexKind::Male);
        add_relative(s, olena, Relation::Sibling, d);
        let d = named(s, "Oleh", SexKind::Male);
        add_relative(s, maria, Relation::Father, d);
        let d = named(s, "Nadia", SexKind::Female);
        add_relative(s, maria, Relation::Sibling, d);
        Family { ivan, maria }
    })
}

fn names(r: &Value) -> Vec<String> {
    r["people"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_string())
        .collect()
}

fn row<'a>(r: &'a Value, name: &str) -> &'a Value {
    r["people"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("{name} is not in {r}"))
}

#[test]
fn a_chosen_couple_brings_both_sides() {
    let host = empty_host();
    let f = family(&host);
    let line = call(
        &host,
        "get_around_chart",
        json!({"people": [f.maria, f.ivan]}),
    );
    assert_eq!(line["truncated"], json!(false));
    assert_eq!(line["count"].as_u64().unwrap() as usize, names(&line).len());
    assert!(!names(&line).contains(&"Taras Koval".to_string()));
    assert_eq!(row(&line, "Maria Koval")["relation"], "starting person");

    let sibs = call(
        &host,
        "get_around_chart",
        json!({"people": [f.maria, f.ivan], "kin": "siblings"}),
    );
    for want in ["Taras Koval", "Nadia Koval"] {
        assert!(names(&sibs).contains(&want.to_string()), "{want}");
    }
    assert!(!names(&sibs).contains(&"Stepan Koval".to_string()));
    assert_eq!(row(&sibs, "Taras Koval")["relation"], "brother");
    assert_eq!(row(&sibs, "Nadia Koval")["relation"], "sister");
    assert_eq!(row(&sibs, "Petro Koval")["relation"], "father");
    assert_eq!(row(&sibs, "Taras Koval")["appearsEarlier"], json!(false));

    for order in [[f.maria, f.ivan], [f.ivan, f.maria]] {
        let all = call(
            &host,
            "get_around_chart",
            json!({"people": order, "kin": "all", "generations": 5}),
        );
        for want in ["Taras Koval", "Nadia Koval", "Stepan Koval", "Vasyl Koval"] {
            assert!(names(&all).contains(&want.to_string()), "{want}: {all}");
        }
        assert_eq!(row(&all, "Stepan Koval")["relation"], "uncle");
        // Each person once.
        let mut seen = std::collections::HashSet::new();
        for p in all["people"].as_array().unwrap() {
            assert!(seen.insert(p["id"].as_str().unwrap().to_string()));
        }
    }
    // One chosen person is the ordinary chart around him.
    let one = call(&host, "get_around_chart", json!({"people": [f.ivan]}));
    assert!(names(&one).contains(&"Petro Koval".to_string()));
}

#[test]
fn bad_input_is_the_error_other_tools_give() {
    let host = empty_host();
    let f = family(&host);
    let mut fifty_one: Vec<Uuid> = vec![f.ivan; 1];
    fifty_one.extend((1..51).map(Uuid::from_u128));
    for args in [
        json!({"people": []}),
        json!({"people": fifty_one}),
        json!({"people": ["not-an-id"]}),
        json!({"people": [f.ivan], "kin": "cousins"}),
        json!({}),
    ] {
        let msg = call_err(&host, "get_around_chart", args.clone());
        assert!(!msg.is_empty(), "{args}");
    }
    let unknown = Uuid::from_u128(77);
    let msg = call_err(
        &host,
        "get_around_chart",
        json!({"people": [f.ivan, unknown]}),
    );
    assert!(
        msg.contains(&unknown.to_string()) || msg.contains("not found"),
        "{msg}"
    );
    // The same words as a missing person gets from another tool.
    let other = call_err(&host, "get_relatives", json!({"id": unknown}));
    assert_eq!(msg, other);
}

#[test]
fn no_tree_open_clear_error() {
    let host = no_tree_host();
    let msg = call_err(
        &host,
        "get_around_chart",
        json!({"people": [Uuid::from_u128(1)]}),
    );
    assert!(!msg.is_empty());
}

#[test]
#[ignore = "heavy: runs before merge"]
fn heavy_twenty_people_of_a_5000_person_tree() {
    let tree = synthetic_tree(5_000, 1_500);
    let mut ids: Vec<_> = tree.persons.keys().copied().collect();
    ids.sort();
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("project");
    let mut p = Project::create(&proj).unwrap();
    p.save_tree(&tree, 2_461_000).unwrap();
    drop(p);
    let session = Session::open(&proj, SessionClock::system()).unwrap();
    let host = TestHost {
        session: Mutex::new(Some(session)),
        keys: Mutex::new(HashMap::from([(hash(KEY), ctx())])),
        changes: Mutex::new(Vec::new()),
        keyless: Mutex::default(),
        _dir: Some(dir),
        changed_under_lock: Mutex::default(),
    };
    let chosen: Vec<_> = ids.iter().step_by(250).take(20).copied().collect();
    for kin in ["line", "siblings", "all"] {
        let t = Instant::now();
        let r = call(
            &host,
            "get_around_chart",
            json!({"people": chosen, "kin": kin}),
        );
        let took = t.elapsed();
        assert!(took < Duration::from_secs(30), "{kin} took {took:?}");
        assert_eq!(r["truncated"], json!(false), "{kin}");
        assert!(r["count"].as_u64().unwrap() >= 20, "{kin}");
        assert_eq!(r["count"].as_u64().unwrap() as usize, names(&r).len());
    }
}

/// Brothers and sisters reached through a family that has children only (no father or mother entered) are in the
/// chart, and so is the unnamed-siblings marker of that family.
#[test]
fn a_family_without_partners_brings_its_children_and_marker() {
    let host = empty_host();
    let f = family(&host);
    with(&host, |s| {
        // Oleh (Maria's father) has no parents: his brother goes into a family with children only.
        let oleh = s
            .tree()
            .persons
            .values()
            .find(|p| p.names.iter().any(|n| n.given.as_deref() == Some("Oleh")))
            .unwrap()
            .common
            .id;
        let d = named(s, "Ostap", SexKind::Male);
        add_relative(s, oleh, Relation::Sibling, d);
        let childless_of_partners = s
            .tree()
            .families
            .values()
            .find(|x| x.partner1.is_none() && x.partner2.is_none() && x.children.len() == 2)
            .unwrap()
            .common
            .id;
        s.apply(EditCommand::AddSiblingMarker {
            family: childless_of_partners,
        })
        .unwrap();
    });
    for order in [[f.maria, f.ivan], [f.ivan, f.maria]] {
        let all = call(
            &host,
            "get_around_chart",
            json!({"people": order, "kin": "all", "generations": 5}),
        );
        assert!(names(&all).contains(&"Ostap Koval".to_string()), "{all}");
        let markers = all["people"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["name"] == "more brothers and sisters, not named")
            .count();
        assert_eq!(markers, 1, "{all}");
    }
}
