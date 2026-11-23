#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Place records (schema 15): the migration from 14, adding in one transaction, the dedupe on
//! book + record id, the filters and the counts.

use gt_core::Uuid;
use gt_store::{PlaceRecordFilter, PlaceRecordInput, PlaceRecordPerson, Project, SCHEMA_VERSION};
use rusqlite::Connection;

fn place(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn rec(kind: &str, date: &str, rec_id: &str, surname: &str) -> PlaceRecordInput {
    PlaceRecordInput {
        kind: kind.into(),
        date: date.into(),
        book: "TEST-1".into(),
        page: "12".into(),
        rec_id: rec_id.into(),
        people: vec![PlaceRecordPerson {
            role: "child".into(),
            given: "Ivan".into(),
            surname: surname.into(),
            as_written: format!("Ivan {surname}"),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn filter(p: Uuid) -> PlaceRecordFilter {
    PlaceRecordFilter {
        places: vec![p],
        ..Default::default()
    }
}

#[test]
fn schema_14_project_gains_the_place_record_tables() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("Old.genetree");
    drop(Project::create(&dir).unwrap());
    {
        let c = Connection::open(dir.join("tree.db")).unwrap();
        c.execute_batch(
            "DROP TABLE place_record_person; DROP TABLE place_record; PRAGMA user_version = 14;",
        )
        .unwrap();
    }
    let mut p = Project::open(&dir).unwrap();
    assert_eq!(p.schema_version(), SCHEMA_VERSION);
    assert_eq!(SCHEMA_VERSION, 15);
    let out = p
        .add_place_records(place(1), &[rec("birth", "1850", "a:1", "Koval")], None, 5)
        .unwrap();
    assert_eq!((out.added, out.duplicates), (1, 0));
}

#[test]
fn the_same_book_and_record_id_is_added_once() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = Project::create(&tmp.path().join("p")).unwrap();
    let recs = vec![
        rec("birth", "1850", "a:1", "Koval"),
        rec("death", "1851-03", "a:2", "Koval"),
    ];
    let first = p.add_place_records(place(1), &recs, None, 1).unwrap();
    assert_eq!((first.added, first.duplicates), (2, 0));
    let again = p.add_place_records(place(1), &recs, None, 2).unwrap();
    assert_eq!((again.added, again.duplicates), (0, 2));
    assert_eq!(p.place_record_keys(&filter(place(1))).unwrap().len(), 2);
    // No record id: nothing to dedupe on, both are kept.
    let loose = rec("other", "1860", "", "Shevchuk");
    let two = p
        .add_place_records(place(1), &[loose.clone(), loose], None, 3)
        .unwrap();
    assert_eq!(two.added, 2);
    assert_eq!(
        p.count_place_records_present("TEST-1", &["a:1", "a:9", ""])
            .unwrap(),
        1
    );
}

#[test]
fn filters_counts_and_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = Project::create(&tmp.path().join("p")).unwrap();
    let mut mine = rec("marriage", "3.5.1862", "a:3", "Melnyk");
    mine.people[0].person = Some(place(77));
    mine.unsure = true;
    p.add_place_records(
        place(1),
        &[
            rec("birth", "1850", "a:1", "Koval"),
            rec("birth", "1855", "a:4", "Bondar"),
            rec("death", "1851", "a:2", "Koval"),
            rec("census", "", "a:5", "Koval"),
            mine,
        ],
        None,
        1,
    )
    .unwrap();
    p.add_place_records(place(2), &[rec("birth", "1850", "b:1", "Koval")], None, 1)
        .unwrap();

    // Dated records in year order, the undated last; another place is not mixed in.
    let keys = p.place_record_keys(&filter(place(1))).unwrap();
    assert_eq!(keys.len(), 5);
    let years: Vec<_> = keys.iter().map(|k| k.1).collect();
    assert_eq!(
        years,
        vec![Some(1850), Some(1851), Some(1855), Some(1862), None]
    );

    let mut f = filter(place(1));
    f.kinds = vec!["birth".into()];
    assert_eq!(p.place_record_keys(&f).unwrap().len(), 2);
    f.kinds.clear();
    f.year_from = Some(1851);
    f.year_to = Some(1855);
    assert_eq!(p.place_record_keys(&f).unwrap().len(), 2);
    f.year_from = None;
    f.year_to = None;
    f.relatives = Some(true);
    let ours = p.place_record_keys(&f).unwrap();
    assert_eq!(ours.len(), 1);
    f.relatives = Some(false);
    assert_eq!(p.place_record_keys(&f).unwrap().len(), 4);
    f.relatives = None;
    f.query = gt_store::fold_search("bONd");
    assert_eq!(p.place_record_keys(&f).unwrap().len(), 1);
    // Cyrillic folds too.
    p.add_place_records(
        place(1),
        &[PlaceRecordInput {
            kind: "revision".into(),
            date: "1816".into(),
            book: "R".into(),
            rec_id: "r:1".into(),
            people: vec![PlaceRecordPerson {
                surname: "Мельник".into(),
                given: "Пилип".into(),
                ..Default::default()
            }],
            ..Default::default()
        }],
        None,
        1,
    )
    .unwrap();
    f.query = gt_store::fold_search("МЕЛЬН");
    assert_eq!(p.place_record_keys(&f).unwrap().len(), 1);

    // Counts ignore the kinds and years of the filter.
    let mut f = filter(place(1));
    f.kinds = vec!["death".into()];
    let c = p.place_record_counts(&f).unwrap();
    assert_eq!(c.total, 6);
    assert!(c.by_kind.contains(&("birth".to_string(), 2)));
    assert!(c.by_decade.contains(&(1850, 3)));
    assert!(c.by_decade.contains(&(1860, 1)));
    assert!(c.by_decade.contains(&(1810, 1)));

    let rows = p.place_record_rows(&[ours[0].0]).unwrap();
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!(
        (r.kind.as_str(), r.year, r.unsure),
        ("marriage", Some(1862), true)
    );
    assert_eq!(r.people[0].person, Some(place(77)));
    // The link can be taken off.
    assert!(p.link_place_record_person(r.people[0].id, None).unwrap());
    f.relatives = Some(true);
    assert!(p.place_record_keys(&f).unwrap().is_empty());
}

#[test]
fn a_big_import_is_one_fast_transaction() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = Project::create(&tmp.path().join("p")).unwrap();
    let recs: Vec<_> = (0..20_000)
        .map(|i| {
            rec(
                "birth",
                &format!("{}", 1800 + i % 100),
                &format!("a:{i}"),
                "Koval",
            )
        })
        .collect();
    let t = std::time::Instant::now();
    let out = p.add_place_records(place(1), &recs, None, 1).unwrap();
    assert_eq!(out.added, 20_000);
    assert!(t.elapsed().as_secs() < 20, "{:?}", t.elapsed());
    assert_eq!(
        p.place_record_keys(&filter(place(1))).unwrap().len(),
        20_000
    );
}
