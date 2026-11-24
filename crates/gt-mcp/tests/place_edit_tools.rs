#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! delete_place, move_place, update_place and remove_own_event through the tools. Invented data only.

mod common;

use common::*;
use serde_json::{Value, json};
use uuid::Uuid;

fn begin(host: &TestHost) -> Value {
    call(host, "begin_batch", json!({"title": "Housekeeping"}))["batch"].clone()
}

fn end(host: &TestHost, batch: &Value) {
    call(
        host,
        "end_batch",
        json!({"batch": batch, "report": "Done."}),
    );
}

fn uuid(v: &Value) -> Uuid {
    v["id"].as_str().unwrap().parse().unwrap()
}

fn new_place(host: &TestHost, batch: &Value, name: &str, parent: Option<Uuid>) -> Uuid {
    let mut args = json!({"batch": batch, "name": name});
    if let Some(p) = parent {
        args["parent"] = json!(p);
    }
    uuid(&call(host, "add_place", args))
}

#[test]
fn delete_place_removes_an_empty_place_and_refuses_one_in_use() {
    let host = empty_host();
    let b = begin(&host);
    let lone = new_place(&host, &b, "Lone", None);
    let top = new_place(&host, &b, "Top", None);
    let _kid = new_place(&host, &b, "Kid", Some(top));
    end(&host, &b);

    let b = begin(&host);
    let msg = call_err(&host, "delete_place", json!({"place_id": lone}));
    assert!(msg.contains("begin_batch"), "{msg}");
    let msg = call_err(&host, "delete_place", json!({"batch": b, "place_id": top}));
    assert!(msg.contains("1 places under it"), "{msg}");
    assert!(with(&host, |s| s.tree().places.contains_key(&top)));

    let r = call(&host, "delete_place", json!({"batch": b, "place_id": lone}));
    assert_eq!(r["deleted"], json!(lone));
    assert!(!with(&host, |s| s.tree().places.contains_key(&lone)));
    let batch_id: Uuid = b.as_str().unwrap().parse().unwrap();
    with(&host, |s| s.undo_batch(batch_id).unwrap());
    assert!(with(&host, |s| s.tree().places.contains_key(&lone)));

    // An event keeps its place.
    let there = with(&host, |s| {
        let mut d = draft("Ivan", "Koval", None, Some("1870"));
        d.birth_place_text = Some("Verbka".into());
        add_person(s, d);
        *s.tree()
            .places
            .keys()
            .find(|id| **id != lone && **id != top)
            .unwrap()
    });
    let b = begin(&host);
    let verbka = with(&host, |s| {
        *s.tree()
            .places
            .iter()
            .find(|(_, p)| p.names[0].name == "Verbka")
            .unwrap()
            .0
    });
    let _ = there;
    let msg = call_err(
        &host,
        "delete_place",
        json!({"batch": b, "place_id": verbka}),
    );
    assert!(msg.contains("1 events"), "{msg}");
}

