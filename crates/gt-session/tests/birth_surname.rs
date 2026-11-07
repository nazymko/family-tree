#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The birth surname shown under a name when the person goes by another surname.

use gt_core::date::calendar::{Calendar, first_jdn};
use gt_gedcom::{ImportOptions, import};
use gt_session::*;
use gt_store::Project;
use uuid::Uuid;

const NOW_MS: i64 = 1_793_664_000_000; // 2026-11-03

fn open_ged(ged: &str) -> (tempfile::TempDir, Session) {
    let today = first_jdn(Calendar::Gregorian, 2026, Some(11), Some(3)).unwrap();
    let out = import(ged.as_bytes(), &ImportOptions::default());
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("project");
    let mut p = Project::create(&proj).unwrap();
    p.save_tree(&out.tree, today).unwrap();
    drop(p);
    let s = Session::open(&proj, SessionClock::fixed(today, NOW_MS)).unwrap();
    (dir, s)
}

const GED: &str = "0 HEAD\n1 SOUR TEST\n2 VERS 1.0\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n\
0 @I1@ INDI\n1 NAME Ivan Petrovych /Koval/\n1 SEX M\n\
0 @I2@ INDI\n1 NAME Maria /Bondar/\n1 SEX F\n\
0 @I3@ INDI\n1 SEX F\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I3@\n0 TRLR\n";

fn id_of(s: &Session, name: &str) -> Uuid {
    s.list_persons(&PersonQuery::default(), 0, 200)
        .items
        .iter()
        .find(|r| r.person.name == name)
        .unwrap_or_else(|| panic!("no person {name}"))
        .person
        .id
}

fn save_name(
    person: Uuid,
    kind: NameKindDto,
    surname: &str,
    preferred: bool,
    private: bool,
) -> EditCommand {
    EditCommand::SaveName {
        person,
        name: NameInput {
            id: None,
            kind,
            kind_text: None,
            preferred,
            private,
            prefix: None,
            given: Some("Olena".into()),
            call: None,
            surname_prefix: None,
            surnames: vec![surname.into()],
            patronymic: None,
            suffix: None,
            nickname: None,
            lang: None,
            date_raw: None,
        },
    }
}

#[test]
fn a_married_woman_gets_her_birth_surname_and_nobody_else_does() {
    let (_d, mut s) = open_ged(GED);
    let ivan = id_of(&s, "Ivan Petrovych Koval");
    let maria = id_of(&s, "Maria Bondar");
    let olena = id_of(&s, "Unknown");

    // A man, a woman whose only name is her birth name, a person with no name: nothing.
    assert_eq!(s.person_ref(ivan).unwrap().birth_surname, None);
    assert_eq!(s.person_ref(maria).unwrap().birth_surname, None);
    assert_eq!(s.person_ref(olena).unwrap().birth_surname, None);

    // Her birth name is the main name: nothing.
    s.apply(save_name(olena, NameKindDto::Birth, "Bondar", true, false))
        .unwrap();
    assert_eq!(s.person_ref(olena).unwrap().birth_surname, None);

    // The married name becomes the main one: the birth surname shows, in the list rows too.
    s.apply(save_name(olena, NameKindDto::Married, "Koval", true, false))
        .unwrap();
    let r = s.person_ref(olena).unwrap();
    assert_eq!(r.name, "Olena Koval");
    assert_eq!(r.birth_surname.as_deref(), Some("Bondar"));
    let row = s
        .list_persons(&PersonQuery::default(), 0, 200)
        .items
        .into_iter()
        .find(|r| r.person.id == olena)
        .unwrap();
    assert_eq!(row.person.birth_surname.as_deref(), Some("Bondar"));
    assert_eq!(s.person_ref(ivan).unwrap().birth_surname, None);

    // The same surname, written with other case and spaces, is no change of surname.
    s.apply(save_name(
        olena,
        NameKindDto::Married,
        " BONDAR ",
        true,
        false,
    ))
    .unwrap();
    assert_eq!(s.person_ref(olena).unwrap().birth_surname, None);
}

#[test]
fn a_private_birth_name_is_not_shown() {
    let (_d, mut s) = open_ged(GED);
    let olena = id_of(&s, "Unknown");
    s.apply(save_name(olena, NameKindDto::Birth, "Bondar", false, true))
        .unwrap();
    s.apply(save_name(olena, NameKindDto::Married, "Koval", true, false))
        .unwrap();
    assert_eq!(s.person_ref(olena).unwrap().birth_surname, None);
}
