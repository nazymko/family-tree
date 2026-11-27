#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! merge_people through the tool: counts, the required reason, refusals, and one undo.
//! Invented people only.

mod common;

use common::*;
use serde_json::json;
use uuid::Uuid;

#[test]
fn merge_people_answers_with_counts_refuses_and_is_one_undo() {
    let host = empty_host();
    let (keep, dup, other) = with(&host, |s| {
        (
            add_person(s, draft("Ivan", "Bondar", None, Some("1830"))),
            add_person(s, draft("Ioan", "Bondar", None, Some("1830"))),
            add_person(s, draft("Maria", "Koval", None, Some("1835"))),
        )
    });
    with(&host, |s| add_note(s, dup, "Ioan sang in the choir."));
    let b = call(&host, "begin_batch", json!({"title": "People"}))["batch"].clone();
    let before = with(&host, |s| s.tree().persons.len());

    // Refusals, each with its reason.
    let msg = call_err(
        &host,
        "merge_people",
        json!({"batch": b, "keep_id": keep, "merge_id": keep, "reason": "x"}),
    );
    assert!(msg.contains("same person"), "{msg}");
    call_err(
        &host,
        "merge_people",
        json!({"batch": b, "keep_id": keep, "merge_id": Uuid::from_u128(9), "reason": "x"}),
    );
    let msg = call_err(
        &host,
        "merge_people",
        json!({"batch": b, "keep_id": keep, "merge_id": dup, "reason": "  "}),
    );
    assert!(msg.contains("reason"), "{msg}");
    call_err(
        &host,
        "merge_people",
        json!({"batch": b, "keep_id": keep, "merge_id": dup}),
    );
    let msg = call_err(
        &host,
        "merge_people",
        json!({"keep_id": keep, "merge_id": dup, "reason": "x"}),
    );
    assert!(msg.contains("begin_batch"), "{msg}");
    assert_eq!(with(&host, |s| s.tree().persons.len()), before);

    let r = call(
        &host,
        "merge_people",
        json!({"batch": b, "keep_id": keep, "merge_id": dup,
               "reason": "Both born 1830 in Verbka, parents Hryhorii and Olha."}),
    );
    assert_eq!(r["removed"], json!(dup));
    assert_eq!(r["moved"]["names"], json!(1));
    assert_eq!(r["moved"]["notes"], json!(1));
    assert_eq!(r["moved"]["eventsFoldedIntoEqualOnes"], json!(1));
    assert!(r["conflicts"].as_array().unwrap().is_empty(), "{r}");
    with(&host, |s| {
        assert!(!s.tree().persons.contains_key(&dup));
        assert!(s.tree().persons.contains_key(&other));
        let k = &s.tree().persons[&keep];
        assert_eq!(k.names.len(), 2);
        assert!(
            k.common
                .notes
                .iter()
                .any(|n| s.tree().notes[n].text.contains("Hryhorii and Olha"))
        );
    });

    let batch_id: Uuid = b.as_str().unwrap().parse().unwrap();
    with(&host, |s| s.undo_batch(batch_id).unwrap());
    with(&host, |s| {
        assert_eq!(s.tree().persons.len(), before);
        assert_eq!(s.tree().persons[&keep].names.len(), 1);
        assert_eq!(s.tree().persons[&dup].names.len(), 1);
    });
}
