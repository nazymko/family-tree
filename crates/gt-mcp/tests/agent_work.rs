#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! What a real agent runs into: a family that is not there yet, a parent it did not name, a second
//! run over the same file, text of any size, dates that read two ways.

mod common;

use common::*;
use gt_mcp::{AgentCtx, Host, handle_message};
use gt_session::{ApiError, EditCommand, ParentRelId, Relation, Session, SexKind};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use uuid::Uuid;

fn begin(host: &TestHost) -> Value {
    call(host, "begin_batch", json!({"title": "Recording 2"}))["batch"].clone()
}

fn owners(host: &TestHost, given: &str, sex: SexKind, born: Option<&str>) -> Uuid {
    let mut d = draft(given, "Koval", None, born);
    d.sex = sex;
    with(host, |s| add_person(s, d))
}

fn counts(host: &TestHost) -> [usize; 6] {
    with(host, |s| {
        let t = s.tree();
        [
            t.persons.len(),
            t.families.len(),
            t.events.len(),
            t.notes.len(),
            t.sources.len(),
            t.citations.len(),
        ]
    })
}

fn log_rows(host: &TestHost) -> u64 {
    call(host, "get_log", json!({}))["total"].as_u64().unwrap()
}

fn on_person(id: Uuid) -> Value {
    json!({"kind": "person", "id": id})
}

// ---- the marker for a person with no family yet

#[test]
fn marker_beside_someone_with_no_parents_makes_a_family() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, Some("1901"));
    let batch = begin(&host);
    let rows = log_rows(&host);
    let first = call(
        &host,
        "add_sibling_marker",
        json!({"batch": batch, "sibling_of": ivan}),
    );
    assert_eq!(first["alreadyThere"], Value::Null, "{first}");
    assert_eq!(log_rows(&host), rows + 1, "one group in the log");

    let family = call(&host, "get_family", json!({"id": first["family"]}));
    assert_eq!(family["partners"], json!([null, null]), "{family}");
    let children = family["children"].as_array().unwrap();
    assert_eq!(children.len(), 2, "{family}");
    assert!(children.iter().any(|c| c["person"]["id"] == json!(ivan)));
    assert!(children.iter().any(|c| c["person"]["id"] == first["id"]));
    let relatives = call(&host, "get_relatives", json!({"id": ivan}));
    assert_eq!(
        relatives["siblings"][0]["siblingMarker"], true,
        "{relatives}"
    );

    let before = counts(&host);
    let rerun = call(
        &host,
        "add_sibling_marker",
        json!({"batch": batch, "sibling_of": ivan}),
    );
    assert_eq!(rerun["alreadyThere"], true, "{rerun}");
    assert_eq!(rerun["id"], first["id"]);
    assert_eq!(counts(&host), before);
    assert_eq!(log_rows(&host), rows + 1);
}

#[test]
fn marker_beside_someone_uses_their_birth_family() {
    let host = empty_host();
    let father = owners(&host, "Petro", SexKind::Male, Some("1870"));
    let ivan = with(&host, |s| {
        add_relative(
            s,
            father,
            Relation::Child,
            draft("Ivan", "Koval", None, Some("1901")),
        )
    });
    let family = with(&host, |s| *s.tree().families.keys().next().unwrap());
    let batch = begin(&host);
    let answer = call(
        &host,
        "add_sibling_marker",
        json!({"batch": batch, "sibling_of": ivan}),
    );
    assert_eq!(answer["family"], json!(family), "{answer}");
    with(&host, |s| assert_eq!(s.tree().families.len(), 1));
    let by_family = call(
        &host,
        "add_sibling_marker",
        json!({"batch": batch, "family": family}),
    );
    assert_eq!(by_family["alreadyThere"], true);
    assert_eq!(by_family["id"], answer["id"]);
}

#[test]
fn marker_beside_someone_with_two_birth_families_asks_which() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, Some("1901"));
    let father = owners(&host, "Petro", SexKind::Male, Some("1870"));
    let other = owners(&host, "Pavlo", SexKind::Male, Some("1871"));
    for parent in [father, other] {
        with(&host, |s| {
            s.apply(EditCommand::LinkExisting {
                anchor: ivan,
                relation: Relation::Father,
                person: parent,
                family: None,
                new_parent_set: Some(true),
                parent_rel: Some(ParentRelId::Birth),
            })
            .unwrap();
        });
    }
    let batch = begin(&host);
    let before = counts(&host);
    let message = call_err(
        &host,
        "add_sibling_marker",
        json!({"batch": batch, "sibling_of": ivan}),
    );
    assert!(message.contains("family"), "{message}");
    assert_eq!(counts(&host), before);
}

