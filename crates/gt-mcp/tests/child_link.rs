#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! remove_own_child_link: an agent takes back a parent-child link it made itself, and only that.

mod common;

use common::*;
use gt_mcp::{AgentCtx, handle_message};
use gt_session::{EditCommand, LivingOverrideDto, LivingPatch, ParentRelId, Relation};
use gt_store::Record;
use serde_json::{Value, json};
use uuid::Uuid;

fn other() -> AgentCtx {
    AgentCtx {
        id: Uuid::from_u128(8),
        name: "Other reader".into(),
    }
}

/// A tool call as the other agent; `Ok` is the parsed answer, `Err` the message.
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
    call(host, "begin_batch", json!({"title": "Recording 4"}))["batch"].clone()
}

fn end(host: &TestHost, batch: &Value) {
    call(
        host,
        "end_batch",
        json!({"batch": batch, "report": "Linked a child."}),
    );
}

fn family_of(host: &TestHost, partner: Uuid) -> Uuid {
    with(host, |s| {
        s.tree()
            .families
            .values()
            .find(|f| f.partner1 == Some(partner))
            .unwrap()
            .common
            .id
    })
}

/// The owner's couple and their family.
fn couple(host: &TestHost, a: &str, b: &str) -> (Uuid, Uuid, Uuid) {
    let (pa, pb) = with(host, |s| {
        let pa = add_person(s, draft(a, "Koval", None, None));
        let pb = add_relative(s, pa, Relation::Partner, draft(b, "Koval", None, None));
        (pa, pb)
    });
    (pa, pb, family_of(host, pa))
}

fn children(host: &TestHost, family: Uuid) -> Vec<Uuid> {
    with(host, |s| {
        s.tree().families[&family]
            .children
            .iter()
            .map(|l| l.person)
            .collect()
    })
}

fn agent_child(host: &TestHost, batch: &Value, family: Uuid, name: &str) -> Uuid {
    let added = call(
        host,
        "add_person",
        json!({"batch": batch, "given": name, "surnames": ["Koval"]}),
    );
    let child: Uuid = serde_json::from_value(added["id"].clone()).unwrap();
    call(
        host,
        "link_people",
        json!({"batch": batch, "person": child, "relation": {"as": "child", "family": family}}),
    );
    child
}

fn take_back(host: &TestHost, batch: &Value, person: Uuid, family: Uuid) -> Value {
    call(
        host,
        "remove_own_child_link",
        json!({"batch": batch, "person": person, "family": family,
               "reason": "The family says Taras is the son of Petro, not Ivan"}),
    )
}

fn ask_err(host: &TestHost, args: Value) -> String {
    call_err(host, "remove_own_child_link", args)
}

#[test]
fn removes_an_own_link_from_an_earlier_batch_and_keeps_the_rest() {
    let host = empty_host();
    let (ivan, olena, family) = couple(&host, "Ivan", "Olena");
    let maria = with(&host, |s| {
        add_relative(
            s,
            ivan,
            Relation::Child,
            draft("Maria", "Koval", None, None),
        )
    });
    let first = begin(&host);
    let taras = agent_child(&host, &first, family, "Taras");
    end(&host, &first);
    assert_eq!(children(&host, family), vec![maria, taras]);

    let second = begin(&host);
    let done = take_back(&host, &second, taras, family);
    assert_eq!(done["id"], json!(taras), "{done}");
    // Only the link went.
    assert_eq!(children(&host, family), vec![maria]);
    with(&host, |s| {
        let f = &s.tree().families[&family];
        assert_eq!(f.partner1, Some(ivan));
        assert_eq!(f.partner2, Some(olena));
        assert!(s.tree().persons.contains_key(&taras));
        assert!(s.tree().persons.contains_key(&maria));
        assert_eq!(s.tree().families.len(), 1);
    });
    // The reason is a research note on the person, naming the family.
    let note = with(&host, |s| {
        let p = &s.tree().persons[&taras];
        assert_eq!(p.common.notes.len(), 1);
        let n = &s.tree().notes[&p.common.notes[0]];
        assert_eq!(n.kind, gt_core::model::NoteKind::Research);
        n.text.clone()
    });
    assert_eq!(
        note,
        format!(
            "Removed from the family of Ivan Koval and Olena Koval (family {family}) as a child. \
             Reason: The family says Taras is the son of Petro, not Ivan"
        )
    );
    // The log keeps the family before and after, under the agent, in the batch.
    let rows = with(&host, |s| s.project().rows_of(family).unwrap());
    let last = rows.last().unwrap();
    let kids = |r: &Option<Record>| match r {
        Some(Record::Family(f)) => f.children.len(),
        _ => panic!("not a family"),
    };
    assert_eq!((kids(&last.before), kids(&last.after)), (2, 1));
    let history = with(&host, |s| s.history(family, 10).unwrap());
    assert_eq!(history[0].label, "Remove child from family");
    let log = call(&host, "get_log", json!({"filters": {"batch": second}}));
    assert!(
        log.to_string().contains("Remove child from family"),
        "{log}"
    );
    assert!(log.to_string().contains("Transcript reader"), "{log}");
    // The owner can undo it: the link is back and the note is gone.
    let report = with(&host, |s| {
        s.undo_batch(serde_json::from_value(second.clone()).unwrap())
            .unwrap()
    });
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(children(&host, family), vec![maria, taras]);
    with(&host, |s| {
        assert!(s.tree().persons[&taras].common.notes.is_empty());
        assert!(s.tree().notes.is_empty());
    });
}

