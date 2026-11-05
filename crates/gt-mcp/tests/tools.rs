#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The read tools against a synthetic tree built through `Session::apply`.

mod common;

use base64::Engine;
use common::*;
use gt_core::date::{DayOrder, GenDate};
use gt_core::model::*;
use gt_core::raw::RawNode;
use gt_mcp::RULES;
use gt_session::{
    CitationArg, CitationInput, EditCommand, EventInput, EventOwner, EventOwnerKind, NoteInput,
    NoteKindDto, OwnerKind, OwnerRef, Relation, Session, SessionClock, SettingsPatch, SourceInput,
    SourceRef,
};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use serde_json::{Value, json};
use std::io::Cursor;
use uuid::Uuid;

fn ids(rows: &Value) -> Vec<String> {
    rows.as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_string())
        .collect()
}

fn jpeg_bytes(w: u32, h: u32) -> Vec<u8> {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb([200, 40, 40])));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Jpeg).unwrap();
    out.into_inner()
}

/// Writes a file outside the project, attaches it to `person` and returns the media id.
fn attach(host: &TestHost, person: Uuid, name: &str, bytes: &[u8]) -> Uuid {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    std::fs::write(&path, bytes).unwrap();
    with(host, |s| {
        let r = s
            .attach_files(
                OwnerRef {
                    kind: OwnerKind::Person,
                    id: person,
                },
                std::slice::from_ref(&path),
            )
            .unwrap();
        assert!(r.failed.is_empty(), "{:?}", r.failed);
        r.attached[0].id
    })
}

#[test]
fn search_finds_by_variant() {
    let host = empty_host();
    let id = with(&host, |s| {
        let id = add_person(s, draft("Ivan", "Koval", Some("Petrovych"), Some("1901")));
        add_name_variant(s, id, "Iwan", "Kowal");
        id
    });
    // The spelling of the extra name.
    let by_extra = call(&host, "search_people", json!({"query": "Kowal"}));
    assert_eq!(ids(&by_extra["items"]), [id.to_string()]);
    // A spelling nobody typed: Coval sounds like Koval.
    let by_sound = call(&host, "search_people", json!({"query": "Kovall"}));
    assert_eq!(ids(&by_sound["items"]), [id.to_string()]);
    // Both names come back with the hit.
    let names = by_extra["items"][0]["names"].as_array().unwrap();
    assert!(names.contains(&json!("Ivan Petrovych Koval")), "{names:?}");
    assert!(names.contains(&json!("Iwan Kowal")), "{names:?}");
}

#[test]
fn search_returns_people_only() {
    let host = empty_host();
    with(&host, |s| {
        let id = add_person(s, draft("Olena", "Koval", None, None));
        add_note(s, id, "Lived near Zhytomyr");
    });
    let found = call(&host, "search_people", json!({"query": "Zhytomyr"}));
    assert_eq!(found["items"], json!([]));
    assert_eq!(found["total"], 0);
}

#[test]
fn namesakes_distinguishable() {
    let host = empty_host();
    let (elder, younger) = with(&host, |s| {
        let elder = add_person(s, draft("Ivan", "Koval", None, Some("abt 1880")));
        let younger = add_person(s, draft("Ivan", "Koval", None, Some("1950")));
        add_relative(
            s,
            elder,
            Relation::Partner,
            draft("Olena", "Shevchuk", None, None),
        );
        add_relative(
            s,
            younger,
            Relation::Father,
            draft("Petro", "Koval", None, None),
        );
        // A death on the elder only.
        s.apply(EditCommand::SaveEvent {
            owner: EventOwner {
                kind: EventOwnerKind::Person,
                id: elder,
            },
            event: EventInput {
                id: None,
                kind: "death".into(),
                date_raw: Some("1944".into()),
                place_text: None,
                place_id: None,
                description: None,
                age: None,
                cause: None,
                negative: false,
                private: false,
                preferred: true,
                participants: None,
            },
        })
        .unwrap();
        (elder, younger)
    });
    let found = call(&host, "search_people", json!({"query": "Ivan Koval"}));
    assert!(found["total"].as_u64().unwrap() >= 2);
    let row = |id: Uuid| {
        found["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == json!(id.to_string()))
            .unwrap()
            .clone()
    };
    let (e, y) = (row(elder), row(younger));
    assert_eq!(e["birthYear"], "c. 1880");
    assert_eq!(e["deathYear"], "1944");
    assert_eq!(e["partners"], json!(["Olena Shevchuk"]));
    assert_eq!(e["parents"], json!([]));
    assert_eq!(y["birthYear"], "1950");
    assert_eq!(y["deathYear"], Value::Null);
    assert_eq!(y["parents"], json!(["Petro Koval"]));
    assert_eq!(y["partners"], json!([]));
}

#[test]
fn read_everything_including_living() {
    let host = empty_host();
    let (id, parent) = with(&host, |s| {
        let id = add_person(s, draft("Olena", "Koval", None, Some("1990")));
        add_note(s, id, "Works as a nurse in Lviv");
        let parent = add_relative(
            s,
            id,
            Relation::Father,
            draft("Ivan", "Koval", None, Some("1960")),
        );
        (id, parent)
    });
    let person = call(&host, "get_person", json!({"id": id}));
    // Living and shown in full: nothing is redacted.
    assert_eq!(person["ref"]["living"], true);
    assert_eq!(person["ref"]["name"], "Olena Koval");
    assert_eq!(person["livingComputed"], true);
    assert_eq!(person["livingOverride"], Value::Null);
    assert_eq!(person["siblingMarker"], false);
    assert_eq!(person["numbers"], json!([]));
    assert_eq!(person["names"][0]["surnames"], json!(["Koval"]));
    assert_eq!(person["events"][0]["kind"], "birth");
    assert_eq!(person["notes"][0]["text"], "Works as a nurse in Lviv");

    let relatives = call(&host, "get_relatives", json!({"id": parent}));
    assert_eq!(relatives["children"][0]["id"], json!(id.to_string()));
    assert!(relatives["children"][0]["familyId"].as_str().is_some());
    let up = call(&host, "get_relatives", json!({"id": id}));
    assert_eq!(up["parents"][0]["name"], "Ivan Koval");
    assert_eq!(up["parents"][0]["birthYear"], "1960");

    let family_id = up["parents"][0]["familyId"].as_str().unwrap();
    let family = call(&host, "get_family", json!({"id": family_id}));
    assert_eq!(family["id"], family_id);
    assert_eq!(family["children"][0]["person"]["name"], "Olena Koval");
    assert!(family["notes"].is_array());
}

#[test]
fn records_can_be_read_by_id() {
    let host = empty_host();
    let (person, source, note) = with(&host, |s| {
        let person = add_person(s, draft("Ivan", "Koval", None, None));
        let source = s
            .apply(EditCommand::SaveSource {
                source: SourceInput {
                    id: None,
                    title: Some("Recording 2".into()),
                    author: None,
                    publication: None,
                    abbreviation: None,
                    text: Some("The whole transcript".into()),
                },
            })
            .unwrap()
            .touched[0];
        s.apply(EditCommand::AddCitation {
            target: OwnerRef {
                kind: OwnerKind::Person,
                id: person,
            },
            citation: CitationArg::New(CitationInput {
                source: SourceRef::Id(source),
                page: Some("00:12:30".into()),
                date_raw: None,
                text: None,
                quality: Some(2),
                evidence: None,
            }),
        })
        .unwrap();
        add_note(s, person, "First note");
        let note = s.tree().notes.keys().next().copied().unwrap();
        (person, source, note)
    });
    let got = call(&host, "get_source", json!({"id": source}));
    assert_eq!(got["title"], "Recording 2");
    assert_eq!(got["text"], "The whole transcript");
    assert_eq!(got["citationCount"], 1);

    let citation = call(&host, "get_person", json!({"id": person}))["citations"][0].clone();
    assert_eq!(citation["source"]["title"], "Recording 2");
    let got = call(&host, "get_citation", json!({"id": citation["id"]}));
    assert_eq!(got["page"], "00:12:30");
    assert_eq!(got["quality"], 2);

    let got = call(&host, "get_note", json!({"id": note}));
    assert_eq!(got["text"], "First note");

    assert!(call_err(&host, "get_note", json!({"id": Uuid::from_u128(1)})).contains("No note"));
}

#[test]
fn get_person_full_notes() {
    let host = empty_host();
    let long = format!("{}кінець", "Довга нотатка про родовід. ".repeat(2_000));
    let id = with(&host, |s| {
        let id = add_person(s, draft("Ivan", "Koval", None, None));
        add_note(s, id, &long);
        id
    });
    let person = call(&host, "get_person", json!({"id": id}));
    assert_eq!(person["notes"][0]["text"].as_str().unwrap(), long);
}

#[test]
fn get_person_includes_notes_on_names() {
    let host = empty_host();
    let id = with(&host, |s| {
        let id = add_person(s, draft("Ivan", "Koval", None, None));
        let name = s.get_person(id).unwrap().names[0].id;
        s.apply(EditCommand::SaveNote {
            target: OwnerRef {
                kind: OwnerKind::Name,
                id: name,
            },
            note: NoteInput {
                id: None,
                kind: NoteKindDto::General,
                text: "Spelled this way in the parish book".into(),
            },
        })
        .unwrap();
        id
    });
    let person = call(&host, "get_person", json!({"id": id}));
    assert_eq!(
        person["names"][0]["notes"][0]["text"],
        "Spelled this way in the parish book"
    );
}

#[test]
fn get_media_image_content() {
    let host = empty_host();
    let id = with(&host, |s| add_person(s, draft("Ivan", "Koval", None, None)));
    let media = attach(&host, id, "portrait.jpg", &jpeg_bytes(400, 200));

    let result = call_raw(&host, "get_media", json!({"id": media, "max_size": 100}));
    assert_ne!(result["isError"], json!(true), "{result}");
    let content = result["content"].as_array().unwrap();
    let details: Value = serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(details["originalName"], "portrait.jpg");
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["mimeType"], "image/jpeg");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(content[1]["data"].as_str().unwrap())
        .unwrap();
    let decoded = image::load_from_memory(&bytes).unwrap();
    assert_eq!(
        (decoded.width(), decoded.height()),
        (100, 50),
        "downsized to max_size"
    );

    // The default never scales a small picture up.
    let result = call_raw(&host, "get_media", json!({"id": media}));
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(result["content"][1]["data"].as_str().unwrap())
        .unwrap();
    assert_eq!(image::load_from_memory(&bytes).unwrap().width(), 400);
}