#[test]
fn marker_takes_one_of_family_and_sibling_of() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, None);
    let batch = begin(&host);
    let none = call_err(&host, "add_sibling_marker", json!({"batch": batch}));
    assert!(none.contains("sibling_of"), "{none}");
    let both = call_err(
        &host,
        "add_sibling_marker",
        json!({"batch": batch, "family": ivan, "sibling_of": ivan}),
    );
    assert!(both.contains("sibling_of"), "{both}");
    let unknown = call_err(
        &host,
        "add_sibling_marker",
        json!({"batch": batch, "sibling_of": Uuid::from_u128(9)}),
    );
    assert!(
        unknown.contains("not found") || unknown.contains("No "),
        "{unknown}"
    );
    // Beside a marker there is nobody to be a sibling of.
    let marker = call(
        &host,
        "add_sibling_marker",
        json!({"batch": batch, "sibling_of": ivan}),
    );
    let message = call_err(
        &host,
        "add_sibling_marker",
        json!({"batch": batch, "sibling_of": marker["id"]}),
    );
    assert!(!message.contains("Delete"), "{message}");
}

// ---- a parent nobody named

#[test]
fn a_child_of_someone_with_a_partner_says_who_became_a_parent() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, Some("1901"));
    let olena = with(&host, |s| {
        add_relative(
            s,
            ivan,
            Relation::Partner,
            draft("Olena", "Koval", None, Some("1905")),
        )
    });
    let batch = begin(&host);
    let new = call(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Mykola", "relation": {"to": ivan, "as": "child"}}),
    );
    let parents = new["parents"].as_array().unwrap();
    assert_eq!(parents.len(), 2, "{new}");
    let names: Vec<&str> = parents
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert!(names.iter().any(|n| n.contains("Olena")), "{names:?}");
    assert!(
        parents
            .iter()
            .any(|p| p["id"] == json!(olena) && p["link"] == "birth"),
        "{new}"
    );

    let linked = owners(&host, "Zina", SexKind::Female, Some("1931"));
    let answer = call(
        &host,
        "link_people",
        json!({"batch": batch, "person": linked, "relation": {"to": ivan, "as": "child"}}),
    );
    assert_eq!(answer["parents"].as_array().unwrap().len(), 2, "{answer}");

    let tools = handle_message(&host, &ctx(), &request("tools/list", json!({}))).unwrap();
    for name in ["add_person", "link_people"] {
        let tool = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap();
        let text = tool["description"].as_str().unwrap();
        assert!(text.contains("add_family"), "{name}: {text}");
        assert!(text.contains("parents"), "{name}: {text}");
    }
}

#[test]
fn a_family_of_one_keeps_the_partner_out() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, Some("1901"));
    with(&host, |s| {
        add_relative(
            s,
            ivan,
            Relation::Partner,
            draft("Olena", "Koval", None, Some("1905")),
        )
    });
    let batch = begin(&host);
    let alone = call(
        &host,
        "add_family",
        json!({"batch": batch, "partner1": ivan}),
    );
    let new = call(
        &host,
        "add_person",
        json!({
            "batch": batch,
            "given": "Mykola",
            "relation": {"to": ivan, "as": "child", "family": alone["id"]},
        }),
    );
    let parents = new["parents"].as_array().unwrap();
    assert_eq!(parents.len(), 1, "{new}");
    assert_eq!(parents[0]["id"], json!(ivan));
}

// ---- reruns

#[test]
fn a_rerun_with_the_number_adds_nobody() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, Some("1901"));
    let batch = begin(&host);
    let args = json!({
        "batch": batch,
        "given": "Mykola",
        "source_file": "rec2",
        "number": "7",
        "birth_date": "1930",
        "relation": {"to": ivan, "as": "child"},
    });
    let rows = log_rows(&host);
    let first = call(&host, "add_person", args.clone());
    assert_eq!(log_rows(&host), rows + 1, "the number is in the same group");
    assert_eq!(
        call(
            &host,
            "find_by_number",
            json!({"source_file": "rec2", "number": "7"})
        )["person"],
        first["id"]
    );
    let before = counts(&host);
    let writes = host.changes.lock().unwrap().len();
    let again = call(&host, "add_person", args);
    assert_eq!(again["alreadyThere"], true, "{again}");
    assert_eq!(again["id"], first["id"]);
    assert_eq!(counts(&host), before);
    assert_eq!(log_rows(&host), rows + 1);
    assert_eq!(host.changes.lock().unwrap().len(), writes);
    // Alone too, and with spaces around the number.
    let alone = call(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Mykola", "source_file": " rec2 ", "number": " 7 "}),
    );
    assert_eq!(alone["id"], first["id"]);
    assert_eq!(counts(&host), before);
}

