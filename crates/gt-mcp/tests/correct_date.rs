#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! correct_event_date: the one change an agent may make to what is already there.

mod common;

use common::*;
use gt_core::model::EventKind;
use gt_session::{EditCommand, EventInput, EventOwner, EventOwnerKind};
use gt_store::Record;
use serde_json::{Value, json};
use uuid::Uuid;

fn begin(host: &TestHost) -> Value {
    call(
        host,
        "begin_batch",
        json!({"title": "Dates the family confirmed"}),
    )["batch"]
        .clone()
}

fn event_of(host: &TestHost, person: Uuid, kind: EventKind) -> Uuid {
    with(host, |s| {
        s.tree()
            .events
            .values()
            .find(|e| {
                e.kind == kind
                    && e.participants
                        .iter()
                        .any(|p| p.target == gt_core::model::ParticipantRef::Person(person))
            })
            .unwrap()
            .common
            .id
    })
}

fn date_of(host: &TestHost, event: Uuid) -> Option<String> {
    with(host, |s| {
        s.tree().events[&event].date.as_ref().map(|d| d.raw.clone())
    })
}

/// An owner's event of `kind` on `person`, with no date.
fn owners_event(host: &TestHost, person: Uuid, kind: &str) -> Uuid {
    with(host, |s| {
        let r = s
            .apply(EditCommand::SaveEvent {
                owner: EventOwner {
                    kind: EventOwnerKind::Person,
                    id: person,
                },
                event: EventInput {
                    id: None,
                    kind: kind.into(),
                    date_raw: None,
                    place_text: None,
                    place_id: None,
                    description: None,
                    age: None,
                    cause: None,
                    negative: false,
                    private: false,
                    preferred: true,
                    godparents: None,
                    participants: None,
                },
            })
            .unwrap();
        *r.touched
            .iter()
            .find(|id| s.tree().events.contains_key(*id))
            .unwrap()
    })
}

fn correct(host: &TestHost, batch: &Value, event: Uuid, date: &str) -> Value {
    call(
        host,
        "correct_event_date",
        json!({"batch": batch, "event": event, "date": date, "reason": "Confirmed by his daughter Olha, 2026"}),
    )
}

#[test]
fn replaces_a_year_with_the_full_date_and_keeps_the_reason() {
    let host = empty_host();
    let ivan = with(&host, |s| {
        add_person(s, draft("Ivan", "Koval", None, Some("1901")))
    });
    let birth = event_of(&host, ivan, EventKind::Birth);
    let batch = begin(&host);
    let done = correct(&host, &batch, birth, "12 MAR 1901");
    assert_eq!(done["id"], json!(birth));
    assert_eq!(date_of(&host, birth).as_deref(), Some("12 MAR 1901"));
    // The reason is in a research note on the event, with the old and the new date.
    let note = with(&host, |s| {
        let e = &s.tree().events[&birth];
        s.tree().notes[e.common.notes.last().unwrap()].text.clone()
    });
    assert_eq!(
        note,
        "Birth date corrected from 1901 to 12 MAR 1901. Reason: Confirmed by his daughter Olha, 2026"
    );
    // The log keeps the event before and after, under the agent, with the batch.
    let rows = with(&host, |s| s.project().rows_of(birth).unwrap());
    let last = rows.last().unwrap();
    let raw = |r: &Option<Record>| match r {
        Some(Record::Event(e)) => e.date.as_ref().map(|d| d.raw.clone()),
        _ => panic!("not an event"),
    };
    assert_eq!(raw(&last.before).as_deref(), Some("1901"));
    assert_eq!(raw(&last.after).as_deref(), Some("12 MAR 1901"));
    let history = with(&host, |s| s.history(birth, 10).unwrap());
    assert_eq!(history[0].label, "Correct date");
    let log = call(&host, "get_log", json!({"filters": {"batch": batch}}));
    assert!(log.to_string().contains("Correct date"), "{log}");
    // Saying it again adds nothing.
    let again = correct(&host, &batch, birth, "12 Mar 1901");
    assert_eq!(again["alreadyThere"], json!(true), "{again}");
    // The owner can undo it: the old date comes back and the note goes.
    with(&host, |s| {
        s.undo_batch(serde_json::from_value(batch.clone()).unwrap())
            .unwrap()
    });
    assert_eq!(date_of(&host, birth).as_deref(), Some("1901"));
    assert!(with(&host, |s| s.tree().events[&birth]
        .common
        .notes
        .is_empty()));
}

#[test]
fn fills_an_empty_date() {
    let host = empty_host();
    let ivan = with(&host, |s| {
        add_person(s, draft("Ivan", "Koval", None, Some("1901")))
    });
    let death = owners_event(&host, ivan, "death");
    assert_eq!(date_of(&host, death), None);
    let batch = begin(&host);
    correct(&host, &batch, death, "3 APR 1975");
    assert_eq!(date_of(&host, death).as_deref(), Some("3 APR 1975"));
    let note = with(&host, |s| {
        let e = &s.tree().events[&death];
        s.tree().notes[&e.common.notes[0]].text.clone()
    });
    assert!(
        note.starts_with("Death date corrected from no date to 3 APR 1975."),
        "{note}"
    );
}