#[test]
fn get_media_document_is_a_resource_blob() {
    let host = empty_host();
    let id = with(&host, |s| add_person(s, draft("Ivan", "Koval", None, None)));
    let pdf = b"%PDF-1.4\nnot really a pdf but it starts like one\n";
    let media = attach(&host, id, "letter.pdf", pdf);
    let result = call_raw(&host, "get_media", json!({"id": media}));
    assert_ne!(result["isError"], json!(true), "{result}");
    let block = &result["content"][1];
    assert_eq!(block["type"], "resource");
    assert_eq!(block["resource"]["mimeType"], "application/pdf");
    assert!(
        block["resource"]["uri"]
            .as_str()
            .unwrap()
            .contains(&media.to_string())
    );
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(block["resource"]["blob"].as_str().unwrap())
        .unwrap();
    assert_eq!(bytes, pdf);
}

#[test]
fn get_media_large_document_gets_a_message() {
    let host = empty_host();
    let id = with(&host, |s| add_person(s, draft("Ivan", "Koval", None, None)));
    let mut big = b"%PDF-1.4\n".to_vec();
    big.resize(8 * 1024 * 1024 + 1, b' ');
    let media = attach(&host, id, "big.pdf", &big);
    let result = call_raw(&host, "get_media", json!({"id": media}));
    assert_ne!(result["isError"], json!(true), "{result}");
    let content = result["content"].as_array().unwrap();
    assert_eq!(content.len(), 2, "details and a message, no blob");
    assert_eq!(content[1]["type"], "text");
    assert!(content[1]["text"].as_str().unwrap().contains("8 MiB"));
}

#[test]
fn paged_lists() {
    let host = empty_host();
    with(&host, |s| {
        for i in 0..230 {
            add_person(s, draft(&format!("Name{i:03}"), "Koval", None, None));
        }
    });
    // The default page is 50 and the largest is 200.
    let first = call(&host, "list_people", json!({}));
    assert_eq!(first["total"], 230);
    assert_eq!(first["offset"], 0);
    assert_eq!(first["limit"], 50);
    assert_eq!(first["items"].as_array().unwrap().len(), 50);
    assert_eq!(first["nextOffset"], 50);
    let big = call(&host, "list_people", json!({"limit": 1000}));
    assert_eq!(big["limit"], 200);
    assert_eq!(big["items"].as_array().unwrap().len(), 200);
    // The pages cover everyone exactly once.
    let mut seen = Vec::new();
    let mut offset = 0;
    loop {
        let page = call(
            &host,
            "list_people",
            json!({"offset": offset, "limit": 100}),
        );
        seen.extend(ids(&page["items"]));
        match page["nextOffset"].as_u64() {
            Some(next) => offset = next,
            None => break,
        }
    }
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 230);
    // Search pages past the session's own limit of 50 hits.
    let found = call(
        &host,
        "search_people",
        json!({"query": "Koval", "offset": 200, "limit": 200}),
    );
    assert_eq!(found["total"], 230);
    assert_eq!(found["items"].as_array().unwrap().len(), 30);
    assert_eq!(found["nextOffset"], Value::Null);
    // A bad page is the agent's to fix, not a protocol failure.
    assert!(call_err(&host, "list_people", json!({"offset": -1})).contains("offset"));
    assert!(call_err(&host, "list_people", json!({"limit": "many"})).contains("limit"));
}