#[test]
fn half_a_number_is_refused() {
    let host = empty_host();
    let batch = begin(&host);
    for args in [
        json!({"batch": batch, "given": "Mykola", "source_file": "rec2"}),
        json!({"batch": batch, "given": "Mykola", "number": "7"}),
        json!({"batch": batch, "given": "Mykola", "source_file": "  ", "number": "7"}),
    ] {
        let message = call_err(&host, "add_person", args);
        assert!(message.contains("number"), "{message}");
    }
    assert_eq!(counts(&host)[0], 0);
}

#[test]
fn a_refused_person_keeps_no_number() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, Some("1901"));
    with(&host, |s| {
        add_relative(
            s,
            ivan,
            Relation::Father,
            draft("Petro", "Koval", None, Some("1870")),
        )
    });
    let batch = begin(&host);
    call_err(
        &host,
        "add_person",
        json!({
            "batch": batch, "given": "Pavlo", "sex": "male", "source_file": "rec2", "number": "2",
            "relation": {"to": ivan, "as": "father"},
        }),
    );
    let found = call(
        &host,
        "find_by_number",
        json!({"source_file": "rec2", "number": "2"}),
    );
    assert_eq!(found["person"], Value::Null);
}

#[test]
fn numbers_are_asked_for_everywhere_an_agent_reads() {
    let host = empty_host();
    let init = handle_message(
        &host,
        &ctx(),
        &request(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}),
        ),
    )
    .unwrap();
    let instructions = init["result"]["instructions"].as_str().unwrap();
    assert!(
        instructions.starts_with(gt_mcp::RULES),
        "rules stay as they are"
    );
    assert!(instructions.contains("source_file"), "{instructions}");
    let tools = handle_message(&host, &ctx(), &request("tools/list", json!({}))).unwrap();
    let add = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "add_person")
        .unwrap()
        .clone();
    assert!(add["description"].as_str().unwrap().contains("every time"));
    assert!(
        add["inputSchema"]["properties"]
            .get("source_file")
            .is_some()
    );
    assert!(add["inputSchema"]["properties"].get("number").is_some());
}

#[test]
fn relinking_says_already_there_in_every_form() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, Some("1901"));
    let olena = owners(&host, "Olena", SexKind::Female, Some("1905"));
    let petro = owners(&host, "Petro", SexKind::Male, Some("1930"));
    let maria = owners(&host, "Maria", SexKind::Female, Some("1932"));
    let stepan = owners(&host, "Stepan", SexKind::Male, Some("1933"));
    let batch = begin(&host);
    let links = [
        (olena, json!({"to": ivan, "as": "partner"})),
        (petro, json!({"to": ivan, "as": "child"})),
        (ivan, json!({"to": maria, "as": "father"})),
        (maria, json!({"to": stepan, "as": "sibling"})),
    ];
    for (person, relation) in &links {
        let args = json!({"batch": batch, "person": person, "relation": relation});
        call(&host, "link_people", args);
    }
    let before = counts(&host);
    let rows = log_rows(&host);
    for (person, relation) in &links {
        let args = json!({"batch": batch, "person": person, "relation": relation});
        let again = call(&host, "link_people", args);
        assert_eq!(again["alreadyThere"], true, "{relation}: {again}");
        assert_eq!(again["id"], json!(person));
    }
    assert_eq!(counts(&host), before);
    assert_eq!(log_rows(&host), rows);
}

#[test]
fn relinking_through_the_family_says_already_there() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, Some("1901"));
    let petro = owners(&host, "Petro", SexKind::Male, Some("1930"));
    let batch = begin(&host);
    let family = call(
        &host,
        "add_family",
        json!({"batch": batch, "partner1": ivan}),
    )["id"]
        .clone();
    let args =
        json!({"batch": batch, "person": petro, "relation": {"as": "child", "family": family}});
    call(&host, "link_people", args.clone());
    let again = call(&host, "link_people", args);
    assert_eq!(again["alreadyThere"], true, "{again}");
    let by_person = call(
        &host,
        "link_people",
        json!({"batch": batch, "person": petro, "relation": {"to": ivan, "as": "child"}}),
    );
    assert_eq!(by_person["alreadyThere"], true, "{by_person}");
}

#[test]
fn another_father_is_still_refused() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, Some("1901"));
    let petro = owners(&host, "Petro", SexKind::Male, Some("1870"));
    let pavlo = owners(&host, "Pavlo", SexKind::Male, Some("1871"));
    let batch = begin(&host);
    let link = |person: Uuid| json!({"batch": batch, "person": person, "relation": {"to": ivan, "as": "father"}});
    call(&host, "link_people", link(petro));
    assert_eq!(
        call(&host, "link_people", link(petro))["alreadyThere"],
        true
    );
    let message = call_err(&host, "link_people", link(pavlo));
    assert!(message.contains("already has a father"), "{message}");
}

