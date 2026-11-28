#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! list_empty_families, remove_empty_family and remove_child_from_family through the tools: the
//! list, a removal of a family the owner made, the refusals, and the undo with the batch.
//! Invented people only.

mod common;

use common::*;
use gt_session::*;
use serde_json::json;
use uuid::Uuid;

fn new_family(s: &mut Session, partner: Option<Uuid>) -> Uuid {
    s.apply(EditCommand::SaveFamily {
        id: None,
        partner1: partner,
        partner2: None,
        kind: UnionKindId::Unknown,
    })
    .unwrap()
    .touched[0]
}

fn link(s: &mut Session, family: Uuid, child: Uuid) {
    s.apply(EditCommand::LinkChild {
        family,
        child,
        rel: (ParentRelId::Birth, ParentRelId::Birth),
        preferred: false,
    })
    .unwrap();
}

#[test]
fn the_three_tools_list_remove_and_undo() {
    let host = empty_host();
    let (father, mother, kid, lone, orphan) = with(&host, |s| {
        (
            add_person(s, draft("Hryhorii", "Bondar", None, Some("1800"))),
            add_person(s, draft("Olha", "Bondar", None, Some("1805"))),
            add_person(s, draft("Ivan", "Bondar", None, Some("1830"))),
            add_person(s, draft("Stepan", "Koval", None, Some("1840"))),
            add_person(s, draft("Orphan", "Shevchuk", None, Some("1850"))),
        )
    });
    let (couple, parentless, single, empty, orphans) = with(&host, |s| {
        let couple = s
            .apply(EditCommand::SaveFamily {
                id: None,
                partner1: Some(father),
                partner2: Some(mother),
                kind: UnionKindId::Married,
            })
            .unwrap()
            .touched[0];
        link(s, couple, kid);
        let parentless = new_family(s, None);
        link(s, parentless, kid);
        let single = new_family(s, Some(lone));
        let empty = new_family(s, None);
        let orphans = new_family(s, None);
        link(s, orphans, orphan);
        (couple, parentless, single, empty, orphans)
    });
    let b = call(&host, "begin_batch", json!({"title": "Cleanup"}))["batch"].clone();
    let before = with(&host, |s| s.tree().clone());

    // The list: three kinds, who made them, what blocks them.
    let all = call(&host, "list_empty_families", json!({}));
    assert_eq!(all["total"], json!(4), "{all}");
    let row = |id: Uuid| {
        all["families"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["id"] == json!(id))
            .unwrap()
            .clone()
    };
    assert_eq!(row(parentless)["kind"], json!("parentless"));
    assert_eq!(
        row(parentless)["children"][0]["hasOtherFamily"],
        json!(true)
    );
    assert_eq!(row(parentless)["origin"]["by"], json!("owner"));
    assert_eq!(row(single)["kind"], json!("singlePartner"));
    assert_eq!(row(empty)["kind"], json!("empty"));
    assert_eq!(row(orphans)["removable"], json!(false));
    assert!(
        row(orphans)["blockers"][0]
            .as_str()
            .unwrap()
            .contains("no parent family")
    );
    let only = call(
        &host,
        "list_empty_families",
        json!({"kind": "singlePartner"}),
    );
    assert_eq!(only["total"], json!(1));
    call_err(&host, "list_empty_families", json!({"kind": "no"}));

    // Refusals, each with its reason, and nothing changed.
    let msg = call_err(
        &host,
        "remove_empty_family",
        json!({"batch": b, "family_id": couple, "reason": "x"}),
    );
    assert!(msg.contains("two partners"), "{msg}");
    let msg = call_err(
        &host,
        "remove_empty_family",
        json!({"batch": b, "family_id": orphans, "reason": "x"}),
    );
    assert!(msg.contains("no parent family"), "{msg}");
    let msg = call_err(
        &host,
        "remove_empty_family",
        json!({"batch": b, "family_id": empty, "reason": "  "}),
    );
    assert!(msg.contains("reason"), "{msg}");
    call_err(
        &host,
        "remove_empty_family",
        json!({"batch": b, "family_id": empty}),
    );
    let msg = call_err(
        &host,
        "remove_empty_family",
        json!({"family_id": empty, "reason": "x"}),
    );
    assert!(msg.contains("begin_batch"), "{msg}");
    let msg = call_err(
        &host,
        "remove_child_from_family",
        json!({"batch": b, "family_id": orphans, "child_id": orphan, "reason": "x"}),
    );
    assert!(msg.contains("no other family"), "{msg}");
    call_err(
        &host,
        "remove_child_from_family",
        json!({"batch": b, "family_id": couple, "child_id": kid, "reason": "x"}),
    );
    assert_eq!(
        with(&host, |s| s.tree().families.len()),
        before.families.len()
    );

    // The successes.
    let r = call(
        &host,
        "remove_empty_family",
        json!({"batch": b, "family_id": empty, "reason": "Made by mistake, nobody in it."}),
    );
    assert_eq!(r["removed"], json!(empty));
    let r = call(
        &host,
        "remove_empty_family",
        json!({"batch": b, "family_id": single, "reason": "A partner with no children."}),
    );
    assert_eq!(r["removed"], json!(single));
    let r = call(
        &host,
        "remove_empty_family",
        json!({"batch": b, "family_id": parentless, "reason": "The child has real parents."}),
    );
    assert_eq!(r["childrenUnlinked"], json!([kid]));
    with(&host, |s| {
        let t = s.tree();
        assert!(!t.families.contains_key(&parentless));
        assert_eq!(t.families_as_child(kid).count(), 1);
    });
    assert_eq!(
        call(&host, "list_empty_families", json!({}))["total"],
        json!(1)
    );

    // One child link of any author.
    let other = with(&host, |s| {
        let other = new_family(s, None);
        link(s, other, kid);
        other
    });
    call(
        &host,
        "remove_child_from_family",
        json!({"batch": b, "family_id": other, "child_id": kid, "reason": "Wrong group."}),
    );
    with(&host, |s| {
        assert_eq!(s.tree().families[&other].children.len(), 0);
    });

    // The batch undoes the three removals and the link.
    let batch_id: Uuid = b.as_str().unwrap().parse().unwrap();
    with(&host, |s| s.undo_batch(batch_id).unwrap());
    with(&host, |s| {
        let t = s.tree();
        for f in [parentless, single, empty] {
            assert!(t.families.contains_key(&f), "{f}");
        }
        assert_eq!(t.families[&parentless].children.len(), 1);
        assert_eq!(t.families[&other].children.len(), 1);
    });
}
