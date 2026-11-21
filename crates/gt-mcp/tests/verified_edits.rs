#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! correct_name, correct_event, move_child and correct_note: an agent changes what someone else
//! wrote, only with evidence (a source, a page, the record's words) and a reason.

mod common;

use common::*;
use gt_core::model::{EventKind, NameKind, ParticipantRef};
use gt_session::{EditCommand, NoteInput, OwnerKind, OwnerRef, SourceInput, UnionKindId};
use serde_json::{Value, json};
use uuid::Uuid;

fn begin(host: &TestHost) -> Value {
    call(host, "begin_batch", json!({"title": "Re-read records"}))["batch"].clone()
}

fn source(host: &TestHost) -> Uuid {
    with(host, |s| {
        s.apply(EditCommand::SaveSource {
            source: SourceInput {
                title: Some("Metric book of Testivka, 1880".into()),
                ..Default::default()
            },
        })
        .unwrap()
        .touched[0]
    })
}

fn evidence(source: Uuid) -> Value {
    json!({"source": source, "page": "p. 14, entry 3", "quote": "Georgiy, son of Grigoriy, Yaremа", "quality": 3})
}

/// Georgiy Hryhorovych Arema, whose right spelling is an alternative name; returns (person, main
/// name id, alternative name id).
fn misread(host: &TestHost) -> (Uuid, Uuid, Uuid) {
    with(host, |s| {
        let p = add_person(
            s,
            draft("Georgiy", "Arema", Some("Hryhorovych"), Some("1850")),
        );
        add_name_variant(s, p, "Georgiy", "Yarema");
        let names = &s.tree().persons[&p].names;
        let main = names.iter().find(|n| n.assertion.preferred).unwrap().id;
        let alt = names.iter().find(|n| !n.assertion.preferred).unwrap().id;
        (p, main, alt)
    })
}

fn names(host: &TestHost, p: Uuid) -> Vec<(String, bool, bool)> {
    with(host, |s| {
        s.tree().persons[&p]
            .names
            .iter()
            .map(|n| {
                (
                    n.surnames.join(" "),
                    n.assertion.preferred,
                    n.kind == NameKind::AsRecorded,
                )
            })
            .collect()
    })
}

fn undo(host: &TestHost, batch: &Value) {
    with(host, |s| {
        s.undo_batch(serde_json::from_value(batch.clone()).unwrap())
            .unwrap()
    });
}

fn citations_of_name(host: &TestHost, p: Uuid, name: Uuid) -> Vec<Uuid> {
    with(host, |s| {
        s.tree().persons[&p]
            .names
            .iter()
            .find(|n| n.id == name)
            .unwrap()
            .citations
            .clone()
    })
}

#[test]
fn correct_name_keeps_the_old_main_name_as_an_alternative() {
    let host = empty_host();
    let (p, main, _) = misread(&host);
    let src = source(&host);
    let batch = begin(&host);
    let done = call(
        &host,
        "correct_name",
        json!({"batch": batch, "person": p, "surnames": ["Yarema"], "evidence": evidence(src),
               "reason": "The record spells it with Ya"}),
    );
    assert_eq!(done["id"], json!(p));
    let n = names(&host, p);
    // The corrected name is the main one; the old text stays, as recorded, not preferred.
    let main_now = n.iter().find(|x| x.1).unwrap();
    assert_eq!(main_now.0, "Yarema");
    assert!(n.iter().any(|x| x.0 == "Arema" && !x.1 && x.2), "{n:?}");
    // The record is cited on the corrected name, with its words and page.
    let cits = citations_of_name(&host, p, main);
    assert_eq!(cits.len(), 1);
    with(&host, |s| {
        let c = &s.tree().citations[&cits[0]];
        assert_eq!(c.source, Some(src));
        assert_eq!(c.page.as_deref(), Some("p. 14, entry 3"));
        assert!(c.text.as_deref().unwrap().contains("Georgiy"));
        assert_eq!(c.quality, Some(3));
        // The history has the old value and the reason.
        let rows = s.project().rows_of(p).unwrap();
        let last = rows.last().unwrap();
        let surname = |r: &Option<gt_store::Record>| match r {
            Some(gt_store::Record::Person(x)) => x
                .names
                .iter()
                .find(|n| n.assertion.preferred)
                .unwrap()
                .surnames
                .join(" "),
            _ => panic!("not a person"),
        };
        assert_eq!(surname(&last.before), "Arema");
        assert_eq!(surname(&last.after), "Yarema");
        let h = s.history(p, 10).unwrap();
        assert!(
            h[0].label.contains("The record spells it with Ya"),
            "{}",
            h[0].label
        );
    });
    // Saying it again changes nothing.
    let again = call(
        &host,
        "correct_name",
        json!({"batch": batch, "person": p, "surnames": ["Yarema"], "evidence": evidence(src), "reason": "again"}),
    );
    assert_eq!(again["alreadyThere"], json!(true));
    undo(&host, &batch);
    let n = names(&host, p);
    assert_eq!(n.iter().find(|x| x.1).unwrap().0, "Arema");
    assert_eq!(n.len(), 2);
}