#[test]
fn relinking_as_something_else_is_no_repeat() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, Some("1901"));
    let petro = owners(&host, "Petro", SexKind::Male, Some("1870"));
    let batch = begin(&host);
    let link = |extra: &str| json!({"batch": batch, "person": petro, "relation": {"to": ivan, "as": "father", "link": extra}});
    call(&host, "link_people", link("birth"));
    let message = call_err(&host, "link_people", link("foster"));
    assert!(message.contains("already"), "{message}");
}

/// One recording worked through as an agent would, with its numbers, reading before it writes.
fn work_through(host: &TestHost, ivan: Uuid, olena: Uuid) {
    let batch = begin(host);
    let family = call(host, "get_relatives", json!({"id": ivan}))["parents"][0]["familyId"].clone();
    call(
        host,
        "add_sibling_marker",
        json!({"batch": batch, "family": family}),
    );
    call(
        host,
        "add_sibling_marker",
        json!({"batch": batch, "sibling_of": olena}),
    );
    let source = call(
        host,
        "add_source",
        json!({"batch": batch, "title": "Recording 2"}),
    )["id"]
        .clone();
    call(
        host,
        "add_citation",
        json!({"batch": batch, "target": on_person(ivan), "source": source, "page": "12:40", "quality": 2}),
    );
    call(
        host,
        "add_note",
        json!({"batch": batch, "target": on_person(ivan), "text": "The recording says Pavlovych; the tree has Petrovych."}),
    );
    call(
        host,
        "link_people",
        json!({"batch": batch, "person": olena, "relation": {"to": ivan, "as": "partner"}}),
    );
    let son = call(
        host,
        "add_person",
        json!({
            "batch": batch, "given": "Mykola", "surnames": ["Koval"], "sex": "male",
            "birth_date": "1930", "source_file": "rec2", "number": "1",
            "relation": {"to": ivan, "as": "child"},
        }),
    );
    call(
        host,
        "add_name",
        json!({"batch": batch, "person": son["id"], "kind": "nickname", "given": "Kolia"}),
    );
    let smith = call(
        host,
        "add_person",
        json!({
            "batch": batch, "sex": "male", "given": "Stepan", "source_file": "rec2", "number": "2",
            "relation": {"to": olena, "as": "father"},
        }),
    );
    call(
        host,
        "add_note",
        json!({"batch": batch, "target": {"kind": "person", "id": smith["id"]}, "text": "A smith, as the recording says."}),
    );
    call(
        host,
        "add_event",
        json!({"batch": batch, "person": ivan, "kind": "residence", "place": "Poltava", "date": "ABT 1930"}),
    );
    call(
        host,
        "end_batch",
        json!({"batch": batch, "report": "Done."}),
    );
}

#[test]
fn a_whole_scenario_run_twice_adds_nothing_the_second_time() {
    let host = empty_host();
    let father = owners(&host, "Petro", SexKind::Male, Some("1870"));
    let mut d = draft("Ivan", "Koval", Some("Petrovych"), Some("1901"));
    d.sex = SexKind::Male;
    let ivan = with(&host, |s| add_relative(s, father, Relation::Child, d));
    let olena = owners(&host, "Olena", SexKind::Female, Some("1905"));
    work_through(&host, ivan, olena);
    let first = counts(&host);
    let writes = log_rows(&host);
    work_through(&host, ivan, olena);
    assert_eq!(counts(&host), first, "same counts, so no new notes");
    assert_eq!(log_rows(&host), writes, "and nothing logged");
}

// ---- text size

#[test]
fn long_text_is_refused_with_where_to_put_it() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, None);
    let batch = begin(&host);
    let source = call(
        &host,
        "add_source",
        json!({"batch": batch, "title": "Book"}),
    )["id"]
        .clone();
    let event = call(
        &host,
        "add_event",
        json!({"batch": batch, "person": ivan, "kind": "residence"}),
    )["id"]
        .clone();
    let note = call(
        &host,
        "add_note",
        json!({"batch": batch, "target": on_person(ivan), "text": "x"}),
    )["id"]
        .clone();
    let citation = call(
        &host,
        "add_citation",
        json!({"batch": batch, "target": on_person(ivan), "source": source}),
    )["id"]
        .clone();
    let before = counts(&host);
    let over = "x".repeat(64 * 1024 + 1);
    for (tool, args) in [
        ("add_note", json!({"target": on_person(ivan), "text": over})),
        ("update_own_note", json!({"note": note, "text": over})),
        (
            "add_event",
            json!({"person": ivan, "kind": "occupation", "description": over}),
        ),
        (
            "update_own_event",
            json!({"event": event, "description": over}),
        ),
        (
            "add_citation",
            json!({"target": on_person(ivan), "source": source, "page": "1", "text": over}),
        ),
        (
            "update_own_citation",
            json!({"citation": citation, "text": over}),
        ),
    ] {
        let mut args = args;
        args["batch"] = batch.clone();
        let message = call_err(&host, tool, args);
        assert!(message.contains("64 KiB"), "{tool}: {message}");
        assert!(
            message.contains("split") || message.contains("Split"),
            "{tool}: {message}"
        );
        assert!(message.contains("source"), "{tool}: {message}");
        assert!(message.len() < 500, "{tool}");
    }
    assert_eq!(counts(&host), before);
    let at_limit = "x".repeat(64 * 1024);
    call(
        &host,
        "add_note",
        json!({"batch": batch, "target": on_person(ivan), "text": at_limit}),
    );
}

