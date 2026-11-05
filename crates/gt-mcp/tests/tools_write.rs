#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The write tools: everything an agent adds goes through the gate, under its key and a batch.

mod common;

use common::*;
use gt_mcp::{Host, handle_message};
use gt_session::{ApiError, EditCommand, OwnerKind, OwnerRef, Relation, Session};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;
use uuid::Uuid;

fn ids_of(rows: &Value) -> Vec<Value> {
    let rows = rows.as_array().unwrap();
    rows.iter().map(|row| row["id"].clone()).collect()
}

fn begin(host: &TestHost) -> Value {
    call(host, "begin_batch", json!({"title": "Recording 2"}))["batch"].clone()
}

fn add(host: &TestHost, batch: &Value, given: &str) -> Value {
    call(
        host,
        "add_person",
        json!({"batch": batch, "given": given, "surnames": ["Koval"]}),
    )["id"]
        .clone()
}

/// A person the owner entered, so not the agent's own.
fn owners_person(host: &TestHost, given: &str, born: Option<&str>) -> Uuid {
    with(host, |s| add_person(s, draft(given, "Koval", None, born)))
}

fn records(host: &TestHost) -> usize {
    with(host, |s| {
        let t = s.tree();
        t.persons.len() + t.families.len() + t.events.len() + t.notes.len() + t.sources.len()
    })
}

#[test]
fn batch_of_other_tree_refused() {
    let (one, other) = (empty_host(), empty_host());
    let batch = begin(&one);
    let message = call_err(
        &other,
        "add_person",
        json!({"batch": batch, "given": "Ivan", "surnames": ["Koval"]}),
    );
    assert!(
        message.contains("not in the tree that is open"),
        "{message}"
    );
    assert_eq!(records(&other), 0);
}

#[test]
fn write_needs_batch() {
    let host = empty_host();
    let id = owners_person(&host, "Ivan", None);
    let before = records(&host);
    let writes = [
        ("end_batch", json!({"report": "Done"})),
        (
            "add_person",
            json!({"given": "Olena", "surnames": ["Koval"]}),
        ),
        (
            "link_people",
            json!({"person": id, "relation": {"to": id, "as": "father"}}),
        ),
        ("add_family", json!({"partner1": id})),
        (
            "add_name",
            json!({"person": id, "kind": "aka", "given": "Ivanko"}),
        ),
        ("add_event", json!({"person": id, "kind": "residence"})),
        (
            "add_note",
            json!({"target": {"kind": "person", "id": id}, "text": "A note"}),
        ),
        ("add_source", json!({"title": "Recording 2"})),
        (
            "add_citation",
            json!({"target": {"kind": "person", "id": id}, "source": id}),
        ),
        ("add_sibling_marker", json!({"family": id})),
        ("remove_own_empty_family", json!({"family": id})),
        (
            "set_number",
            json!({"person": id, "source_file": "a", "number": "1"}),
        ),
        ("set_sex", json!({"person": id, "sex": "male"})),
        ("update_own_note", json!({"note": id, "text": "A note"})),
        ("update_own_event", json!({"event": id})),
        ("update_own_name", json!({"person": id, "name": id})),
        ("update_own_citation", json!({"citation": id})),
    ];
    for (tool, args) in writes {
        let message = call_err(&host, tool, args);
        assert!(message.contains("begin_batch"), "{tool}: {message}");
    }
    assert_eq!(records(&host), before);
}

#[test]
fn begin_batch_needs_a_title_and_short_text() {
    let host = empty_host();
    assert!(call_err(&host, "begin_batch", json!({"title": "  "})).contains("title"));
    let long = "x".repeat(1025);
    assert!(call_err(&host, "begin_batch", json!({"title": long})).contains("1 KiB"));
}

#[test]
fn cyrillic_name_roundtrip() {
    let host = empty_host();
    let batch = begin(&host);
    let id = call(
        &host,
        "add_person",
        json!({
            "batch": batch,
            "given": "Іван",
            "patronymic": "Петрович",
            "surnames": ["Коваль"],
            "sex": "male",
            "birth_date": "1901",
            "birth_place": "Полтава",
        }),
    )["id"]
        .clone();
    let person = call(&host, "get_person", json!({"id": id}));
    let name = &person["names"][0];
    assert_eq!(name["given"], "Іван");
    assert_eq!(name["patronymic"], "Петрович");
    assert_eq!(name["surnames"], json!(["Коваль"]));
    assert_eq!(person["sex"], "male");
    let found = call(&host, "search_people", json!({"query": "Коваль"}));
    assert_eq!(found["items"][0]["id"], id);
    assert_eq!(found["items"][0]["birthYear"], "1901");
}

