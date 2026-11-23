#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Events of a place: the tree's events and the place records in one list, the filters, the
//! TSV index import (grouping by rec_id, books and villages, dedupe). Invented names only.

use gt_core::date::calendar::{Calendar, first_jdn};
use gt_gedcom::{ImportOptions, import};
use gt_session::*;
use gt_store::{PlaceRecordInput, PlaceRecordPerson, Project};
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;

const NOW_MS: i64 = 1_793_664_000_000;

fn open_ged(ged: &str) -> (TempDir, Session) {
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

const GED: &str = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n\
0 @I1@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n1 FAMS @F1@\n1 BIRT\n2 DATE 1870\n2 PLAC Dubky, Zelena district\n\
1 DEAT\n2 DATE 12 MAR 1940\n2 PLAC Dubky, Zelena district\n\
0 @I2@ INDI\n1 NAME Maria /Koval/\n1 SEX F\n1 FAMS @F1@\n1 BIRT\n2 DATE 1875\n2 PLAC Hlukhiv\n\
0 @I3@ INDI\n1 NAME Petro /Koval/\n1 SEX M\n1 FAMC @F1@\n1 BIRT\n2 DATE 1900\n2 PLAC Dubky, Zelena district\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n1 MARR\n2 DATE 1895\n2 PLAC Dubky, Zelena district\n\
0 TRLR\n";

fn place_named(s: &Session, name: &str) -> Uuid {
    s.suggest_places(name, 5)
        .into_iter()
        .find(|p| p.name == name)
        .unwrap()
        .id
}

fn person_named(s: &Session, given: &str) -> Uuid {
    *s.tree()
        .persons
        .iter()
        .find(|(_, p)| {
            p.names
                .first()
                .and_then(|n| n.given.as_deref())
                .is_some_and(|g| g.starts_with(given))
        })
        .unwrap()
        .0
}

fn rec(kind: &str, date: &str, rec_id: &str, surname: &str) -> PlaceRecordInput {
    PlaceRecordInput {
        kind: kind.into(),
        date: date.into(),
        book: "BK-1".into(),
        page: "7".into(),
        rec_id: rec_id.into(),
        people: vec![PlaceRecordPerson {
            role: "child".into(),
            given: "Danylo".into(),
            surname: surname.into(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn q(place: Uuid) -> PlaceEventsQuery {
    PlaceEventsQuery {
        place,
        ..Default::default()
    }
}

#[test]
fn tree_events_and_records_share_one_chronological_list() {
    let (_d, mut s) = open_ged(GED);
    let dubky = place_named(&s, "Dubky");
    s.add_place_records(
        dubky,
        &[
            rec("birth", "1880", "a:1", "Bondar"),
            rec("death", "1865", "a:2", "Bondar"),
            rec("census", "", "a:3", "Melnyk"),
        ],
        None,
    )
    .unwrap();

    let page = s.place_events(&q(dubky)).unwrap();
    // Tree: Ivan's birth, Ivan's death, Petro's birth, the marriage. Records: three.
    assert_eq!(page.total, 7);
    let years: Vec<Option<i32>> = page.items.iter().map(|i| i.year).collect();
    assert_eq!(
        years,
        vec![
            Some(1865),
            Some(1870),
            Some(1880),
            Some(1895),
            Some(1900),
            Some(1940),
            None
        ]
    );
    let marriage = page.items.iter().find(|i| i.kind == "marriage").unwrap();
    assert!(marriage.ours && marriage.source == "tree");
    assert_eq!(marriage.people.len(), 2);
    assert!(marriage.people.iter().all(|p| p.person.is_some()));
    let other = page.items.iter().find(|i| i.rec_id == "a:2").unwrap();
    assert!(!other.ours);
    assert_eq!(other.book, "BK-1");
    assert_eq!(other.people[0].name, "Danylo Bondar");

    // Filters.
    let mut f = q(dubky);
    f.who = Some(PlaceEventsWho::Others);
    assert_eq!(s.place_events(&f).unwrap().total, 3);
    f.who = Some(PlaceEventsWho::Ours);
    assert_eq!(s.place_events(&f).unwrap().total, 4);
    f.who = None;
    f.kinds = Some(vec!["birth".into()]);
    assert_eq!(s.place_events(&f).unwrap().total, 3);
    f.kinds = None;
    f.year_from = Some(1890);
    f.year_to = Some(1900);
    assert_eq!(s.place_events(&f).unwrap().total, 2);
    f.year_from = None;
    f.year_to = None;
    f.query = Some("melnyk".into());
    assert_eq!(s.place_events(&f).unwrap().total, 1);
    f.query = Some("petro".into());
    assert_eq!(s.place_events(&f).unwrap().total, 1);

    // Paging keeps the order.
    let mut f = q(dubky);
    f.limit = Some(3);
    f.offset = Some(2);
    let p = s.place_events(&f).unwrap();
    assert_eq!(p.total, 7);
    assert_eq!(p.items.len(), 3);
    assert_eq!(p.items[0].year, Some(1880));

    // The overview ignores the kinds and years asked for, and counts both sources.
    let mut f = q(dubky);
    f.kinds = Some(vec!["death".into()]);
    let p = s.place_events(&f).unwrap();
    assert_eq!(p.overview_total, 7);
    let birth = p.by_kind.iter().find(|k| k.kind == "birth").unwrap();
    assert_eq!(birth.count, 3);
    assert!(p.by_decade.iter().any(|d| d.decade == 1870 && d.count == 1));
}

#[test]
fn sub_places_are_included_and_a_linked_record_is_ours() {
    let (_d, mut s) = open_ged(GED);
    let dubky = place_named(&s, "Dubky");
    let district = place_named(&s, "Zelena district");
    let ivan = person_named(&s, "Ivan");
    let mut r = rec("marriage", "1894", "a:9", "Koval");
    r.people[0].person = Some(ivan);
    s.add_place_records(dubky, &[r, rec("birth", "1801", "a:10", "Hrytsenko")], None)
        .unwrap();
    // The district holds Dubky's events, and says which village each one is in.
    let page = s.place_events(&q(district)).unwrap();
    assert_eq!(page.total, 6);
    assert!(page.items.iter().all(|i| i.place.as_deref() == Some("Dubky")));
    let mut f = q(district);
    f.who = Some(PlaceEventsWho::Ours);
    // Four tree events and the record that names Ivan.
    assert_eq!(s.place_events(&f).unwrap().total, 5);
    let linked = s
        .place_events(&f)
        .unwrap()
        .items
        .into_iter()
        .find(|i| i.source == "record")
        .unwrap();
    assert!(linked.ours);
    assert_eq!(linked.people[0].person, Some(ivan));
    assert_eq!(linked.people[0].name, "Ivan Koval");
    // A place that is not there.
    assert!(s.place_events(&q(Uuid::from_u128(5))).is_err());
}

#[test]
fn a_record_for_a_place_that_is_not_there_or_a_person_that_is_not_is_refused() {
    let (_d, mut s) = open_ged(GED);
    let dubky = place_named(&s, "Dubky");
    assert!(
        s.add_place_records(Uuid::from_u128(9), &[rec("birth", "1800", "x", "A")], None)
            .is_err()
    );
    let mut r = rec("birth", "1800", "x", "A");
    r.people[0].person = Some(Uuid::from_u128(9));
    assert!(s.add_place_records(dubky, &[r], None).is_err());
}

fn write_tsv(dir: &std::path::Path, name: &str, rows: &[&str]) -> PathBuf {
    let header = "book\tyear\tsection\trec_no\trec_id\tdate\tpage\trole\tgiven\tpatronymic\tsurname\tas_written\testate\tage\tnote\tunsure";
    let p = dir.join(name);
    let mut text = String::from(header);
    for r in rows {
        text.push_str("\r\n");
        text.push_str(r);
    }
    std::fs::write(&p, text).unwrap();
    p
}

#[test]
fn an_index_file_groups_rows_into_records_by_book_and_village() {
    let tmp = tempfile::tempdir().unwrap();
    let multi = write_tsv(
        tmp.path(),
        "multi.tsv",
        &[
            "MB-1\t1850\tbirth\t1\tMB-1:Dubky:1850:birth:1\t12.03.1850\t4\tchild\tDanylo\tPetrovych\tBondar\tДанило Бондар\tpeasant\t\t\t0",
            "MB-1\t1850\tbirth\t1\tMB-1:Dubky:1850:birth:1\t12.03.1850\t4\tfather\tPetro\tIvanovych\tBondar\t\t\t30\t\t0",
            "MB-1\t1851\tdeath\t2\tMB-1:Dubky:1851:death:2\t\t5\tdeceased\tHanna\t\tMelnyk\t\t\t60\tcause unclear\t1",
            "MB-1\t1850\tbirth\t1\tMB-1:Berezy:1850:birth:1\t1850\t2\tchild\tMarko\t\tSavchuk\t\t\t\t\t0",
        ],
    );
    let single = write_tsv(
        tmp.path(),
        "single.tsv",
        &[
            "SB-2\t1816\trevision\t3\t\t1816\t9\thead\tPylyp\tStepanovych\tBondar\t\t\t40\t\t0",
            "SB-2\t1816\trevision\t3\t\t1816\t9\twife\tOlena\t\tBondar\t\t\t38\t\t0",
        ],
    );
    let parsed = parse_index_files(&[multi, single]).unwrap();
    assert_eq!((parsed.files, parsed.rows, parsed.bad_rows), (2, 6, 0));
    let scan = parsed.scan();
    // Three records of the multi-village book (two villages), one of the single-village book.
    assert_eq!(scan.groups.len(), 3);
    let dubky = scan.groups.iter().find(|g| g.village == "Dubky").unwrap();
    assert_eq!((dubky.records, dubky.rows), (2, 3));
    assert_eq!((dubky.year_from, dubky.year_to), (Some(1850), Some(1851)));
    let single = scan.groups.iter().find(|g| g.book == "SB-2").unwrap();
    assert_eq!(single.village, "");
    assert_eq!((single.records, single.rows), (1, 2));
    assert_eq!(single.kinds[0].kind, "revision");

    // Mapping to places, preview, then import; again, and nothing is added twice.
    let (_d, mut s) = open_ged(GED);
    let dubky_place = place_named(&s, "Dubky");
    let hlukhiv = place_named(&s, "Hlukhiv");
    let map = vec![
        PlaceImportMapDto {
            book: "MB-1".into(),
            village: "Dubky".into(),
            place: Some(dubky_place),
        },
        PlaceImportMapDto {
            book: "SB-2".into(),
            village: String::new(),
            place: Some(hlukhiv),
        },
    ];
    let pre = s.place_import_preview(&parsed, &map).unwrap();
    assert_eq!((pre.total_new, pre.total_duplicates, pre.unmapped), (3, 0, 1));
    let (parts, unmapped) = parsed.split(&map);
    assert_eq!(unmapped, 1);
    let mut added = 0;
    for (_, place, recs) in &parts {
        added += s.add_place_records(*place, recs, None).unwrap().added;
    }
    assert_eq!(added, 3);
    let again = s.place_import_preview(&parsed, &map).unwrap();
    assert_eq!((again.total_new, again.total_duplicates), (0, 3));
    for (_, place, recs) in &parts {
        assert_eq!(s.add_place_records(*place, recs, None).unwrap().added, 0);
    }

    let page = s.place_events(&q(hlukhiv)).unwrap();
    let revision = page.items.iter().find(|i| i.kind == "revision").unwrap();
    assert_eq!(revision.people.len(), 2);
    assert_eq!(revision.people[0].age, "40");
    let d = s.place_events(&q(dubky_place)).unwrap();
    let death = d.items.iter().find(|i| i.kind == "death" && i.source == "record").unwrap();
    assert!(death.unsure);
    assert_eq!(death.year, Some(1851));
    let birth = d.items.iter().find(|i| i.rec_id == "MB-1:Dubky:1850:birth:1").unwrap();
    assert_eq!(birth.people.len(), 2);
    assert_eq!(birth.people[0].as_written, "Данило Бондар");
    assert_eq!(birth.page, "4");
}

#[test]
fn a_place_is_made_from_text() {
    let (_d, mut s) = open_ged(GED);
    let r = s
        .apply(EditCommand::CreatePlace {
            text: "Zarichne, Zelena district".into(),
        })
        .unwrap();
    let id = r.focus.unwrap();
    assert_eq!(s.place_option(id).unwrap().name, "Zarichne");
    // The same text finds it again.
    let again = s
        .apply(EditCommand::CreatePlace {
            text: "Zarichne, Zelena district".into(),
        })
        .unwrap();
    assert_eq!(again.focus, Some(id));
}