#[test]
fn a_source_holds_a_whole_transcript_and_no_more() {
    let host = empty_host();
    let batch = begin(&host);
    let word = "слово ";
    let at_limit = word.repeat(1024 * 1024 / word.len());
    assert!(at_limit.len() <= 1024 * 1024);
    let source = call(
        &host,
        "add_source",
        json!({"batch": batch, "title": "Transcript", "text": at_limit}),
    );
    // The whole text matches the word: this is what has to stay quick.
    let start = Instant::now();
    call(&host, "search_people", json!({"query": "слово"}));
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "{:?}",
        start.elapsed()
    );
    let got = call(
        &host,
        "get_source",
        json!({"id": source["id"], "text_limit": 10}),
    );
    assert_eq!(got["text"].as_str().unwrap().chars().count(), 10);
    let over = "x".repeat(1024 * 1024 + 1);
    let message = call_err(
        &host,
        "add_source",
        json!({"batch": batch, "title": "Long", "text": over}),
    );
    assert!(message.contains("1 MiB"), "{message}");
}

#[test]
fn a_long_report_is_refused() {
    let host = empty_host();
    let batch = begin(&host);
    let over = "x".repeat(64 * 1024 + 1);
    let message = call_err(&host, "end_batch", json!({"batch": batch, "report": over}));
    assert!(message.contains("64 KiB"), "{message}");
}

#[test]
fn rows_show_the_start_of_a_long_description_or_citation() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, None);
    let olena = owners(&host, "Olena", SexKind::Female, None);
    let batch = begin(&host);
    let long = "абв ".repeat(9_000);
    assert!(long.len() < 64 * 1024 + 1 && long.len() > 16 * 1024);
    let family = call(
        &host,
        "add_family",
        json!({"batch": batch, "partner1": ivan, "partner2": olena}),
    )["id"]
        .clone();
    let event = call(
        &host,
        "add_event",
        json!({"batch": batch, "person": ivan, "kind": "residence", "description": long}),
    )["id"]
        .clone();
    call(
        &host,
        "add_event",
        json!({"batch": batch, "family": family, "kind": "marriage", "description": long}),
    );
    let source = call(
        &host,
        "add_source",
        json!({"batch": batch, "title": "Book"}),
    )["id"]
        .clone();
    let citation = call(
        &host,
        "add_citation",
        json!({"batch": batch, "target": on_person(ivan), "source": source, "text": long}),
    )["id"]
        .clone();
    let reply = call_raw(&host, "get_person", json!({"id": ivan}));
    assert!(
        reply.to_string().len() < 20_000,
        "{}",
        reply.to_string().len()
    );
    let person = call(&host, "get_person", json!({"id": ivan}));
    let row = &person["events"][0];
    assert_eq!(row["descriptionTruncated"], true, "{row}");
    assert!(
        row["descriptionHint"]
            .as_str()
            .unwrap()
            .contains("get_event")
    );
    assert_eq!(person["citations"][0]["textTruncated"], true);
    assert!(
        person["citations"][0]["textHint"]
            .as_str()
            .unwrap()
            .contains("get_citation")
    );

    for (tool, id, field) in [
        ("get_event", &event, "description"),
        ("get_citation", &citation, "text"),
    ] {
        let whole = call(&host, tool, json!({"id": id}));
        assert_eq!(whole[field].as_str().unwrap(), long.trim(), "{tool}");
        let piece = call(
            &host,
            tool,
            json!({"id": id, "text_offset": 4, "text_limit": 4}),
        );
        assert_eq!(piece[field], "абв ", "{tool}");
        assert_eq!(piece["nextTextOffset"], 8, "{tool}");
    }
    let reply = call_raw(&host, "get_family", json!({"id": family}));
    assert!(
        reply.to_string().len() < 20_000,
        "{}",
        reply.to_string().len()
    );
    let reply = call_raw(&host, "list_events", json!({}));
    assert!(
        reply.to_string().len() < 20_000,
        "{}",
        reply.to_string().len()
    );
    let reply = call_raw(&host, "list_citations", json!({}));
    assert!(
        reply.to_string().len() < 20_000,
        "{}",
        reply.to_string().len()
    );
}

