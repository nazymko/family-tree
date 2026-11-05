#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! A scripted client over real HTTP works through `fixtures/transcript_summary.md` against a
//! synthetic tree, twice, the way an agent following the rules would: it finds people before it
//! adds anyone, uses the marker for unnamed brothers and sisters, puts doubt and inference in
//! notes, and gives every person its number. The tree is then checked against the owner's
//! decisions.

#[path = "common/client.rs"]
mod client;
mod common;

use client::HttpClient;
use common::*;
use gt_mcp::serve;
use gt_session::{EditCommand, LivingOverrideDto, LivingPatch, Relation, SexKind};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

const FIXTURE: &str = include_str!("fixtures/transcript_summary.md");
const FILE: &str = "transcript_summary.md";

/// The fixture's numbered items, by number.
fn items() -> BTreeMap<u32, String> {
    FIXTURE
        .lines()
        .filter_map(|line| {
            let (n, text) = line.split_once(". ")?;
            Some((n.parse().ok()?, text.to_string()))
        })
        .collect()
}

/// Every capitalised word of the fixture must be one of these: invented names, places, and the
/// plain words that start a sentence. A real name has no way in.
const ALLOWED: &[&str] = &[
    "Recording",
    "The",
    "Every",
    "Her",
    "She",
    "Their",
    "Because",
    "Another",
    "He",
    "His",
    "Olena",
    "Koval",
    "Ivan",
    "Pavlo",
    "Pavlovych",
    "Petrovych",
    "Mykola",
    "Halyna",
    "Stepan",
    "Petro",
    "Maria",
    "Hanna",
    "Yaroslav",
    "Anna",
    "Poltava",
    "MAR",
    "April",
    "March",
    "Olena's",
    "Mykola's",
    "Ivan's",
];

#[test]
fn the_fixture_holds_only_invented_names() {
    let items = items();
    assert_eq!(
        items.keys().copied().collect::<Vec<_>>(),
        (1..=11).collect::<Vec<_>>()
    );
    for word in FIXTURE.split(|c: char| !(c.is_alphanumeric() || c == '\'')) {
        let Some(first) = word.chars().next() else {
            continue;
        };
        if first.is_uppercase() {
            assert!(
                ALLOWED.contains(&word),
                "{word:?} is not in the synthetic list"
            );
        }
    }
}

struct Seed {
    host: TestHost,
}

fn person(
    s: &mut gt_session::Session,
    anchor: Option<Uuid>,
    relation: Relation,
    d: gt_session::PersonDraft,
) -> Uuid {
    match anchor {
        Some(a) => add_relative(s, a, relation, d),
        None => add_person(s, d),
    }
}

/// The owner's tree: two Ivan Kovals, a family over four generations, one man the owner marked
/// deceased and one woman who is alive.
fn seeded() -> Seed {
    let host = empty_host();
    with(&host, |s| {
        let mut d = draft("Petro", "Koval", None, Some("1870"));
        d.sex = SexKind::Male;
        let petro = person(s, None, Relation::Child, d);
        let mut d = draft("Maria", "Koval", None, Some("1875"));
        d.sex = SexKind::Female;
        person(s, Some(petro), Relation::Partner, d);
        let mut d = draft("Ivan", "Koval", Some("Petrovych"), Some("1901"));
        d.sex = SexKind::Male;
        let ivan = person(s, Some(petro), Relation::Child, d);
        let mut d = draft("Olena", "Koval", None, Some("1905"));
        d.sex = SexKind::Female;
        person(s, Some(ivan), Relation::Partner, d);
        let mut d = draft("Mykola", "Koval", None, Some("1930"));
        d.sex = SexKind::Male;
        let mykola = person(s, Some(ivan), Relation::Child, d);
        let mut d = draft("Yaroslav", "Koval", None, Some("1950"));
        d.sex = SexKind::Unknown;
        let yaroslav = person(s, Some(mykola), Relation::Child, d);
        s.apply(EditCommand::UpdatePerson {
            id: yaroslav,
            sex: None,
            sex_text: None,
            living_override: LivingPatch::Set(LivingOverrideDto::Deceased),
            private: None,
        })
        .unwrap();
        let mut d = draft("Anna", "Koval", None, Some("1985"));
        d.sex = SexKind::Female;
        person(s, Some(mykola), Relation::Child, d);
        let mut d = draft("Ivan", "Koval", Some("Hryhorovych"), Some("1899"));
        d.sex = SexKind::Male;
        person(s, None, Relation::Child, d);
    });
    Seed { host }
}

fn counts(host: &TestHost) -> [usize; 7] {
    with(host, |s| {
        let t = s.tree();
        [
            t.persons.len(),
            t.families.len(),
            t.events.len(),
            t.notes.len(),
            t.sources.len(),
            t.citations.len(),
            t.persons.values().filter(|p| p.sibling_marker).count(),
        ]
    })
}