#[test]
fn bad_date_refused() {
    let host = empty_host();
    let batch = begin(&host);
    let id = add(&host, &batch, "Ivan");
    let event = call(
        &host,
        "add_event",
        json!({"batch": batch, "person": id, "kind": "residence"}),
    )["id"]
        .clone();
    let before = records(&host);
    let bad = "the spring after the war";
    let writes = [
        (
            "add_person",
            json!({"given": "Olena", "surnames": ["Koval"], "birth_date": bad}),
        ),
        (
            "add_event",
            json!({"person": id, "kind": "residence", "date": bad}),
        ),
        (
            "add_name",
            json!({"person": id, "kind": "aka", "given": "Ivanko", "date": bad}),
        ),
        (
            "add_citation",
            json!({"target": {"kind": "person", "id": id}, "source": id, "date": bad}),
        ),
        ("update_own_event", json!({"event": event, "date": bad})),
    ];
    for (tool, mut args) in writes {
        args["batch"] = batch.clone();
        let message = call_err(&host, tool, args);
        assert!(message.contains(bad), "{tool}: {message}");
        assert!(message.contains("note"), "{tool}: {message}");
    }
    assert_eq!(records(&host), before);
    // A date as a source gives it, and words a source used in brackets, are stored.
    for date in ["12 MAR 1901", "ABT 1880", "(after the war)"] {
        call(
            &host,
            "add_event",
            json!({"batch": batch, "person": id, "kind": "residence", "date": date}),
        );
    }
}

#[test]
fn changed_called_after_write() {
    let host = empty_host();
    let batch = begin(&host);
    assert!(
        host.changes.lock().unwrap().is_empty(),
        "a batch is no record"
    );
    let answer = call(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Ivan", "surnames": ["Koval"]}),
    );
    let id: Uuid = serde_json::from_value(answer["id"].clone()).unwrap();
    let changes = host.changes.lock().unwrap().clone();
    assert_eq!(changes.len(), 1);
    assert!(changes[0].0.contains(&id));
    assert_eq!(changes[0].1, ctx());
    assert_eq!(answer["touched"], json!(changes[0].0));
    assert!(
        !*host.changed_under_lock.lock().unwrap(),
        "changed ran with the session lock held"
    );
}

/// `changed` takes the session's own lock, as an app refreshing its views would. Called from
/// inside `with_session` it would wait for itself forever.
struct LockingHost(TestHost);

impl Host for LockingHost {
    fn with_session<T>(
        &self,
        f: &mut dyn FnMut(&mut Session) -> Result<T, ApiError>,
    ) -> Result<T, ApiError> {
        self.0.with_session(f)
    }

    fn authenticate(&self, key_sha256: &[u8; 32]) -> Option<gt_mcp::AgentCtx> {
        self.0.authenticate(key_sha256)
    }

    fn changed(&self, _touched: &[Uuid], _agent: &gt_mcp::AgentCtx) {
        drop(self.0.session.lock().unwrap());
    }
}

#[test]
fn a_write_does_not_deadlock_on_a_non_reentrant_lock() {
    let host = Arc::new(LockingHost(empty_host()));
    let batch = begin(&host.0);
    let (sent, got) = mpsc::channel();
    let worker = Arc::clone(&host);
    std::thread::spawn(move || {
        let body = request(
            "tools/call",
            json!({"name": "add_person", "arguments": {"batch": batch, "given": "Ivan"}}),
        );
        sent.send(handle_message(&*worker, &ctx(), &body)).unwrap();
    });
    let reply = got
        .recv_timeout(Duration::from_secs(30))
        .expect("the write never came back")
        .unwrap();
    assert_ne!(reply["result"]["isError"], json!(true), "{reply}");
}

#[test]
fn refusal_is_tool_error_not_protocol_error() {
    let host = empty_host();
    let id = owners_person(&host, "Ivan", Some("1901"));
    let batch = begin(&host);
    let result = call_raw(
        &host,
        "add_event",
        json!({"batch": batch, "person": id, "kind": "birth", "date": "1902"}),
    );
    assert_eq!(result["isError"], true, "{result}");
    let message = result["content"][0]["text"].as_str().unwrap();
    // The gate's own words, for the agent to adjust by.
    assert!(message.contains("already has a birth"), "{message}");
    assert!(message.contains("note"), "{message}");
    assert!(host.changes.lock().unwrap().is_empty());
}

#[test]
fn a_repeat_returns_the_record_and_notifies_nothing() {
    let host = empty_host();
    let id = owners_person(&host, "Ivan", None);
    let batch = begin(&host);
    let first = call(
        &host,
        "add_note",
        json!({"batch": batch, "target": {"kind": "person", "id": id}, "text": "From the recording"}),
    );
    assert_eq!(first["alreadyThere"], Value::Null);
    assert_eq!(host.changes.lock().unwrap().len(), 1);
    let again = call(
        &host,
        "add_note",
        json!({"batch": batch, "target": {"kind": "person", "id": id}, "text": "From the recording"}),
    );
    assert_eq!(again["alreadyThere"], true);
    assert_eq!(again["id"], first["id"]);
    assert_eq!(
        host.changes.lock().unwrap().len(),
        1,
        "a repeat changed nothing"
    );
}