// ---- dates

#[test]
fn a_date_that_reads_two_ways_is_refused_everywhere() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, None);
    let batch = begin(&host);
    let event = call(
        &host,
        "add_event",
        json!({"batch": batch, "person": ivan, "kind": "residence"}),
    )["id"]
        .clone();
    let source = call(
        &host,
        "add_source",
        json!({"batch": batch, "title": "Book"}),
    )["id"]
        .clone();
    let before = counts(&host);
    for date in ["03/04/1901", "3.4.1901", "ABT 03-04-1901"] {
        for (tool, mut args) in [
            ("add_person", json!({"given": "Olena", "birth_date": date})),
            (
                "add_event",
                json!({"person": ivan, "kind": "occupation", "date": date}),
            ),
            ("update_own_event", json!({"event": event, "date": date})),
            (
                "add_name",
                json!({"person": ivan, "kind": "aka", "given": "Ivanko", "date": date}),
            ),
            (
                "add_citation",
                json!({"target": on_person(ivan), "source": source, "date": date}),
            ),
        ] {
            args["batch"] = batch.clone();
            let message = call_err(&host, tool, args);
            assert!(message.contains("two ways"), "{tool} {date}: {message}");
            assert!(message.contains("note"), "{tool} {date}: {message}");
        }
    }
    assert_eq!(counts(&host), before);
}

#[test]
fn a_date_that_reads_one_way_is_stored_without_a_warning() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, None);
    let batch = begin(&host);
    for date in ["13/04/1901", "03/03/1901", "12 MAR 1901"] {
        let answer = call(
            &host,
            "add_event",
            json!({"batch": batch, "person": ivan, "kind": "residence", "date": date}),
        );
        assert_eq!(answer["warnings"], Value::Null, "{date}: {answer}");
    }
}

#[test]
fn a_double_year_comes_back_as_a_warning() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, None);
    let batch = begin(&host);
    let answer = call(
        &host,
        "add_event",
        json!({"batch": batch, "person": ivan, "kind": "residence", "date": "12 MAR 1901/02"}),
    );
    let warnings = answer["warnings"].as_array().unwrap();
    assert!(
        warnings[0].as_str().unwrap().contains("Double year"),
        "{answer}"
    );
}

// ---- one lock per write

/// Counts how many times a tool takes the session.
struct CountingHost {
    inner: TestHost,
    locks: AtomicUsize,
}

impl Host for CountingHost {
    fn with_session<T>(
        &self,
        f: &mut dyn FnMut(&mut Session) -> Result<T, ApiError>,
    ) -> Result<T, ApiError> {
        self.locks.fetch_add(1, Ordering::SeqCst);
        self.inner.with_session(f)
    }

    fn authenticate(&self, key_sha256: &[u8; 32]) -> Option<AgentCtx> {
        self.inner.authenticate(key_sha256)
    }

    fn changed(&self, touched: &[Uuid], agent: &AgentCtx) {
        self.inner.changed(touched, agent);
    }
}

#[test]
fn a_write_reads_what_it_needs_under_the_one_lock() {
    let host = CountingHost {
        inner: empty_host(),
        locks: AtomicUsize::new(0),
    };
    let ivan = owners(&host.inner, "Ivan", SexKind::Male, None);
    let petro = owners(&host.inner, "Petro", SexKind::Male, None);
    let batch = begin(&host.inner);
    let family = call(
        &host.inner,
        "add_family",
        json!({"batch": batch, "partner1": ivan}),
    )["id"]
        .clone();
    let event = call(
        &host.inner,
        "add_event",
        json!({"batch": batch, "person": ivan, "kind": "residence"}),
    )["id"]
        .clone();
    let note = call(
        &host.inner,
        "add_note",
        json!({"batch": batch, "target": on_person(ivan), "text": "x"}),
    )["id"]
        .clone();
    let name = call(
        &host.inner,
        "add_name",
        json!({"batch": batch, "person": ivan, "kind": "aka", "given": "Ivanko"}),
    )["id"]
        .clone();
    for (tool, args) in [
        (
            "update_own_note",
            json!({"batch": batch, "note": note, "text": "y"}),
        ),
        (
            "update_own_event",
            json!({"batch": batch, "event": event, "date": "1901"}),
        ),
        (
            "update_own_name",
            json!({"batch": batch, "person": ivan, "name": name, "given": "Ivas", "date": "1901"}),
        ),
        (
            "link_people",
            json!({"batch": batch, "person": petro, "relation": {"as": "child", "family": family}}),
        ),
        (
            "add_event",
            json!({"batch": batch, "person": ivan, "kind": "census", "date": "1910"}),
        ),
        (
            "add_person",
            json!({"batch": batch, "given": "Olena", "birth_date": "1905", "source_file": "a", "number": "1"}),
        ),
    ] {
        host.locks.store(0, Ordering::SeqCst);
        let reply = handle_message(
            &host,
            &ctx(),
            &request("tools/call", json!({"name": tool, "arguments": args})),
        )
        .unwrap();
        assert_ne!(reply["result"]["isError"], json!(true), "{tool}: {reply}");
        assert_eq!(host.locks.load(Ordering::SeqCst), 1, "{tool}");
    }
}