#[test]
fn make_preferred_promotes_the_alternative_and_keeps_the_old_main() {
    let host = empty_host();
    let (p, _, alt) = misread(&host);
    let src = source(&host);
    let batch = begin(&host);
    call(
        &host,
        "correct_name",
        json!({"batch": batch, "person": p, "name": alt, "make_preferred": true,
               "evidence": evidence(src), "reason": "The record reads Yarema"}),
    );
    let n = names(&host, p);
    assert_eq!(n.iter().find(|x| x.1).unwrap().0, "Yarema");
    assert!(n.iter().any(|x| x.0 == "Arema" && !x.1));
    assert_eq!(citations_of_name(&host, p, alt).len(), 1);
    undo(&host, &batch);
    assert_eq!(names(&host, p).iter().find(|x| x.1).unwrap().0, "Arema");
}

#[test]
fn corrections_need_real_evidence_and_a_reason() {
    let host = empty_host();
    let (p, _, _) = misread(&host);
    let src = source(&host);
    let batch = begin(&host);
    let ask = |args: Value| call_err(&host, "correct_name", args);
    let base = |ev: Value, reason: &str| json!({"batch": batch, "person": p, "surnames": ["Yarema"], "evidence": ev, "reason": reason});
    let m = ask(json!({"batch": batch, "person": p, "surnames": ["Yarema"], "reason": "r"}));
    assert!(m.contains("evidence"), "{m}");
    let m = ask(base(
        json!({"source": Uuid::from_u128(99), "page": "1", "quote": "q"}),
        "r",
    ));
    assert!(m.contains("source"), "{m}");
    let m = ask(base(
        json!({"source": src, "page": "1", "quote": "  "}),
        "r",
    ));
    assert!(m.contains("quote") || m.contains("words"), "{m}");
    let m = ask(base(json!({"source": src, "page": " ", "quote": "q"}), "r"));
    assert!(m.contains("page"), "{m}");
    let m = ask(base(json!({"source": src, "page": "1", "quote": "q"}), " "));
    assert!(m.contains("reason"), "{m}");
    // Outside a batch, or in a closed one, nothing is written.
    call(
        &host,
        "end_batch",
        json!({"batch": batch, "report": "Nothing."}),
    );
    let m = ask(base(json!({"source": src, "page": "1", "quote": "q"}), "r"));
    assert!(m.contains("closed"), "{m}");
    assert_eq!(names(&host, p).iter().find(|x| x.1).unwrap().0, "Arema");
}

fn owners_event(host: &TestHost, p: Uuid, kind: EventKind) -> Uuid {
    with(host, |s| {
        s.tree()
            .events
            .values()
            .find(|e| {
                e.kind == kind
                    && e.participants
                        .iter()
                        .any(|x| x.target == ParticipantRef::Person(p))
            })
            .unwrap()
            .common
            .id
    })
}