#[test]
fn move_place_and_update_place_through_the_tools() {
    let host = empty_host();
    let b = begin(&host);
    let a = new_place(&host, &b, "A", None);
    let child = new_place(&host, &b, "Child", Some(a));
    let church = new_place(&host, &b, "Church", None);
    end(&host, &b);

    let b = begin(&host);
    let msg = call_err(
        &host,
        "move_place",
        json!({"batch": b, "place_id": a, "new_parent_id": child}),
    );
    assert!(msg.contains("under itself"), "{msg}");
    call_err(&host, "move_place", json!({"batch": b, "place_id": a}));
    call(
        &host,
        "move_place",
        json!({"batch": b, "place_id": church, "new_parent_id": child}),
    );
    assert_eq!(
        with(&host, |s| s.tree().places[&church].parent),
        Some(child)
    );
    let r = call(
        &host,
        "move_place",
        json!({"batch": b, "place_id": church, "new_parent_id": null}),
    );
    assert!(r.get("alreadyThere").is_none());
    assert_eq!(with(&host, |s| s.tree().places[&church].parent), None);

    call(
        &host,
        "update_place",
        json!({"batch": b, "place_id": church, "add_alt_names": ["Hopchytsia"], "type": "church"}),
    );
    let msg = call_err(
        &host,
        "update_place",
        json!({"batch": b, "place_id": church, "remove_alt_names": ["Church"]}),
    );
    assert!(msg.contains("primary name"), "{msg}");
    let msg = call_err(
        &host,
        "update_place",
        json!({"batch": b, "place_id": church, "lat": 46.5, "lon": 32.5}),
    );
    assert!(msg.contains("source"), "{msg}");
    call(
        &host,
        "update_place",
        json!({"batch": b, "place_id": church, "remove_alt_names": ["hopchytsia"],
               "lat": 46.5, "lon": 32.5, "source": "GeoNames 1"}),
    );
    with(&host, |s| {
        let p = &s.tree().places[&church];
        assert_eq!(p.names.len(), 1);
        assert_eq!(p.lat, Some(46.5));
        assert_eq!(p.kind.as_deref(), Some("church"));
    });
}

fn owner_birth(host: &TestHost) -> (Uuid, Uuid) {
    with(host, |s| {
        let person = add_person(s, draft("Ivan", "Koval", None, Some("1870")));
        let event = *s
            .tree()
            .events
            .iter()
            .find(|(_, e)| {
                e.participants.iter().any(
                    |p| matches!(p.target, gt_core::model::ParticipantRef::Person(id) if id == person),
                )
            })
            .unwrap()
            .0;
        (person, event)
    })
}

#[test]
fn remove_own_event_takes_back_an_agent_event_with_its_citations() {
    let host = empty_host();
    let (person, owner_event) = owner_birth(&host);
    let b = begin(&host);
    let src = uuid(&call(
        &host,
        "add_source",
        json!({"batch": b, "title": "Book"}),
    ));
    let event = uuid(&call(
        &host,
        "add_event",
        json!({"batch": b, "person": person, "kind": "residence", "date": "1900"}),
    ));
    call(
        &host,
        "add_citation",
        json!({"batch": b, "target": {"kind": "event", "id": event}, "source": src}),
    );
    end(&host, &b);
    assert_eq!(with(&host, |s| s.tree().citations.len()), 1);

    let b = begin(&host);
    // An event the owner made is refused, and so is a missing reason.
    let msg = call_err(
        &host,
        "remove_own_event",
        json!({"batch": b, "event": owner_event, "reason": "Wrong"}),
    );
    assert!(msg.contains("yourself"), "{msg}");
    assert!(with(&host, |s| s.tree().events.contains_key(&owner_event)));
    call_err(
        &host,
        "remove_own_event",
        json!({"batch": b, "event": event}),
    );
    call_err(
        &host,
        "remove_own_event",
        json!({"batch": b, "event": event, "reason": "  "}),
    );
    call_err(
        &host,
        "remove_own_event",
        json!({"event": event, "reason": "Wrong"}),
    );

    let r = call(
        &host,
        "remove_own_event",
        json!({"batch": b, "event": event, "reason": "The book names another Ivan"}),
    );
    assert!(r.get("alreadyThere").is_none());
    with(&host, |s| {
        assert!(!s.tree().events.contains_key(&event));
        assert_eq!(s.tree().citations.len(), 0, "the citation goes with it");
        assert!(s.tree().persons[&person].common.notes.iter().any(|n| {
            s.tree().notes[n]
                .text
                .contains("The book names another Ivan")
        }));
    });
    // Taking it back again is a repeat.
    let again = call(
        &host,
        "remove_own_event",
        json!({"batch": b, "event": event, "reason": "The book names another Ivan"}),
    );
    assert_eq!(again["alreadyThere"], json!(true));

    // The batch's undo brings the event back with its citation, and the note goes.
    let batch_id: Uuid = b.as_str().unwrap().parse().unwrap();
    with(&host, |s| s.undo_batch(batch_id).unwrap());
    with(&host, |s| {
        assert!(s.tree().events.contains_key(&event));
        assert_eq!(s.tree().citations.len(), 1);
        assert!(
            !s.tree().persons[&person]
                .common
                .notes
                .iter()
                .any(|n| s.tree().notes[n].text.contains("another Ivan"))
        );
    });
}