// ---- smaller things

#[test]
fn the_marker_refuses_in_words_for_an_agent() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, Some("1901"));
    let zina = owners(&host, "Zina", SexKind::Female, Some("1931"));
    let batch = begin(&host);
    let marker = call(
        &host,
        "add_sibling_marker",
        json!({"batch": batch, "sibling_of": ivan}),
    )["id"]
        .clone();
    let other = call(
        &host,
        "add_family",
        json!({"batch": batch, "partner1": zina}),
    )["id"]
        .clone();
    let source = call(
        &host,
        "add_source",
        json!({"batch": batch, "title": "Book"}),
    )["id"]
        .clone();
    let before = counts(&host);
    let on_marker = json!({"kind": "person", "id": marker});
    let mut tries = vec![
        ("add_note", json!({"target": on_marker, "text": "x"})),
        (
            "add_citation",
            json!({"target": on_marker, "source": source}),
        ),
        (
            "set_number",
            json!({"person": marker, "source_file": "f", "number": "9"}),
        ),
        ("set_sex", json!({"person": marker, "sex": "male"})),
        (
            "add_name",
            json!({"person": marker, "kind": "aka", "given": "X"}),
        ),
        ("add_event", json!({"person": marker, "kind": "residence"})),
        ("add_family", json!({"partner1": marker})),
        ("add_family", json!({"partner1": zina, "partner2": marker})),
        (
            "link_people",
            json!({"person": marker, "relation": {"as": "child", "family": other}}),
        ),
        (
            "add_person",
            json!({"given": "X", "relation": {"to": marker, "as": "father"}}),
        ),
    ];
    for relation in ["father", "mother", "partner", "child", "sibling"] {
        tries.push((
            "link_people",
            json!({"person": marker, "relation": {"to": zina, "as": relation}}),
        ));
        tries.push((
            "link_people",
            json!({"person": zina, "relation": {"to": marker, "as": relation}}),
        ));
    }
    for (tool, mut args) in tries {
        args["batch"] = batch.clone();
        let message = call_err(&host, tool, args);
        assert!(!message.contains("Delete"), "{tool}: {message}");
        assert!(message.contains("nobody named"), "{tool}: {message}");
    }
    assert_eq!(counts(&host), before);
    let found = call(
        &host,
        "find_by_number",
        json!({"source_file": "f", "number": "9"}),
    );
    assert_eq!(found["person"], Value::Null);
}

#[test]
fn setting_the_sex_it_has_writes_nothing() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, None);
    let batch = begin(&host);
    let rows = log_rows(&host);
    let writes = host.changes.lock().unwrap().len();
    let answer = call(
        &host,
        "set_sex",
        json!({"batch": batch, "person": ivan, "sex": "male"}),
    );
    assert_eq!(answer["alreadyThere"], true, "{answer}");
    assert_eq!(answer["id"], json!(ivan));
    assert_eq!(log_rows(&host), rows);
    assert_eq!(host.changes.lock().unwrap().len(), writes);
}

#[test]
fn saying_again_what_an_own_record_says_writes_nothing() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, None);
    let batch = begin(&host);
    let note = call(
        &host,
        "add_note",
        json!({"batch": batch, "target": on_person(ivan), "text": "x"}),
    )["id"]
        .clone();
    let rows = log_rows(&host);
    let again = call(
        &host,
        "update_own_note",
        json!({"batch": batch, "note": note, "text": "x"}),
    );
    assert_eq!(again["alreadyThere"], true, "{again}");
    assert_eq!(again["id"], note);
    assert_eq!(log_rows(&host), rows);
}

#[test]
fn a_wrong_event_kind_lists_the_kinds() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, None);
    let batch = begin(&host);
    let message = call_err(
        &host,
        "add_event",
        json!({"batch": batch, "person": ivan, "kind": "funeral"}),
    );
    assert!(message.contains("funeral"), "{message}");
    assert!(message.contains("burial"), "{message}");
    assert!(message.contains("marriage_banns"), "{message}");
    assert!(message.contains("custom:"), "{message}");
}

