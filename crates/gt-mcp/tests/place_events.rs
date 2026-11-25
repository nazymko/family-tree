#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! add_place_records and list_place_events: an agent adds records about people who are not in the
//! tree to a place and reads the place's events back. Invented names and villages only.

mod common;

use common::*;
use serde_json::{Value, json};
use uuid::Uuid;

fn village(host: &TestHost) -> (Uuid, Uuid) {
    with(host, |s| {
        let mut d = draft("Hryhorii", "Koval", None, Some("1860"));
        d.birth_place_text = Some("Zelenivka, Testova district".into());
        let person = add_person(s, d);
        let place = s.tree().events.values().find_map(|e| e.place).unwrap();
        (person, place)
    })
}

fn record(rec_id: &str, kind: &str, date: &str, surname: &str) -> Value {
    json!({
        "kind": kind, "date": date, "book": "BK-9", "page": "3", "rec_id": rec_id,
        "people": [{"role": "child", "given": "Danylo", "surname": surname}],
    })
}

#[test]
fn an_agent_adds_records_and_reads_the_places_events() {
    let host = empty_host();
    let (person, place) = village(&host);
    let batch = call(&host, "begin_batch", json!({"title": "Book BK-9"}))["batch"].clone();
    let r = call(
        &host,
        "add_place_records",
        json!({"batch": batch, "place": place, "records": [
            record("a:1", "birth", "1850", "Bondar"),
            record("a:2", "death", "12.03.1851", "Bondar"),
        ]}),
    );
    assert_eq!(
        (r["added"].as_u64(), r["duplicates"].as_u64()),
        (Some(2), Some(0))
    );

    // The same again adds nothing.
    let again = call(
        &host,
        "add_place_records",
        json!({"batch": batch, "place": place, "records": [record("a:1", "birth", "1850", "Bondar")]}),
    );
    assert_eq!(again["added"], json!(0));
    assert_eq!(again["alreadyThere"], json!(true));

    let all = call(&host, "list_place_events", json!({"place": place}));
    // Hryhorii's birth and the two records, in year order.
    assert_eq!(all["total"], json!(3));
    let years: Vec<_> = all["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["year"].clone())
        .collect();
    assert_eq!(years, vec![json!(1850), json!(1851), json!(1860)]);
    assert_eq!(all["items"][2]["people"][0]["person"], json!(person));
    assert_eq!(all["items"][0]["book"], json!("BK-9"));

    let others = call(
        &host,
        "list_place_events",
        json!({"place": place, "who": "others"}),
    );
    assert_eq!(others["total"], json!(2));
    let ours = call(
        &host,
        "list_place_events",
        json!({"place": place, "who": "ours"}),
    );
    assert_eq!(ours["total"], json!(1));
    let deaths = call(
        &host,
        "list_place_events",
        json!({"place": place, "kinds": ["death"]}),
    );
    assert_eq!(deaths["total"], json!(1));
    let named = call(
        &host,
        "list_place_events",
        json!({"place": place, "query": "bondar", "limit": 1}),
    );
    assert_eq!(named["total"], json!(2));
    assert_eq!(named["items"].as_array().unwrap().len(), 1);
    assert_eq!(named["nextOffset"], json!(1));
    let ranged = call(
        &host,
        "list_place_events",
        json!({"place": place, "year_from": 1851, "year_to": 1855}),
    );
    assert_eq!(ranged["total"], json!(1));
}

#[test]
fn a_write_needs_the_agents_own_open_batch_and_valid_records() {
    let host = empty_host();
    let (_, place) = village(&host);
    let rec = record("a:1", "birth", "1850", "Bondar");
    let msg = call_err(
        &host,
        "add_place_records",
        json!({"batch": Uuid::from_u128(5), "place": place, "records": [rec.clone()]}),
    );
    assert!(msg.to_lowercase().contains("batch"), "{msg}");

    let batch = call(&host, "begin_batch", json!({"title": "x"}))["batch"].clone();
    let bad_kind = call_err(
        &host,
        "add_place_records",
        json!({"batch": batch, "place": place, "records": [record("a:1", "baptism", "1850", "B")]}),
    );
    assert!(bad_kind.contains("kind"), "{bad_kind}");
    let nobody = call_err(
        &host,
        "add_place_records",
        json!({"batch": batch, "place": place, "records": [{"kind": "birth"}]}),
    );
    assert!(nobody.contains("nobody"), "{nobody}");
    let no_place = call_err(
        &host,
        "add_place_records",
        json!({"batch": batch, "place": Uuid::from_u128(77), "records": [rec.clone()]}),
    );
    assert!(!no_place.is_empty());
    // A link to a person that is not in the tree is refused; a real one is kept.
    let mut linked = rec.clone();
    linked["people"][0]["person"] = json!(Uuid::from_u128(99));
    call_err(
        &host,
        "add_place_records",
        json!({"batch": batch, "place": place, "records": [linked]}),
    );
}