#[test]
fn reruns_work_across_batches() {
    let host = empty_host();
    let run = |host: &TestHost| -> Vec<Value> {
        let batch = begin(host);
        // A rerun finds its people by number first.
        let number = json!({"source_file": "Recording 2", "number": "12"});
        let found = call(host, "find_by_number", number.clone())["person"].clone();
        let person = if found.is_null() {
            let person = add(host, &batch, "Ivan");
            let mut set = number.clone();
            set["batch"] = batch.clone();
            set["person"] = person.clone();
            assert_eq!(call(host, "set_number", set)["id"], person);
            person
        } else {
            found
        };
        let target = json!({"kind": "person", "id": person});
        let source = call(
            host,
            "add_source",
            json!({"batch": batch, "title": "Recording 2"}),
        );
        let citation = call(
            host,
            "add_citation",
            json!({"batch": batch, "target": target, "source": source["id"], "page": "12:40", "quality": 2}),
        );
        let note = call(
            host,
            "add_note",
            json!({"batch": batch, "target": target, "text": "From the recording"}),
        );
        let event = call(
            host,
            "add_event",
            json!({"batch": batch, "person": person, "kind": "residence", "place": "Poltava"}),
        );
        let name = call(
            host,
            "add_name",
            json!({"batch": batch, "person": person, "kind": "aka", "given": "Ivanko"}),
        );
        let family = call(
            host,
            "add_family",
            json!({"batch": batch, "partner1": person}),
        );
        let marker = call(
            host,
            "add_sibling_marker",
            json!({"batch": batch, "family": family["id"]}),
        );
        call(
            host,
            "end_batch",
            json!({"batch": batch, "report": "All added."}),
        );
        vec![
            person,
            source["id"].clone(),
            citation["id"].clone(),
            note["id"].clone(),
            event["id"].clone(),
            name["id"].clone(),
            family["id"].clone(),
            marker["id"].clone(),
        ]
    };
    let first = run(&host);
    let added = records(&host);
    // Closing a batch tells the app too, naming nobody: that is no write.
    let writes = |host: &TestHost| {
        let changes = host.changes.lock().unwrap();
        changes.iter().filter(|(ids, _)| !ids.is_empty()).count()
    };
    let before = writes(&host);
    assert_eq!(run(&host), first);
    assert_eq!(records(&host), added);
    assert_eq!(writes(&host), before);
}

#[test]
fn a_repeat_says_already_there_for_each_kind_of_addition() {
    let host = empty_host();
    let batch = begin(&host);
    let person = add(&host, &batch, "Ivan");
    let target = json!({"kind": "person", "id": person});
    let source = call(
        &host,
        "add_source",
        json!({"batch": batch, "title": "Recording 2"}),
    )["id"]
        .clone();
    let family = call(
        &host,
        "add_family",
        json!({"batch": batch, "partner1": person}),
    )["id"]
        .clone();
    for (tool, args) in [
        ("add_source", json!({"title": "Recording 2"})),
        ("add_note", json!({"target": target, "text": "A note"})),
        ("add_citation", json!({"target": target, "source": source})),
        (
            "add_event",
            json!({"person": person, "kind": "residence", "date": "1901"}),
        ),
        (
            "add_name",
            json!({"person": person, "kind": "aka", "given": "Ivanko"}),
        ),
        ("add_family", json!({"partner1": person})),
        ("add_sibling_marker", json!({"family": family})),
        (
            "set_number",
            json!({"person": person, "source_file": "a", "number": "1"}),
        ),
    ] {
        let mut args = args;
        args["batch"] = batch.clone();
        let first = call(&host, tool, args.clone());
        let again = call(&host, tool, args);
        assert_eq!(again["alreadyThere"], true, "{tool}: {again}");
        assert_eq!(again["id"], first["id"], "{tool}");
    }
}

#[test]
fn a_closed_batch_takes_no_more_and_keeps_its_report() {
    let host = empty_host();
    let batch = begin(&host);
    let report = "Added Ivan. Asked the family about his brothers.";
    call(
        &host,
        "end_batch",
        json!({"batch": batch, "report": report}),
    );
    let message = call_err(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Ivan"}),
    );
    assert!(message.contains("closed"), "{message}");
    let batch_id: Uuid = serde_json::from_value(batch).unwrap();
    with(&host, |s| {
        let stored = s.project().batch_get(batch_id).unwrap().unwrap();
        assert_eq!(stored.report.as_deref(), Some(report));
    });
}

#[test]
fn every_write_is_logged_against_the_key_and_its_batch() {
    let host = empty_host();
    let batch = begin(&host);
    let other_batch = begin(&host);
    let id = add(&host, &batch, "Ivan");
    let target = json!({"kind": "person", "id": id});
    call(
        &host,
        "add_note",
        json!({"batch": batch, "target": target, "text": "A note"}),
    );
    call(
        &host,
        "set_sex",
        json!({"batch": batch, "person": id, "sex": "male"}),
    );
    add(&host, &other_batch, "Olena");

    let log = call(&host, "get_log", json!({"filters": {"batch": batch}}));
    assert_eq!(log["total"], 3, "{log}");
    for row in log["items"].as_array().unwrap() {
        assert_eq!(row["actor"]["name"], "Transcript reader", "{row}");
        assert_eq!(row["actor"]["id"], json!(ctx().id), "{row}");
        assert_eq!(row["batch"]["id"], batch, "{row}");
    }
    let everything = call(&host, "get_log", json!({"filters": {"actorKind": "agent"}}));
    assert_eq!(everything["total"], 4);
}