#[test]
fn sources_and_places_page_and_filter() {
    let host = empty_host();
    with(&host, |s| {
        for i in 0..210 {
            s.apply(EditCommand::SaveSource {
                source: SourceInput {
                    id: None,
                    title: Some(format!("Recording {i:03}")),
                    author: None,
                    publication: None,
                    abbreviation: None,
                    text: None,
                },
            })
            .unwrap();
        }
        let mut born = draft("Ivan", "Koval", None, Some("1901"));
        born.birth_place_text = Some("Житомир, Україна".into());
        add_person(s, born);
    });
    let all = call(&host, "list_sources", json!({"limit": 200}));
    assert_eq!(all["total"], 210);
    assert_eq!(all["items"].as_array().unwrap().len(), 200);
    let rest = call(&host, "list_sources", json!({"offset": 200, "limit": 200}));
    assert_eq!(rest["items"].as_array().unwrap().len(), 10);
    let one = call(&host, "list_sources", json!({"query": "recording 007"}));
    assert_eq!(one["total"], 1);
    assert_eq!(one["items"][0]["title"], "Recording 007");

    let places = call(&host, "list_places", json!({"query": "житомир"}));
    assert_eq!(places["total"], 1);
    assert_eq!(places["items"][0]["full"], "Житомир, Україна");
    assert_eq!(
        call(&host, "list_places", json!({"query": "Lviv"}))["total"],
        0
    );
}

#[test]
fn tree_summary_counts_and_home_person() {
    let host = empty_host();
    let id = with(&host, |s| {
        let id = add_person(s, draft("Ivan", "Koval", None, None));
        add_note(s, id, "A note");
        s.set_settings(SettingsPatch {
            home_person: Some(Some(id)),
            ..Default::default()
        })
        .unwrap();
        id
    });
    let summary = call(&host, "tree_summary", json!({}));
    assert_eq!(summary["name"], "project");
    assert_eq!(summary["counts"]["people"], 1);
    assert_eq!(summary["counts"]["notes"], 1);
    assert_eq!(summary["counts"]["families"], 0);
    assert_eq!(summary["homePerson"]["id"], json!(id.to_string()));
    assert_eq!(summary["homePerson"]["name"], "Ivan Koval");
    assert!(
        summary.get("dir").is_none(),
        "where the tree lives is not the agent's business"
    );
}

#[test]
fn log_and_history_read_back() {
    let host = empty_host();
    let id = with(&host, |s| {
        let id = add_person(s, draft("Ivan", "Koval", None, None));
        add_note(s, id, "A note");
        id
    });
    let log = call(
        &host,
        "get_log",
        json!({"filters": {"person": id}, "limit": 1}),
    );
    assert_eq!(log["total"], 2);
    assert_eq!(log["items"].as_array().unwrap().len(), 1);
    assert_eq!(log["nextOffset"], 1);
    assert_eq!(log["items"][0]["actor"]["kind"], "user");
    let none = call(&host, "get_log", json!({"filters": {"actorKind": "agent"}}));
    assert_eq!(none["total"], 0);
    assert!(
        call_err(&host, "get_log", json!({"filters": {"batch": "not-an-id"}})).contains("filters")
    );

    let history = call(&host, "get_history", json!({"id": id}));
    let items = history["items"].as_array().unwrap();
    assert_eq!(history["total"], 2);
    assert_eq!(items.first().unwrap()["op"], "update", "{history}");
    assert_eq!(items.last().unwrap()["op"], "create", "{history}");
}

#[test]
fn find_by_number_reads_the_agents_numbering() {
    let host = empty_host();
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("project");
    let id = Uuid::now_v7();
    // No write tool yet: the number is stored directly, the way `AddNumber` will store it.
    let mut tree = gt_core::tree::Tree::new();
    let mut person = with(&host, |s| {
        let new = add_person(s, draft("Ivan", "Koval", None, None));
        s.tree().persons[&new].clone()
    });
    person.common.id = id;
    person.common.ext_ids.push(gt_core::model::ExternalId {
        kind: gt_core::model::ExtIdKind::Exid {
            scheme: "agent:Recording 2".into(),
        },
        value: "12".into(),
    });
    tree.persons.insert(id, person);
    let mut project = gt_store::Project::create(&proj).unwrap();
    project.save_tree(&tree, 2_460_000).unwrap();
    drop(project);
    *host.session.lock().unwrap() = Some(Session::open(&proj, SessionClock::system()).unwrap());

    let hit = call(
        &host,
        "find_by_number",
        json!({"source_file": "Recording 2", "number": "12"}),
    );
    assert_eq!(hit["person"], json!(id.to_string()));
    // A number is unique within one file only.
    let other_file = call(
        &host,
        "find_by_number",
        json!({"source_file": "Recording 3", "number": "12"}),
    );
    assert_eq!(other_file["person"], Value::Null);
    let other_number = call(
        &host,
        "find_by_number",
        json!({"source_file": "Recording 2", "number": "13"}),
    );
    assert_eq!(other_number["person"], Value::Null);
    assert_eq!(
        call(&host, "get_person", json!({"id": id}))["numbers"],
        json!([{
            "scheme": "Exid",
            "name": "agent:Recording 2",
            "value": "12",
            "sourceFile": "Recording 2",
            "number": "12",
        }])
    );
}

#[test]
fn no_tree_open_clear_error() {
    let host = no_tree_host();
    let id = Uuid::from_u128(1);
    for (tool, args) in [
        ("tree_summary", json!({})),
        ("search_people", json!({"query": "Koval"})),
        ("list_people", json!({})),
        ("get_person", json!({"id": id})),
        ("get_relatives", json!({"id": id})),
        ("get_family", json!({"id": id})),
        ("get_source", json!({"id": id})),
        ("get_note", json!({"id": id})),
        ("get_citation", json!({"id": id})),
        ("list_sources", json!({})),
        ("list_places", json!({})),
        ("get_media", json!({"id": id})),
        ("get_log", json!({})),
        ("get_history", json!({"id": id})),
        ("find_by_number", json!({"source_file": "a", "number": "1"})),
        ("get_event", json!({"id": id})),
        ("get_place", json!({"id": id})),
        ("get_repository", json!({"id": id})),
        ("list_media", json!({})),
        ("list_notes", json!({})),
        ("list_families", json!({})),
        ("list_repositories", json!({})),
        ("list_events", json!({})),
        ("list_citations", json!({})),
    ] {
        let message = call_err(&host, tool, args);
        assert!(message.contains("No tree is open"), "{tool}: {message}");
    }
    // The rules need no tree.
    assert_eq!(
        call_raw(&host, "get_rules", json!({}))["content"][0]["text"],
        RULES
    );
}