#[test]
fn every_kind_the_refusal_lists_can_be_used() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, None);
    let batch = begin(&host);
    let message = call_err(
        &host,
        "add_event",
        json!({"batch": batch, "person": ivan, "kind": "funeral"}),
    );
    let (_, list) = message.split_once(": ").unwrap();
    let list = list.split(", or custom").next().unwrap();
    let kinds: Vec<&str> = list.split(", ").collect();
    assert_eq!(kinds.len(), 33, "{kinds:?}");
    let old = owners(&host, "Olena", SexKind::Female, Some("1805"));
    for kind in kinds {
        let reply = call_raw(
            &host,
            "add_event",
            json!({"batch": batch, "person": old, "kind": kind}),
        );
        let text = reply["content"][0]["text"].as_str().unwrap();
        assert!(!text.contains("not a kind"), "{kind}: {text}");
    }
}

#[test]
fn a_family_refuses_what_only_a_person_has() {
    let host = empty_host();
    let ivan = owners(&host, "Ivan", SexKind::Male, None);
    let batch = begin(&host);
    let family = call(
        &host,
        "add_family",
        json!({"batch": batch, "partner1": ivan}),
    )["id"]
        .clone();
    let before = counts(&host);
    for kind in ["death", "birth", "burial", "baptism", "occupation"] {
        let message = call_err(
            &host,
            "add_event",
            json!({"batch": batch, "family": family, "kind": kind, "date": "1950"}),
        );
        assert!(message.contains("marriage"), "{kind}: {message}");
        assert!(message.contains("person"), "{kind}: {message}");
    }
    assert_eq!(counts(&host), before);
    for kind in [
        "marriage",
        "divorce",
        "engagement",
        "residence",
        "custom:Wedding feast",
    ] {
        call(
            &host,
            "add_event",
            json!({"batch": batch, "family": family, "kind": kind, "date": "1950"}),
        );
    }
    let event = call(
        &host,
        "add_event",
        json!({"batch": batch, "family": family, "kind": "census"}),
    )["id"]
        .clone();
    let message = call_err(
        &host,
        "update_own_event",
        json!({"batch": batch, "event": event, "kind": "death"}),
    );
    assert!(message.contains("marriage"), "{message}");
}

#[test]
fn an_empty_report_is_refused() {
    let host = empty_host();
    let batch = begin(&host);
    for report in ["", "   \n"] {
        let message = call_err(
            &host,
            "end_batch",
            json!({"batch": batch, "report": report}),
        );
        assert!(message.contains("report"), "{message}");
    }
    call(
        &host,
        "end_batch",
        json!({"batch": batch, "report": "Nothing to add."}),
    );
}

#[test]
fn closing_a_batch_refreshes_the_app_without_the_lock() {
    let host = empty_host();
    let batch = begin(&host);
    call(
        &host,
        "end_batch",
        json!({"batch": batch, "report": "Done."}),
    );
    let changes = host.changes.lock().unwrap().clone();
    assert_eq!(changes.len(), 1);
    assert!(changes[0].0.is_empty(), "no record changed");
    assert_eq!(changes[0].1, ctx());
    assert!(!*host.changed_under_lock.lock().unwrap());
    // Refused, nothing to refresh.
    call_err(
        &host,
        "end_batch",
        json!({"batch": batch, "report": "Again."}),
    );
    assert_eq!(host.changes.lock().unwrap().len(), 1);
}

#[test]
fn own_tools_say_an_empty_field_can_be_filled() {
    let host = empty_host();
    let tools = handle_message(&host, &ctx(), &request("tools/list", json!({}))).unwrap();
    for name in ["update_own_event", "update_own_name", "update_own_citation"] {
        let tool = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap();
        let text = tool["description"].as_str().unwrap();
        assert!(text.contains("empty"), "{name}: {text}");
    }
}

#[test]
fn a_death_on_someone_already_counted_dead_stays_allowed() {
    let host = empty_host();
    let old = owners(&host, "Olena", SexKind::Female, Some("1805"));
    let living = |host: &TestHost| call(host, "get_person", json!({"id": old}))["living"].clone();
    let before = living(&host);
    assert_eq!(before, false);
    let batch = begin(&host);
    call(
        &host,
        "add_event",
        json!({"batch": batch, "person": old, "kind": "death", "date": "1880"}),
    );
    assert_eq!(living(&host), before);
    // The rule itself is untouched: on someone who counts as alive it is refused.
    let young = owners(&host, "Mykola", SexKind::Male, Some("1990"));
    let message = call_err(
        &host,
        "add_event",
        json!({"batch": batch, "person": young, "kind": "death", "date": "2020"}),
    );
    assert!(message.contains("alive or dead"), "{message}");
}