#[test]
fn removes_an_own_link_in_the_same_open_batch_and_a_repeat_is_already_there() {
    let host = empty_host();
    let (_, _, family) = couple(&host, "Ivan", "Olena");
    let batch = begin(&host);
    let taras = agent_child(&host, &batch, family, "Taras");
    take_back(&host, &batch, taras, family);
    assert!(children(&host, family).is_empty());
    // The family stays, empty, for the agent to remove if it is its own.
    with(&host, |s| assert!(s.tree().families.contains_key(&family)));
    let again = take_back(&host, &batch, taras, family);
    assert_eq!(again["alreadyThere"], json!(true), "{again}");
    with(&host, |s| {
        assert_eq!(s.tree().persons[&taras].common.notes.len(), 1);
    });
}

#[test]
fn the_child_can_then_be_linked_to_the_right_family() {
    let host = empty_host();
    let (_, _, wrong) = couple(&host, "Ivan", "Olena");
    let (petro, _) = with(&host, |s| {
        let pa = add_person(s, draft("Petro", "Hrytsenko", None, None));
        let pb = add_relative(
            s,
            pa,
            Relation::Partner,
            draft("Hanna", "Hrytsenko", None, None),
        );
        (pa, pb)
    });
    let right = family_of(&host, petro);
    let first = begin(&host);
    let taras = agent_child(&host, &first, wrong, "Taras");
    // A second birth family is refused while the wrong link stands.
    let m = call_err(
        &host,
        "link_people",
        json!({"batch": first, "person": taras, "relation": {"as": "child", "family": right}}),
    );
    assert!(m.contains("birth"), "{m}");
    take_back(&host, &first, taras, wrong);
    let linked = call(
        &host,
        "link_people",
        json!({"batch": first, "person": taras, "relation": {"as": "child", "family": right}}),
    );
    assert_ne!(linked["alreadyThere"], json!(true), "{linked}");
    assert_eq!(children(&host, right), vec![taras]);
    assert!(children(&host, wrong).is_empty());
}

#[test]
fn refuses_links_that_are_not_the_agents_own() {
    let host = empty_host();
    let (ivan, _, family) = couple(&host, "Ivan", "Olena");
    let maria = with(&host, |s| {
        add_relative(
            s,
            ivan,
            Relation::Child,
            draft("Maria", "Koval", None, None),
        )
    });
    let batch = begin(&host);
    let m = ask_err(
        &host,
        json!({"batch": batch, "person": maria, "family": family, "reason": "Wrong"}),
    );
    assert!(m.contains("you made yourself"), "{m}");
    assert_eq!(children(&host, family), vec![maria]);

    // Another connection's link.
    let theirs =
        call_as(&host, &other(), "begin_batch", json!({"title": "Theirs"})).unwrap()["batch"]
            .clone();
    let added = call_as(
        &host,
        &other(),
        "add_person",
        json!({"batch": theirs, "given": "Stepan", "surnames": ["Koval"]}),
    )
    .unwrap();
    let stepan: Uuid = serde_json::from_value(added["id"].clone()).unwrap();
    call_as(
        &host,
        &other(),
        "link_people",
        json!({"batch": theirs, "person": stepan, "relation": {"as": "child", "family": family}}),
    )
    .unwrap();
    let m = ask_err(
        &host,
        json!({"batch": batch, "person": stepan, "family": family, "reason": "Wrong"}),
    );
    assert!(m.contains("you made yourself"), "{m}");
    // The other connection's batch cannot be used either.
    let m = ask_err(
        &host,
        json!({"batch": theirs, "person": stepan, "family": family, "reason": "Wrong"}),
    );
    assert!(m.contains("another connection"), "{m}");
    assert_eq!(children(&host, family), vec![maria, stepan]);

    // The owner changed the agent's link since.
    let taras = agent_child(&host, &batch, family, "Taras");
    with(&host, |s| {
        s.apply(EditCommand::UpdateChildLink {
            family,
            child: taras,
            rel: (ParentRelId::Adopted, ParentRelId::Adopted),
            preferred: true,
        })
        .unwrap();
    });
    let m = ask_err(
        &host,
        json!({"batch": batch, "person": taras, "family": family, "reason": "Wrong"}),
    );
    assert!(m.contains("you made yourself"), "{m}");
    assert!(children(&host, family).contains(&taras));
    with(&host, |s| assert!(s.tree().notes.is_empty()));
}