#[test]
fn refusals() {
    let host = empty_host();
    let ivan = with(&host, |s| {
        add_person(s, draft("Ivan", "Koval", None, Some("1901")))
    });
    let birth = event_of(&host, ivan, EventKind::Birth);
    let residence = owners_event(&host, ivan, "residence");
    let batch = begin(&host);
    let ask = |args: Value| call_err(&host, "correct_event_date", args);
    let m = ask(json!({"batch": batch, "event": residence, "date": "1920", "reason": "Letter"}));
    assert!(m.contains("birth or a death"), "{m}");
    let m = ask(json!({"batch": batch, "event": birth, "date": "03/04/1901", "reason": "Letter"}));
    assert!(m.contains("two ways"), "{m}");
    let m = ask(json!({"batch": batch, "event": birth, "date": "12 MAR 1901"}));
    assert!(m.contains("reason"), "{m}");
    let m = ask(json!({"batch": batch, "event": birth, "date": "12 MAR 1901", "reason": "  "}));
    assert!(m.contains("reason"), "{m}");
    let m = ask(json!({"batch": batch, "event": birth, "date": "someday", "reason": "Letter"}));
    assert!(m.contains("not understood"), "{m}");
    assert_eq!(date_of(&host, birth).as_deref(), Some("1901"));

    // Someone born in 1900 counts as dead by age; 1990 would make them count as alive.
    let olena = with(&host, |s| {
        add_person(s, draft("Olena", "Koval", None, Some("1900")))
    });
    let born = event_of(&host, olena, EventKind::Birth);
    let m = ask(json!({"batch": batch, "event": born, "date": "1990", "reason": "Letter"}));
    assert!(m.contains("alive or dead"), "{m}");
    assert_eq!(date_of(&host, born).as_deref(), Some("1900"));
    // The other way, a date over 100 years ago may make someone count as dead.
    let petro = with(&host, |s| {
        add_person(s, draft("Petro", "Koval", None, Some("1990")))
    });
    let petro_born = event_of(&host, petro, EventKind::Birth);
    correct(&host, &batch, petro_born, "1900");
    assert_eq!(date_of(&host, petro_born).as_deref(), Some("1900"));

    call(
        &host,
        "end_batch",
        json!({"batch": batch, "report": "Nothing changed."}),
    );
    let m = ask(json!({"batch": batch, "event": birth, "date": "12 MAR 1901", "reason": "Letter"}));
    assert!(m.contains("closed"), "{m}");
    assert_eq!(date_of(&host, birth).as_deref(), Some("1901"));
}

#[test]
fn a_death_before_the_birth_or_a_birth_after_the_death_is_refused() {
    let host = empty_host();
    let ivan = with(&host, |s| {
        add_person(s, draft("Ivan", "Koval", None, Some("1901")))
    });
    let birth = event_of(&host, ivan, EventKind::Birth);
    let death = owners_event(&host, ivan, "death");
    let batch = begin(&host);
    correct(&host, &batch, death, "1950");
    let ask = |args: Value| call_err(&host, "correct_event_date", args);
    let m = ask(json!({"batch": batch, "event": death, "date": "1890", "reason": "Letter"}));
    assert!(m.contains("before") && m.contains("note"), "{m}");
    let m = ask(json!({"batch": batch, "event": birth, "date": "12 MAR 1960", "reason": "Letter"}));
    assert!(m.contains("after") && m.contains("note"), "{m}");
    assert_eq!(date_of(&host, death).as_deref(), Some("1950"));
    assert_eq!(date_of(&host, birth).as_deref(), Some("1901"));
    // A date that may still fit is not refused.
    correct(&host, &batch, birth, "ABT 1903");
}

#[test]
fn a_date_or_reason_too_long_is_refused() {
    let host = empty_host();
    let ivan = with(&host, |s| {
        add_person(s, draft("Ivan", "Koval", None, Some("1901")))
    });
    let birth = event_of(&host, ivan, EventKind::Birth);
    let batch = begin(&host);
    let long_date = format!("12 MAR 1901 {}", "(as the family says) ".repeat(20));
    let m = call_err(
        &host,
        "correct_event_date",
        json!({"batch": batch, "event": birth, "date": long_date, "reason": "Letter"}),
    );
    assert!(m.contains("date"), "{m}");
    let m = call_err(
        &host,
        "correct_event_date",
        json!({"batch": batch, "event": birth, "date": "12 MAR 1901", "reason": "Confirmed. ".repeat(200)}),
    );
    assert!(m.contains("reason"), "{m}");
    assert_eq!(date_of(&host, birth).as_deref(), Some("1901"));
}