#[test]
fn bad_arguments_are_tool_errors() {
    let host = empty_host();
    assert!(call_err(&host, "get_person", json!({})).contains("id"));
    assert!(call_err(&host, "get_person", json!({"id": "not-a-uuid"})).contains("id"));
    assert!(call_err(&host, "search_people", json!({})).contains("query"));
    assert!(call_err(&host, "get_person", json!({"id": Uuid::from_u128(1)})).contains("No person"));
}

#[test]
fn cyrillic_name_roundtrip() {
    let host = empty_host();
    let id = with(&host, |s| {
        let id = add_person(s, draft("Іван", "Коваль", Some("Петрович"), Some("1901")));
        add_note(s, id, "Записано зі слів бабусі");
        id
    });
    let person = call(&host, "get_person", json!({"id": id}));
    assert_eq!(person["ref"]["name"], "Іван Петрович Коваль");
    assert_eq!(person["names"][0]["patronymic"], "Петрович");
    assert_eq!(person["notes"][0]["text"], "Записано зі слів бабусі");
    let found = call(&host, "search_people", json!({"query": "Коваль"}));
    assert_eq!(found["items"][0]["name"], "Іван Петрович Коваль");
    assert_eq!(ids(&found["items"]), [id.to_string()]);
    // The text on the wire is the letters themselves.
    let raw = call_raw(&host, "get_person", json!({"id": id}));
    assert!(
        raw["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Коваль")
    );
}

// ---- limits on what one call can cost ----

#[test]
fn free_text_arguments_are_capped_at_one_kib() {
    let host = empty_host();
    let over = "я".repeat(513);
    let at_limit = "a".repeat(1024);
    for (tool, args) in [
        ("search_people", json!({"query": over})),
        ("list_sources", json!({"query": over})),
        ("list_places", json!({"query": over})),
        ("list_notes", json!({"query": over})),
        ("list_media", json!({"query": over})),
        ("list_families", json!({"query": over})),
        ("list_repositories", json!({"query": over})),
        ("list_events", json!({"query": over})),
        ("list_citations", json!({"query": over})),
        (
            "find_by_number",
            json!({"source_file": over, "number": "1"}),
        ),
        (
            "find_by_number",
            json!({"source_file": "a", "number": over}),
        ),
        ("get_log", json!({"filters": {"actorKind": over}})),
    ] {
        let message = call_err(&host, tool, args);
        assert!(message.contains("1 KiB"), "{tool}: {message}");
    }
    let fine = call(&host, "search_people", json!({"query": at_limit}));
    assert_eq!(fine["total"], 0);
}

#[test]
fn get_history_pages_like_the_log() {
    let host = empty_host();
    let id = with(&host, |s| {
        let id = add_person(s, draft("Ivan", "Koval", None, None));
        for i in 0..120 {
            add_note(s, id, &format!("Note {i}"));
        }
        id
    });
    let first = call(&host, "get_history", json!({"id": id}));
    assert_eq!(first["total"], 121);
    assert_eq!(first["offset"], 0);
    assert_eq!(first["limit"], 50);
    assert_eq!(first["items"].as_array().unwrap().len(), 50);
    assert_eq!(first["nextOffset"], 50);
    let capped = call(&host, "get_history", json!({"id": id, "limit": 1000}));
    assert_eq!(capped["limit"], 50);
    assert_eq!(capped["items"].as_array().unwrap().len(), 50);
    let last = call(&host, "get_history", json!({"id": id, "offset": 100}));
    assert_eq!(last["items"].as_array().unwrap().len(), 21);
    assert_eq!(last["nextOffset"], Value::Null);
    assert_eq!(last["items"][20]["op"], "create");
    assert_eq!(
        call(&host, "get_log", json!({"limit": 1000}))["limit"],
        50,
        "the log pages at 50 too"
    );
}

#[test]
fn get_person_bounds_its_notes_and_says_how_to_read_the_rest() {
    let host = empty_host();
    let id = with(&host, |s| {
        let id = add_person(s, draft("Ivan", "Koval", None, None));
        for i in 0..60 {
            add_note(s, id, &format!("Note number {i:03}"));
        }
        id
    });
    let first = call(&host, "get_person", json!({"id": id}));
    assert_eq!(first["notes"].as_array().unwrap().len(), 20);
    assert_eq!(first["notesTotal"], 60);
    assert_eq!(first["notesNextOffset"], 20);
    assert!(
        first["notesHint"]
            .as_str()
            .unwrap()
            .contains("notes_offset")
    );
    // Reading on reaches every note, in order.
    let mut seen = Vec::new();
    let mut offset = 0;
    loop {
        let page = call(
            &host,
            "get_person",
            json!({"id": id, "notes_offset": offset}),
        );
        for note in page["notes"].as_array().unwrap() {
            seen.push(note["text"].as_str().unwrap().to_string());
        }
        match page["notesNextOffset"].as_u64() {
            Some(next) => offset = next,
            None => break,
        }
    }
    let expected: Vec<String> = (0..60).map(|i| format!("Note number {i:03}")).collect();
    assert_eq!(seen, expected);
}

#[test]
fn get_person_notes_stop_at_a_size_budget_but_never_before_the_first() {
    let host = empty_host();
    let long = "x".repeat(30_000);
    let id = with(&host, |s| {
        let id = add_person(s, draft("Ivan", "Koval", None, None));
        add_note(s, id, &long);
        add_note(s, id, &long);
        id
    });
    let first = call(&host, "get_person", json!({"id": id}));
    assert_eq!(first["notes"].as_array().unwrap().len(), 1);
    assert_eq!(first["notes"][0]["text"].as_str().unwrap(), long);
    assert_eq!(first["notesNextOffset"], 1);
    let second = call(&host, "get_person", json!({"id": id, "notes_offset": 1}));
    assert_eq!(second["notes"][0]["text"].as_str().unwrap(), long);
    assert_eq!(second["notesNextOffset"], Value::Null);
}

