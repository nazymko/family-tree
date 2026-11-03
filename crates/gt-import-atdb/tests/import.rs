#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use gt_core::date::DateValue;
use gt_core::model::*;
use gt_core::tree::Tree;
use gt_gedcom::ImportOptions;
use gt_import_atdb::{fixture, import_atdb, is_atdb};
use std::path::{Path, PathBuf};

fn person_named<'a>(t: &'a Tree, given: &str, lang: Option<&str>) -> &'a Person {
    t.persons
        .values()
        .find(|p| {
            p.names
                .iter()
                .any(|n| n.given.as_deref() == Some(given) && n.lang.as_deref() == lang)
        })
        .unwrap_or_else(|| panic!("no person {given}"))
}

fn event_of<'a>(t: &'a Tree, p: &Person, kind: &EventKind) -> &'a Event {
    t.events_of(p.common.id)
        .map(|(e, _)| e)
        .find(|e| &e.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?} event"))
}

fn fixture_path(dir: &Path) -> PathBuf {
    let p = dir.join("family tree #1.atdb");
    drop(fixture::build_family_fixture(&p));
    p
}

fn import_fixture() -> gt_gedcom::ImportOutcome {
    let dir = tempfile::tempdir().unwrap();
    import_atdb(&fixture_path(dir.path()), &ImportOptions::default()).unwrap()
}

#[test]
fn maps_names_with_patronymic_and_maiden_name() {
    let t = import_fixture().tree;
    assert_eq!(t.persons.len(), 5);

    let ivan = person_named(&t, "Ivan", None);
    assert_eq!(ivan.sex, Sex::Male);
    assert_eq!(ivan.names.len(), 1);
    let n = &ivan.names[0];
    assert_eq!(n.patronymic.as_deref(), Some("Petrovych"));
    assert_eq!(n.surnames, vec!["Kovalenko".to_string()]);
    assert!(n.assertion.preferred);

    let maria = person_named(&t, "Maria", None);
    assert_eq!(maria.sex, Sex::Female);
    assert_eq!(maria.names.len(), 2);
    assert_eq!(maria.names[0].kind, NameKind::Married);
    assert_eq!(maria.names[0].surnames, vec!["Kovalenko".to_string()]);
    assert!(maria.names[0].assertion.preferred);
    assert_eq!(maria.names[1].kind, NameKind::Birth);
    assert_eq!(maria.names[1].surnames, vec!["Shevchenko".to_string()]);
    assert_eq!(maria.names[1].patronymic.as_deref(), Some("Hryhorivna"));
}

#[test]
fn keeps_a_language_variant_as_a_second_name() {
    let t = import_fixture().tree;
    // "uk" is the main language: its name has no lang tag; the "en" one is a variant.
    let taras = person_named(&t, "Taras", None);
    assert_eq!(taras.names.len(), 2);
    assert_eq!(taras.names[0].lang, None);
    assert_eq!(taras.names[1].lang.as_deref(), Some("en"));
    assert_eq!(taras.names[1].kind, NameKind::Aka);
    assert!(!taras.names[1].assertion.preferred);
}

#[test]
fn builds_families_from_birth_and_marriage_events() {
    let t = import_fixture().tree;
    // Ivan+Maria and Petro+Olha: each pair is both "parents" and "married"; they merge.
    assert_eq!(t.families.len(), 2);
    let ivan = person_named(&t, "Ivan", None).common.id;
    let maria = person_named(&t, "Maria", None).common.id;
    let petro = person_named(&t, "Petro", None).common.id;
    let olha = person_named(&t, "Olha", None).common.id;
    let taras = person_named(&t, "Taras", None).common.id;

    let f1 = t
        .families
        .values()
        .find(|f| f.partner1 == Some(ivan))
        .unwrap();
    assert_eq!(f1.partner2, Some(maria));
    assert_eq!(f1.kind, UnionKind::Married);
    assert_eq!(f1.children.len(), 1);
    assert_eq!(f1.children[0].person, petro);
    assert!(f1.children[0].preferred);

    let f2 = t
        .families
        .values()
        .find(|f| f.partner1 == Some(petro))
        .unwrap();
    assert_eq!(f2.partner2, Some(olha));
    assert_eq!(f2.children[0].person, taras);

    // The marriage event belongs to the family.
    let marriage = t
        .events
        .values()
        .find(|e| {
            e.kind == EventKind::Marriage && e.date.as_ref().is_some_and(|d| d.raw.contains("1924"))
        })
        .unwrap();
    assert_eq!(
        marriage.participants,
        vec![Participant {
            target: ParticipantRef::Family(f1.common.id),
            role: Role::Principal
        }]
    );
    assert_eq!(t.families_as_child(olha).count(), 0);
    assert_eq!(t.families_as_child(petro).count(), 1);
}

