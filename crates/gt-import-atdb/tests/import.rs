#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use gt_core::date::DateValue;
use gt_core::model::*;
use gt_core::tree::Tree;
use gt_gedcom::ImportOptions;
use gt_import_atdb::{fixture, import_atdb, is_atdb};
use std::path::{Path, PathBuf};
use uuid::Uuid;

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

// ---------------------------------------------------------------- fix round 1

/// The fixture plus extra rows, imported.
fn import_with(extra_sql: &str) -> gt_gedcom::ImportOutcome {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.atdb");
    let conn = fixture::build_family_fixture(&path);
    conn.execute_batch(extra_sql).unwrap();
    drop(conn);
    import_atdb(&path, &ImportOptions::default()).unwrap()
}

const FAMILY_CASES: &str = "
INSERT INTO Persons (id, sex) VALUES (6,1),(7,2),(8,1),(9,1),(10,2);
INSERT INTO ValuesStr (f_id, rec_table, rec_id, vstr) VALUES
 (66,13,6,'Yuri'),(66,13,7,'Zoya'),(66,13,8,'Kid'),(66,13,9,'TwinA'),(66,13,10,'TwinB');
INSERT INTO Events (id, et_id) VALUES (10,3),(11,1),(12,1),(13,3);
INSERT INTO EventDetails (p_id, e_id, er_id, e_ord, p_ord) VALUES (6,10,5,1,1),(7,10,6,1,2);
INSERT INTO EventDetails (p_id, e_id, er_id, e_ord, p_ord) VALUES (8,11,1,1,1),(6,11,2,1,2);
INSERT INTO EventDetails (p_id, e_id, er_id, e_ord, p_ord) VALUES (9,12,1,1,1),(10,12,1,1,2),(6,12,2,1,3),(7,12,3,1,4);
INSERT INTO EventDetails (p_id, e_id, er_id, e_ord, p_ord) VALUES (7,13,5,1,1),(6,13,6,1,2);
";

#[test]
fn family_edge_cases_single_parent_twins_swapped_spouses() {
    let t = import_with(FAMILY_CASES).tree;
    let yuri = person_named(&t, "Yuri", None).common.id;
    let zoya = person_named(&t, "Zoya", None).common.id;
    let kid = person_named(&t, "Kid", None).common.id;
    let twin_a = person_named(&t, "TwinA", None).common.id;
    let twin_b = person_named(&t, "TwinB", None).common.id;

    // Yuri and Zoya are in 3 events (marriage, twin birth, marriage with swapped roles): one family.
    let couple: Vec<&Family> = t
        .families
        .values()
        .filter(|f| {
            [f.partner1, f.partner2].contains(&Some(yuri))
                && [f.partner1, f.partner2].contains(&Some(zoya))
        })
        .collect();
    assert_eq!(couple.len(), 1);
    let f = couple[0];
    assert_eq!(
        (f.partner1, f.partner2),
        (Some(yuri), Some(zoya)),
        "male first"
    );
    assert_eq!(f.kind, UnionKind::Married);
    let kids: Vec<Uuid> = f.children.iter().map(|c| c.person).collect();
    assert_eq!(
        kids,
        vec![twin_a, twin_b],
        "both twins linked, nothing else"
    );
    assert!(f.children.iter().all(|c| c.preferred));

    // Both marriage events belong to that one family.
    let on_couple = t
        .events
        .values()
        .filter(|e| e.kind == EventKind::Marriage)
        .filter(|e| {
            e.participants
                .iter()
                .any(|p| p.target == ParticipantRef::Family(f.common.id))
        })
        .count();
    assert_eq!(on_couple, 2);

    // Single-parent birth: its own family with one partner and the child.
    let single = t
        .families
        .values()
        .find(|f| f.children.iter().any(|c| c.person == kid))
        .unwrap();
    assert_eq!((single.partner1, single.partner2), (Some(yuri), None));
    assert_ne!(single.common.id, f.common.id);
    assert_eq!(single.children.len(), 1);
    // 2 from the fixture, the couple, the single parent.
    assert_eq!(t.families.len(), 4);
}