#[test]
fn a_cited_transcript_does_not_come_back_with_every_citation() {
    let host = empty_host();
    let transcript = "Слова запису. ".repeat(10_000) + "Кінець.";
    let (person, source) = with(&host, |s| {
        let person = add_person(s, draft("Ivan", "Koval", None, Some("1901")));
        let source = s
            .apply(EditCommand::SaveSource {
                source: SourceInput {
                    id: None,
                    title: Some("Recording 2".into()),
                    author: None,
                    publication: None,
                    abbreviation: None,
                    text: Some(transcript.clone()),
                },
            })
            .unwrap()
            .touched[0];
        s.apply(EditCommand::AddCitation {
            target: OwnerRef {
                kind: OwnerKind::Person,
                id: person,
            },
            citation: CitationArg::New(CitationInput {
                source: SourceRef::Id(source),
                page: None,
                date_raw: None,
                text: None,
                quality: None,
                evidence: None,
            }),
        })
        .unwrap();
        (person, source)
    });
    let raw = call_raw(&host, "get_person", json!({"id": person}));
    let text = raw["content"][0]["text"].as_str().unwrap();
    assert!(!text.contains("Слова запису"), "{} bytes", text.len());
    let shown: Value = serde_json::from_str(text).unwrap();
    assert_eq!(
        shown["citations"][0]["source"]["textLength"],
        transcript.chars().count()
    );
    assert_eq!(
        call(&host, "get_source", json!({"id": source}))["text"]
            .as_str()
            .unwrap(),
        transcript
    );
}

#[test]
fn long_text_can_be_read_in_pieces() {
    let host = empty_host();
    let text: String = "Абвгд".repeat(100);
    let (source, note) = with(&host, |s| {
        let person = add_person(s, draft("Ivan", "Koval", None, None));
        add_note(s, person, &text);
        let note = s.tree().notes.keys().next().copied().unwrap();
        let source = s
            .apply(EditCommand::SaveSource {
                source: SourceInput {
                    id: None,
                    title: Some("Recording 2".into()),
                    author: None,
                    publication: None,
                    abbreviation: None,
                    text: Some(text.clone()),
                },
            })
            .unwrap()
            .touched[0];
        (source, note)
    });
    for (tool, id) in [("get_source", source), ("get_note", note)] {
        let mut joined = String::new();
        let mut offset = 0;
        loop {
            let piece = call(
                &host,
                tool,
                json!({"id": id, "text_offset": offset, "text_limit": 70}),
            );
            assert_eq!(piece["textLength"], 500, "{tool}");
            joined.push_str(piece["text"].as_str().unwrap());
            match piece["nextTextOffset"].as_u64() {
                Some(next) => offset = next,
                None => break,
            }
        }
        assert_eq!(joined, text, "{tool}");
    }
}

#[test]
fn descriptions_say_what_to_page() {
    let host = empty_host();
    let list = gt_mcp::handle_message(&host, &ctx(), &request("tools/list", json!({}))).unwrap();
    let description = |name: &str| -> String {
        list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap()["description"]
            .as_str()
            .unwrap()
            .to_string()
    };
    for (tool, word) in [
        ("get_person", "notes_offset"),
        ("get_source", "text_limit"),
        ("get_note", "text_limit"),
        ("get_history", "offset"),
        ("get_log", "offset"),
        ("list_notes", "offset"),
        ("get_media", "max_size"),
    ] {
        assert!(description(tool).contains(word), "{tool}");
    }
}

// ---- files that cannot be shown as pictures ----

#[test]
fn a_picture_that_will_not_decode_comes_back_as_a_blob() {
    let host = empty_host();
    let id = with(&host, |s| add_person(s, draft("Ivan", "Koval", None, None)));
    let media = attach(&host, id, "broken.jpg", &jpeg_bytes(40, 20));
    let path = with(&host, |s| s.media_original_path(media).unwrap());
    let garbage = b"this is not a picture at all".to_vec();
    std::fs::write(&path, &garbage).unwrap();
    let result = call_raw(&host, "get_media", json!({"id": media}));
    let block = &result["content"][1];
    assert_eq!(block["type"], "resource", "{result}");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(block["resource"]["blob"].as_str().unwrap())
        .unwrap();
    assert_eq!(bytes, garbage);
}

#[test]
fn a_picture_file_too_big_to_decode_is_refused_with_a_clear_message() {
    let host = empty_host();
    let id = with(&host, |s| add_person(s, draft("Ivan", "Koval", None, None)));
    let media = attach(&host, id, "huge.jpg", &jpeg_bytes(40, 20));
    let path = with(&host, |s| s.media_original_path(media).unwrap());
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_len(257 * 1024 * 1024).unwrap();
    drop(file);
    let result = call_raw(&host, "get_media", json!({"id": media}));
    let content = result["content"].as_array().unwrap();
    assert_eq!(content.len(), 2, "details and a message: {result}");
    let message = content[1]["text"].as_str().unwrap();
    assert!(message.contains("256 MiB"), "{message}");
    let details: Value = serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(details["originalName"], "huge.jpg");
}

// ---- read everything ----

/// Text no other field holds, so finding it proves this field was shown.
fn mark(field: &str) -> String {
    format!("mark-{field}")
}

fn date(field: &str) -> GenDate {
    GenDate::parse(&mark(field), DayOrder::Dmy)
}

fn raw_node(field: &str) -> RawNode {
    RawNode {
        tag: mark(&format!("{field}-tag")),
        xref: Some(mark(&format!("{field}-xref"))),
        value: Some(mark(&format!("{field}-value"))),
        pos: 7,
        children: vec![RawNode {
            tag: mark(&format!("{field}-child")),
            xref: None,
            value: None,
            pos: 8,
            children: Vec::new(),
        }],
    }
}

/// Everything the tree can hold on a record, with a link of each kind to the given records.
fn common(id: Uuid, field: &str, links: &Links) -> Common {
    Common {
        id,
        private: true,
        changed: Some(1_700_000_123),
        citations: vec![links.citation],
        notes: vec![links.note],
        media: vec![MediaLink {
            media: links.media,
            region: Some(Region {
                x: 0.25,
                y: 0.5,
                w: 0.125,
                h: 0.75,
            }),
            primary: true,
        }],
        ext_ids: vec![
            ExternalId {
                kind: ExtIdKind::Xref,
                value: mark(&format!("{field}-xref-id")),
            },
            ExternalId {
                kind: ExtIdKind::Uid,
                value: mark(&format!("{field}-uid")),
            },
            ExternalId {
                kind: ExtIdKind::FamilySearch,
                value: mark(&format!("{field}-familysearch")),
            },
            ExternalId {
                kind: ExtIdKind::AncestryApid,
                value: mark(&format!("{field}-apid")),
            },
            ExternalId {
                kind: ExtIdKind::Exid {
                    scheme: mark(&format!("{field}-exid-scheme")),
                },
                value: mark(&format!("{field}-exid")),
            },
            ExternalId {
                kind: ExtIdKind::Other(mark(&format!("{field}-other-scheme"))),
                value: mark(&format!("{field}-other")),
            },
        ],
        unknown: vec![raw_node(field)],
    }
}

#[derive(Clone, Copy)]
struct Links {
    citation: Uuid,
    note: Uuid,
    media: Uuid,
}