#[test]
fn maps_dates_partial_range_and_place() {
    let t = import_fixture().tree;
    let ivan = person_named(&t, "Ivan", None);
    let d = event_of(&t, ivan, &EventKind::Birth).date.clone().unwrap();
    assert!(matches!(&d.value, DateValue::Exact(x) if x.year == 1900 && x.month.is_none()));
    assert_eq!(d.raw, "1900");

    let d = event_of(&t, ivan, &EventKind::Death).date.clone().unwrap();
    assert!(
        matches!(&d.value, DateValue::Exact(x) if (x.year, x.month, x.day) == (1970, Some(11), Some(5)))
    );

    let marriage = t
        .events
        .values()
        .find(|e| e.kind == EventKind::Marriage)
        .unwrap();
    assert!(matches!(
        marriage.date.as_ref().unwrap().value,
        DateValue::Between(..)
    ));

    let taras = person_named(&t, "Taras", None);
    let d = event_of(&t, taras, &EventKind::Birth).date.clone().unwrap();
    assert!(
        matches!(&d.value, DateValue::Exact(x) if (x.year, x.month, x.day) == (1952, Some(6), None))
    );

    let petro = person_named(&t, "Petro", None);
    let birth = event_of(&t, petro, &EventKind::Birth);
    let place = &t.places[&birth.place.unwrap()];
    assert_eq!(place.names[0].name, "Village");
    assert_eq!(
        t.places[&place.parent.unwrap()].names[0].name,
        "Kyiv Oblast"
    );
    assert_eq!(place.raw.as_deref(), Some("Village, Kyiv Oblast"));
}

#[test]
fn reports_what_it_does_not_understand() {
    let out = import_fixture();
    let (t, r) = (&out.tree, &out.report);
    // Unknown event type: a custom event with the person as principal.
    let taras = person_named(t, "Taras", None);
    let custom = event_of(t, taras, &EventKind::Custom("Event type 7".into()));
    assert_eq!(custom.participants.len(), 1);
    assert_eq!(
        r.unmapped_tags.get("event type 7 (kept as a custom event)"),
        Some(&1)
    );
    // Unknown person field: counted and kept on the person.
    assert_eq!(r.unmapped_tags.get("person field 69"), Some(&1));
    let raw = &taras.common.unknown;
    assert_eq!(raw.len(), 1);
    assert_eq!(raw[0].tag, "_ATDB_FIELD_69");
    assert_eq!(raw[0].value.as_deref(), Some("a field nobody maps"));
    // Unknown date qualifier.
    assert_eq!(
        r.unmapped_tags
            .get("date qualifier 2 (read as an exact date)"),
        Some(&1)
    );
    // Counts and producer.
    assert_eq!(r.counts.get("persons"), Some(&5));
    assert_eq!(r.counts.get("families"), Some(&2));
    assert_eq!(r.counts.get("places"), Some(&2));
    assert!(r.producer.as_deref().unwrap().contains("version 8"));
    assert_eq!(r.version, None);
}

#[test]
fn documents_become_media_and_resolve_next_to_the_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.atdb");
    let conn = fixture::build_family_fixture(&path);
    std::fs::write(dir.path().join("photo.jpg"), b"x").unwrap();
    conn.execute_batch(
        "INSERT INTO Documents (id, path) VALUES (1, 'photo.jpg'), (2, 'missing.jpg');
         INSERT INTO DocumentDetails (d_id, rec_table, rec_id, ismain) VALUES (1, 13, 1, 1), (2, 13, 1, 0);",
    )
    .unwrap();
    drop(conn);
    let out = import_atdb(&path, &ImportOptions::default()).unwrap();
    assert_eq!(out.tree.media.len(), 2);
    assert_eq!(out.resolved_media.len(), 1);
    assert!(out.resolved_media[0].1.ends_with("photo.jpg"));
    assert_eq!(out.report.missing_media.len(), 1);
    let ivan = person_named(&out.tree, "Ivan", None);
    assert_eq!(ivan.common.media.len(), 2);
    assert!(ivan.common.media[0].primary);
}

#[test]
fn sources_become_citations() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.atdb");
    let conn = fixture::build_family_fixture(&path);
    conn.execute_batch(
        "INSERT INTO Sources (id, stype) VALUES (1, 0);
         INSERT INTO ValuesStr (f_id, rec_table, rec_id, vstr) VALUES (109,16,1,'Parish book'), (110,16,1,'page 3');
         INSERT INTO SourceDetails (s_id, rec_table, rec_id) VALUES (1, 13, 1);",
    )
    .unwrap();
    drop(conn);
    let out = import_atdb(&path, &ImportOptions::default()).unwrap();
    assert_eq!(out.tree.sources.len(), 1);
    let s = out.tree.sources.values().next().unwrap();
    assert_eq!(s.title.as_deref(), Some("Parish book"));
    assert_eq!(s.text.as_deref(), Some("field 110: page 3"));
    let ivan = person_named(&out.tree, "Ivan", None);
    assert_eq!(ivan.common.citations.len(), 1);
}

#[test]
fn never_writes_to_the_file_and_detects_atdb() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixture_path(dir.path());
    let before = std::fs::read(&path).unwrap();
    assert!(is_atdb(&path));
    import_atdb(&path, &ImportOptions::default()).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let names: Vec<String> = dir
        .path()
        .read_dir()
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        vec!["family tree #1.atdb".to_string()],
        "no journal or other file"
    );
}

#[test]
fn other_files_are_not_atdb() {
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("a.sqlite");
    rusqlite_free_sqlite(&plain);
    assert!(!is_atdb(&plain));
    assert!(import_atdb(&plain, &ImportOptions::default()).is_err());
    let text = dir.path().join("a.ged");
    std::fs::write(&text, "0 HEAD\n").unwrap();
    assert!(!is_atdb(&text));
    assert!(!is_atdb(&dir.path().join("nope.atdb")));
}

/// A SQLite file with one unrelated table, made through the fixture schema's engine.
fn rusqlite_free_sqlite(path: &Path) {
    let conn = fixture::build_family_fixture(path);
    conn.execute_batch("DROP TABLE EventDetails").unwrap();
}