#[test]
fn marriage_without_children_makes_a_childless_married_family() {
    let t = import_with(
        "INSERT INTO Persons (id, sex) VALUES (6,1),(7,2);
         INSERT INTO ValuesStr (f_id, rec_table, rec_id, vstr) VALUES (66,13,6,'Yuri'),(66,13,7,'Zoya');
         INSERT INTO Events (id, et_id) VALUES (10,3);
         INSERT INTO EventDetails (p_id, e_id, er_id) VALUES (6,10,5),(7,10,6);",
    )
    .tree;
    let yuri = person_named(&t, "Yuri", None).common.id;
    let f = t
        .families
        .values()
        .find(|f| f.partner1 == Some(yuri))
        .unwrap();
    assert!(f.children.is_empty());
    assert_eq!(f.kind, UnionKind::Married);
}

#[test]
fn range_endpoints_are_the_two_years() {
    let t = import_fixture().tree;
    let marriage = t
        .events
        .values()
        .find(|e| {
            e.kind == EventKind::Marriage && e.date.as_ref().is_some_and(|d| d.raw.contains("1924"))
        })
        .unwrap();
    match &marriage.date.as_ref().unwrap().value {
        DateValue::Between(a, b) => {
            assert_eq!((a.year, a.month, a.day), (1924, None, None));
            assert_eq!((b.year, b.month, b.day), (1925, None, None));
        }
        other => panic!("not a range: {other:?}"),
    }
}

#[test]
fn unknown_roles_and_extra_parents_are_kept_and_counted() {
    let out = import_with(
        "INSERT INTO Persons (id, sex) VALUES (6,1),(7,2),(11,1),(12,1);
         INSERT INTO ValuesStr (f_id, rec_table, rec_id, vstr) VALUES
          (66,13,6,'Yuri'),(66,13,7,'Zoya'),(66,13,11,'Witness'),(66,13,12,'Second');
         INSERT INTO Events (id, et_id) VALUES (10,1),(11,2),(12,3);
         INSERT INTO EventDetails (p_id, e_id, er_id) VALUES (5,10,1),(3,10,2),(4,10,3),(11,10,9),(12,10,2);
         INSERT INTO EventDetails (p_id, e_id, er_id) VALUES (7,11,4),(11,11,8);
         INSERT INTO EventDetails (p_id, e_id, er_id) VALUES (6,12,5),(7,12,6),(11,12,10);",
    );
    let (t, r) = (&out.tree, &out.report);
    let witness = person_named(t, "Witness", None);
    let roles: Vec<Role> = t
        .events_of(witness.common.id)
        .map(|(_, r)| r.clone())
        .collect();
    assert_eq!(roles.len(), 3, "kept on birth, death and marriage");
    for n in ["role 9", "role 8", "role 10"] {
        assert!(roles.contains(&Role::Other(n.into())), "{n}");
    }
    let second = person_named(t, "Second", None);
    let roles: Vec<Role> = t
        .events_of(second.common.id)
        .map(|(_, r)| r.clone())
        .collect();
    assert_eq!(roles, vec![Role::Other("role 2".into())]);
    for k in [
        "role 9 on event type 1 (kept as a participant)",
        "role 2 on event type 1 (kept as a participant)",
        "role 8 on event type 2 (kept as a participant)",
        "role 10 on event type 3 (kept as a participant)",
    ] {
        assert_eq!(r.unmapped_tags.get(k), Some(&1), "{k}");
    }
}

#[test]
fn string_fields_of_events_and_documents_are_kept_and_counted_without_values() {
    let out = import_with(
        "INSERT INTO ValuesStr (f_id, rec_table, rec_id, vstr) VALUES
          (31,7,1,'event note text'),(32,4,1,'caption text'),(33,9,1,'family text');
         INSERT INTO Documents (id, path) VALUES (1, 'missing.jpg');",
    );
    let r = &out.report;
    assert_eq!(r.unmapped_tags.get("event field 31"), Some(&1));
    assert_eq!(r.unmapped_tags.get("document field 32"), Some(&1));
    assert_eq!(r.unmapped_tags.get("family field 33"), Some(&1));
    assert!(
        !r.to_text().contains("note text"),
        "values are not in the report"
    );
    let ivan = person_named(&out.tree, "Ivan", None);
    let birth = event_of(&out.tree, ivan, &EventKind::Birth);
    assert!(
        birth
            .common
            .unknown
            .iter()
            .any(|n| n.tag == "_ATDB_FIELD_31")
    );
    let media = out.tree.media.values().next().unwrap();
    assert!(
        media
            .common
            .unknown
            .iter()
            .any(|n| n.tag == "_ATDB_FIELD_32")
    );
}