#[test]
fn no_tool_takes_alive_or_dead() {
    let host = empty_host();
    let list = handle_message(&host, &ctx(), &request("tools/list", json!({}))).unwrap();
    for tool in list["result"]["tools"].as_array().unwrap() {
        let schema = tool["inputSchema"].to_string().to_lowercase();
        for word in ["living", "alive", "deceased", "dead", "override"] {
            assert!(!schema.contains(word), "{}: {word}", tool["name"]);
        }
    }

    // Given anyway, it is not read.
    let batch = begin(&host);
    let id = add(&host, &batch, "Ivan");
    let parsed: Uuid = serde_json::from_value(id.clone()).unwrap();
    let was = call(&host, "get_person", json!({"id": id}))["living"].clone();
    for value in ["deceased", "living"] {
        call(
            &host,
            "set_sex",
            json!({"batch": batch, "person": id, "sex": "male", "living_override": value, "livingOverride": value}),
        );
    }
    with(&host, |s| {
        assert!(s.tree().persons[&parsed].living_override.is_none());
    });
    assert_eq!(call(&host, "get_person", json!({"id": id}))["living"], was);
    let new = call(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Olena", "living_override": "deceased"}),
    )["id"]
        .clone();
    let new: Uuid = serde_json::from_value(new).unwrap();
    with(&host, |s| {
        assert!(s.tree().persons[&new].living_override.is_none());
    });
}

#[test]
fn a_write_that_would_change_alive_or_dead_is_refused() {
    let host = empty_host();
    let id = owners_person(&host, "Ivan", None);
    let batch = begin(&host);
    let before = records(&host);
    let was = call(&host, "get_person", json!({"id": id}))["living"].clone();
    assert_eq!(was, true);
    for (kind, date) in [("death", "1950"), ("burial", "1950"), ("birth", "1801")] {
        let message = call_err(
            &host,
            "add_event",
            json!({"batch": batch, "person": id, "kind": kind, "date": date}),
        );
        assert!(message.contains("alive or dead"), "{kind}: {message}");
    }
    assert_eq!(call(&host, "get_person", json!({"id": id}))["living"], was);
    assert_eq!(records(&host), before);
}