fn assertion() -> Assertion {
    Assertion {
        preferred: true,
        proof: Some(Proof::Disproven),
        confidence: Some(3),
    }
}

/// One of each record, every field set. Struct literals: a new model field stops this compiling.
struct Seeded {
    person: Uuid,
    name: Uuid,
    family: Uuid,
    event: Uuid,
    place: Uuid,
    source: Uuid,
    repository: Uuid,
    citation: Uuid,
    media: Uuid,
    note: Uuid,
    orphan_note: Uuid,
    orphan_media: Uuid,
    orphan_family: Uuid,
    orphan_repository: Uuid,
    orphan_citation: Uuid,
    orphan_event: Uuid,
}

fn seed_everything(tree: &mut gt_core::tree::Tree) -> Seeded {
    let n = |i: u128| Uuid::from_u128(0x1000 + i);
    let ids = Seeded {
        person: n(1),
        name: n(2),
        family: n(3),
        event: n(4),
        place: n(5),
        source: n(6),
        repository: n(7),
        citation: n(8),
        media: n(9),
        note: n(10),
        orphan_note: n(11),
        orphan_media: n(12),
        orphan_family: n(13),
        orphan_repository: n(14),
        orphan_citation: n(15),
        orphan_event: n(16),
    };
    let parent_place = n(20);
    let child = n(21);
    let partner = n(22);
    // The note, citation and media link to one another, and through them to second records.
    let links = Links {
        citation: ids.citation,
        note: ids.note,
        media: ids.media,
    };
    let second = Links {
        citation: n(30),
        note: n(31),
        media: n(32),
    };
    let plain = |id: Uuid| Common {
        id,
        private: false,
        changed: None,
        citations: Vec::new(),
        notes: Vec::new(),
        media: Vec::new(),
        ext_ids: Vec::new(),
        unknown: Vec::new(),
    };
    let plain_name = |given: &str| Name {
        id: Uuid::now_v7(),
        kind: NameKind::Birth,
        assertion: Assertion {
            preferred: true,
            proof: None,
            confidence: None,
        },
        private: false,
        prefix: None,
        given: Some(given.into()),
        call: None,
        surname_prefix: None,
        surnames: vec!["Koval".into()],
        patronymic: None,
        suffix: None,
        nickname: None,
        lang: None,
        date: None,
        raw: None,
        citations: Vec::new(),
        notes: Vec::new(),
        unknown: Vec::new(),
    };
    let plain_person = |id: Uuid, given: &str| Person {
        common: plain(id),
        sex: Sex::Unknown,
        names: vec![plain_name(given)],
        living_override: None,
    };
    tree.persons.insert(child, plain_person(child, "Olena"));
    tree.persons.insert(partner, plain_person(partner, "Petro"));
    tree.persons.insert(
        ids.person,
        Person {
            common: common(ids.person, "person", &links),
            sex: Sex::Other(mark("person-sex")),
            names: vec![Name {
                id: ids.name,
                kind: NameKind::Other(mark("name-kind")),
                assertion: assertion(),
                private: true,
                prefix: Some(mark("name-prefix")),
                given: Some(mark("name-given")),
                call: Some(mark("name-call")),
                surname_prefix: Some(mark("name-surname-prefix")),
                surnames: vec![mark("name-surname")],
                patronymic: Some(mark("name-patronymic")),
                suffix: Some(mark("name-suffix")),
                nickname: Some(mark("name-nickname")),
                lang: Some(mark("name-lang")),
                date: Some(date("name-date")),
                raw: Some(mark("name-raw")),
                citations: vec![second.citation],
                notes: vec![second.note],
                unknown: vec![raw_node("name")],
            }],
            living_override: Some(LivingOverride::Deceased),
        },
    );
    tree.families.insert(
        ids.family,
        Family {
            common: common(ids.family, "family", &links),
            partner1: Some(ids.person),
            partner2: Some(partner),
            kind: UnionKind::Other(mark("family-kind")),
            children: vec![ChildLink {
                person: child,
                rel1: ParentRel::Other(mark("child-rel1")),
                rel2: ParentRel::Foster,
                preferred: true,
                assertion: assertion(),
                citations: vec![second.citation],
                unknown: vec![raw_node("child")],
            }],
        },
    );
    tree.events.insert(
        ids.event,
        Event {
            common: common(ids.event, "event", &links),
            kind: EventKind::Custom(mark("event-kind")),
            date: Some(date("event-date")),
            place: Some(ids.place),
            description: Some(mark("event-description")),
            age: Some(mark("event-age")),
            cause: Some(mark("event-cause")),
            negative: true,
            assertion: assertion(),
            participants: vec![
                Participant {
                    target: ParticipantRef::Person(ids.person),
                    role: Role::Principal,
                },
                Participant {
                    target: ParticipantRef::Family(ids.family),
                    role: Role::Other(mark("event-role")),
                },
            ],
        },
    );
    tree.places.insert(
        ids.place,
        Place {
            common: common(ids.place, "place", &links),
            names: vec![PlaceName {
                name: mark("place-name"),
                lang: Some(mark("place-lang")),
                date: Some(date("place-date")),
            }],
            parent: Some(parent_place),
            kind: Some(mark("place-kind")),
            lat: Some(50.25),
            lon: Some(28.75),
            raw: Some(mark("place-raw")),
        },
    );
    tree.places.insert(
        parent_place,
        Place {
            common: plain(parent_place),
            names: vec![PlaceName {
                name: mark("parent-place"),
                lang: None,
                date: None,
            }],
            parent: None,
            kind: None,
            lat: None,
            lon: None,
            raw: None,
        },
    );
    tree.sources.insert(
        ids.source,
        Source {
            common: common(ids.source, "source", &links),
            title: Some(mark("source-title")),
            author: Some(mark("source-author")),
            publication: Some(mark("source-publication")),
            abbreviation: Some(mark("source-abbreviation")),
            text: Some(mark("source-text")),
            repositories: vec![RepoRef {
                repository: ids.repository,
                call_number: Some(mark("source-call-number")),
            }],
        },
    );
    tree.repositories.insert(
        ids.repository,
        Repository {
            common: common(ids.repository, "repository", &links),
            name: mark("repository-name"),
            address: Some(mark("repository-address")),
            urls: vec![mark("repository-url")],
        },
    );
    tree.citations.insert(
        ids.citation,
        Citation {
            common: common(ids.citation, "citation", &second),
            source: Some(ids.source),
            page: Some(mark("citation-page")),
            date: Some(date("citation-date")),
            text: Some(mark("citation-text")),
            quality: Some(2),
            evidence: Evidence {
                original: Some(true),
                primary_info: Some(false),
                direct: Some(true),
            },
        },
    );
    tree.citations.insert(
        second.citation,
        Citation {
            common: plain(second.citation),
            source: None,
            page: Some(mark("second-citation")),
            date: None,
            text: None,
            quality: None,
            evidence: Evidence {
                original: None,
                primary_info: None,
                direct: None,
            },
        },
    );
    tree.media.insert(
        ids.media,
        Media {
            common: common(ids.media, "media", &second),
            path: mark("media-path"),
            original_name: Some(mark("media-original-name")),
            mime: Some(mark("media-mime")),
            title: Some(mark("media-title")),
            date: Some(date("media-date")),
            description: Some(mark("media-description")),
            sha256: Some(mark("media-sha")),
        },
    );
    let plain_media = |id: Uuid, title: &str| Media {
        common: plain(id),
        path: mark("plain-path"),
        original_name: None,
        mime: None,
        title: Some(title.into()),
        date: None,
        description: None,
        sha256: None,
    };
    tree.media
        .insert(second.media, plain_media(second.media, "second-media"));
    tree.media.insert(
        ids.orphan_media,
        plain_media(ids.orphan_media, "orphan-media"),
    );
    let plain_note = |id: Uuid, text: &str| Note {
        common: plain(id),
        kind: NoteKind::General,
        text: mark(text),
        lang: None,
        shared: false,
    };
    tree.notes.insert(
        ids.note,
        Note {
            common: common(ids.note, "note", &second),
            kind: NoteKind::Other(mark("note-kind")),
            text: mark("note-text"),
            lang: Some(mark("note-lang")),
            shared: true,
        },
    );
    tree.notes
        .insert(second.note, plain_note(second.note, "second-note"));
    tree.notes
        .insert(ids.orphan_note, plain_note(ids.orphan_note, "orphan-note"));
    tree.families.insert(
        ids.orphan_family,
        Family {
            common: plain(ids.orphan_family),
            partner1: None,
            partner2: None,
            kind: UnionKind::Unknown,
            children: Vec::new(),
        },
    );
    tree.repositories.insert(
        ids.orphan_repository,
        Repository {
            common: plain(ids.orphan_repository),
            name: "orphan-repository".into(),
            address: None,
            urls: Vec::new(),
        },
    );
    tree.citations.insert(
        ids.orphan_citation,
        Citation {
            common: plain(ids.orphan_citation),
            source: None,
            page: Some("orphan-citation".into()),
            date: None,
            text: None,
            quality: None,
            evidence: Evidence {
                original: None,
                primary_info: None,
                direct: None,
            },
        },
    );
    tree.events.insert(
        ids.orphan_event,
        Event {
            common: plain(ids.orphan_event),
            kind: EventKind::Custom("orphan-event".into()),
            date: None,
            place: None,
            description: None,
            age: None,
            cause: None,
            negative: false,
            assertion: Assertion {
                preferred: false,
                proof: None,
                confidence: None,
            },
            participants: Vec::new(),
        },
    );
    ids
}