fn log_total(c: &HttpClient) -> u64 {
    c.call("get_log", json!({"limit": 1}))["total"]
        .as_u64()
        .unwrap()
}

/// The one person with this name and birth year, found as an agent does: search, then compare.
fn find(c: &HttpClient, query: &str, year: &str) -> Uuid {
    let found = c.call("search_people", json!({"query": query}));
    let matches: Vec<&Value> = found["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["birthYear"] == year)
        .collect();
    assert_eq!(matches.len(), 1, "{query} {year}: {found}");
    matches[0]["id"].as_str().unwrap().parse().unwrap()
}

/// How many people called Ivan the search finds.
fn ivans(c: &HttpClient) -> usize {
    let found = c.call("search_people", json!({"query": "Ivan Koval"}));
    let named = |p: &&Value| p["name"].as_str().unwrap().starts_with("Ivan");
    found["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(named)
        .count()
}

/// The search row of one person (name, years, sex, living).
fn row(c: &HttpClient, id: Uuid) -> Value {
    let found = c.call("search_people", json!({"query": "Koval", "limit": 50}));
    let rows = found["items"].as_array().unwrap();
    let id = id.to_string();
    rows.iter()
        .find(|p| p["id"] == id.as_str())
        .unwrap()
        .clone()
}

fn on(kind: &str, id: impl ToString) -> Value {
    json!({"kind": kind, "id": id.to_string()})
}

struct Run {
    batch: Uuid,
    ivan: Uuid,
    stepan: Uuid,
    anna: Uuid,
    yaroslav: Uuid,
    mykola: Uuid,
    halyna: Value,
}

/// One pass over the file, as the rules say.
fn work(c: &HttpClient, items: &BTreeMap<u32, String>) -> Run {
    let opened = c.call(
        "begin_batch",
        json!({"title": "Recording 2 summary", "source_file": FILE}),
    );
    let batch = opened["batch"].clone();
    let note = |target: Value, text: String| {
        c.call(
            "add_note",
            json!({"batch": batch, "target": target, "text": text}),
        );
    };
    let source = c.call(
        "add_source",
        json!({"batch": batch, "title": "Recording 2 summary"}),
    )["id"]
        .clone();

    // 1. Two Ivan Kovals: the year tells them apart. The patronymic stays, the other version is a note.
    assert_eq!(ivans(c), 2);
    let ivan = find(c, "Ivan Koval", "1901");
    c.call(
        "add_citation",
        json!({"batch": batch, "target": on("person", ivan), "source": source, "page": "item 1", "quality": 2}),
    );
    note(
        on("person", ivan),
        format!(
            "Item 1 says Pavlovych; the tree says Petrovych. {}",
            items[&1]
        ),
    );
    // 2. The speaker is already there.
    let olena = find(c, "Olena Koval", "1905");
    // 3. A son who exists, and brothers and sisters with no names.
    let mykola = find(c, "Mykola Koval", "1930");
    c.call(
        "add_sibling_marker",
        json!({"batch": batch, "sibling_of": mykola}),
    );
    // 4. A daughter who is new: with one partnership, her mother is set too.
    let halyna = c.call(
        "add_person",
        json!({"batch": batch, "given": "Halyna", "surnames": ["Koval"], "sex": "female",
               "birth_date": "1932", "source_file": FILE, "number": "4",
               "relation": {"to": ivan, "as": "child"}}),
    );
    // 5. A brother with a known father and an either-or mother: the father is linked, the mother is a note.
    let stepan = c.call(
        "add_person",
        json!({"batch": batch, "given": "Stepan", "surnames": ["Koval"], "sex": "male",
               "birth_date": "12 MAR 1903", "source_file": FILE, "number": "5"}),
    )["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    let petro = find(c, "Petro Koval", "1870");
    c.call(
        "link_people",
        json!({"batch": batch, "person": petro, "relation": {"to": stepan, "as": "father"}}),
    );
    // Forbidden attempts, each of which must be refused: a second father.
    let other_ivan = find(c, "Ivan Koval", "1899");
    let message = c.call_err(
        "link_people",
        json!({"batch": batch, "person": other_ivan, "relation": {"to": stepan, "as": "father"}}),
    );
    assert!(message.contains("already has a father"), "{message}");
    note(
        on("person", stepan),
        format!("Mother unsettled. {}", items[&5]),
    );
    // 6. An inference is a note.
    note(
        on("person", stepan),
        format!("Inference, not stated in the recording. {}", items[&6]),
    );
    // 7. A father with no name.
    let father = c.call(
        "add_person",
        json!({"batch": batch, "sex": "male", "source_file": FILE, "number": "7",
               "relation": {"to": olena, "as": "father"}}),
    );
    note(
        on("person", father["id"].as_str().unwrap()),
        format!("Name not given. {}", items[&7]),
    );
    // 8. A death on a person the owner marked deceased is allowed.
    let yaroslav = find(c, "Yaroslav Koval", "1950");
    c.call(
        "add_event",
        json!({"batch": batch, "person": yaroslav, "kind": "death", "date": "2003"}),
    );
    // So is one on a man the app already counts as dead by his age.
    c.call(
        "add_event",
        json!({"batch": batch, "person": ivan, "kind": "death", "date": "1975"}),
    );
    // Sex on the owner's person is refused, not silently ignored.
    let sexed = c.call_err(
        "set_sex",
        json!({"batch": batch, "person": yaroslav, "sex": "male"}),
    );
    assert!(sexed.contains("would change sex"), "{sexed}");
    // The owner's patronymic cannot be overwritten with the other version.
    let names = c.call("get_person", json!({"id": ivan}))["names"].clone();
    let overwrite = c.call_err(
        "update_own_name",
        json!({"batch": batch, "person": ivan, "name": names[0]["id"], "patronymic": "Pavlovych"}),
    );
    assert!(overwrite.contains("patronymic"), "{overwrite}");
    assert!(overwrite.contains("Agents only add"), "{overwrite}");
    // 9. A death on someone alive is refused, so it becomes a note.
    let anna = find(c, "Anna Koval", "1985");
    let refused = c.call_err(
        "add_event",
        json!({"batch": batch, "person": anna, "kind": "death", "date": "2019"}),
    );
    assert!(refused.contains("alive or dead"), "{refused}");
    note(
        on("person", anna),
        format!("Death not recorded: only the owner can. {}", items[&9]),
    );
    // 10. The other Ivan Koval is not connected. Nothing to write; the report says so.
    // 11. A date that reads two ways is refused, so it is written in a note as given.
    let family = c.call("get_relatives", json!({"id": ivan}))["partners"][0]["familyId"].clone();
    let refused = c.call_err(
        "add_event",
        json!({"batch": batch, "family": family, "kind": "marriage", "date": "03/04/1927"}),
    );
    assert!(refused.contains("two ways"), "{refused}");
    note(
        on("family", family.as_str().unwrap()),
        format!("Married 03/04/1927, as written. {}", items[&11]),
    );

    c.call(
        "end_batch",
        json!({"batch": batch, "report": "Added Halyna, Stepan, an unnamed father and one marker. \
               Not connected: the other Ivan Koval, born 1899. Unsettled: Stepan's mother, the date of the \
               marriage, the death of Anna Koval. Question for the family: which Ivan is meant in item 10?"}),
    );
    Run {
        batch: batch.as_str().unwrap().parse().unwrap(),
        ivan,
        stepan,
        anna,
        yaroslav,
        mykola,
        halyna,
    }
}

#[test]
fn the_file_is_worked_through_over_http_and_a_rerun_adds_nothing() {
    let host = Arc::new(seeded().host);
    let server = serve(Arc::clone(&host), 0).unwrap();
    let c = HttpClient::new(server.port());
    let items = items();
    let before = with(&host, |s| s.tree().clone());
    let people_before = c.call("tree_summary", json!({}))["counts"]["people"]
        .as_u64()
        .unwrap();

    let first = work(&c, &items);
    let after_first = counts(&host);
    let log_after_first = log_total(&c);

    // Most people were there: only three are new, and the marker is the fourth entry.
    let people_now = c.call("tree_summary", json!({}))["counts"]["people"]
        .as_u64()
        .unwrap();
    assert_eq!(
        people_now,
        people_before + 3,
        "Halyna, Stepan, the unnamed father"
    );
    assert_eq!(ivans(&c), 2, "no second Ivan");
    assert_eq!(after_first[6], 1, "one marker");

    // Unnamed brothers and sisters: the marker, no name or date.
    let siblings = c.call("get_relatives", json!({"id": first.mykola}))["siblings"].clone();
    assert!(
        siblings
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["siblingMarker"] == true),
        "{siblings}"
    );

    // The new daughter has both parents and the answer says who.
    let parents = first.halyna["parents"].as_array().unwrap();
    assert_eq!(parents.len(), 2, "{}", first.halyna);
    assert!(
        parents
            .iter()
            .any(|p| p["name"].as_str().unwrap().contains("Olena"))
    );

    // Either-or: Stepan has his father and no mother; an inference links nothing.
    let relatives = c.call("get_relatives", json!({"id": first.stepan}));
    assert_eq!(
        relatives["parents"].as_array().unwrap().len(),
        1,
        "{relatives}"
    );
    assert_eq!(relatives["parents"][0]["name"], "Petro Koval");
    assert!(
        relatives["siblings"].as_array().unwrap().is_empty(),
        "{relatives}"
    );
    let stepan = c.call("get_person", json!({"id": first.stepan}));
    assert!(
        stepan.to_string().contains("Inference, not stated"),
        "{stepan}"
    );
    assert!(stepan.to_string().contains("Mother unsettled"), "{stepan}");

    // The contested patronymic is kept, and the other version is a note.
    let ivan_row = row(&c, first.ivan);
    assert_eq!(
        (ivan_row["living"].clone(), ivan_row["deathYear"].clone()),
        (json!(false), json!("1975"))
    );
    let name = ivan_row["name"].as_str().unwrap().to_string();
    assert!(
        name.contains("Petrovych") && !name.contains("Pavlovych"),
        "{name}"
    );
    let ivan = c.call("get_person", json!({"id": first.ivan}));
    assert!(ivan.to_string().contains("Item 1 says Pavlovych"), "{ivan}");

    // Death: refused on the living woman, allowed on the man the owner marked; neither status moves.
    let anna = row(&c, first.anna);
    assert_eq!(anna["living"], true, "{anna}");
    assert!(anna["deathYear"].is_null(), "{anna}");
    let anna_full = c.call("get_person", json!({"id": first.anna}));
    assert!(
        anna_full.to_string().contains("Death not recorded"),
        "{anna_full}"
    );
    let yaroslav = row(&c, first.yaroslav);
    assert_eq!(yaroslav["living"], false, "{yaroslav}");
    assert_eq!(yaroslav["deathYear"], "2003", "{yaroslav}");
    assert_eq!(
        yaroslav["sex"], "unknown",
        "an agent cannot fill sex on the owner's person"
    );

    // A rerun adds nothing at all: same counts, so no new notes, and no new log rows.
    let second = work(&c, &items);
    assert_eq!(counts(&host), after_first);
    assert_eq!(log_total(&c), log_after_first);
    assert_ne!(second.batch, first.batch);

    // Undoing the first batch puts the tree back exactly as the owner had it.
    let report = with(&host, |s| s.undo_batch(first.batch)).unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    // Everything but the "last changed" stamps, which an undo is itself a change of.
    let plain = |t: &gt_core::tree::Tree| {
        let mut v = serde_json::to_value(t).unwrap();
        without_stamps(&mut v);
        v
    };
    with(&host, |s| {
        assert!(
            plain(&before) == plain(s.tree()),
            "the tree differs from before the agent"
        )
    });
    server.stop();

    // And what is stored agrees: close the session, open the project again and read it back.
    let dir = host._dir.as_ref().unwrap().path().join("project");
    drop(host.session.lock().unwrap().take());
    let stored = gt_store::Project::open(&dir).unwrap().load_tree().unwrap();
    assert!(
        plain(&before) == plain(&stored),
        "the stored tree differs from before the agent"
    );
}

/// The gate cannot tell that a source's "this parent or that one" was either-or: a mother link
/// into an empty role is an addition and passes. Keeping it out is the rules' job (rule 8) and
/// the agent's; the tool descriptions say so too. This pins down what the gate does.
#[test]
fn the_gate_lets_a_mother_into_an_empty_role_so_either_or_is_the_rules_job() {
    let host = Arc::new(seeded().host);
    let server = serve(Arc::clone(&host), 0).unwrap();
    let c = HttpClient::new(server.port());
    let batch = c.call("begin_batch", json!({"title": "Either-or"}))["batch"].clone();
    let stepan = c.call(
        "add_person",
        json!({"batch": batch, "given": "Stepan", "surnames": ["Koval"], "source_file": FILE, "number": "5"}),
    )["id"]
        .clone();
    let maria = find(&c, "Maria Koval", "1875");
    let linked = c.call(
        "link_people",
        json!({"batch": batch, "person": maria, "relation": {"to": stepan, "as": "mother"}}),
    );
    assert!(linked["family"].is_string(), "{linked}");
    // A second mother, though, is refused.
    let other = find(&c, "Olena Koval", "1905");
    let message = c.call_err(
        "link_people",
        json!({"batch": batch, "person": other, "relation": {"to": stepan, "as": "mother"}}),
    );
    assert!(message.contains("already has a mother"), "{message}");
    server.stop();
}

/// Drops every `changed` stamp of a record's `common` part.
fn without_stamps(v: &mut Value) {
    match v {
        Value::Object(o) => {
            if let Some(Value::Object(c)) = o.get_mut("common") {
                c.remove("changed");
            }
            o.values_mut().for_each(without_stamps);
        }
        Value::Array(a) => a.iter_mut().for_each(without_stamps),
        _ => {}
    }
}
