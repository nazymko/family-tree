#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! `revert_change`: undoing one change against whatever the record has become since.

use gt_core::model::*;
use gt_session::revert::{Reverted, revert_change};
use gt_store::{Change, Record};
use proptest::prelude::*;
use std::collections::HashSet;
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn common(n: u128) -> Common {
    Common {
        id: id(n),
        private: false,
        changed: Some(1),
        citations: vec![],
        notes: vec![],
        media: vec![],
        ext_ids: vec![],
        unknown: vec![],
    }
}

fn name(n: u128, given: &str) -> Name {
    Name {
        id: id(n),
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
        citations: vec![],
        notes: vec![],
        unknown: vec![],
    }
}

fn person(n: u128, given: &str) -> Person {
    Person {
        common: common(n),
        sex: Sex::Male,
        names: vec![name(n + 1000, given)],
        living_override: None,
    }
}

fn note(n: u128, text: &str) -> Note {
    Note {
        common: common(n),
        kind: NoteKind::General,
        text: text.into(),
        lang: None,
        shared: false,
    }
}

fn link(person: Uuid) -> ChildLink {
    ChildLink {
        person,
        rel1: ParentRel::Birth,
        rel2: ParentRel::Birth,
        preferred: true,
        assertion: Assertion {
            preferred: false,
            proof: None,
            confidence: None,
        },
        citations: vec![],
        unknown: vec![],
    }
}

fn family(n: u128, partner1: Option<Uuid>, children: Vec<ChildLink>) -> Family {
    Family {
        common: common(n),
        partner1,
        partner2: None,
        kind: UnionKind::Married,
        children,
    }
}

fn change(before: Option<Record>, after: Option<Record>) -> Change {
    let rec = before.as_ref().or(after.as_ref()).unwrap();
    Change {
        kind: rec.kind(),
        id: rec.id(),
        before,
        after,
    }
}

fn none() -> HashSet<Uuid> {
    HashSet::new()
}

#[test]
fn untouched_returns_before_exactly() {
    let before = person(1, "Ivan");
    let mut after = before.clone();
    after.names[0].given = Some("Ivanko".into());
    after.common.changed = Some(50);
    let mut current = after.clone();
    current.common.changed = Some(99); // only the stamp differs
    let ch = change(
        Some(Record::Person(before.clone())),
        Some(Record::Person(after)),
    );
    assert_eq!(
        revert_change(&ch, Some(&Record::Person(current)), &none()),
        Reverted::Write(Some(Record::Person(before)))
    );
}

#[test]
fn undo_one_of_two_notes_keeps_other() {
    let before = person(1, "Ivan");
    let mut after = before.clone();
    after.common.notes = vec![id(10)];
    let mut current = after.clone();
    current.common.notes = vec![id(10), id(11)];
    current.common.changed = Some(77);
    let ch = change(Some(Record::Person(before)), Some(Record::Person(after)));
    let Reverted::Write(Some(Record::Person(p))) =
        revert_change(&ch, Some(&Record::Person(current)), &none())
    else {
        panic!("expected a write");
    };
    assert_eq!(p.common.notes, vec![id(11)]);
    assert_eq!(p.common.changed, Some(77), "the caller stamps `changed`");
}

#[test]
fn later_owner_scalar_edit_survives_and_conflicts() {
    let before = person(1, "Ivan");
    let mut after = before.clone();
    after.names[0].given = Some("Ivanko".into());
    after.common.private = true;
    let mut current = after.clone();
    current.names[0].given = Some("Iwan".into()); // the owner typed over it
    let ch = change(Some(Record::Person(before)), Some(Record::Person(after)));
    let Reverted::Conflict(Some(Record::Person(p))) =
        revert_change(&ch, Some(&Record::Person(current)), &none())
    else {
        panic!("expected a conflict");
    };
    assert_eq!(p.names[0].given.as_deref(), Some("Iwan"));
    assert!(!p.common.private, "what could be undone still is");
}

