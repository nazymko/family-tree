#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! set_place_coordinates: an agent gives a place with no point its coordinates, with a source.

mod common;

use common::*;
use gt_mcp::{AgentCtx, handle_message};
use gt_session::{EditCommand, MapScope, MapScopeKind};
use serde_json::{Value, json};
use uuid::Uuid;

const SOURCE: &str = "GeoNames 702550";

fn other() -> AgentCtx {
    AgentCtx {
        id: Uuid::from_u128(8),
        name: "Other reader".into(),
    }
}

fn call_as(host: &TestHost, who: &AgentCtx, tool: &str, args: Value) -> Result<Value, String> {
    let reply = handle_message(
        host,
        who,
        &request("tools/call", json!({"name": tool, "arguments": args})),
    )
    .unwrap();
    let result = reply["result"].clone();
    let text = result["content"][0]["text"].as_str().unwrap().to_string();
    if result["isError"] == json!(true) {
        Err(text)
    } else {
        Ok(serde_json::from_str(&text).unwrap())
    }
}

fn begin(host: &TestHost) -> Value {
    call(host, "begin_batch", json!({"title": "Places on the map"}))["batch"].clone()
}

/// The owner's person born in a synthetic village, and the village's id.
fn village(host: &TestHost) -> Uuid {
    with(host, |s| {
        let mut d = draft("Ivan", "Koval", None, Some("1901"));
        d.birth_place_text = Some("Zelenivka, Testova oblast".into());
        add_person(s, d);
        *s.tree()
            .places
            .iter()
            .find(|(_, p)| p.names.first().is_some_and(|n| n.name == "Zelenivka"))
            .unwrap()
            .0
    })
}

fn point(host: &TestHost, place: Uuid) -> (Option<f64>, Option<f64>) {
    with(host, |s| {
        let p = &s.tree().places[&place];
        (p.lat, p.lon)
    })
}

fn set(host: &TestHost, batch: &Value, place: Uuid, lat: f64, lon: f64) -> Result<Value, String> {
    call_as(
        host,
        &ctx(),
        "set_place_coordinates",
        json!({"batch": batch, "place": place, "lat": lat, "lon": lon, "source": SOURCE}),
    )
}

fn close(a: Option<f64>, b: f64) -> bool {
    a.is_some_and(|a| (a - b).abs() < 1e-9)
}

#[test]
fn sets_an_empty_point_with_its_source_and_the_map_shows_it() {
    let host = empty_host();
    let place = village(&host);
    let batch = begin(&host);
    let done = call(
        &host,
        "set_place_coordinates",
        json!({"batch": batch, "place": place, "lat": 46.6333, "lon": 32.6167,
               "source": SOURCE, "note": "The parish of the birth record is in this district."}),
    );
    assert_eq!(done["id"], json!(place), "{done}");
    let (lat, lon) = point(&host, place);
    assert!(
        close(lat, 46.6333) && close(lon, 32.6167),
        "{lat:?} {lon:?}"
    );
    // The source is kept in a research note on the place, which get_place shows.
    let got = call(&host, "get_place", json!({"id": place}));
    let text = got["notes"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        text.starts_with(
            "Coordinates 46.633300, 32.616700 (latitude, longitude). Source: GeoNames 702550"
        ),
        "{got}"
    );
    assert!(text.contains("The parish of the birth record"), "{text}");
    // The Map reads the same point.
    let m = with(&host, |s| {
        s.map_data(&MapScope {
            kind: MapScopeKind::All,
            person: None,
            generations: None,
        })
    });
    let shown = m.places.iter().find(|p| p.id == place).unwrap();
    assert!(close(shown.lat, 46.6333) && close(shown.lon, 32.6167));
    // In the log under the batch, with the owner's label.
    let log = call(&host, "get_log", json!({"filters": {"batch": batch}}));
    assert!(log.to_string().contains("Set place coordinates"), "{log}");
    // The same point again adds nothing, not even a second note.
    let again = set(&host, &batch, place, 46.633_300_000_1, 32.6167).unwrap();
    assert_eq!(again["alreadyThere"], json!(true), "{again}");
    let notes = with(&host, |s| s.tree().places[&place].common.notes.len());
    assert_eq!(notes, 1);
    // The owner can undo it: the point and the note go.
    with(&host, |s| {
        s.undo_batch(serde_json::from_value(batch.clone()).unwrap())
            .unwrap()
    });
    assert_eq!(point(&host, place), (None, None));
    assert!(with(&host, |s| s.tree().places[&place]
        .common
        .notes
        .is_empty()));
}

#[test]
fn refuses_a_point_the_owner_set() {
    let host = empty_host();
    let place = village(&host);
    with(&host, |s| {
        s.apply(EditCommand::SetPlaceCoordinates {
            place,
            lat_e6: Some(50_000_000),
            lon_e6: Some(30_000_000),
        })
        .unwrap()
    });
    let batch = begin(&host);
    let e = set(&host, &batch, place, 46.6333, 32.6167).unwrap_err();
    assert!(e.contains("Agents only add"), "{e}");
    assert_eq!(point(&host, place), (Some(50.0), Some(30.0)));
    // Saying the owner's own point is no change, and so no refusal.
    let same = set(&host, &batch, place, 50.0, 30.0).unwrap();
    assert_eq!(same["alreadyThere"], json!(true));
}

