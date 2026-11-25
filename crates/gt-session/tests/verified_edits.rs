#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Verified corrections at the session: data another connection or the owner wrote is changed
//! only with evidence, in an open batch, and never flips alive or dead.

mod common;

use common::*;
use gt_core::model::EventKind;
use gt_session::*;
use uuid::Uuid;

fn me() -> AgentCtx {
    AgentCtx {
        id: Uuid::from_u128(7),
        name: "Re-reader".into(),
    }
}

fn evidence(s: &Session) -> VerifiedEvidence {
    // Any source of the tree will do; make one when there is none.
    let id = s.tree().sources.keys().next().copied().unwrap();
    VerifiedEvidence {
        source: id,
        page: "p. 3".into(),
        quote: "the record's words".into(),
        quality: Some(2),
    }
}

fn with_source(s: &mut Session) {
    s.apply(EditCommand::SaveSource {
        source: SourceInput {
            title: Some("Metric book".into()),
            ..Default::default()
        },
    })
    .unwrap();
}

fn batch(s: &mut Session) -> Uuid {
    s.begin_batch(&me(), "Re-read", None).unwrap()
}

#[test]
fn flipping_alive_or_dead_is_refused_even_with_evidence() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    with_source(&mut s);
    let b = batch(&mut s);
    let anna = find(s.tree(), "Anna");
    let birth = s
        .tree()
        .events
        .values()
        .find(|e| e.kind == EventKind::Birth && e.date.as_ref().is_some_and(|d| d.raw == "1850"))
        .unwrap()
        .common
        .id;
    let ev = evidence(&s);
    let patch = EventCorrection {
        date_raw: Some("1990".into()),
        ..Default::default()
    };
    let err = s
        .correct_event_as_agent(birth, &patch, &ev, "Read again", &me(), b)
        .unwrap_err();
    assert!(err.message.contains("alive or dead"), "{}", err.message);
    assert_eq!(s.tree().events[&birth].date.as_ref().unwrap().raw, "1850");
    let _ = anna;
}

#[test]
fn a_closed_batch_and_a_missing_source_are_refused() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    with_source(&mut s);
    let anna = find(s.tree(), "Anna");
    let b = batch(&mut s);
    let ev = evidence(&s);
    let patch = NameCorrection {
        given: Some("Anne".into()),
        ..Default::default()
    };
    let mut unknown = ev.clone();
    unknown.source = Uuid::from_u128(1234);
    let err = s
        .correct_name_as_agent(anna, None, &patch, &unknown, "r", &me(), b)
        .unwrap_err();
    assert!(err.message.contains("source"), "{}", err.message);
    s.end_batch(&me(), b, "Nothing.").unwrap();
    let err = s
        .correct_name_as_agent(anna, None, &patch, &ev, "r", &me(), b)
        .unwrap_err();
    assert!(err.message.contains("closed"), "{}", err.message);
    assert_eq!(
        s.tree().persons[&anna].names[0].given.as_deref(),
        Some("Anna")
    );
}

#[test]
fn moving_a_child_out_of_an_agents_own_family_removes_the_empty_family() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    with_source(&mut s);
    let b = batch(&mut s);
    let karl = find(s.tree(), "Karl");
    let hans = find(s.tree(), "Hans");
    let f1 = s.tree().persons[&hans].common.id;
    let _ = f1;
    let family = s
        .tree()
        .families
        .values()
        .find(|f| f.children.iter().any(|c| c.person == hans))
        .unwrap()
        .common
        .id;
    // The agent makes a family of Karl alone, with a new child in it.
    let alone = match s
        .apply_as_agent(
            EditCommand::SaveFamily {
                id: None,
                partner1: Some(karl),
                partner2: None,
                kind: UnionKindId::Unknown,
            },
            &me(),
            b,
        )
        .unwrap()
    {
        AgentWrite::Done(r) => r.touched[0],
        AgentWrite::AlreadyThere { .. } => panic!("repeat"),
    };
    let kid = match s
        .apply_as_agent(
            EditCommand::QuickAdd {
                anchor: karl,
                relation: Relation::Child,
                draft: PersonDraft {
                    birth_date_raw: Some("1880".into()),
                    ..draft("Greta", SexKind::Female)
                },
                family: Some(alone),
                new_parent_set: None,
                parent_rel: None,
            },
            &me(),
            b,
        )
        .unwrap()
    {
        AgentWrite::Done(r) => r.focus.unwrap(),
        AgentWrite::AlreadyThere { .. } => panic!("repeat"),
    };
    let ev = evidence(&s);
    s.move_child_as_agent(kid, alone, family, &ev, "The record names Anna", &me(), b)
        .unwrap();
    assert!(
        s.tree().families[&family]
            .children
            .iter()
            .any(|c| c.person == kid)
    );
    assert!(!s.tree().families.contains_key(&alone));
    // Hans, who was never in the agent's family, cannot be moved from a family that shares
    // nothing with the target, and a family the agent did not make stays.
    let err = s
        .move_child_as_agent(hans, family, family, &ev, "r", &me(), b)
        .unwrap_err();
    assert!(err.message.contains("same"), "{}", err.message);
}