#[test]
fn remove_own_citation_takes_back_an_agent_citation_and_keeps_the_source() {
    use gt_session::{CitationArg, CitationInput, EditCommand, OwnerKind, OwnerRef, SourceRef};
    let host = empty_host();
    let (person, _) = owner_birth(&host);
    let b = begin(&host);
    let src = uuid(&call(
        &host,
        "add_source",
        json!({"batch": b, "title": "Book"}),
    ));
    let mine = uuid(&call(
        &host,
        "add_citation",
        json!({"batch": b, "target": {"kind": "person", "id": person}, "source": src, "page": "12"}),
    ));
    end(&host, &b);
    // The owner cites the same source on the same person.
    let theirs = with(&host, |s| {
        let before: Vec<Uuid> = s.tree().citations.keys().copied().collect();
        s.apply(EditCommand::AddCitation {
            target: OwnerRef {
                kind: OwnerKind::Person,
                id: person,
            },
            citation: CitationArg::New(CitationInput {
                source: SourceRef::Id(src),
                page: Some("99".into()),
                date_raw: None,
                text: None,
                quality: None,
                evidence: None,
            }),
        })
        .unwrap();
        *s.tree()
            .citations
            .keys()
            .find(|k| !before.contains(k))
            .unwrap()
    });
    assert_eq!(with(&host, |s| s.tree().citations.len()), 2);

    let b = begin(&host);
    let msg = call_err(
        &host,
        "remove_own_citation",
        json!({"citation": mine, "reason": "Misread"}),
    );
    assert!(msg.contains("begin_batch"), "{msg}");
    let msg = call_err(
        &host,
        "remove_own_citation",
        json!({"batch": b, "citation": theirs, "reason": "Misread"}),
    );
    assert!(msg.contains("yourself"), "{msg}");
    assert!(with(&host, |s| s.tree().citations.contains_key(&theirs)));
    call_err(
        &host,
        "remove_own_citation",
        json!({"batch": b, "citation": mine}),
    );
    call_err(
        &host,
        "remove_own_citation",
        json!({"batch": b, "citation": mine, "reason": "  "}),
    );
    assert!(with(&host, |s| s.tree().citations.contains_key(&mine)));

    call(
        &host,
        "remove_own_citation",
        json!({"batch": b, "citation": mine, "reason": "The page names another Ivan"}),
    );
    with(&host, |s| {
        assert!(!s.tree().citations.contains_key(&mine));
        assert!(s.tree().citations.contains_key(&theirs));
        assert!(s.tree().sources.contains_key(&src), "the source survives");
        let p = &s.tree().persons[&person];
        assert!(!p.common.citations.contains(&mine));
        assert!(p.common.citations.contains(&theirs));
        assert!(
            p.common
                .notes
                .iter()
                .any(|n| s.tree().notes[n].text.contains("another Ivan"))
        );
    });

    // The batch's undo brings the citation back with its link, and the note goes.
    let batch_id: Uuid = b.as_str().unwrap().parse().unwrap();
    with(&host, |s| s.undo_batch(batch_id).unwrap());
    with(&host, |s| {
        assert!(s.tree().citations.contains_key(&mine));
        assert!(s.tree().persons[&person].common.citations.contains(&mine));
        assert!(
            !s.tree().persons[&person]
                .common
                .notes
                .iter()
                .any(|n| s.tree().notes[n].text.contains("another Ivan"))
        );
    });
}