#[test]
fn created_person_with_later_note_is_kept_with_family_link() {
    let created = person(1, "Ivan");
    let mut current = created.clone();
    current.common.notes = vec![id(10)];
    let create = change(None, Some(Record::Person(created.clone())));
    assert_eq!(
        revert_change(&create, Some(&Record::Person(current.clone())), &none()),
        Reverted::Conflict(Some(Record::Person(current)))
    );

    let kept = HashSet::from([created.common.id]);
    // The family was there before; the change added the child and another one.
    let before = family(2, Some(id(5)), vec![]);
    let after = family(2, Some(id(5)), vec![link(id(1)), link(id(6))]);
    let ch = change(
        Some(Record::Family(before)),
        Some(Record::Family(after.clone())),
    );
    let Reverted::Conflict(Some(Record::Family(f))) =
        revert_change(&ch, Some(&Record::Family(after)), &kept)
    else {
        panic!("expected a conflict");
    };
    let children: Vec<Uuid> = f.children.iter().map(|c| c.person).collect();
    assert_eq!(children, vec![id(1)], "the unrelated new child is undone");

    // A partner scalar that names the kept person stays.
    let before = family(3, None, vec![]);
    let after = family(3, Some(id(1)), vec![]);
    let ch = change(
        Some(Record::Family(before)),
        Some(Record::Family(after.clone())),
    );
    let Reverted::Conflict(Some(Record::Family(f))) =
        revert_change(&ch, Some(&Record::Family(after)), &kept)
    else {
        panic!("expected a conflict");
    };
    assert_eq!(f.partner1, Some(id(1)));

    // A family created in the same change is one of those links, though nobody touched it.
    let created_family = family(4, Some(id(1)), vec![]);
    let ch = change(None, Some(Record::Family(created_family.clone())));
    assert_eq!(
        revert_change(&ch, Some(&Record::Family(created_family.clone())), &kept),
        Reverted::Conflict(Some(Record::Family(created_family)))
    );
}

#[test]
fn created_note_with_edited_text_kept_and_reference_kept() {
    let created = note(10, "Born in Kyiv, says the recording");
    let mut edited = created.clone();
    edited.text = "Born in Kyiv (confirmed)".into();
    let create = change(None, Some(Record::Note(created)));
    assert_eq!(
        revert_change(&create, Some(&Record::Note(edited.clone())), &none()),
        Reverted::Conflict(Some(Record::Note(edited)))
    );

    let before = person(1, "Ivan");
    let mut after = before.clone();
    after.common.notes = vec![id(10)];
    let ch = change(
        Some(Record::Person(before)),
        Some(Record::Person(after.clone())),
    );
    let kept = HashSet::from([id(10)]);
    let Reverted::Conflict(Some(Record::Person(p))) =
        revert_change(&ch, Some(&Record::Person(after)), &kept)
    else {
        panic!("expected a conflict");
    };
    assert_eq!(p.common.notes, vec![id(10)]);
}

#[test]
fn created_absent_and_deleted_present_change_nothing() {
    let p = person(1, "Ivan");
    let create = change(None, Some(Record::Person(p.clone())));
    assert_eq!(revert_change(&create, None, &none()), Reverted::Unchanged);
    let delete = change(Some(Record::Person(p.clone())), None);
    assert_eq!(
        revert_change(&delete, None, &none()),
        Reverted::Write(Some(Record::Person(p.clone())))
    );
    assert_eq!(
        revert_change(&delete, Some(&Record::Person(p)), &none()),
        Reverted::Unchanged
    );
}

#[test]
fn removed_item_returns_at_its_old_index_and_others_keep_their_order() {
    let before = family(2, None, vec![link(id(1)), link(id(2)), link(id(3))]);
    let mut after = before.clone();
    after.children.remove(1);
    let mut current = after.clone();
    current.children.push(link(id(4)));
    let ch = change(Some(Record::Family(before)), Some(Record::Family(after)));
    let Reverted::Write(Some(Record::Family(f))) =
        revert_change(&ch, Some(&Record::Family(current)), &none())
    else {
        panic!("expected a write");
    };
    let order: Vec<Uuid> = f.children.iter().map(|c| c.person).collect();
    assert_eq!(order, vec![id(1), id(2), id(3), id(4)]);
}

#[test]
fn item_edited_since_is_matched_by_identity_and_kept() {
    // A child link added by the change, whose relation someone changed since.
    let before = family(2, None, vec![]);
    let after = family(2, None, vec![link(id(1))]);
    let mut current = after.clone();
    current.children[0].rel1 = ParentRel::Adopted;
    let ch = change(Some(Record::Family(before)), Some(Record::Family(after)));
    assert_eq!(
        revert_change(&ch, Some(&Record::Family(current.clone())), &none()),
        Reverted::Conflict(Some(Record::Family(current)))
    );
    // A name the change edited: only the changed field goes back, the owner's field stays.
    let before = person(1, "Ivan");
    let mut after = before.clone();
    after.names[0].given = Some("Ivanko".into());
    let mut current = after.clone();
    current.names[0].nickname = Some("Vanya".into());
    let ch = change(Some(Record::Person(before)), Some(Record::Person(after)));
    let Reverted::Write(Some(Record::Person(p))) =
        revert_change(&ch, Some(&Record::Person(current)), &none())
    else {
        panic!("expected a write");
    };
    assert_eq!(p.names[0].given.as_deref(), Some("Ivan"));
    assert_eq!(p.names[0].nickname.as_deref(), Some("Vanya"));
}