#[test]
fn own_point_can_be_corrected_until_someone_else_changes_it() {
    let host = empty_host();
    let place = village(&host);
    let batch = begin(&host);
    set(&host, &batch, place, 46.0, 32.0).unwrap();
    // Its own point, even from a later batch: a correction, with a note saying from what.
    call(
        &host,
        "end_batch",
        json!({"batch": batch, "report": "Set one place."}),
    );
    let batch2 = begin(&host);
    set(&host, &batch2, place, 46.5, 32.5).unwrap();
    assert_eq!(point(&host, place), (Some(46.5), Some(32.5)));
    let last = with(&host, |s| {
        let p = &s.tree().places[&place];
        s.tree().notes[p.common.notes.last().unwrap()].text.clone()
    });
    assert!(
        last.starts_with("Coordinates corrected from 46.000000, 32.000000 to 46.500000, 32.500000"),
        "{last}"
    );
    // Another connection may not move it.
    let theirs =
        call_as(&host, &other(), "begin_batch", json!({"title": "Other"})).unwrap()["batch"]
            .clone();
    let e = call_as(
        &host,
        &other(),
        "set_place_coordinates",
        json!({"batch": theirs, "place": place, "lat": 1.0, "lon": 1.0, "source": SOURCE}),
    )
    .unwrap_err();
    assert!(e.contains("Agents only add"), "{e}");
    // Once the owner moves it, it is the owner's.
    with(&host, |s| {
        s.apply(EditCommand::SetPlaceCoordinates {
            place,
            lat_e6: Some(47_000_000),
            lon_e6: Some(33_000_000),
        })
        .unwrap()
    });
    let e = set(&host, &batch2, place, 46.5, 32.5).unwrap_err();
    assert!(e.contains("Agents only add"), "{e}");
    // A point the owner cleared is empty again: an agent may fill it.
    with(&host, |s| {
        s.apply(EditCommand::SetPlaceCoordinates {
            place,
            lat_e6: None,
            lon_e6: None,
        })
        .unwrap()
    });
    set(&host, &batch2, place, 46.5, 32.5).unwrap();
    assert_eq!(point(&host, place), (Some(46.5), Some(32.5)));
}

#[test]
fn ranges_source_and_place_are_checked() {
    let host = empty_host();
    let place = village(&host);
    let batch = begin(&host);
    for (lat, lon) in [
        (json!(90.5), json!(0)),
        (json!(-91), json!(0)),
        (json!(0), json!(180.1)),
        (json!(0), json!(-181)),
        (json!("46.6"), json!(32.6)),
        (json!(null), json!(32.6)),
    ] {
        let e = call_err(
            &host,
            "set_place_coordinates",
            json!({"batch": batch, "place": place, "lat": lat, "lon": lon, "source": SOURCE}),
        );
        assert!(e.contains("degrees"), "{lat} {lon}: {e}");
    }
    let e = call_err(
        &host,
        "set_place_coordinates",
        json!({"batch": batch, "place": place, "lat": 1, "lon": 1, "source": "  "}),
    );
    assert!(e.contains("source"), "{e}");
    let e = call_err(
        &host,
        "set_place_coordinates",
        json!({"batch": batch, "place": place, "lat": 1, "lon": 1}),
    );
    assert!(e.contains("source"), "{e}");
    let e = call_err(
        &host,
        "set_place_coordinates",
        json!({"batch": batch, "place": Uuid::nil(), "lat": 1, "lon": 1, "source": SOURCE}),
    );
    assert!(!e.is_empty());
    // The edges themselves are places on the globe.
    set(&host, &batch, place, -90.0, 180.0).unwrap();
    assert_eq!(point(&host, place), (Some(-90.0), Some(180.0)));
    // Nothing refused left a mark: one note, from the one write.
    assert_eq!(
        with(&host, |s| s.tree().places[&place].common.notes.len()),
        1
    );
}

#[test]
fn the_session_refuses_what_json_cannot_carry() {
    let host = empty_host();
    let place = village(&host);
    let batch: Uuid = serde_json::from_value(begin(&host)).unwrap();
    for (lat, lon) in [
        (f64::NAN, 1.0),
        (1.0, f64::INFINITY),
        (f64::NEG_INFINITY, 1.0),
    ] {
        let e = with(&host, |s| {
            s.set_place_coordinates_as_agent(place, lat, lon, SOURCE, None, &ctx(), batch)
                .unwrap_err()
        });
        assert_eq!(e.code, gt_session::ErrorCode::Invalid);
    }
    assert_eq!(point(&host, place), (None, None));
}
