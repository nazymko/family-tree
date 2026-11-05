#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The name as it was written in the source (`raw`) survives an edit that leaves the parts it was
//! made from alone, and goes when one of those parts changes, so that it never sits beside parts
//! it no longer matches.

mod common;

use common::*;
use gt_session::*;

const IVAN: &str = "0 @I1@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n";

/// The form the name editor saves: the stored name, field by field, with `change` applied.
fn save(s: &mut Session, change: impl FnOnce(&mut NameInput)) {
    let ivan = find(s.tree(), "Ivan");
    let n = s.tree().persons[&ivan].names[0].clone();
    let mut input = NameInput {
        id: Some(n.id),
        kind: NameKindDto::Birth,
        kind_text: None,
        preferred: n.assertion.preferred,
        private: n.private,
        prefix: n.prefix.clone(),
        given: n.given.clone(),
        call: n.call.clone(),
        surname_prefix: n.surname_prefix.clone(),
        surnames: n.surnames.clone(),
        patronymic: n.patronymic.clone(),
        suffix: n.suffix.clone(),
        nickname: n.nickname.clone(),
        lang: n.lang.clone(),
        date_raw: None,
    };
    change(&mut input);
    s.apply(EditCommand::SaveName {
        person: ivan,
        name: input,
    })
    .unwrap();
}

fn raw(s: &Session) -> Option<String> {
    s.tree().persons.values().next().unwrap().names[0]
        .raw
        .clone()
}

#[test]
fn an_import_keeps_the_name_as_written() {
    let (_d, s) = open_tree(&tree_of(IVAN));
    assert_eq!(raw(&s).as_deref(), Some("Ivan /Koval/"));
}

#[test]
fn editing_only_the_nickname_keeps_the_raw_text() {
    let (_d, mut s) = open_tree(&tree_of(IVAN));
    save(&mut s, |n| n.nickname = Some("Vanya".into()));
    assert_eq!(raw(&s).as_deref(), Some("Ivan /Koval/"));
    let ivan = find(s.tree(), "Ivan");
    assert_eq!(
        s.tree().persons[&ivan].names[0].nickname.as_deref(),
        Some("Vanya")
    );
    // Saving it again, untouched, changes nothing either.
    save(&mut s, |_| {});
    assert_eq!(raw(&s).as_deref(), Some("Ivan /Koval/"));
}

#[test]
fn language_and_preferred_flag_do_not_touch_it() {
    let (_d, mut s) = open_tree(&tree_of(IVAN));
    save(&mut s, |n| {
        n.lang = Some("uk".into());
        n.private = true;
    });
    assert_eq!(raw(&s).as_deref(), Some("Ivan /Koval/"));
}

#[test]
fn changing_a_part_of_the_name_drops_the_raw_text() {
    for change in [
        (|n: &mut NameInput| n.given = Some("Ivanko".into())) as fn(&mut NameInput),
        |n| n.surnames = vec!["Kovalenko".into()],
        |n| n.patronymic = Some("Petrovych".into()),
        |n| n.prefix = Some("Dr.".into()),
        |n| n.suffix = Some("Jr.".into()),
        |n| n.surname_prefix = Some("von".into()),
    ] {
        let (_d, mut s) = open_tree(&tree_of(IVAN));
        save(&mut s, change);
        assert_eq!(raw(&s), None, "a changed part must not keep a stale raw");
    }
}