#[test]
fn reorder_is_undone_around_a_later_addition() {
    let before = family(2, None, vec![link(id(1)), link(id(2))]);
    let after = family(2, None, vec![link(id(2)), link(id(1))]);
    let mut current = after.clone();
    current.children.push(link(id(3)));
    let ch = change(Some(Record::Family(before)), Some(Record::Family(after)));
    let Reverted::Write(Some(Record::Family(f))) =
        revert_change(&ch, Some(&Record::Family(current)), &none())
    else {
        panic!("expected a write");
    };
    let order: Vec<Uuid> = f.children.iter().map(|c| c.person).collect();
    assert_eq!(order, vec![id(1), id(2), id(3)]);
}

#[test]
fn a_date_goes_back_whole_or_not_at_all() {
    use gt_core::date::{DayOrder, GenDate};
    let date = |raw: &str| Some(GenDate::parse(raw, DayOrder::Dmy));
    let mut before = person(1, "Ivan");
    before.names[0].date = date("1 Jan 1900");
    let mut after = before.clone();
    after.names[0].date = date("2 Feb 1901");
    let mut current = after.clone();
    current.names[0].date = date("2 Feb 1900");
    let ch = change(Some(Record::Person(before)), Some(Record::Person(after)));
    assert_eq!(
        revert_change(&ch, Some(&Record::Person(current.clone())), &none()),
        Reverted::Conflict(Some(Record::Person(current)))
    );
}

#[test]
fn surnames_are_one_fact() {
    let mut before = person(1, "Ivan");
    before.names[0].surnames = vec!["Koval".into()];
    let mut after = before.clone();
    after.names[0].surnames = vec!["Kovalenko".into()];
    let mut current = after.clone();
    current.names[0].surnames = vec!["Kovalchuk".into()];
    let ch = change(
        Some(Record::Person(before.clone())),
        Some(Record::Person(after)),
    );
    assert_eq!(
        revert_change(&ch, Some(&Record::Person(current.clone())), &none()),
        Reverted::Conflict(Some(Record::Person(current)))
    );

    // An added surname with a later one next to it is no longer a list to pick apart either.
    let mut after = before.clone();
    after.names[0].surnames = vec!["Koval".into(), "Bondar".into()];
    let mut current = after.clone();
    current.names[0].surnames = vec!["Koval".into(), "Bondar".into(), "Melnyk".into()];
    let ch = change(
        Some(Record::Person(before.clone())),
        Some(Record::Person(after.clone())),
    );
    assert_eq!(
        revert_change(&ch, Some(&Record::Person(current.clone())), &none()),
        Reverted::Conflict(Some(Record::Person(current)))
    );

    // Still as the change wrote it: back it goes.
    let ch = change(
        Some(Record::Person(before.clone())),
        Some(Record::Person(after.clone())),
    );
    assert_eq!(
        revert_change(&ch, Some(&Record::Person(after)), &none()),
        Reverted::Write(Some(Record::Person(before)))
    );
}

fn event(n: u128, participants: Vec<Participant>) -> Event {
    Event {
        common: common(n),
        kind: EventKind::Birth,
        date: None,
        place: None,
        description: None,
        age: None,
        cause: None,
        negative: false,
        assertion: Assertion {
            preferred: true,
            proof: None,
            confidence: None,
        },
        participants,
    }
}

fn takes_part(n: u128, role: Role) -> Participant {
    Participant {
        target: ParticipantRef::Person(id(n)),
        role,
    }
}

#[test]
fn a_replaced_participant_changed_since_stays_untouched() {
    let before = event(1, vec![takes_part(5, Role::Witness)]);
    let after = event(1, vec![takes_part(5, Role::Godparent)]);
    let current = event(1, vec![takes_part(5, Role::Clergy)]);
    let ch = change(Some(Record::Event(before)), Some(Record::Event(after)));
    assert_eq!(
        revert_change(&ch, Some(&Record::Event(current.clone())), &none()),
        Reverted::Conflict(Some(Record::Event(current)))
    );
}

