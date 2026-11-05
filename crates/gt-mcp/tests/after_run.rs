#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! What a real agent run showed: nameless people, nickname-only names, a family left dangling by
//! a refused link, the batch answer's id, and what the descriptions say.

#[path = "common/client.rs"]
mod client;
mod common;

use client::HttpClient;
use common::*;
use gt_mcp::{handle_message, serve};
use gt_session::{EditCommand, NameInput, NameKindDto, PersonDraft, SexKind};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

fn begin(host: &TestHost) -> Value {
    call(host, "begin_batch", json!({"title": "Recording 3"}))["batch"].clone()
}

fn people(host: &TestHost) -> usize {
    with(host, |s| s.tree().persons.len())
}

fn nameless() -> PersonDraft {
    PersonDraft {
        given: None,
        surnames: Vec::new(),
        patronymic: None,
        sex: SexKind::Unknown,
        birth_date_raw: None,
        birth_place_text: None,
    }
}

fn description(host: &TestHost, name: &str) -> String {
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

// 1. Nobody is created without a name.

#[test]
fn a_nameless_person_is_refused_whichever_way_it_is_made() {
    let host = empty_host();
    let ivan = with(&host, |s| add_person(s, draft("Ivan", "Koval", None, None)));
    let batch = begin(&host);
    let before = people(&host);
    let blanks = json!({"given": "  ", "surnames": [" "]});
    let attempts = [
        json!({"batch": batch}),
        json!({"batch": batch, "sex": "male", "birth_date": "1901"}),
        json!({"batch": batch, "given": blanks["given"], "surnames": blanks["surnames"]}),
        json!({"batch": batch, "relation": {"to": ivan, "as": "father"}}),
        json!({"batch": batch, "relation": {"to": ivan, "as": "mother"}}),
        json!({"batch": batch, "relation": {"to": ivan, "as": "partner"}}),
        json!({"batch": batch, "relation": {"to": ivan, "as": "child"}}),
        json!({"batch": batch, "relation": {"to": ivan, "as": "sibling"}}),
    ];
    for args in attempts {
        let message = call_err(&host, "add_person", args.clone());
        assert!(message.contains("add_sibling_marker"), "{args}: {message}");
        assert!(message.contains("sibling_of"), "{message}");
        assert!(message.contains("nameless"), "{message}");
    }
    assert_eq!(people(&host), before);
    assert_eq!(with(&host, |s| s.tree().families.len()), 0);
}

#[test]
fn a_nameless_person_of_the_owner_stays_readable_and_gets_notes() {
    let host = empty_host();
    let ghost = with(&host, |s| add_person(s, nameless()));
    let batch = begin(&host);
    call(
        &host,
        "add_note",
        json!({"batch": batch, "target": {"kind": "person", "id": ghost},
               "text": "Her father; name not given."}),
    );
    let got = call(&host, "get_person", json!({"id": ghost}));
    assert!(got.to_string().contains("name not given"), "{got}");
    assert_eq!(people(&host), 1);
}

#[test]
fn a_named_person_and_the_marker_still_work_beside_the_rule() {
    let host = empty_host();
    let ivan = with(&host, |s| add_person(s, draft("Ivan", "Koval", None, None)));
    let batch = begin(&host);
    let marker = call(
        &host,
        "add_sibling_marker",
        json!({"batch": batch, "sibling_of": ivan}),
    );
    assert!(marker["id"].is_string(), "{marker}");
    call(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Olena", "relation": {"to": ivan, "as": "mother"}}),
    );
}

// 2. A name that is only a nickname.

fn nickname_name(person: Uuid, nickname: &str) -> EditCommand {
    EditCommand::SaveName {
        person,
        name: NameInput {
            id: None,
            kind: NameKindDto::Nickname,
            kind_text: None,
            preferred: false,
            private: false,
            prefix: None,
            given: None,
            call: None,
            surname_prefix: None,
            surnames: Vec::new(),
            patronymic: None,
            suffix: None,
            nickname: Some(nickname.into()),
            lang: None,
            date_raw: None,
        },
    }
}

#[test]
fn a_nickname_alone_is_a_name_for_an_agent_and_no_given_name_is_made_up() {
    let host = empty_host();
    let ivan = with(&host, |s| add_person(s, draft("Ivan", "Koval", None, None)));
    let batch = begin(&host);
    let added = call(
        &host,
        "add_name",
        json!({"batch": batch, "person": ivan, "kind": "nickname", "nickname": "Vanko"}),
    );
    assert!(added["id"].is_string(), "{added}");
    with(&host, |s| {
        let p = &s.tree().persons[&ivan];
        let name = p.names.iter().find(|n| n.nickname.is_some()).unwrap();
        assert_eq!(name.nickname.as_deref(), Some("Vanko"));
        assert_eq!(name.given, None, "no given name was made up");
        assert!(name.surnames.is_empty());
        assert!(!name.assertion.preferred);
    });
    // The same again is a repeat, not a second name.
    let again = call(
        &host,
        "add_name",
        json!({"batch": batch, "person": ivan, "kind": "nickname", "nickname": "Vanko"}),
    );
    assert_eq!(again["alreadyThere"], true, "{again}");
    // Nothing at all is still no name.
    let message = call_err(
        &host,
        "add_name",
        json!({"batch": batch, "person": ivan, "kind": "nickname"}),
    );
    assert!(message.contains("given name or a surname"), "{message}");
}

#[test]
fn the_owners_editor_still_wants_a_given_name_or_a_surname() {
    let host = empty_host();
    let ivan = with(&host, |s| add_person(s, draft("Ivan", "Koval", None, None)));
    let err = with(&host, |s| s.apply(nickname_name(ivan, "Vanko"))).unwrap_err();
    assert!(err.message.contains("given name or a surname"), "{err:?}");
}

// 3. remove_own_empty_family

fn family_of(host: &TestHost, batch: &Value, partner: Uuid) -> String {
    call(
        host,
        "add_family",
        json!({"batch": batch, "partner1": partner}),
    )["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn an_empty_family_made_in_this_batch_can_be_removed_and_undone() {
    let host = empty_host();
    let ivan = with(&host, |s| add_person(s, draft("Ivan", "Koval", None, None)));
    let batch = begin(&host);
    let family = family_of(&host, &batch, ivan);
    let out = call(
        &host,
        "remove_own_empty_family",
        json!({"batch": batch, "family": family}),
    );
    assert!(out["touched"].to_string().contains(&family), "{out}");
    assert_eq!(with(&host, |s| s.tree().families.len()), 0);
    // Logged like any write, under the batch.
    let log = call(&host, "get_log", json!({"batch": batch}));
    assert!(log.to_string().contains("Delete family"), "{log}");
    // The owner can undo it.
    with(&host, |s| {
        let filter = gt_store::LogFilter {
            batch: Some(batch.as_str().unwrap().parse().unwrap()),
            ..Default::default()
        };
        let (groups, _) = s.project().log_page(&filter, 0, 10).unwrap();
        // Newest first: the removal.
        assert_eq!(groups[0].label, "Delete family");
        s.undo_group(groups[0].group).unwrap();
    });
    assert_eq!(with(&host, |s| s.tree().families.len()), 1);
}

#[test]
fn a_family_with_anything_in_it_or_not_made_here_stays() {
    let host = empty_host();
    let ivan = with(&host, |s| add_person(s, draft("Ivan", "Koval", None, None)));
    let petro = with(&host, |s| {
        add_person(s, draft("Petro", "Koval", None, None))
    });
    let stepan = with(&host, |s| {
        add_person(s, draft("Stepan", "Koval", None, None))
    });
    let owners = with(&host, |s| {
        s.apply(EditCommand::SaveFamily {
            id: None,
            partner1: Some(ivan),
            partner2: None,
            kind: gt_session::UnionKindId::Unknown,
        })
        .unwrap()
        .touched[0]
    });
    let old_batch = begin(&host);
    let from_old_batch = family_of(&host, &old_batch, petro);
    call(
        &host,
        "end_batch",
        json!({"batch": old_batch, "report": "Done"}),
    );

    let batch = begin(&host);
    let with_child = family_of(&host, &batch, stepan);
    let olena = call(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Olena"}),
    )["id"]
        .clone();
    call(
        &host,
        "link_people",
        json!({"batch": batch, "person": olena, "relation": {"as": "child", "family": with_child}}),
    );
    let with_note = family_of(&host, &batch, olena.as_str().unwrap().parse().unwrap());
    call(
        &host,
        "add_note",
        json!({"batch": batch, "target": {"kind": "family", "id": with_note}, "text": "Hm"}),
    );
    let with_event = {
        let id = call(
            &host,
            "add_family",
            json!({"batch": batch, "partner1": petro, "partner2": stepan}),
        )["id"]
            .as_str()
            .unwrap()
            .to_string();
        call(
            &host,
            "add_event",
            json!({"batch": batch, "family": id, "kind": "marriage"}),
        );
        id
    };
    for family in [
        owners.to_string(),
        from_old_batch,
        with_child,
        with_note,
        with_event,
    ] {
        let message = call_err(
            &host,
            "remove_own_empty_family",
            json!({"batch": batch, "family": family}),
        );
        assert!(
            message.contains("stays") || message.contains("Only a family you made"),
            "{message}"
        );
    }
    assert_eq!(with(&host, |s| s.tree().families.len()), 5);
    // A family that does not exist.
    let missing = Uuid::from_u128(99).to_string();
    call_err(
        &host,
        "remove_own_empty_family",
        json!({"batch": batch, "family": missing}),
    );
    // Without a batch it is refused like every write.
    let message = call_err(
        &host,
        "remove_own_empty_family",
        json!({"family": Uuid::from_u128(5).to_string()}),
    );
    assert!(message.contains("begin_batch"), "{message}");
}

#[test]
fn another_connections_family_is_not_removed() {
    let host = empty_host();
    let ivan = with(&host, |s| add_person(s, draft("Ivan", "Koval", None, None)));
    let batch = begin(&host);
    let family = family_of(&host, &batch, ivan);
    // Another connection, with its own batch.
    let other = gt_mcp::AgentCtx {
        id: Uuid::from_u128(8),
        name: "Another".into(),
    };
    let theirs = with(&host, |s| s.begin_batch(&other, "Theirs", None).unwrap());
    let err = with(&host, |s| {
        s.remove_own_empty_family(family.parse().unwrap(), &other, theirs)
    })
    .unwrap_err();
    assert!(err.message.contains("Only a family you made"), "{err:?}");
    assert_eq!(with(&host, |s| s.tree().families.len()), 1);
}

#[test]
fn removing_a_family_cannot_change_who_counts_as_living() {
    let host = empty_host();
    let ivan = with(&host, |s| {
        add_person(s, draft("Ivan", "Koval", None, Some("1901")))
    });
    let before = with(&host, |s| s.tree().persons[&ivan].living_override.clone());
    let batch = begin(&host);
    let family = family_of(&host, &batch, ivan);
    call(
        &host,
        "remove_own_empty_family",
        json!({"batch": batch, "family": family}),
    );
    let after = with(&host, |s| s.tree().persons[&ivan].living_override.clone());
    assert_eq!(before, after);
}

// 4, 5, 6. The wire and the words.

#[test]
fn begin_batch_answers_batch_and_id_alike_over_http() {
    let host = Arc::new(empty_host());
    let server = serve(Arc::clone(&host), 0).unwrap();
    let c = HttpClient::new(server.port());
    let opened = c.call("begin_batch", json!({"title": "Recording 3"}));
    assert!(opened["batch"].is_string(), "{opened}");
    assert_eq!(opened["batch"], opened["id"], "{opened}");
    // Either name works as the batch argument's value.
    let ivan = c.call(
        "add_person",
        json!({"batch": opened["id"], "given": "Ivan", "surnames": ["Koval"]}),
    )["id"]
        .clone();
    // A nameless parent is refused over the wire, and the marker is the way.
    let message = c.call_err(
        "add_person",
        json!({"batch": opened["id"], "relation": {"to": ivan, "as": "father"}}),
    );
    assert!(message.contains("add_sibling_marker"), "{message}");
    let marker = c.call(
        "add_sibling_marker",
        json!({"batch": opened["id"], "sibling_of": ivan}),
    );
    assert!(marker["id"].is_string(), "{marker}");
    // The family a refused link left behind goes away over the wire too.
    let family = c.call(
        "add_family",
        json!({"batch": opened["id"], "partner1": ivan}),
    )["id"]
        .clone();
    c.call(
        "remove_own_empty_family",
        json!({"batch": opened["id"], "family": family}),
    );
    // A nickname alone is a name over the wire.
    let named = c.call(
        "add_name",
        json!({"batch": opened["id"], "person": ivan, "kind": "nickname", "nickname": "Vanko"}),
    );
    assert!(named["id"].is_string(), "{named}");
    // The tool is listed.
    let list = c.rpc("tools/list", json!({}));
    let listed = list["result"]["tools"].to_string();
    assert!(listed.contains("remove_own_empty_family"), "{listed}");
}

#[test]
fn the_descriptions_and_instructions_say_what_the_run_taught() {
    let host = empty_host();
    let begin_text = description(&host, "begin_batch");
    assert!(
        begin_text.contains("\"batch\"") && begin_text.contains("\"id\""),
        "{begin_text}"
    );
    let end_text = description(&host, "end_batch");
    assert!(
        end_text.contains("only what the server accepted") && end_text.contains("get_log"),
        "{end_text}"
    );
    let add = description(&host, "add_person");
    for needle in [
        "nameless",
        "add_sibling_marker",
        "sibling_of",
        "birth event",
        "Do not assume a parent",
        "remove_own_empty_family",
    ] {
        assert!(add.contains(needle), "{needle}: {add}");
    }
    assert!(
        !add.contains("empty name"),
        "no longer advises an empty name"
    );
    let link = description(&host, "link_people");
    assert!(link.contains("remove_own_empty_family"), "{link}");
    let name = description(&host, "add_name");
    assert!(name.contains("only a nickname"), "{name}");
    let remove = description(&host, "remove_own_empty_family");
    assert!(
        remove.contains("no children") && remove.contains("open batch"),
        "{remove}"
    );
    assert!(description(&host, "add_event").contains("birth event"));

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
    assert!(instructions.contains("birth event"), "{instructions}");
    assert!(instructions.contains("no name at all"), "{instructions}");
    assert!(instructions.contains("check get_log"), "{instructions}");
}
