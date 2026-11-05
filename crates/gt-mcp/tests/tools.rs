#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The read tools against a synthetic tree built through `Session::apply`.

mod common;

use base64::Engine;
use common::*;
use gt_mcp::RULES;
use gt_session::*;
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
    assert!(
        history["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["op"] == "create"),
        "{history}"
    );
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
        json!([{"sourceFile": "Recording 2", "number": "12"}])
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

/// What every write tool does after its commit: the app hears the records and their people.
#[test]
fn notify_changed_adds_the_people_and_names_the_agent() {
    let host = empty_host();
    let (person, note) = with(&host, |s| {
        let person = add_person(s, draft("Olena", "Koval", None, None));
        s.apply(EditCommand::SaveNote {
            target: OwnerRef {
                kind: OwnerKind::Person,
                id: person,
            },
            note: NoteInput {
                id: None,
                kind: NoteKindDto::General,
                text: "Born in the old house".into(),
            },
        })
        .unwrap();
        let detail = s.get_person(person).unwrap();
        (person, detail.notes[0].id)
    });
    gt_mcp::notify_changed(&host, &[note], &ctx());
    let changes = host.changes.lock().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].0, vec![note, person]);
    assert_eq!(changes[0].1, ctx());
}

/// The tree went away in between: the app still hears, with the ids it was given.
#[test]
fn notify_changed_with_no_tree_passes_the_ids_on() {
    let host = no_tree_host();
    let id = Uuid::from_u128(5);
    gt_mcp::notify_changed(&host, &[id], &ctx());
    assert_eq!(host.changes.lock().unwrap()[0].0, vec![id]);
}

#[test]
fn key_hash_is_sha256_of_the_key() {
    assert_eq!(gt_mcp::key_hash(KEY), hash(KEY));
}
