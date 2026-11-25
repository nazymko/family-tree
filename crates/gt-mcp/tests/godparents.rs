#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! add_godparent and remove_own_godparent: an agent adds the godparents a metric record names to a
//! baptism, a repeat adds nothing, and it takes off only its own.

mod common;

use common::*;
use gt_mcp::{AgentCtx, handle_message};
use gt_session::{EditCommand, EventInput, EventOwner, EventOwnerKind, GodparentInput};
use serde_json::{Value, json};
use uuid::Uuid;

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
    call(host, "begin_batch", json!({"title": "Metric book 1906"}))["batch"].clone()
}

/// A child and a man in the tree; the agent adds the child's baptism.
fn setup() -> (TestHost, Uuid, Uuid, Value, String) {
    let host = empty_host();
    let (child, uncle) = with(&host, |s| {
        (
            add_person(s, draft("Petro", "Koval", None, Some("1 Feb 1906"))),
            add_person(s, draft("Ivan", "Koval", None, Some("1870"))),
        )
    });
    let batch = begin(&host);
    let event = call(
        &host,
        "add_event",
        json!({"batch": batch, "person": child, "kind": "baptism", "date": "6 FEB 1906", "place": "Ivanivka"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string();
    (host, child, uncle, batch, event)
}

#[test]
fn an_agent_adds_godparents_by_name_and_by_person_and_reads_them_back() {
    let (host, child, uncle, batch, event) = setup();
    let by_name = call(
        &host,
        "add_godparent",
        json!({"batch": batch, "event": event, "name": "Maria Ivanivna Shevchuk, a widow of Ivanivka", "role": "godmother"}),
    );
    assert!(by_name.get("alreadyThere").is_none(), "{by_name}");
    let by_person = call(
        &host,
        "add_godparent",
        json!({"batch": batch, "event": event, "person": uncle, "role": "godfather", "note": "the uncle"}),
    );

    let read = call(&host, "get_event", json!({"id": event}));
    let gods = read["godparents"].as_array().unwrap();
    assert_eq!(gods.len(), 2, "{read}");
    assert_eq!(gods[0]["id"], by_name["id"]);
    assert_eq!(gods[0]["role"], "godmother");
    assert_eq!(
        gods[0]["name"],
        "Maria Ivanivna Shevchuk, a widow of Ivanivka"
    );
    assert_eq!(gods[1]["id"], by_person["id"]);
    assert_eq!(gods[1]["personId"], json!(uncle));
    assert_eq!(gods[1]["note"], "the uncle");

    let person = call(&host, "get_person", json!({"id": child}));
    let baptism = person["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "baptism")
        .unwrap();
    assert_eq!(baptism["godparents"].as_array().unwrap().len(), 2);
    let godfather = call(&host, "get_person", json!({"id": uncle}));
    assert_eq!(godfather["godchildren"][0]["person"]["id"], json!(child));
    assert_eq!(godfather["godchildren"][0]["role"], "godfather");
}

#[test]
fn a_repeat_is_already_there_and_nothing_is_added() {
    let (host, _, uncle, batch, event) = setup();
    let first = call(
        &host,
        "add_godparent",
        json!({"batch": batch, "event": event, "name": "Maria Shevchuk", "role": "godmother"}),
    );
    let again = call(
        &host,
        "add_godparent",
        json!({"batch": batch, "event": event, "name": "  maria   SHEVCHUK ", "role": "godmother"}),
    );
    assert_eq!(again["alreadyThere"], true);
    assert_eq!(again["id"], first["id"]);
    call(
        &host,
        "add_godparent",
        json!({"batch": batch, "event": event, "person": uncle, "role": "godfather"}),
    );
    let twice = call(
        &host,
        "add_godparent",
        json!({"batch": batch, "event": event, "person": uncle, "role": "godfather"}),
    );
    assert_eq!(twice["alreadyThere"], true);
    let read = call(&host, "get_event", json!({"id": event}));
    assert_eq!(read["godparents"].as_array().unwrap().len(), 2);
}

#[test]
fn what_is_no_godparent_is_refused() {
    let (host, child, uncle, batch, event) = setup();
    let both = call_err(
        &host,
        "add_godparent",
        json!({"batch": batch, "event": event, "person": uncle, "name": "Ivan", "role": "godfather"}),
    );
    assert!(both.contains("exactly one"), "{both}");
    let neither = call_err(
        &host,
        "add_godparent",
        json!({"batch": batch, "event": event, "role": "godfather"}),
    );
    assert!(neither.contains("exactly one"), "{neither}");
    let role = call_err(
        &host,
        "add_godparent",
        json!({"batch": batch, "event": event, "name": "Ivan", "role": "uncle"}),
    );
    assert!(role.contains("godfather or godmother"), "{role}");
    let own = call_err(
        &host,
        "add_godparent",
        json!({"batch": batch, "event": event, "person": child, "role": "godfather"}),
    );
    assert!(own.contains("own godparent"), "{own}");
    // A birth has no godparents.
    let birth = with(&host, |s| {
        s.tree()
            .events
            .values()
            .find(|e| e.kind == gt_core::model::EventKind::Birth)
            .unwrap()
            .common
            .id
    });
    let wrong = call_err(
        &host,
        "add_godparent",
        json!({"batch": batch, "event": birth, "name": "Ivan", "role": "godfather"}),
    );
    assert!(wrong.contains("baptism or christening"), "{wrong}");
}

#[test]
fn an_agent_takes_off_its_own_godparent_and_no_one_elses() {
    let (host, child, _, batch, event) = setup();
    let mine = call(
        &host,
        "add_godparent",
        json!({"batch": batch, "event": event, "name": "Maria Shevchuk", "role": "godmother"}),
    )["id"]
        .clone();
    // The owner adds one to the same event.
    let event_id = Uuid::parse_str(event.as_str()).unwrap();
    with(&host, |s| {
        s.apply(EditCommand::AddGodparent {
            event: event_id,
            godparent: GodparentInput {
                id: None,
                person: None,
                name: Some("Hryhorii Bondar".into()),
                role: "godfather".into(),
                note: None,
            },
        })
        .unwrap();
    });
    let owners = with(&host, |s| s.tree().events[&event_id].godparents[1].id);

    // Another agent may not take off this agent's godparent.
    let theirs = call_as(&host, &other(), "begin_batch", json!({"title": "Other"})).unwrap();
    let refused = call_as(
        &host,
        &other(),
        "remove_own_godparent",
        json!({"batch": theirs["batch"], "event": event, "godparent": mine}),
    )
    .unwrap_err();
    assert!(refused.contains("added yourself"), "{refused}");
    // Nor the owner's.
    let refused = call_err(
        &host,
        "remove_own_godparent",
        json!({"batch": batch, "event": event, "godparent": owners}),
    );
    assert!(refused.contains("added yourself"), "{refused}");
    // Nor through update_own_event, which keeps the godparents.
    call(
        &host,
        "update_own_event",
        json!({"batch": batch, "event": event, "description": "Baptised by the parish priest"}),
    );
    assert_eq!(
        with(&host, |s| s.tree().events[&event_id].godparents.len()),
        2
    );

    let done = call(
        &host,
        "remove_own_godparent",
        json!({"batch": batch, "event": event, "godparent": mine}),
    );
    assert_eq!(done["id"], mine);
    let left = with(&host, |s| s.tree().events[&event_id].godparents.clone());
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].id, owners);
    // Gone already: nothing to do.
    let again = call(
        &host,
        "remove_own_godparent",
        json!({"batch": batch, "event": event, "godparent": mine}),
    );
    assert_eq!(again["alreadyThere"], true);
    let _ = child;
}

#[test]
fn an_agent_cannot_take_off_a_godparent_the_owner_saved_with_the_event() {
    let host = empty_host();
    let (child, event) = with(&host, |s| {
        let child = add_person(s, draft("Petro", "Koval", None, None));
        let r = s
            .apply(EditCommand::SaveEvent {
                owner: EventOwner {
                    kind: EventOwnerKind::Person,
                    id: child,
                },
                event: EventInput {
                    id: None,
                    kind: "baptism".into(),
                    date_raw: Some("1906".into()),
                    place_text: None,
                    place_id: None,
                    description: None,
                    age: None,
                    cause: None,
                    negative: false,
                    private: false,
                    preferred: true,
                    participants: None,
                    godparents: Some(vec![GodparentInput {
                        id: None,
                        person: None,
                        name: Some("Hryhorii Bondar".into()),
                        role: "godfather".into(),
                        note: None,
                    }]),
                },
            })
            .unwrap();
        let event = *r
            .touched
            .iter()
            .find(|id| s.tree().events.contains_key(*id))
            .unwrap();
        (child, event)
    });
    let god = with(&host, |s| s.tree().events[&event].godparents[0].id);
    let batch = begin(&host);
    let refused = call_err(
        &host,
        "remove_own_godparent",
        json!({"batch": batch, "event": event, "godparent": god}),
    );
    assert!(refused.contains("added yourself"), "{refused}");
    let _ = child;
}

#[test]
fn the_instructions_say_how_baptisms_and_godparents_go_in() {
    for words in [
        "metric record",
        "add_godparent",
        "remove_own_godparent",
        "Do not create people",
    ] {
        assert!(gt_mcp::INSTRUCTIONS.contains(words), "{words}");
    }
    assert!(gt_mcp::INSTRUCTIONS.starts_with(gt_mcp::RULES));
}
