#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! add_place, merge_places and the single `kind` filter of list_place_events, through the tools.
//! Invented villages only.

mod common;

use common::*;
use serde_json::{Value, json};
use uuid::Uuid;

fn batch(host: &TestHost) -> Value {
    call(host, "begin_batch", json!({"title": "Places"}))["batch"].clone()
}

fn id_of(v: &Value) -> Uuid {
    v["id"].as_str().unwrap().parse().unwrap()
}

#[test]
fn add_place_makes_a_place_once_and_the_batch_undoes_it() {
    let host = empty_host();
    let b = batch(&host);
    let district = call(
        &host,
        "add_place",
        json!({"batch": b, "name": "Zelena district", "type": "district"}),
    );
    assert!(district.get("alreadyThere").is_none());
    let district = id_of(&district);
    let village = call(
        &host,
        "add_place",
        json!({"batch": b, "name": "Verbka", "type": "village", "parent": district,
               "lat": 46.5, "lon": 32.25, "source": "GeoNames 1", "note": "Named in the books."}),
    );
    let village = id_of(&village);
    with(&host, |s| {
        let p = &s.tree().places[&village];
        assert_eq!(p.kind.as_deref(), Some("village"));
        assert_eq!((p.lat, p.lon), (Some(46.5), Some(32.25)));
        assert_eq!(
            s.place_option(village).unwrap().full,
            "Verbka, Zelena district"
        );
        assert_eq!(p.common.notes.len(), 1);
    });

    // The same name under the same parent answers with the existing one and adds nothing.
    let places = with(&host, |s| s.tree().places.len());
    let again = call(
        &host,
        "add_place",
        json!({"batch": b, "name": "VERBKA", "parent": district}),
    );
    assert_eq!(again["alreadyThere"], json!(true));
    assert_eq!(id_of(&again), village);
    assert_eq!(with(&host, |s| s.tree().places.len()), places);

    // Refusals: no batch, a coordinate without its source, a parent that is not there.
    let msg = call_err(&host, "add_place", json!({"name": "Boblyv"}));
    assert!(msg.contains("begin_batch"), "{msg}");
    call_err(
        &host,
        "add_place",
        json!({"batch": b, "name": "Boblyv", "lat": 1, "lon": 1}),
    );
    call_err(
        &host,
        "add_place",
        json!({"batch": b, "name": "Boblyv", "lat": 1}),
    );
    call_err(
        &host,
        "add_place",
        json!({"batch": b, "name": "Boblyv", "parent": Uuid::from_u128(3)}),
    );
    assert_eq!(with(&host, |s| s.tree().places.len()), places);

    // The owner undoes it with the batch.
    let batch_id: Uuid = b.as_str().unwrap().parse().unwrap();
    with(&host, |s| s.undo_batch(batch_id).unwrap());
    assert_eq!(with(&host, |s| s.tree().places.len()), 0);
}

#[test]
fn merge_places_answers_with_counts_and_is_one_undo() {
    let host = empty_host();
    let (keep, dup) = with(&host, |s| {
        for (name, place) in [
            ("Hryhorii", "Hopchytsia, North"),
            ("Danylo", "Hopchytsia, South"),
        ] {
            let mut d = draft(name, "Koval", None, Some("1860"));
            d.birth_place_text = Some(place.into());
            add_person(s, d);
        }
        let find = |full: &str| {
            *s.tree()
                .places
                .keys()
                .find(|id| s.place_option(**id).unwrap().full == full)
                .unwrap()
        };
        (find("Hopchytsia, North"), find("Hopchytsia, South"))
    });
    let b = batch(&host);
    let r = call(
        &host,
        "add_place_records",
        json!({"batch": b, "place": dup, "records": [{
            "kind": "birth", "book": "BK-2", "rec_id": "x:1",
            "people": [{"role": "child", "given": "Anna"}]}]}),
    );
    assert_eq!(r["added"], json!(1));

    let merged = call(
        &host,
        "merge_places",
        json!({"batch": b, "keep_id": keep, "merge_ids": [dup]}),
    );
    assert_eq!(merged["removed"], json!(1));
    assert_eq!(merged["repointed"]["events"], json!(1));
    assert_eq!(merged["repointed"]["placeRecords"], json!(1));
    let events = call(&host, "list_place_events", json!({"place": keep}));
    assert_eq!(events["total"], json!(3));
    // The merged place is gone for readers.
    call_err(&host, "list_place_events", json!({"place": dup}));

    // Refusals.
    call_err(
        &host,
        "merge_places",
        json!({"batch": b, "keep_id": keep, "merge_ids": [keep]}),
    );
    let msg = call_err(
        &host,
        "merge_places",
        json!({"keep_id": keep, "merge_ids": [dup]}),
    );
    assert!(msg.contains("begin_batch"), "{msg}");
    call_err(&host, "merge_places", json!({"batch": b, "keep_id": keep}));

    let batch_id: Uuid = b.as_str().unwrap().parse().unwrap();
    with(&host, |s| s.undo_batch(batch_id).unwrap());
    let back = call(&host, "list_place_events", json!({"place": dup}));
    assert_eq!(back["total"], json!(2), "the birth and the record are back");
    assert_eq!(
        call(&host, "list_place_events", json!({"place": keep}))["total"],
        json!(1)
    );
}

#[test]
fn list_place_events_takes_a_single_kind() {
    let host = empty_host();
    let place = with(&host, |s| {
        let mut d = draft("Hryhorii", "Koval", None, Some("1860"));
        d.birth_place_text = Some("Zelenivka".into());
        add_person(s, d);
        *s.tree().places.keys().next().unwrap()
    });
    let b = batch(&host);
    let rec = |id: &str, kind: &str| {
        json!({"kind": kind, "book": "BK-9", "rec_id": id,
               "people": [{"role": "child", "given": "Danylo"}]})
    };
    call(
        &host,
        "add_place_records",
        json!({"batch": b, "place": place, "records": [
            rec("a:1", "birth"), rec("a:2", "death"), rec("a:3", "death")]}),
    );
    let one = call(
        &host,
        "list_place_events",
        json!({"place": place, "kind": "death"}),
    );
    assert_eq!(one["total"], json!(2));
    let list = call(
        &host,
        "list_place_events",
        json!({"place": place, "kinds": ["death"]}),
    );
    assert_eq!(list["total"], json!(2));
    let births = call(
        &host,
        "list_place_events",
        json!({"place": place, "kind": "birth"}),
    );
    assert_eq!(births["total"], json!(2), "the tree's birth and the record");
    // A kind that is none, and both at once, are told so.
    let msg = call_err(
        &host,
        "list_place_events",
        json!({"place": place, "kind": "baptism"}),
    );
    assert!(msg.contains("kind"), "{msg}");
    let msg = call_err(
        &host,
        "list_place_events",
        json!({"place": place, "kind": "death", "kinds": ["birth"]}),
    );
    assert!(msg.contains("not both"), "{msg}");
}