#[test]
fn a_replaced_participant_goes_back_around_a_later_one() {
    let before = event(1, vec![takes_part(5, Role::Witness)]);
    let after = event(1, vec![takes_part(5, Role::Godparent)]);
    let current = event(
        1,
        vec![takes_part(5, Role::Godparent), takes_part(6, Role::Clergy)],
    );
    let ch = change(Some(Record::Event(before)), Some(Record::Event(after)));
    let Reverted::Write(Some(Record::Event(e))) =
        revert_change(&ch, Some(&Record::Event(current)), &none())
    else {
        panic!("expected a write");
    };
    assert_eq!(
        e.participants,
        vec![takes_part(5, Role::Witness), takes_part(6, Role::Clergy)]
    );
}

#[test]
fn undoing_a_replacement_again_changes_nothing() {
    // The first undo put the old item back and someone has added another since.
    let before = event(
        1,
        vec![takes_part(5, Role::Witness), takes_part(7, Role::Parent)],
    );
    let after = event(
        1,
        vec![takes_part(5, Role::Godparent), takes_part(7, Role::Parent)],
    );
    let mut current = before.clone();
    current.participants.push(takes_part(6, Role::Clergy));
    let ch = change(Some(Record::Event(before)), Some(Record::Event(after)));
    assert_eq!(
        revert_change(&ch, Some(&Record::Event(current)), &none()),
        Reverted::Unchanged
    );
}

#[test]
fn a_replaced_media_link_changed_since_stays_untouched() {
    let picture = |primary: bool, w: Option<f32>| MediaLink {
        media: id(9),
        region: w.map(|w| Region {
            x: 0.0,
            y: 0.0,
            w,
            h: 1.0,
        }),
        primary,
    };
    let with = |links: Vec<MediaLink>| {
        let mut p = person(1, "Ivan");
        p.common.media = links;
        p
    };
    let before = with(vec![picture(true, None)]);
    let after = with(vec![picture(false, None)]);
    let current = with(vec![picture(false, Some(0.5))]);
    let ch = change(
        Some(Record::Person(before.clone())),
        Some(Record::Person(after.clone())),
    );
    assert_eq!(
        revert_change(&ch, Some(&Record::Person(current.clone())), &none()),
        Reverted::Conflict(Some(Record::Person(current)))
    );
    // Nobody touched it: the old link comes back.
    assert_eq!(
        revert_change(&ch, Some(&Record::Person(after)), &none()),
        Reverted::Write(Some(Record::Person(before)))
    );
}

fn arb_person() -> impl Strategy<Value = Person> {
    (
        any::<bool>(),
        proptest::sample::subsequence(vec![101u128, 102, 103, 104], 0..=4),
        proptest::sample::subsequence(vec![201u128, 202, 203], 0..=3),
        "[A-Z][a-z]{0,6}",
        proptest::option::of(1i64..1000),
        proptest::sample::select(vec![Sex::Male, Sex::Female, Sex::Other("x".into())]),
    )
        .prop_map(|(private, notes, cits, given, changed, sex)| {
            let mut p = person(1, &given);
            p.common.private = private;
            p.common.notes = notes.into_iter().map(id).collect();
            p.common.citations = cits.into_iter().map(id).collect();
            p.common.changed = changed;
            p.sex = sex;
            p
        })
}

proptest! {
    #[test]
    fn apply_then_revert_is_identity(before in arb_person(), after in arb_person(), stamp in 1i64..1000) {
        let mut current = after.clone();
        current.common.changed = Some(stamp);
        let ch = change(
            Some(Record::Person(before.clone())),
            Some(Record::Person(after)),
        );
        prop_assert_eq!(
            revert_change(&ch, Some(&Record::Person(current)), &none()),
            Reverted::Write(Some(Record::Person(before)))
        );
    }

    /// Something added after the change survives its undo, and everything else goes back.
    #[test]
    fn revert_under_a_later_note_keeps_only_that_note(before in arb_person(), after in arb_person()) {
        let later = id(999);
        let mut current = after.clone();
        current.common.notes.push(later);
        let same = {
            let mut a = after.clone();
            a.common.changed = before.common.changed;
            a == before
        };
        let ch = change(
            Some(Record::Person(before.clone())),
            Some(Record::Person(after)),
        );
        let got = revert_change(&ch, Some(&Record::Person(current)), &none());
        if same {
            // The change only moved the stamp: nothing to undo.
            prop_assert_eq!(got, Reverted::Unchanged);
            return Ok(());
        }
        let Reverted::Write(Some(Record::Person(mut p))) = got else {
            return Err(TestCaseError::fail(format!("expected a write, got {got:?}")));
        };
        let pos = p.common.notes.iter().position(|n| *n == later);
        prop_assert!(pos.is_some());
        p.common.notes.retain(|n| *n != later);
        p.common.changed = before.common.changed;
        prop_assert_eq!(p, before);
    }
}