#[test]
fn correct_event_changes_date_place_and_description_with_a_citation() {
    let host = empty_host();
    let p = with(&host, |s| {
        add_person(s, draft("Ivan", "Testenko", None, Some("1850")))
    });
    let birth = owners_event(&host, p, EventKind::Birth);
    let src = source(&host);
    let batch = begin(&host);
    call(
        &host,
        "correct_event",
        json!({"batch": batch, "event": birth, "date": "12 MAR 1850", "place": "Testivka, Testovyi povit",
               "description": "Baptised the next day", "evidence": evidence(src), "reason": "The record gives the day"}),
    );
    with(&host, |s| {
        let e = &s.tree().events[&birth];
        assert_eq!(e.date.as_ref().unwrap().raw, "12 MAR 1850");
        assert_eq!(e.description.as_deref(), Some("Baptised the next day"));
        assert!(e.place.is_some());
        assert_eq!(e.common.citations.len(), 1);
        assert_eq!(s.tree().citations[&e.common.citations[0]].source, Some(src));
        let h = s.history(birth, 10).unwrap();
        assert!(h[0].label.contains("The record gives the day"));
    });
    undo(&host, &batch);
    with(&host, |s| {
        let e = &s.tree().events[&birth];
        assert_eq!(e.date.as_ref().unwrap().raw, "1850");
        assert!(e.description.is_none() && e.place.is_none());
        assert!(e.common.citations.is_empty());
    });
}

#[test]
fn correct_event_still_refuses_to_change_alive_or_dead() {
    let host = empty_host();
    let p = with(&host, |s| {
        add_person(s, draft("Olena", "Testenko", None, Some("1900")))
    });
    let birth = owners_event(&host, p, EventKind::Birth);
    let src = source(&host);
    let batch = begin(&host);
    let m = call_err(
        &host,
        "correct_event",
        json!({"batch": batch, "event": birth, "date": "1990", "evidence": evidence(src), "reason": "r"}),
    );
    assert!(m.contains("alive or dead"), "{m}");
    with(&host, |s| {
        assert_eq!(s.tree().events[&birth].date.as_ref().unwrap().raw, "1900");
    });
}

struct Fam {
    child: Uuid,
    father: Uuid,
    only: Uuid,
    both: Uuid,
}

fn families(host: &TestHost) -> Fam {
    with(host, |s| {
        let father = add_person(s, draft("Ivan", "Testenko", None, Some("1850")));
        let mother = add_person(s, draft("Maria", "Testenko", None, Some("1855")));
        let child = add_relative(
            s,
            father,
            gt_session::Relation::Child,
            draft("Petro", "Testenko", None, Some("1880")),
        );
        let only = s
            .tree()
            .families
            .values()
            .find(|f| f.children.iter().any(|c| c.person == child))
            .unwrap()
            .common
            .id;
        let both = s
            .apply(EditCommand::SaveFamily {
                id: None,
                partner1: Some(father),
                partner2: Some(mother),
                kind: UnionKindId::Unknown,
            })
            .unwrap()
            .touched
            .iter()
            .copied()
            .find(|id| s.tree().families.contains_key(id) && *id != only)
            .unwrap();
        Fam {
            child,
            father,
            only,
            both,
        }
    })
}

fn child_in(host: &TestHost, fam: Uuid, child: Uuid) -> bool {
    with(host, |s| {
        s.tree()
            .families
            .get(&fam)
            .is_some_and(|f| f.children.iter().any(|c| c.person == child))
    })
}