#[test]
fn place_parent_loops_and_missing_parents_are_cut_and_counted() {
    let out = import_with(
        "INSERT INTO Places (id, parent_id) VALUES (3, 4), (4, 3), (5, 99);
         INSERT INTO ValuesStr (f_id, rec_table, rec_id, vstr) VALUES (93,14,3,'A'),(93,14,4,'B'),(93,14,5,'C');",
    );
    let r = &out.report;
    assert_eq!(
        r.unmapped_tags.get("place parent loop (parent dropped)"),
        Some(&2)
    );
    assert_eq!(
        r.unmapped_tags.get("place whose parent is not in the file"),
        Some(&1)
    );
    for p in out.tree.places.values() {
        let mut cur = p.parent;
        let mut steps = 0;
        while let Some(id) = cur {
            steps += 1;
            assert!(steps < 10, "parent chain must end");
            cur = out.tree.places[&id].parent;
        }
    }
}

#[test]
fn media_links_point_at_real_media_and_resolved_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.atdb");
    let conn = fixture::build_family_fixture(&path);
    std::fs::write(dir.path().join("photo.jpg"), b"x").unwrap();
    conn.execute_batch(
        "INSERT INTO Documents (id, path) VALUES (1, 'photo.jpg'), (2, 'missing.jpg');
         INSERT INTO DocumentDetails (d_id, rec_table, rec_id, ismain) VALUES (1, 13, 1, 1), (2, 7, 1, 0);",
    )
    .unwrap();
    drop(conn);
    let out = import_atdb(&path, &ImportOptions::default()).unwrap();
    let linked: Vec<Uuid> = out
        .tree
        .persons
        .values()
        .map(|p| &p.common)
        .chain(out.tree.events.values().map(|e| &e.common))
        .flat_map(|c| c.media.iter().map(|l| l.media))
        .collect();
    assert_eq!(linked.len(), 2);
    assert!(linked.iter().all(|id| out.tree.media.contains_key(id)));
    assert_eq!(out.resolved_media.len(), 1);
    let (id, file) = &out.resolved_media[0];
    assert!(linked.contains(id));
    assert!(out.tree.media.contains_key(id));
    assert!(file.is_file());
}

mod errors {
    use super::*;
    use gt_import_atdb::AtdbError;

    #[test]
    fn foreign_sqlite_is_not_atdb() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.sqlite");
        fixture::build_family_fixture(&p)
            .execute_batch("DROP TABLE Events")
            .unwrap();
        let r = import_atdb(&p, &ImportOptions::default());
        assert!(matches!(r, Err(AtdbError::NotAtdb(_))), "{r:?}");
    }

    #[test]
    fn empty_and_corrupt_files_are_not_atdb_and_stay_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty.atdb");
        std::fs::write(&empty, b"").unwrap();
        let junk = dir.path().join("junk.atdb");
        std::fs::write(&junk, b"SQLite format 3\0 then garbage garbage garbage").unwrap();
        for p in [&empty, &junk] {
            let before = std::fs::read(p).unwrap();
            let r = import_atdb(p, &ImportOptions::default());
            assert!(matches!(r, Err(AtdbError::NotAtdb(_))), "{p:?}: {r:?}");
            assert_eq!(std::fs::read(p).unwrap(), before);
        }
    }

    #[test]
    fn missing_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let r = import_atdb(&dir.path().join("nope.atdb"), &ImportOptions::default());
        assert!(
            matches!(r, Err(AtdbError::Open { .. } | AtdbError::NotAtdb(_))),
            "{r:?}"
        );
    }

    #[test]
    fn wrong_column_type_names_the_table() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.atdb");
        let conn = fixture::build_family_fixture(&p);
        conn.execute_batch(
            "INSERT INTO ValuesStr (f_id, rec_table, rec_id, vstr) VALUES (66, 13, 'abc', 'x')",
        )
        .unwrap();
        drop(conn);
        match import_atdb(&p, &ImportOptions::default()) {
            Err(AtdbError::Table(t, _)) => assert_eq!(t, "ValuesStr"),
            other => panic!("{other:?}"),
        }
    }
}