/// Asserts every `Common` field of a record is in `shown`.
fn assert_common_shown(shown: &str, field: &str, value: &Value) {
    for part in [
        "xref-id",
        "uid",
        "familysearch",
        "apid",
        "exid-scheme",
        "exid",
        "other-scheme",
        "other",
        "tag",
        "xref",
        "value",
        "child",
    ] {
        let wanted = mark(&format!("{field}-{part}"));
        assert!(shown.contains(&wanted), "{wanted} is not in {shown}");
    }
    assert_eq!(value["private"], true, "{field}: private");
    assert_eq!(value["changed"], 1_700_000_123_i64, "{field}: changed");
    for scheme in [
        "Xref",
        "Uid",
        "FamilySearch",
        "AncestryApid",
        "Exid",
        "Other",
    ] {
        assert!(
            value["numbers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n["scheme"] == scheme),
            "{field}: scheme {scheme}"
        );
    }
    assert_eq!(value["unknown"][0]["pos"], 7, "{field}: raw node");
    assert_eq!(
        value["unknown"][0]["children"][0]["pos"], 8,
        "{field}: raw child"
    );
}

fn assert_links_shown(value: &Value, field: &str, s: &Links) {
    let has = |list: &str, id: Uuid| {
        value[list]
            .as_array()
            .unwrap_or_else(|| panic!("{field}: no {list} in {value}"))
            .iter()
            .any(|row| row["id"] == json!(id) || row["media"]["id"] == json!(id))
    };
    assert!(has("citations", s.citation), "{field}: citation link");
    assert!(has("notes", s.note), "{field}: note link");
    assert!(has("media", s.media), "{field}: media link");
    let media = &value["media"][0];
    assert_eq!(media["primary"], true, "{field}: primary");
    assert_eq!(media["region"]["x"], 0.25, "{field}: region x");
    assert_eq!(media["region"]["y"], 0.5, "{field}: region y");
    assert_eq!(media["region"]["w"], 0.125, "{field}: region w");
    assert_eq!(media["region"]["h"], 0.75, "{field}: region h");
}

fn shows_all(tool: &str, args: Value, host: &TestHost, marks: &[&str]) -> (String, Value) {
    let result = call_raw(host, tool, args);
    assert_ne!(result["isError"], json!(true), "{tool}: {result}");
    let shown = result["content"][0]["text"].as_str().unwrap().to_string();
    for field in marks {
        let wanted = mark(field);
        assert!(
            shown.contains(&wanted),
            "{tool}: {wanted} is not in {shown}"
        );
    }
    let value = serde_json::from_str(&shown).unwrap();
    (shown, value)
}

#[test]
fn every_field_of_every_record_is_readable() {
    let host = empty_host();
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("project");
    let mut tree = gt_core::tree::Tree::new();
    let ids = seed_everything(&mut tree);
    let mut project = gt_store::Project::create(&proj).unwrap();
    project.save_tree(&tree, 2_460_000).unwrap();
    drop(project);
    *host.session.lock().unwrap() = Some(Session::open(&proj, SessionClock::system()).unwrap());
    let links = Links {
        citation: ids.citation,
        note: ids.note,
        media: ids.media,
    };
    let second = Links {
        citation: Uuid::from_u128(0x1000 + 30),
        note: Uuid::from_u128(0x1000 + 31),
        media: Uuid::from_u128(0x1000 + 32),
    };

    // Person, with the name and the event it takes part in.
    let (shown, person) = shows_all(
        "get_person",
        json!({"id": ids.person}),
        &host,
        &[
            "person-sex",
            "name-kind",
            "name-prefix",
            "name-given",
            "name-call",
            "name-surname-prefix",
            "name-surname",
            "name-patronymic",
            "name-suffix",
            "name-nickname",
            "name-lang",
            "name-date",
            "name-raw",
            "name-tag",
            "name-child",
            "second-citation",
            "second-note",
            "event-kind",
        ],
    );
    assert_common_shown(&shown, "person", &person);
    assert_links_shown(&person, "person", &links);
    assert_eq!(person["livingOverride"], "deceased");
    let name = &person["names"][0];
    assert_eq!(name["id"], json!(ids.name));
    assert_eq!(name["private"], true);
    assert_eq!(name["preferred"], true);
    assert_eq!(name["proof"], "disproven");
    assert_eq!(name["confidence"], 3);
    assert_eq!(name["unknown"][0]["pos"], 7);

    // Family, with the child link and its relationship.
    let (shown, family) = shows_all(
        "get_family",
        json!({"id": ids.family}),
        &host,
        &[
            "family-kind",
            "child-rel1",
            "child-tag",
            "child-child",
            "second-citation",
        ],
    );
    assert_common_shown(&shown, "family", &family);
    assert_links_shown(&family, "family", &links);
    let partner_row = &family["partners"][0];
    assert_eq!(partner_row["id"], json!(ids.person));
    let kid = &family["children"][0];
    assert_eq!(kid["rel"][1], "foster");
    assert_eq!(kid["preferred"], true);
    assert_eq!(kid["assertion"]["proof"], "disproven");
    assert_eq!(kid["assertion"]["confidence"], 3);
    assert_eq!(kid["citations"][0]["id"], json!(second.citation));

    // Event.
    let (shown, event) = shows_all(
        "get_event",
        json!({"id": ids.event}),
        &host,
        &[
            "event-kind",
            "event-date",
            "event-description",
            "event-age",
            "event-cause",
            "event-role",
            "place-name",
        ],
    );
    assert_common_shown(&shown, "event", &event);
    assert_links_shown(&event, "event", &links);
    assert_eq!(event["negative"], true);
    assert_eq!(event["preferred"], true);
    assert_eq!(event["proof"], "disproven");
    assert_eq!(event["confidence"], 3);
    assert_eq!(event["place"]["id"], json!(ids.place));
    let parts = event["participants"].as_array().unwrap();
    assert_eq!(parts[0]["person"]["id"], json!(ids.person));
    assert_eq!(parts[0]["role"], "principal");
    assert_eq!(parts[1]["family"], json!(ids.family));
    // An event reads the same from the person it belongs to.
    assert_eq!(person["events"][0]["proof"], "disproven");
    assert_eq!(family["events"][0]["id"], json!(ids.event));

    // Place.
    let (shown, place) = shows_all(
        "get_place",
        json!({"id": ids.place}),
        &host,
        &[
            "place-name",
            "place-lang",
            "place-date",
            "place-kind",
            "place-raw",
            "parent-place",
        ],
    );
    assert_common_shown(&shown, "place", &place);
    assert_links_shown(&place, "place", &links);
    assert_eq!(place["lat"], 50.25);
    assert_eq!(place["lon"], 28.75);
    assert_eq!(
        place["parent"]["id"],
        json!(Uuid::from_u128(0x1000 + 20)),
        "{place}"
    );

    // Source.
    let (shown, source) = shows_all(
        "get_source",
        json!({"id": ids.source}),
        &host,
        &[
            "source-title",
            "source-author",
            "source-publication",
            "source-abbreviation",
            "source-text",
            "source-call-number",
            "repository-name",
        ],
    );
    assert_common_shown(&shown, "source", &source);
    assert_links_shown(&source, "source", &links);
    assert_eq!(source["repositories"][0]["id"], json!(ids.repository));

    // Repository, and the way back to its sources.
    let (shown, repository) = shows_all(
        "get_repository",
        json!({"id": ids.repository}),
        &host,
        &[
            "repository-name",
            "repository-address",
            "repository-url",
            "source-title",
            "source-call-number",
        ],
    );
    assert_common_shown(&shown, "repository", &repository);
    assert_links_shown(&repository, "repository", &links);
    assert_eq!(repository["sources"][0]["id"], json!(ids.source));

    // Citation, whose links go on to second records.
    let (shown, citation) = shows_all(
        "get_citation",
        json!({"id": ids.citation}),
        &host,
        &[
            "citation-page",
            "citation-date",
            "citation-text",
            "source-title",
            "second-citation",
            "second-note",
        ],
    );
    assert_common_shown(&shown, "citation", &citation);
    assert_links_shown(&citation, "citation", &second);
    assert_eq!(citation["quality"], 2);
    assert_eq!(citation["evidence"]["original"], true);
    assert_eq!(citation["evidence"]["primaryInfo"], false);
    assert_eq!(citation["evidence"]["direct"], true);

    // Media, details in the first block.
    let (shown, media) = shows_all(
        "get_media",
        json!({"id": ids.media}),
        &host,
        &[
            "media-path",
            "media-original-name",
            "media-mime",
            "media-title",
            "media-date",
            "media-description",
            "media-sha",
            "second-citation",
            "second-note",
        ],
    );
    assert_common_shown(&shown, "media", &media);
    assert_links_shown(&media, "media", &second);

    // Note.
    let (shown, note) = shows_all(
        "get_note",
        json!({"id": ids.note}),
        &host,
        &[
            "note-kind",
            "note-text",
            "note-lang",
            "second-citation",
            "second-note",
        ],
    );
    assert_common_shown(&shown, "note", &note);
    assert_links_shown(&note, "note", &second);
    assert_eq!(note["shared"], true);
    assert_eq!(note["kind"], "other");

    // A record nothing links to can still be found.
    for (tool, id) in [
        ("list_notes", ids.orphan_note),
        ("list_media", ids.orphan_media),
        ("list_families", ids.orphan_family),
        ("list_repositories", ids.orphan_repository),
        ("list_citations", ids.orphan_citation),
        ("list_events", ids.orphan_event),
    ] {
        let page = call(&host, tool, json!({"limit": 200}));
        assert!(
            ids_of(&page).contains(&id.to_string()),
            "{tool}: {id} is not in {page}"
        );
        assert_eq!(page["total"], page["items"].as_array().unwrap().len());
    }
    // And by its text, where it has any.
    for (tool, query) in [
        ("list_notes", "orphan-note"),
        ("list_media", "orphan-media"),
        ("list_repositories", "orphan-repository"),
        ("list_citations", "orphan-citation"),
        ("list_events", "orphan-event"),
    ] {
        let page = call(&host, tool, json!({"query": query}));
        assert_eq!(page["total"], 1, "{tool}: {page}");
    }
    let sources = call(&host, "list_sources", json!({}));
    assert!(
        sources.to_string().contains(&mark("source-call-number")),
        "{sources}"
    );
    let places = call(&host, "list_places", json!({"query": "mark-place-name"}));
    assert_eq!(places["items"][0]["id"], json!(ids.place));
}

fn ids_of(page: &Value) -> Vec<String> {
    ids(&page["items"])
}