#[test]
fn move_child_moves_the_link_between_families_sharing_a_parent() {
    let host = empty_host();
    let f = families(&host);
    let src = source(&host);
    let batch = begin(&host);
    let done = call(
        &host,
        "move_child",
        json!({"batch": batch, "person": f.child, "from_family": f.only, "to_family": f.both,
               "evidence": evidence(src), "reason": "The record names the mother"}),
    );
    assert_eq!(done["id"], json!(f.child));
    assert!(child_in(&host, f.both, f.child));
    assert!(!child_in(&host, f.only, f.child));
    with(&host, |s| {
        let link = s.tree().families[&f.both]
            .children
            .iter()
            .find(|c| c.person == f.child)
            .unwrap();
        assert_eq!(link.citations.len(), 1);
        assert_eq!(s.tree().citations[&link.citations[0]].source, Some(src));
        // A note on the child records the move and the reason.
        let notes = &s.tree().persons[&f.child].common.notes;
        assert!(notes.iter().any(|n| {
            s.tree().notes[n]
                .text
                .contains("The record names the mother")
        }));
    });
    let again = call(
        &host,
        "move_child",
        json!({"batch": batch, "person": f.child, "from_family": f.only, "to_family": f.both,
               "evidence": evidence(src), "reason": "again"}),
    );
    assert_eq!(again["alreadyThere"], json!(true));
    undo(&host, &batch);
    assert!(child_in(&host, f.only, f.child));
    assert!(!child_in(&host, f.both, f.child));
    let _ = f.father;
}

#[test]
fn move_child_refuses_families_that_share_no_parent() {
    let host = empty_host();
    let f = families(&host);
    let src = source(&host);
    let stranger_family = with(&host, |s| {
        let a = add_person(s, draft("Stepan", "Other", None, Some("1850")));
        let b = add_person(s, draft("Hanna", "Other", None, Some("1855")));
        s.apply(EditCommand::SaveFamily {
            id: None,
            partner1: Some(a),
            partner2: Some(b),
            kind: UnionKindId::Unknown,
        })
        .unwrap()
        .touched
        .iter()
        .copied()
        .find(|id| s.tree().families.contains_key(id))
        .unwrap()
    });
    let batch = begin(&host);
    let m = call_err(
        &host,
        "move_child",
        json!({"batch": batch, "person": f.child, "from_family": f.only, "to_family": stranger_family,
               "evidence": evidence(src), "reason": "r"}),
    );
    assert!(m.contains("share no parent"), "{m}");
    assert!(child_in(&host, f.only, f.child));
    let m = call_err(
        &host,
        "move_child",
        json!({"batch": batch, "person": f.child, "from_family": f.only, "to_family": f.both, "reason": "r"}),
    );
    assert!(m.contains("evidence"), "{m}");
}

/// A new family of `a` and `b`; returns its id.
fn couple(host: &TestHost, a: Uuid, b: Uuid) -> Uuid {
    with(host, |s| {
        s.apply(EditCommand::SaveFamily {
            id: None,
            partner1: Some(a),
            partner2: Some(b),
            kind: UnionKindId::Unknown,
        })
        .unwrap()
        .touched
        .iter()
        .copied()
        .find(|id| s.tree().families.contains_key(id))
        .unwrap()
    })
}

fn birth_link(host: &TestHost, fam: Uuid, child: Uuid) {
    with(host, |s| {
        s.apply(EditCommand::LinkChild {
            family: fam,
            child,
            rel: (
                gt_session::ParentRelId::Birth,
                gt_session::ParentRelId::Birth,
            ),
            preferred: true,
        })
        .unwrap();
    });
}

#[test]
fn move_child_replaces_the_mother_when_both_families_have_two_parents() {
    let host = empty_host();
    let f = families(&host);
    let src = source(&host);
    let mother_b = with(&host, |s| {
        add_person(s, draft("Olha", "Testenko", None, Some("1858")))
    });
    let other = couple(&host, f.father, mother_b);
    birth_link(&host, f.both, f.child);
    with(&host, |s| {
        s.apply(EditCommand::UnlinkChild {
            family: f.only,
            child: f.child,
        })
        .unwrap();
    });
    let batch = begin(&host);
    call(
        &host,
        "move_child",
        json!({"batch": batch, "person": f.child, "from_family": f.both, "to_family": other,
               "evidence": evidence(src), "reason": "The record names Olha"}),
    );
    assert!(child_in(&host, other, f.child));
    assert!(!child_in(&host, f.both, f.child));
    undo(&host, &batch);
    assert!(child_in(&host, f.both, f.child));
    assert!(!child_in(&host, other, f.child));
}

