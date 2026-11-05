#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! A name change says what changed: the field, from what, to what.

mod common;

use common::*;
use gt_core::date::GenDate;
use gt_core::model::{Name, NameKind, Person, Proof, Sex};
use gt_core::raw::RawNode;
use gt_session::*;
use uuid::Uuid;

fn name(n: u128) -> Name {
    Name {
        id: Uuid::from_u128(n),
        kind: NameKind::Birth,
        assertion: gt_core::model::Assertion {
            preferred: false,
            proof: None,
            confidence: None,
        },
        private: false,
        prefix: None,
        given: Some("Ivan".into()),
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

fn person(names: Vec<Name>) -> Person {
    let mut p = Person {
        common: gt_core::model::Common {
            id: Uuid::from_u128(1),
            private: false,
            changed: None,
            citations: vec![],
            notes: vec![],
            media: vec![],
            ext_ids: vec![],
            unknown: vec![],
        },
        sex: Sex::Male,
        names,
        living_override: None,
        sibling_marker: false,
    };
    p.sibling_marker = false;
    p
}

fn one(before: &Name, after: &Name) -> Vec<FieldChangeDto> {
    person_field_changes(&person(vec![before.clone()]), &person(vec![after.clone()]))
}

fn only(v: &[FieldChangeDto]) -> (&str, Option<&str>, Option<&str>) {
    assert_eq!(v.len(), 1, "{v:?}");
    (
        v[0].field.as_str(),
        v[0].before.as_deref(),
        v[0].after.as_deref(),
    )
}

#[test]
fn every_text_field_reports_from_and_to() {
    type Set = fn(&mut Name, Option<String>);
    let cases: [(&str, Set); 8] = [
        ("prefix", |n, v| n.prefix = v),
        ("given", |n, v| n.given = v),
        ("call", |n, v| n.call = v),
        ("surnamePrefix", |n, v| n.surname_prefix = v),
        ("patronymic", |n, v| n.patronymic = v),
        ("suffix", |n, v| n.suffix = v),
        ("nickname", |n, v| n.nickname = v),
        ("lang", |n, v| n.lang = v),
    ];
    for (field, set) in cases {
        let (mut a, mut b) = (name(1), name(1));
        set(&mut a, Some("Old ".into()));
        set(&mut b, Some(" New".into()));
        let d = one(&a, &b);
        assert_eq!(only(&d), (field, Some("Old "), Some(" New")), "{field}");
        // empty on one side
        set(&mut a, None);
        let d = one(&a, &b);
        assert_eq!(only(&d), (field, None, Some(" New")), "{field}");
    }
    let (mut a, mut b) = (name(1), name(1));
    a.raw = Some("Koval, Ivan".into());
    b.raw = Some("Ivan Koval".into());
    assert_eq!(
        only(&one(&a, &b)),
        ("raw", Some("Koval, Ivan"), Some("Ivan Koval"))
    );
}

#[test]
fn surname_change_reads_ivan_koval_to_kovalchuk() {
    let a = name(1);
    let mut b = name(1);
    b.surnames = vec!["Kovalchuk".into()];
    assert_eq!(
        only(&one(&a, &b)),
        ("surnames", Some("Koval"), Some("Kovalchuk"))
    );
    b.surnames = vec!["Koval".into(), "Petrenko".into()];
    assert_eq!(
        only(&one(&a, &b)),
        ("surnames", Some("Koval"), Some("Koval, Petrenko"))
    );
}

#[test]
fn kind_flags_assertion_date_and_counts() {
    let a = name(1);
    let mut b = name(1);
    b.kind = NameKind::Other("Pen name".into());
    assert_eq!(
        only(&one(&a, &b)),
        ("kind", Some("birth"), Some("other:Pen name"))
    );
    let mut b = name(1);
    b.kind = NameKind::Married;
    assert_eq!(only(&one(&a, &b)), ("kind", Some("birth"), Some("married")));
    let mut b = name(1);
    b.assertion.preferred = true;
    assert_eq!(only(&one(&a, &b)), ("preferred", Some("no"), Some("yes")));
    let mut b = name(1);
    b.assertion.proof = Some(Proof::Proven);
    assert_eq!(only(&one(&a, &b)), ("proof", None, Some("proven")));
    let mut b = name(1);
    b.assertion.confidence = Some(3);
    assert_eq!(only(&one(&a, &b)), ("confidence", None, Some("3")));
    let mut b = name(1);
    b.private = true;
    assert_eq!(only(&one(&a, &b)), ("private", Some("no"), Some("yes")));
    let mut b = name(1);
    b.date = Some(GenDate::parse("about 1850", gt_core::DayOrder::Dmy));
    assert_eq!(only(&one(&a, &b)), ("date", None, Some("about 1850")));
    let mut b = name(1);
    b.citations = vec![Uuid::from_u128(9), Uuid::from_u128(10)];
    assert_eq!(only(&one(&a, &b)), ("citations", None, Some("2")));
    let mut b = name(1);
    b.notes = vec![Uuid::from_u128(9)];
    assert_eq!(only(&one(&a, &b)), ("notes", None, Some("1")));
    let mut b = name(1);
    b.unknown = vec![RawNode {
        tag: "_X".into(),
        xref: None,
        value: None,
        pos: 0,
        children: vec![],
    }];
    assert_eq!(only(&one(&a, &b)), ("unknown", None, Some("1")));
}

#[test]
fn two_fields_at_once_and_nothing_changed() {
    let a = name(1);
    let mut b = name(1);
    b.given = Some("Olena".into());
    b.surnames = vec!["Kovalenko".into()];
    let d = one(&a, &b);
    let got: Vec<_> = d
        .iter()
        .map(|f| (f.field.as_str(), f.before.as_deref(), f.after.as_deref()))
        .collect();
    assert_eq!(
        got,
        vec![
            ("given", Some("Ivan"), Some("Olena")),
            ("surnames", Some("Koval"), Some("Kovalenko"))
        ]
    );
    assert!(
        d.iter().all(|f| f.name_number.is_none()),
        "one name: no name label"
    );
    assert!(one(&a, &a).is_empty());
}

#[test]
fn sex_changes_are_reported() {
    let mut b = person(vec![name(1)]);
    b.sex = Sex::Female;
    let d = person_field_changes(&person(vec![name(1)]), &b);
    assert_eq!(only(&d), ("sex", Some("male"), Some("female")));
    b.sex = Sex::Other("fluid".into());
    let d = person_field_changes(&person(vec![name(1)]), &b);
    assert_eq!(only(&d), ("sex", Some("male"), Some("other:fluid")));
}

#[test]
fn added_and_removed_names_list_filled_fields_against_empty() {
    let mut second = name(2);
    second.kind = NameKind::Married;
    second.given = Some("Olena".into());
    second.surnames = vec!["Koval".into()];
    let before = person(vec![name(1)]);
    let after = person(vec![name(1), second.clone()]);
    let d = person_field_changes(&before, &after);
    assert!(
        d.iter()
            .all(|f| f.before.is_none() && f.name_number == Some(2))
    );
    assert!(d.iter().all(|f| f.name_kind.as_deref() == Some("married")));
    let fields: Vec<_> = d.iter().map(|f| f.field.as_str()).collect();
    assert_eq!(
        fields,
        ["kind", "preferred", "private", "given", "surnames"]
    );
    // reversed: removed
    let d = person_field_changes(&after, &before);
    assert!(
        d.iter()
            .all(|f| f.after.is_none() && f.name_number == Some(2))
    );
    assert_eq!(d[3].before.as_deref(), Some("Olena"));
}

#[test]
fn which_name_is_named_only_with_several_names() {
    let mut second = name(2);
    second.kind = NameKind::Married;
    let before = person(vec![name(1), second.clone()]);
    let mut changed = second;
    changed.surnames = vec!["Kovalchuk".into()];
    let after = person(vec![name(1), changed]);
    let d = person_field_changes(&before, &after);
    assert_eq!(d.len(), 1);
    assert_eq!(
        (d[0].name_number, d[0].name_kind.as_deref()),
        (Some(2), Some("married"))
    );
}

#[test]
fn moving_the_main_name_is_a_change_of_position_and_preferred() {
    let (a, b) = (name(1), name(2));
    let before = person(vec![a.clone(), b.clone()]);
    let after = person(vec![b.clone(), a.clone()]);
    let d = person_field_changes(&before, &after);
    let got: Vec<_> = d
        .iter()
        .map(|f| {
            (
                f.field.as_str(),
                f.name_number,
                f.before.as_deref(),
                f.after.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            ("position", Some(1), Some("2"), Some("1")),
            ("position", Some(2), Some("1"), Some("2")),
        ]
    );
    // preferred handed over
    let mut a2 = a.clone();
    a2.assertion.preferred = true;
    let mut b2 = b.clone();
    b2.assertion.preferred = true;
    let before = person(vec![a2, b.clone()]);
    let after = person(vec![a.clone(), b2]);
    let d = person_field_changes(&before, &after);
    let got: Vec<_> = d
        .iter()
        .map(|f| {
            (
                f.field.as_str(),
                f.name_number,
                f.before.as_deref(),
                f.after.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            ("preferred", Some(1), Some("yes"), Some("no")),
            ("preferred", Some(2), Some("no"), Some("yes")),
        ]
    );
}

// ---- through a real session

fn save_name(person: Uuid, id: Uuid, given: &str, surname: &str) -> EditCommand {
    EditCommand::SaveName {
        person,
        name: NameInput {
            id: Some(id),
            kind: NameKindDto::Birth,
            kind_text: None,
            preferred: true,
            private: false,
            prefix: None,
            given: Some(given.into()),
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

fn fields(v: &[FieldChangeDto]) -> Vec<(String, Option<String>, Option<String>)> {
    v.iter()
        .map(|f| (f.field.clone(), f.before.clone(), f.after.clone()))
        .collect()
}

fn surname_change(s: &str, from: &str, to: &str) -> (String, Option<String>, Option<String>) {
    let _ = s;
    ("surnames".into(), Some(from.into()), Some(to.into()))
}

#[test]
fn save_name_shows_from_to_in_history_and_log_also_after_undo_and_redo() {
    let tree = tree_of("0 @I1@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n");
    let (_dir, mut s) = open_tree(&tree);
    let ivan = find(s.tree(), "Ivan");
    let nid = s.tree().persons[&ivan].names[0].id;
    let pref = s.tree().persons[&ivan].names[0].assertion.preferred;
    let _ = pref;
    // make the stored name equal to what SaveName writes, so only the surname differs
    s.apply(save_name(ivan, nid, "Ivan", "Koval")).unwrap();
    s.apply(save_name(ivan, nid, "Ivan", "Kovalchuk")).unwrap();
    // The import text "Ivan /Koval/" goes with the changed surname, so the row shows that too.
    let raw = |from: Option<&str>, to: Option<&str>| {
        (
            "raw".to_string(),
            from.map(String::from),
            to.map(String::from),
        )
    };
    let want = vec![
        surname_change("", "Koval", "Kovalchuk"),
        raw(Some("Ivan /Koval/"), None),
    ];

    let latest = |s: &Session| s.history(ivan, 20).unwrap()[0].clone();
    let last_log = |s: &Session| {
        let page = s.log(LogFilterDto::default(), 0, 5).unwrap();
        let row = &page.items[0];
        let ch = row.changes.iter().find(|c| c.kind == "person").unwrap();
        (row.label.clone(), fields(&ch.fields))
    };

    assert_eq!(fields(&latest(&s).fields), want);
    assert_eq!(last_log(&s).1, want);

    // undo: back to Koval, so the undo row says Kovalchuk -> Koval
    s.undo().unwrap().unwrap();
    let back = vec![
        surname_change("", "Kovalchuk", "Koval"),
        raw(None, Some("Ivan /Koval/")),
    ];
    assert_eq!(fields(&latest(&s).fields), back);
    assert_eq!(last_log(&s).1, back);
    // the original row still says Koval -> Kovalchuk
    let h = s.history(ivan, 20).unwrap();
    assert_eq!(fields(&h[1].fields), want);

    // redo: Koval -> Kovalchuk again
    s.redo().unwrap().unwrap();
    assert_eq!(fields(&latest(&s).fields), want);
    assert_eq!(last_log(&s).1, want);
}

#[test]
fn create_row_has_no_fields() {
    let tree = tree_of("0 @I1@ INDI\n1 NAME Olena /Koval/\n1 SEX F\n");
    let (_dir, mut s) = open_tree(&tree);
    let olena = find(s.tree(), "Olena");
    s.apply(EditCommand::QuickAdd {
        anchor: olena,
        relation: Relation::Child,
        draft: draft("Ivan", SexKind::Male),
        family: None,
        new_parent_set: None,
        parent_rel: None,
    })
    .unwrap();
    let ivan = find(s.tree(), "Ivan");
    let h = s.history(ivan, 20).unwrap();
    assert!(h.iter().any(|r| r.op == HistoryOp::Create));
    assert!(h.iter().all(|r| r.fields.is_empty()), "{h:?}");
}