#[test]
fn unnamed_brothers_and_sisters_take_the_marker_and_nothing_else() {
    let host = empty_host();
    let batch = begin(&host);
    let child = add(&host, &batch, "Olena");
    let father = call(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Ivan", "surnames": ["Koval"], "relation": {"to": child, "as": "father"}}),
    );
    let family = father["family"].clone();
    assert!(family.is_string(), "{father}");
    let marker = call(
        &host,
        "add_sibling_marker",
        json!({"batch": batch, "family": family}),
    );
    let id = marker["id"].clone();
    assert_eq!(marker["family"], family, "{marker}");

    let shown = call(&host, "get_family", json!({"id": family}));
    let entry = shown["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["person"]["id"] == id)
        .unwrap()
        .clone();
    assert_eq!(entry["person"]["siblingMarker"], true);

    assert!(call_err(&host, "get_person", json!({"id": id})).contains("no details"));
    for (tool, args) in [
        (
            "add_name",
            json!({"person": id, "kind": "aka", "given": "Petro"}),
        ),
        (
            "add_event",
            json!({"person": id, "kind": "birth", "date": "1901"}),
        ),
        ("set_sex", json!({"person": id, "sex": "male"})),
    ] {
        let mut args = args;
        args["batch"] = batch.clone();
        let message = call_err(&host, tool, args);
        assert!(message.contains("nobody named"), "{tool}: {message}");
    }
    // It is nobody's relative to link to.
    let message = call_err(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Petro", "relation": {"to": id, "as": "sibling"}}),
    );
    assert!(message.contains("nobody named"), "{message}");

    let person_description = tools_description(&host, "add_person");
    assert!(
        person_description.contains("add_sibling_marker"),
        "{person_description}"
    );
    let marker_description = tools_description(&host, "add_sibling_marker");
    assert!(
        marker_description.contains("no names or dates"),
        "{marker_description}"
    );
}

fn tools_description(host: &TestHost, name: &str) -> String {
    let list = handle_message(host, &ctx(), &request("tools/list", json!({}))).unwrap();
    list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == name)
        .unwrap()["description"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn the_descriptions_carry_the_owners_rules() {
    let host = empty_host();
    for tool in ["add_person", "add_event", "add_note", "add_name"] {
        let text = tools_description(&host, tool);
        assert!(text.contains("note"), "{tool}: {text}");
    }
    for (tool, word) in [
        ("add_person", "Search"),
        ("add_person", "no names"),
        ("add_event", "Never invent"),
        ("add_event", "alive or dead"),
        ("set_sex", "alive or dead"),
        ("add_citation", "quality"),
        ("begin_batch", "first"),
        ("end_batch", "report"),
        ("set_number", "find_by_number"),
        ("update_own_note", "your own"),
        ("update_own_event", "your own"),
        ("update_own_name", "your own"),
        ("update_own_citation", "your own"),
    ] {
        let text = tools_description(&host, tool);
        assert!(text.contains(word), "{tool}: {word}: {text}");
    }
    for (rule, words) in [(5, "notes"), (6, "marker")] {
        assert!(gt_mcp::RULES.contains(words), "rule {rule}");
    }
}

#[test]
fn update_own_changes_what_this_agent_added_and_only_that() {
    let host = empty_host();
    let owners = owners_person(&host, "Ivan", Some("1901"));
    let (owners_note, owners_name, owners_event) = with(&host, |s| {
        add_note(s, owners, "The owner's note");
        let tree = s.tree();
        (
            *tree.notes.keys().next().unwrap(),
            tree.persons[&owners].names[0].id,
            *tree.events.keys().next().unwrap(),
        )
    });
    let batch = begin(&host);
    let target = json!({"kind": "person", "id": owners});
    let source = call(
        &host,
        "add_source",
        json!({"batch": batch, "title": "Recording 2"}),
    )["id"]
        .clone();

    // Notes.
    let note = call(
        &host,
        "add_note",
        json!({"batch": batch, "target": target, "text": "First hearing", "kind": "research"}),
    )["id"]
        .clone();
    let changed = call(
        &host,
        "update_own_note",
        json!({"batch": batch, "note": note, "text": "Second hearing"}),
    );
    assert_eq!(changed["id"], note);
    let read = call(&host, "get_note", json!({"id": note}));
    assert_eq!(read["text"], "Second hearing");
    assert_eq!(read["kind"], "research", "the kind stays unless given");

    // Events: what is not given stays.
    let event = call(
        &host,
        "add_event",
        json!({
            "batch": batch,
            "person": owners,
            "kind": "residence",
            "date": "1920",
            "place": "Poltava",
            "description": "With his brother",
            "age": "19",
            "cause": "Work",
        }),
    )["id"]
        .clone();
    call(
        &host,
        "update_own_event",
        json!({"batch": batch, "event": event, "description": "With his sister"}),
    );
    let read = call(&host, "get_event", json!({"id": event}));
    assert_eq!(read["description"], "With his sister");
    assert_eq!(read["kind"], "residence");
    assert_eq!(read["date"]["raw"], "1920", "{read}");
    assert_eq!(read["age"], "19");
    assert_eq!(read["cause"], "Work");
    assert!(
        read["place"].is_object() || read["place"].is_string(),
        "{read}"
    );
    call(
        &host,
        "update_own_event",
        json!({"batch": batch, "event": event, "date": "1921", "kind": "custom:Lived there"}),
    );
    let read = call(&host, "get_event", json!({"id": event}));
    assert_eq!(read["date"]["raw"], "1921");
    assert_eq!(read["description"], "With his sister");
    assert_eq!(read["kind"], "custom:Lived there", "{read}");

    // Names: what is not given stays.
    let name = call(
        &host,
        "add_name",
        json!({"batch": batch, "person": owners, "kind": "aka", "given": "Ivanko", "surnames": ["Kovalenko"], "nickname": "Vanya"}),
    )["id"]
        .clone();
    call(
        &host,
        "update_own_name",
        json!({"batch": batch, "person": owners, "name": name, "given": "Ivanko Petrovych"}),
    );
    let person = call(&host, "get_person", json!({"id": owners}));
    let row = person["names"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == name)
        .unwrap();
    assert_eq!(row["given"], "Ivanko Petrovych");
    assert_eq!(row["surnames"], json!(["Kovalenko"]));
    assert_eq!(row["nickname"], "Vanya");
    assert_eq!(row["kind"], "aka");
    assert_eq!(row["preferred"], false);
    assert_eq!(person["names"][0]["preferred"], true, "{person}");

    // Citations.
    let citation = call(
        &host,
        "add_citation",
        json!({"batch": batch, "target": target, "source": source, "page": "12:40", "quality": 1}),
    )["id"]
        .clone();
    call(
        &host,
        "update_own_citation",
        json!({"batch": batch, "citation": citation, "quality": 2, "text": "Heard again"}),
    );
    let read = call(&host, "get_citation", json!({"id": citation}));
    assert_eq!(read["quality"], 2);
    assert_eq!(read["page"], "12:40");
    assert_eq!(read["text"], "Heard again");

    // What the owner wrote stays as it is.
    for (tool, args) in [
        (
            "update_own_note",
            json!({"note": owners_note, "text": "Changed"}),
        ),
        (
            "update_own_event",
            json!({"event": owners_event, "date": "1902"}),
        ),
        (
            "update_own_name",
            json!({"person": owners, "name": owners_name, "given": "Changed"}),
        ),
    ] {
        let mut args = args;
        args["batch"] = batch.clone();
        let message = call_err(&host, tool, args);
        assert!(message.contains("did not create"), "{tool}: {message}");
    }
    let shown = call(&host, "get_note", json!({"id": owners_note}));
    assert_eq!(shown["text"], "The owner's note");
    for (tool, args) in [
        (
            "update_own_note",
            json!({"note": Uuid::from_u128(9), "text": "x"}),
        ),
        ("update_own_event", json!({"event": Uuid::from_u128(9)})),
        (
            "update_own_name",
            json!({"person": owners, "name": Uuid::from_u128(9)}),
        ),
    ] {
        let mut args = args;
        args["batch"] = batch.clone();
        call_err(&host, tool, args);
    }
}

#[test]
fn a_note_goes_on_whatever_a_note_can_go_on_and_is_updated_there() {
    let host = empty_host();
    let batch = begin(&host);
    let person = call(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Ivan", "birth_date": "1901"}),
    )["id"]
        .clone();
    let family = call(
        &host,
        "add_family",
        json!({"batch": batch, "partner1": person}),
    )["id"]
        .clone();
    let event = call(
        &host,
        "add_event",
        json!({"batch": batch, "family": family, "kind": "marriage", "date": "1925"}),
    );
    let name = call(&host, "get_person", json!({"id": person}))["names"][0]["id"].clone();
    let source = call(
        &host,
        "add_source",
        json!({"batch": batch, "title": "Recording 2"}),
    )["id"]
        .clone();
    let citation = call(
        &host,
        "add_citation",
        json!({"batch": batch, "target": {"kind": "family", "id": family}, "source": source}),
    );
    assert_ne!(citation["id"], family);
    let on_name = call(
        &host,
        "add_citation",
        json!({"batch": batch, "target": {"kind": "name", "id": name}, "source": source, "page": "1"}),
    );
    assert_ne!(on_name["id"], name);
    for (kind, id) in [
        ("person", &person),
        ("family", &family),
        ("event", &event["id"]),
        ("name", &name),
        ("citation", &citation["id"]),
    ] {
        let target = json!({"kind": kind, "id": id});
        let note = call(
            &host,
            "add_note",
            json!({"batch": batch, "target": target, "text": format!("On a {kind}")}),
        )["id"]
            .clone();
        let answer = call(
            &host,
            "update_own_note",
            json!({"batch": batch, "note": note, "text": format!("Still on a {kind}")}),
        );
        assert_eq!(answer["id"], note, "{kind}");
        assert_eq!(
            call(&host, "get_note", json!({"id": note}))["text"],
            format!("Still on a {kind}")
        );
    }
}

#[test]
fn add_person_places_someone_by_relation() {
    let host = empty_host();
    let batch = begin(&host);
    let child = add(&host, &batch, "Olena");
    let relation = |to: &Value, kind: &str| json!({"to": to, "as": kind});

    let father = call(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Ivan", "relation": relation(&child, "father")}),
    );
    let family = father["family"].clone();
    assert_eq!(
        call(&host, "get_person", json!({"id": father["id"]}))["sex"],
        "male"
    );
    let up = call(&host, "get_relatives", json!({"id": child}));
    assert_eq!(up["parents"][0]["id"], father["id"]);
    assert_eq!(up["parents"][0]["familyId"], family);

    let sibling = call(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Petro", "relation": relation(&child, "sibling")}),
    );
    assert_eq!(sibling["family"], family);

    // A second father by birth is refused, in the gate's words, and a stepfather is not.
    let message = call_err(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Oleh", "relation": relation(&child, "father")}),
    );
    assert!(message.contains("already has a father"), "{message}");
    assert!(message.contains("step"), "{message}");
    let mother = call(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Oksana", "relation": relation(&child, "mother")}),
    );
    let step = call(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Oleh", "relation": {"to": child, "as": "father", "step": true}}),
    );
    let up = call(&host, "get_relatives", json!({"id": child}));
    let parents = ids_of(&up["parents"]);
    for parent in [&father["id"], &mother["id"], &step["id"]] {
        assert!(parents.contains(parent), "{up}");
    }

    // An adopted child, of the family it was taken into.
    let adopted = call(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Marta", "relation": {"to": father["id"], "as": "child", "link": "adopted", "family": family}}),
    );
    assert_eq!(adopted["family"], family);
    let shown = call(&host, "get_family", json!({"id": family}));
    let entry = shown["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["person"]["id"] == adopted["id"])
        .unwrap()
        .clone();
    assert_eq!(entry["rel"], json!(["adopted", "adopted"]), "{entry}");

    for (relation, word) in [
        (json!({"as": "father"}), "relation.to"),
        (json!({"to": child}), "relation.as"),
        (json!({"to": child, "as": "cousin"}), "relation.as"),
        (
            json!({"to": child, "as": "father", "step": true, "link": "adopted"}),
            "step",
        ),
        (
            json!({"to": child, "as": "father", "link": "uncle"}),
            "relation.link",
        ),
        (json!("father"), "relation"),
    ] {
        let message = call_err(
            &host,
            "add_person",
            json!({"batch": batch, "given": "Nobody", "relation": relation}),
        );
        assert!(message.contains(word), "{word}: {message}");
    }
}

#[test]
fn link_people_connects_who_is_already_there() {
    let host = empty_host();
    let batch = begin(&host);
    let child = add(&host, &batch, "Olena");
    let father = add(&host, &batch, "Ivan");
    let mother = add(&host, &batch, "Oksana");
    let stepfather = add(&host, &batch, "Oleh");
    let second_child = add(&host, &batch, "Petro");
    let third_child = add(&host, &batch, "Marta");

    let linked = call(
        &host,
        "link_people",
        json!({"batch": batch, "person": father, "relation": {"to": child, "as": "father"}}),
    );
    let family = linked["family"].clone();
    assert_eq!(linked["id"], father);
    call(
        &host,
        "link_people",
        json!({"batch": batch, "person": mother, "relation": {"to": child, "as": "mother"}}),
    );
    call(
        &host,
        "link_people",
        json!({"batch": batch, "person": stepfather, "relation": {"to": child, "as": "father", "step": true}}),
    );
    let up = call(&host, "get_relatives", json!({"id": child}));
    let parents = ids_of(&up["parents"]);
    for parent in [&father, &mother, &stepfather] {
        assert!(parents.contains(parent), "{up}");
    }

    // Straight into a family, with no one to name: a child of both partners.
    let joined = call(
        &host,
        "link_people",
        json!({"batch": batch, "person": second_child, "relation": {"as": "child", "family": family}}),
    );
    assert_eq!(joined["id"], second_child);
    assert_eq!(joined["family"], family);
    call(
        &host,
        "link_people",
        json!({"batch": batch, "person": third_child, "relation": {"as": "child", "family": family, "link": "foster"}}),
    );
    let shown = call(&host, "get_family", json!({"id": family}));
    let rel_of = |person: &Value| {
        shown["children"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["person"]["id"] == *person)
            .unwrap()["rel"]
            .clone()
    };
    assert_eq!(rel_of(&second_child), json!(["birth", "birth"]), "{shown}");
    assert_eq!(rel_of(&third_child), json!(["foster", "foster"]), "{shown}");

    // The same person twice in a family is no new link.
    let again = call(
        &host,
        "link_people",
        json!({"batch": batch, "person": second_child, "relation": {"as": "child", "family": family}}),
    );
    assert_eq!(again["alreadyThere"], true, "{again}");
    assert_eq!(again["id"], second_child);

    for (relation, word) in [
        (json!({"as": "father"}), "relation.to"),
        (json!({"as": "child"}), "relation.to"),
        (
            json!({"as": "child", "family": family, "step": true}),
            "step",
        ),
    ] {
        let message = call_err(
            &host,
            "link_people",
            json!({"batch": batch, "person": second_child, "relation": relation}),
        );
        assert!(message.contains(word), "{word}: {message}");
    }
}

#[test]
fn a_child_linked_to_a_second_family_keeps_the_first_as_preferred() {
    let host = empty_host();
    let batch = begin(&host);
    let child = add(&host, &batch, "Olena");
    let foster = call(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Oleh", "sex": "male", "relation": {"to": child, "as": "father", "link": "foster"}}),
    );
    let father = add(&host, &batch, "Ivan");
    let birth_family = call(
        &host,
        "add_family",
        json!({"batch": batch, "partner1": father}),
    )["id"]
        .clone();
    call(
        &host,
        "link_people",
        json!({"batch": batch, "person": child, "relation": {"as": "child", "family": birth_family}}),
    );
    let child_id: Uuid = serde_json::from_value(child).unwrap();
    let first: Uuid = serde_json::from_value(foster["family"].clone()).unwrap();
    let second: Uuid = serde_json::from_value(birth_family).unwrap();
    with(&host, |s| {
        let preferred = |family: &Uuid| {
            s.tree().families[family]
                .children
                .iter()
                .find(|c| c.person == child_id)
                .unwrap()
                .preferred
        };
        assert!(preferred(&first));
        assert!(!preferred(&second));
    });
}

#[test]
fn a_child_with_a_birth_family_goes_into_no_other_by_family_alone() {
    let host = empty_host();
    let batch = begin(&host);
    let child = add(&host, &batch, "Olena");
    call(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Ivan", "relation": {"to": child, "as": "father"}}),
    );
    let foster = add(&host, &batch, "Oleh");
    let other = call(
        &host,
        "add_family",
        json!({"batch": batch, "partner1": foster}),
    )["id"]
        .clone();
    let message = call_err(
        &host,
        "link_people",
        json!({"batch": batch, "person": child, "relation": {"as": "child", "family": other, "link": "foster"}}),
    );
    assert!(message.contains("already has a birth family"), "{message}");
    // Naming the foster parent says what the link is, and that passes.
    let done = call(
        &host,
        "link_people",
        json!({"batch": batch, "person": child, "relation": {"to": foster, "as": "child", "family": other, "link": "foster"}}),
    );
    assert_eq!(done["family"], other);
}

#[test]
fn arguments_are_checked_before_anything_is_written() {
    let host = empty_host();
    let batch = begin(&host);
    let id = add(&host, &batch, "Ivan");
    let source = call(
        &host,
        "add_source",
        json!({"batch": batch, "title": "Recording 2"}),
    )["id"]
        .clone();
    let target = json!({"kind": "person", "id": id});
    let over = "x".repeat(1025);
    let before = records(&host);
    for (tool, args, word) in [
        ("add_person", json!({"batch": "nonsense"}), "begin_batch"),
        ("add_person", json!({"given": over}), "1 KiB"),
        ("add_person", json!({"surnames": "Koval"}), "surnames"),
        ("add_person", json!({"surnames": [1]}), "surnames"),
        ("add_person", json!({"surnames": [over]}), "1 KiB"),
        ("add_person", json!({"sex": "robot"}), "sex"),
        (
            "add_family",
            json!({"partner1": id, "kind": "friends"}),
            "kind",
        ),
        ("add_family", json!({"partner1": "nonsense"}), "partner1"),
        ("add_family", json!({}), "someone"),
        ("add_name", json!({"person": id, "kind": "alias"}), "kind"),
        (
            "add_name",
            json!({"person": id, "kind": "aka"}),
            "given name",
        ),
        (
            "add_event",
            json!({"kind": "residence"}),
            "person or a family",
        ),
        (
            "add_event",
            json!({"person": id, "family": id, "kind": "residence"}),
            "person or a family",
        ),
        ("add_event", json!({"person": id}), "kind"),
        (
            "add_event",
            json!({"person": id, "kind": "wedding party"}),
            "kind",
        ),
        ("add_note", json!({"target": target}), "text"),
        (
            "add_note",
            json!({"target": target, "text": "x", "kind": "gossip"}),
            "kind",
        ),
        (
            "add_note",
            json!({"target": {"kind": "planet", "id": id}, "text": "x"}),
            "target",
        ),
        ("add_note", json!({"target": target, "text": "  "}), "text"),
        ("add_source", json!({}), "title"),
        ("add_citation", json!({"target": target}), "source"),
        (
            "add_citation",
            json!({"target": target, "source": source, "quality": 4}),
            "quality",
        ),
        (
            "add_citation",
            json!({"target": target, "source": source, "quality": "good"}),
            "quality",
        ),
        (
            "add_citation",
            json!({"target": target, "source": Uuid::from_u128(9)}),
            "source",
        ),
        (
            "set_number",
            json!({"person": id, "source_file": " ", "number": "1"}),
            "source file",
        ),
        ("set_sex", json!({"person": id, "sex": "robot"}), "sex"),
    ] {
        let mut args = args;
        if args.get("batch").is_none() {
            args["batch"] = batch.clone();
        }
        let message = call_err(&host, tool, args);
        assert!(message.contains(word), "{tool}: {word}: {message}");
    }
    assert_eq!(records(&host), before);
}

#[test]
fn long_writing_is_not_held_to_the_short_limit() {
    let host = empty_host();
    let batch = begin(&host);
    let id = add(&host, &batch, "Ivan");
    let target = json!({"kind": "person", "id": id});
    let long = "слово ".repeat(2_000);
    let note = call(
        &host,
        "add_note",
        json!({"batch": batch, "target": target, "text": long, "kind": "transcription"}),
    )["id"]
        .clone();
    assert_eq!(
        call(&host, "get_note", json!({"id": note}))["text"]
            .as_str()
            .unwrap()
            .trim(),
        long.trim()
    );
    let source = call(
        &host,
        "add_source",
        json!({"batch": batch, "title": "Recording 2", "author": "Olena Koval", "publication": "Family", "abbreviation": "R2", "text": long}),
    )["id"]
        .clone();
    let read = call(&host, "get_source", json!({"id": source}));
    assert_eq!(read["author"], "Olena Koval");
    assert_eq!(read["abbreviation"], "R2");
    assert_eq!(read["publication"], "Family");
    assert_eq!(read["text"].as_str().unwrap().trim(), long.trim());
    call(
        &host,
        "add_citation",
        json!({"batch": batch, "target": target, "source": source, "text": long}),
    );
    call(&host, "end_batch", json!({"batch": batch, "report": long}));
}

#[test]
fn set_sex_is_the_gates_to_decide() {
    let host = empty_host();
    let owners = owners_person(&host, "Ivan", None);
    let batch = begin(&host);
    let mine = add(&host, &batch, "Olena");
    let done = call(
        &host,
        "set_sex",
        json!({"batch": batch, "person": mine, "sex": "female"}),
    );
    assert_eq!(done["id"], mine);
    assert_eq!(
        call(&host, "get_person", json!({"id": mine}))["sex"],
        "female"
    );
    // What is on the owner's record stays, even a sex nobody has said.
    let message = call_err(
        &host,
        "set_sex",
        json!({"batch": batch, "person": owners, "sex": "female"}),
    );
    assert!(message.contains("did not create"), "{message}");
}

#[test]
fn the_owners_edit_between_two_writes_is_no_obstacle() {
    let host = empty_host();
    let batch = begin(&host);
    let id = add(&host, &batch, "Ivan");
    let parsed: Uuid = serde_json::from_value(id.clone()).unwrap();
    with(&host, |s| {
        s.apply(EditCommand::QuickAdd {
            anchor: parsed,
            relation: Relation::Partner,
            draft: draft("Olena", "Koval", None, None),
            family: None,
            new_parent_set: None,
            parent_rel: None,
        })
        .unwrap();
        s.apply(EditCommand::SaveNote {
            target: OwnerRef {
                kind: OwnerKind::Person,
                id: parsed,
            },
            note: gt_session::NoteInput {
                id: None,
                kind: gt_session::NoteKindDto::General,
                text: "The owner's".into(),
            },
        })
        .unwrap();
    });
    call(
        &host,
        "add_note",
        json!({"batch": batch, "target": {"kind": "person", "id": id}, "text": "The agent's"}),
    );
    assert_eq!(
        call(&host, "get_person", json!({"id": id}))["notes"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn only_the_tools_named_here_can_write() {
    // A tool that is not named here must be argued for: the gate takes the rest.
    let host = empty_host();
    let list = handle_message(&host, &ctx(), &request("tools/list", json!({}))).unwrap();
    let writes: Vec<&str> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .filter(|n| {
            !n.starts_with("get_")
                && !n.starts_with("list_")
                && !n.starts_with("search_")
                && !n.starts_with("find_")
                && *n != "tree_summary"
        })
        .collect();
    assert_eq!(
        writes,
        [
            "begin_batch",
            "end_batch",
            "add_person",
            "link_people",
            "add_family",
            "add_name",
            "remove_own_empty_family",
            "add_event",
            "add_note",
            "add_source",
            "add_citation",
            "add_sibling_marker",
            "set_number",
            "set_sex",
            "update_own_note",
            "update_own_event",
            "update_own_name",
            "update_own_citation",
        ]
    );
}