#[test]
fn refuses_a_closed_batch_a_missing_reason_and_wrong_ids() {
    let host = empty_host();
    let (_, _, family) = couple(&host, "Ivan", "Olena");
    let batch = begin(&host);
    let taras = agent_child(&host, &batch, family, "Taras");
    let stranger = with(&host, |s| {
        add_person(s, draft("Pavlo", "Koval", None, None))
    });

    let m = ask_err(
        &host,
        json!({"batch": batch, "person": taras, "family": family}),
    );
    assert!(m.contains("reason"), "{m}");
    let m = ask_err(
        &host,
        json!({"batch": batch, "person": taras, "family": family, "reason": "  "}),
    );
    assert!(m.contains("reason"), "{m}");
    let m = ask_err(
        &host,
        json!({"batch": batch, "person": stranger, "family": family, "reason": "Wrong"}),
    );
    assert!(m.contains("not a child"), "{m}");
    let nobody = Uuid::from_u128(99);
    let m = ask_err(
        &host,
        json!({"batch": batch, "person": nobody, "family": family, "reason": "Wrong"}),
    );
    assert!(m.contains("person"), "{m}");
    let m = ask_err(
        &host,
        json!({"batch": batch, "person": taras, "family": nobody, "reason": "Wrong"}),
    );
    assert!(m.contains("family"), "{m}");
    assert_eq!(children(&host, family), vec![taras]);

    end(&host, &batch);
    let m = ask_err(
        &host,
        json!({"batch": batch, "person": taras, "family": family, "reason": "Wrong"}),
    );
    assert!(m.contains("closed"), "{m}");
    let m = ask_err(
        &host,
        json!({"batch": nobody, "person": taras, "family": family, "reason": "Wrong"}),
    );
    assert!(m.contains("batch"), "{m}");
    assert_eq!(children(&host, family), vec![taras]);
    with(&host, |s| assert!(s.tree().notes.is_empty()));
}

#[test]
fn never_changes_whether_someone_counts_as_alive_or_dead() {
    let host = empty_host();
    // Ivan was born in 1850, so his children are proven dead; the owner marked Taras deceased.
    let ivan = with(&host, |s| {
        let ivan = add_person(s, draft("Ivan", "Koval", None, Some("1850")));
        add_relative(
            s,
            ivan,
            Relation::Partner,
            draft("Olena", "Koval", None, Some("1855")),
        );
        ivan
    });
    let family = family_of(&host, ivan);
    let batch = begin(&host);
    let added = call(
        &host,
        "add_person",
        json!({"batch": batch, "given": "Taras", "surnames": ["Koval"]}),
    );
    let taras: Uuid = serde_json::from_value(added["id"].clone()).unwrap();
    with(&host, |s| {
        s.apply(EditCommand::UpdatePerson {
            id: taras,
            sex: None,
            sex_text: None,
            living_override: LivingPatch::Set(LivingOverrideDto::Deceased),
            private: None,
        })
        .unwrap();
    });
    call(
        &host,
        "link_people",
        json!({"batch": batch, "person": taras, "relation": {"as": "child", "family": family}}),
    );
    // Without the link the rule would count Taras as living: refused.
    let m = ask_err(
        &host,
        json!({"batch": batch, "person": taras, "family": family, "reason": "Wrong"}),
    );
    assert!(m.contains("alive or dead"), "{m}");
    assert_eq!(children(&host, family), vec![taras]);
    with(&host, |s| assert!(s.tree().notes.is_empty()));
}