#[test]
fn move_child_still_refuses_a_second_birth_mother_through_another_family() {
    let host = empty_host();
    let f = families(&host);
    let src = source(&host);
    let mother_b = with(&host, |s| {
        add_person(s, draft("Olha", "Testenko", None, Some("1858")))
    });
    let mother_c = with(&host, |s| {
        add_person(s, draft("Hanna", "Other", None, Some("1859")))
    });
    let other = couple(&host, f.father, mother_b);
    let third = couple(&host, mother_c, f.father);
    birth_link(&host, f.both, f.child);
    birth_link(&host, third, f.child);
    let batch = begin(&host);
    let m = call_err(
        &host,
        "move_child",
        json!({"batch": batch, "person": f.child, "from_family": f.both, "to_family": other,
               "evidence": evidence(src), "reason": "r"}),
    );
    assert!(
        m.contains("parent in that role") || m.contains("birth family"),
        "{m}"
    );
    assert!(child_in(&host, f.both, f.child));
    assert!(!child_in(&host, other, f.child));
}

fn note_with_story(host: &TestHost, story: Option<&str>) -> (Uuid, Uuid) {
    with(host, |s| {
        let p = add_person(s, draft("Ivan", "Testenko", None, Some("1990")));
        s.apply(EditCommand::SaveNote {
            target: OwnerRef {
                kind: OwnerKind::Person,
                id: p,
            },
            note: NoteInput {
                story: story.map(Into::into),
                id: None,
                kind: gt_session::NoteKindDto::Research,
                text: "Read as Arema.".into(),
            },
        })
        .unwrap();
        (p, s.get_person(p).unwrap().notes[0].id)
    })
}

#[test]
fn correct_note_replaces_the_text_and_keeps_the_old_in_history() {
    let host = empty_host();
    let (p, note) = note_with_story(&host, None);
    let src = source(&host);
    let batch = begin(&host);
    call(
        &host,
        "correct_note",
        json!({"batch": batch, "note": note, "text": "Read as Yarema.", "evidence": evidence(src), "reason": "Re-read"}),
    );
    with(&host, |s| {
        assert_eq!(s.tree().notes[&note].text, "Read as Yarema.");
        let c = &s.tree().persons[&p].common.citations;
        assert_eq!(c.len(), 1);
        let rows = s.project().rows_of(note).unwrap();
        let last = rows.last().unwrap();
        let text = |r: &Option<gt_store::Record>| match r {
            Some(gt_store::Record::Note(n)) => n.text.clone(),
            _ => panic!("not a note"),
        };
        assert_eq!(text(&last.before), "Read as Arema.");
    });
    undo(&host, &batch);
    assert_eq!(
        with(&host, |s| s.tree().notes[&note].text.clone()),
        "Read as Arema."
    );
}

#[test]
fn correct_note_with_a_story_needs_the_story_or_clear_story() {
    let host = empty_host();
    let (_, note) = note_with_story(&host, Some("A retelling of Arema."));
    let src = source(&host);
    let batch = begin(&host);
    let args = |extra: Value| {
        let mut a = json!({"batch": batch, "note": note, "text": "Read as Yarema.",
                           "evidence": evidence(src), "reason": "Re-read"});
        a.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        a
    };
    let m = call_err(&host, "correct_note", args(json!({})));
    assert!(m.contains("story"), "{m}");
    call(
        &host,
        "correct_note",
        args(json!({"story": "A retelling of Yarema."})),
    );
    with(&host, |s| {
        assert_eq!(
            s.tree().notes[&note].story.as_deref(),
            Some("A retelling of Yarema.")
        );
    });
    call(
        &host,
        "correct_note",
        json!({"batch": batch, "note": note, "text": "Read as Yarema, twice.", "clear_story": true,
               "evidence": evidence(src), "reason": "Re-read"}),
    );
    with(&host, |s| assert!(s.tree().notes[&note].story.is_none()));
}
