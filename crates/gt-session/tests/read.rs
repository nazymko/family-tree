#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use gt_core::date::calendar::{Calendar, first_jdn};
use gt_gedcom::{ImportOptions, import};
use gt_session::*;
use gt_store::Project;
use std::fmt::Write as _;
use tempfile::TempDir;
use uuid::Uuid;

const NOW_MS: i64 = 1_793_664_000_000; // 2026-11-03

fn today() -> i32 {
    first_jdn(Calendar::Gregorian, 2026, Some(11), Some(3)).unwrap()
}

fn open_ged(ged: &str) -> (TempDir, Session) {
    let out = import(ged.as_bytes(), &ImportOptions::default());
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("project");
    let mut p = Project::create(&proj).unwrap();
    p.save_tree(&out.tree, today()).unwrap();
    drop(p);
    let s = Session::open(&proj, SessionClock::fixed(today(), NOW_MS)).unwrap();
    (dir, s)
}

const HEAD: &str = "0 HEAD\n1 SOUR TEST\n2 VERS 1.0\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n";

fn indi(n: u32, name: &str, extra: &str) -> String {
    format!("0 @I{n}@ INDI\n1 NAME {name}\n{extra}")
}

fn ged(records: &[String]) -> String {
    format!("{HEAD}{}0 TRLR\n", records.concat())
}

fn query() -> PersonQuery {
    PersonQuery::default()
}

fn names(page: &Page<PersonRow>) -> Vec<String> {
    page.items.iter().map(|r| r.person.name.clone()).collect()
}

/// Anna (b. 1850) and Karl (b. 1848, d. 1901), their children Hans (1875), Greta (1872) and
/// Otto (1880, listed first), and a source on Anna's birth.
fn family_ged() -> String {
    ged(&[
        "0 @S1@ SOUR\n1 TITL Hamburg parish register\n".to_string(),
        indi(
            1,
            "Anna /Schmidt/",
            "1 SEX F\n1 BIRT\n2 DATE 3 APR 1850\n2 PLAC Hamburg, Germany\n2 SOUR @S1@\n3 PAGE p. 12\n1 OCCU Seamstress\n",
        ),
        indi(
            2,
            "Karl /Schmidt/",
            "1 SEX M\n1 BIRT\n2 DATE 1848\n1 DEAT\n2 DATE 3 MAR 1901\n",
        ),
        indi(3, "Hans /Schmidt/", "1 SEX M\n1 BIRT\n2 DATE 1875\n"),
        indi(4, "Greta /Schmidt/", "1 SEX F\n1 BIRT\n2 DATE 1872\n"),
        indi(5, "Otto /Schmidt/", "1 SEX M\n1 BIRT\n2 DATE 1880\n"),
        "0 @F1@ FAM\n1 HUSB @I2@\n1 WIFE @I1@\n1 CHIL @I5@\n1 CHIL @I3@\n1 CHIL @I4@\n1 MARR\n2 DATE 1 JUN 1871\n2 PLAC Hamburg, Germany\n".to_string(),
    ])
}

fn person_id(s: &Session, name: &str) -> Uuid {
    s.list_persons(&query(), 0, 200)
        .items
        .iter()
        .find(|r| r.person.name == name)
        .unwrap_or_else(|| panic!("no person {name}"))
        .person
        .id
}

#[test]
fn open_info_counts() {
    let bytes = std::fs::read("../../tests/fixtures/gedcom/basic551.ged").unwrap();
    let out = import(&bytes, &ImportOptions::default());
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("basic");
    let mut p = Project::create(&proj).unwrap();
    p.save_tree(&out.tree, today()).unwrap();
    drop(p);
    let s = Session::open(&proj, SessionClock::fixed(today(), NOW_MS)).unwrap();
    let info = s.info();
    assert_eq!(info.person_count, 2);
    assert_eq!(info.home_person, None);
    assert_eq!(info.source_version.as_deref(), Some("2024.1"));
    assert_eq!(info.name, "basic");
    s.close();
}

#[test]
fn open_missing_folder_is_no_project() {
    let dir = tempfile::tempdir().unwrap();
    let err = Session::open(dir.path(), SessionClock::fixed(today(), NOW_MS))
        .err()
        .unwrap();
    assert_eq!(err.code, ErrorCode::NoProject);
}

#[test]
fn list_sorted_by_surname_icu() {
    let (_d, s) = open_ged(&ged(&[
        indi(1, "Zed /Zoe/", ""),
        indi(2, "Ann /Munro/", ""),
        indi(3, "Bob /Müller/", ""),
        indi(4, "Cy /Eva/", ""),
        indi(5, "Di /Mueller/", ""),
        indi(6, "Ed /Émile/", ""),
    ]));
    let page = s.list_persons(&query(), 0, 50);
    assert_eq!(
        names(&page),
        [
            "Ed Émile",
            "Cy Eva",
            "Di Mueller",
            "Bob Müller",
            "Ann Munro",
            "Zed Zoe"
        ]
    );
    let desc = s.list_persons(
        &PersonQuery {
            desc: true,
            ..query()
        },
        0,
        50,
    );
    assert_eq!(names(&desc).first().map(String::as_str), Some("Zed Zoe"));
}

#[test]
fn sort_by_given_birth_changed_keeps_undated_last() {
    let (_d, s) = open_ged(&ged(&[
        indi(1, "Bea /X/", "1 BIRT\n2 DATE 1900\n"),
        indi(2, "Al /X/", "1 BIRT\n2 DATE 1950\n"),
        indi(3, "Cy /X/", ""),
    ]));
    let by_given = s.list_persons(
        &PersonQuery {
            sort: SortKey::Given,
            ..query()
        },
        0,
        10,
    );
    assert_eq!(names(&by_given), ["Al X", "Bea X", "Cy X"]);
    for desc in [false, true] {
        let by_birth = s.list_persons(
            &PersonQuery {
                sort: SortKey::Birth,
                desc,
                ..query()
            },
            0,
            10,
        );
        assert_eq!(by_birth.items.last().unwrap().person.name, "Cy X");
    }
    let first = s.list_persons(
        &PersonQuery {
            sort: SortKey::Birth,
            ..query()
        },
        0,
        10,
    );
    assert_eq!(first.items[0].person.name, "Bea X");
}

#[test]
fn list_limit_clamped_to_200() {
    let recs: Vec<String> = (1..=250)
        .map(|i| indi(i, &format!("P{i} /Same/"), ""))
        .collect();
    let (_d, s) = open_ged(&ged(&recs));
    let page = s.list_persons(&query(), 0, 5000);
    assert_eq!(page.items.len(), 200);
    assert_eq!(page.total, 250);
    let rest = s.list_persons(&query(), 200, 5000);
    assert_eq!(rest.items.len(), 50);
}

#[test]
fn offset_beyond_total_returns_empty_items_with_total() {
    let (_d, s) = open_ged(&family_ged());
    let page = s.list_persons(&query(), 99, 10);
    assert!(page.items.is_empty());
    assert_eq!(page.total, 5);
    assert_eq!(page.offset, 99);
}

#[test]
fn filters_combine() {
    let (_d, s) = open_ged(&ged(&[
        "0 @S1@ SOUR\n1 TITL Register\n".to_string(),
        // living, no source
        indi(
            1,
            "Anna /Mueller/",
            "1 NAME Anna /Schmidt/\n2 TYPE married\n",
        ),
        // living, with a source
        indi(2, "Berta /Mueller/", "1 BIRT\n2 DATE 1990\n2 SOUR @S1@\n"),
        // deceased, no source
        indi(3, "Carl /Mueller/", "1 DEAT Y\n"),
        indi(4, "Dora /Vogel/", "1 NAME Dora /Müller/\n2 TYPE aka\n"),
    ]));
    let q = PersonQuery {
        living: LivingFilter::Living,
        no_source: true,
        ..query()
    };
    assert_eq!(
        names(&s.list_persons(&q, 0, 50)),
        ["Anna Mueller", "Dora Vogel"]
    );
    let q = PersonQuery {
        living: LivingFilter::Deceased,
        ..query()
    };
    assert_eq!(names(&s.list_persons(&q, 0, 50)), ["Carl Mueller"]);
    // Any name variant, ignoring diacritics and case.
    let q = PersonQuery {
        text: "SCHMIDT".into(),
        ..query()
    };
    assert_eq!(names(&s.list_persons(&q, 0, 50)), ["Anna Mueller"]);
    let q = PersonQuery {
        text: "muller dora".into(),
        ..query()
    };
    assert_eq!(names(&s.list_persons(&q, 0, 50)), ["Dora Vogel"]);
    let q = PersonQuery {
        no_birth_date: true,
        ..query()
    };
    assert_eq!(s.list_persons(&q, 0, 50).total, 3);
}

#[test]
fn private_only_filter() {
    let (_d, s) = open_ged(&ged(&[
        indi(1, "Anna /A/", "1 RESN privacy\n"),
        indi(2, "Bob /B/", ""),
    ]));
    let q = PersonQuery {
        private_only: true,
        ..query()
    };
    let page = s.list_persons(&q, 0, 50);
    assert_eq!(page.total, 1);
    assert!(page.items[0].person.private);
}

#[test]
fn letters_offsets_point_at_first_row_of_letter() {
    let (_d, s) = open_ged(&ged(&[
        indi(1, "A /Adams/", ""),
        indi(2, "B /Albrecht/", ""),
        indi(3, "C /Baker/", ""),
        indi(4, "D /Émile/", ""),
        indi(5, "E /Eva/", ""),
        indi(6, "F /Zoe/", ""),
    ]));
    let page = s.list_persons(&query(), 0, 50);
    let letters: Vec<(String, u32)> = page
        .letters
        .unwrap()
        .into_iter()
        .map(|l| (l.letter, l.offset))
        .collect();
    assert_eq!(
        letters,
        [
            ("A".into(), 0),
            ("B".into(), 2),
            ("E".into(), 3),
            ("Z".into(), 5)
        ]
    );
    let by_given = s.list_persons(
        &PersonQuery {
            sort: SortKey::Given,
            ..query()
        },
        0,
        50,
    );
    assert!(by_given.letters.is_none());
}

#[test]
fn non_latin_goes_to_other_scripts() {
    let (_d, s) = open_ged(&ged(&[
        indi(1, "A /Adams/", ""),
        indi(2, "B /Иванов/", ""),
        indi(3, "C /李/", ""),
        indi(4, "D /Zoe/", ""),
    ]));
    let page = s.list_persons(&query(), 0, 50);
    let letters: Vec<(String, u32)> = page
        .letters
        .unwrap()
        .into_iter()
        .map(|l| (l.letter, l.offset))
        .collect();
    assert_eq!(
        letters,
        [
            ("A".into(), 0),
            ("Z".into(), 1),
            ("Other scripts".into(), 2)
        ]
    );
}

#[test]
fn living_flag_matches_core_rule_undated_is_living() {
    let (_d, s) = open_ged(&ged(&[
        indi(1, "Undated /P/", ""),
        indi(2, "Old /P/", "1 BIRT\n2 DATE 1920\n"),
        indi(3, "Young /P/", "1 BIRT\n2 DATE 1990\n"),
        indi(4, "Dead /P/", "1 DEAT\n"),
        indi(5, "Forced /P/", "1 BIRT\n2 DATE 1920\n1 _LIVING Y\n"),
    ]));
    let live = |name: &str| {
        s.list_persons(&query(), 0, 50)
            .items
            .iter()
            .find(|r| r.person.name == name)
            .unwrap()
            .person
            .living
    };
    assert!(live("Undated P"));
    assert!(!live("Old P"));
    assert!(live("Young P"));
    assert!(!live("Dead P"));
    // The rule alone, shown next to the override, in the detail.
    let d = s.get_person(person_id(&s, "Old P")).unwrap();
    assert!(!d.living_computed);
    assert!(!d.person_ref.living);
}

#[test]
fn family_view_has_parents_partners_children_in_birth_order() {
    let (_d, s) = open_ged(&family_ged());
    let anna = person_id(&s, "Anna Schmidt");
    let v = s.family_view(anna).unwrap();
    assert_eq!(v.focus.name, "Anna Schmidt");
    assert_eq!(v.focus.lifespan, "b. 1850");
    assert_eq!(v.focus_summary, "b. 3 Apr 1850, Hamburg");
    assert!(v.parent_sets.is_empty());
    assert_eq!(v.partnerships.len(), 1);
    let p = &v.partnerships[0];
    assert_eq!(p.partner.as_ref().unwrap().name, "Karl Schmidt");
    assert_eq!(p.partner.as_ref().unwrap().lifespan, "1848 \u{2013} 1901");
    assert_eq!(p.summary.as_deref(), Some("m. 1 Jun 1871, Hamburg"));
    assert_eq!(p.events.len(), 1);
    let kids: Vec<&str> = p.children.iter().map(|c| c.person.name.as_str()).collect();
    assert_eq!(kids, ["Greta Schmidt", "Hans Schmidt", "Otto Schmidt"]);
    assert_eq!(p.children[0].birth, "1872");
    assert_eq!(p.children[0].rel, (ParentRelId::Birth, ParentRelId::Birth));

    let hans = person_id(&s, "Hans Schmidt");
    let v = s.family_view(hans).unwrap();
    assert_eq!(v.parent_sets.len(), 1);
    let ps = &v.parent_sets[0];
    assert_eq!(ps.parents.0.as_ref().unwrap().name, "Karl Schmidt");
    assert_eq!(ps.parents.1.as_ref().unwrap().name, "Anna Schmidt");
    let sibs: Vec<&str> = v.siblings.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(sibs, ["Greta Schmidt", "Otto Schmidt"]);
}

#[test]
fn family_view_with_cycle_terminates() {
    // I1 is a child of F1 whose husband is I1, and an ancestor of themself through F2/F3.
    let (_d, s) = open_ged(&ged(&[
        indi(1, "Loop /One/", ""),
        indi(2, "Loop /Two/", ""),
        "0 @F1@ FAM\n1 HUSB @I1@\n1 CHIL @I1@\n1 CHIL @I2@\n".to_string(),
        "0 @F2@ FAM\n1 HUSB @I2@\n1 CHIL @I1@\n".to_string(),
    ]));
    let id = person_id(&s, "Loop One");
    let v = s.family_view(id).unwrap();
    assert!(v.siblings.iter().all(|p| p.id != id));
    assert_eq!(v.partnerships.len(), 1);
    assert!(v.partnerships[0].partner.is_none());
    let _ = s.get_person(id).unwrap();
}

#[test]
fn person_with_12_partners_and_40_children() {
    let mut recs = vec![indi(1, "Many /Partner/", "1 SEX M\n")];
    let mut next = 2;
    for f in 0..12 {
        let wife = next;
        next += 1;
        recs.push(indi(wife, &format!("Wife{f} /Partner/"), "1 SEX F\n"));
        let kids = if f < 4 { 4 } else { 3 }; // 4*4 + 8*3 = 40
        let mut fam = format!("0 @F{f}@ FAM\n1 HUSB @I1@\n1 WIFE @I{wife}@\n");
        for k in 0..kids {
            let kid = next;
            next += 1;
            recs.push(indi(kid, &format!("Kid{f}x{k} /Partner/"), ""));
            writeln!(fam, "1 CHIL @I{kid}@").unwrap();
        }
        recs.push(fam);
    }
    let (_d, s) = open_ged(&ged(&recs));
    let v = s.family_view(person_id(&s, "Many Partner")).unwrap();
    assert_eq!(v.partnerships.len(), 12);
    assert_eq!(
        v.partnerships
            .iter()
            .map(|p| p.children.len())
            .sum::<usize>(),
        40
    );
    let d = s.get_person(person_id(&s, "Many Partner")).unwrap();
    assert_eq!(d.partnerships.len(), 12);
}

#[test]
fn person_detail_names_events_roles_citations() {
    let (_d, s) = open_ged(&family_ged());
    let d = s.get_person(person_id(&s, "Anna Schmidt")).unwrap();
    assert_eq!(d.sex, SexKind::Female);
    assert_eq!(d.names.len(), 1);
    assert_eq!(d.names[0].surnames, ["Schmidt"]);
    assert!(d.names[0].preferred);
    assert_eq!(d.names[0].display, "Anna Schmidt");
    let birth = d.events.iter().find(|e| e.kind == "birth").unwrap();
    assert_eq!(birth.label, "Birth");
    assert!(!birth.is_fact);
    assert_eq!(birth.date.as_ref().unwrap().display, "3 Apr 1850");
    assert_eq!(birth.place.as_ref().unwrap().full, "Hamburg, Germany");
    assert_eq!(birth.place.as_ref().unwrap().name, "Hamburg");
    assert_eq!(birth.participants.len(), 1);
    assert_eq!(birth.participants[0].role, "principal");
    assert_eq!(birth.participants[0].person.name, "Anna Schmidt");
    let cit = &birth.citations[0];
    assert_eq!(cit.page.as_deref(), Some("p. 12"));
    let src = cit.source.as_ref().unwrap();
    assert_eq!(src.title.as_deref(), Some("Hamburg parish register"));
    assert_eq!(src.citation_count, 1);
    let occ = d.events.iter().find(|e| e.kind == "occupation").unwrap();
    assert!(occ.is_fact);
    assert_eq!(d.partnerships.len(), 1);
    assert!(d.parent_sets.is_empty());
}

#[test]
fn display_name_orders() {
    let (_d, s) = open_ged(&ged(&[
        indi(1, "Anna /Schmidt/", ""),
        "0 @I2@ INDI\n".to_string(),
        indi(3, "/Solo/", ""),
    ]));
    let tree = s.tree();
    let anna = tree
        .persons
        .values()
        .find(|p| display_name(p, NameOrder::GivenFirst) == "Anna Schmidt")
        .unwrap();
    assert_eq!(display_name(anna, NameOrder::SurnameFirst), "Schmidt, Anna");
    let nameless = tree.persons.values().find(|p| p.names.is_empty()).unwrap();
    assert_eq!(display_name(nameless, NameOrder::GivenFirst), "Unknown");
    let solo = tree
        .persons
        .values()
        .find(|p| display_name(p, NameOrder::GivenFirst) == "Solo")
        .unwrap();
    assert_eq!(display_name(solo, NameOrder::SurnameFirst), "Solo");
}

#[test]
fn name_order_setting_changes_listed_names() {
    let (_d, mut s) = open_ged(&ged(&[indi(1, "Anna /Schmidt/", "")]));
    s.set_settings(SettingsPatch {
        name_order: Some(NameOrder::SurnameFirst),
        ..SettingsPatch::default()
    })
    .unwrap();
    assert_eq!(names(&s.list_persons(&query(), 0, 5)), ["Schmidt, Anna"]);
}

#[test]
fn lifespan_formats() {
    let (_d, s) = open_ged(&ged(&[
        indi(1, "A /T/", "1 BIRT\n2 DATE ABT 1968\n1 DEAT\n2 DATE 2021\n"),
        indi(2, "B /T/", "1 BIRT\n2 DATE 1970\n"),
        indi(3, "C /T/", "1 DEAT\n2 DATE 1921\n"),
        indi(4, "D /T/", ""),
        indi(
            5,
            "E /T/",
            "1 BIRT\n2 DATE BEF 1800\n1 DEAT\n2 DATE AFT 1850\n",
        ),
        indi(
            6,
            "F /T/",
            "1 BIRT\n2 DATE BET 1801 AND 1805\n1 DEAT\n2 DATE (unknown)\n",
        ),
        indi(7, "G /T/", "1 BIRT\n2 DATE 1850\n1 DEAT Y\n"),
    ]));
    let tree = s.tree();
    let span = |n: &str| {
        let p = tree
            .persons
            .values()
            .find(|p| display_name(p, NameOrder::GivenFirst) == n)
            .unwrap();
        lifespan(tree, p)
    };
    assert_eq!(span("A T"), "c. 1968 \u{2013} 2021");
    assert_eq!(span("B T"), "b. 1970");
    assert_eq!(span("C T"), "d. 1921");
    assert_eq!(span("D T"), "");
    assert_eq!(span("E T"), "bef. 1800 \u{2013} aft. 1850");
    assert_eq!(span("F T"), "c. 1801 \u{2013} ?");
    assert_eq!(span("G T"), "1850 \u{2013} ?");
    // The list rows carry the same text.
    let row = s
        .list_persons(&query(), 0, 10)
        .items
        .into_iter()
        .find(|r| r.person.name == "B T")
        .unwrap();
    assert_eq!(row.person.lifespan, "b. 1970");
    assert_eq!(row.birth, "1970");
    assert_eq!(row.death, "");
}

#[test]
fn unknown_id_is_not_found() {
    let (_d, s) = open_ged(&family_ged());
    let id = Uuid::now_v7();
    assert_eq!(s.get_person(id).unwrap_err().code, ErrorCode::NotFound);
    assert_eq!(s.family_view(id).unwrap_err().code, ErrorCode::NotFound);
}

#[test]
fn search_limit_clamped_and_person_ref_attached() {
    let recs: Vec<String> = (1..=60)
        .map(|i| indi(i, &format!("Sam{i} /Schmidt/"), ""))
        .collect();
    let (_d, s) = open_ged(&ged(&recs));
    let hits = s.search("Schmidt", 500).unwrap();
    assert_eq!(hits.len(), 50);
    assert!(hits.iter().all(|h| h.kind == HitKindDto::Person));
    for h in &hits {
        let p = h.person.as_ref().unwrap();
        assert_eq!(p.id, h.id);
        assert!(p.name.ends_with("Schmidt"));
    }
    assert!(s.search("   ", 10).unwrap().is_empty());
}

#[test]
fn person_search_hit_subtitle_is_lifespan_not_raw_index_text() {
    let recs = vec![indi(
        1,
        "Hana /Yamada/",
        "1 BIRT
2 DATE 1901
2 PLAC Kyoto
1 DEAT
2 DATE 1980
",
    )];
    let (_d, s) = open_ged(&ged(&recs));
    let hits = s.search("Yamada", 10).unwrap();
    let h = hits
        .iter()
        .find(|h| h.kind == HitKindDto::Person)
        .expect("person hit");
    let snip = h.snippet.as_deref().expect("lifespan subtitle");
    assert_eq!(snip, h.person.as_ref().unwrap().lifespan);
    assert!(
        !snip.contains('/') && !snip.contains('['),
        "raw index text leaked: {snip}"
    );
}

#[test]
fn settings_round_trip_through_meta() {
    let (dir, mut s) = open_ged(&family_ged());
    let anna = person_id(&s, "Anna Schmidt");
    s.set_settings(SettingsPatch {
        living_max_age: Some(80),
        home_person: Some(Some(anna)),
        ..SettingsPatch::default()
    })
    .unwrap();
    assert_eq!(s.info().home_person, Some(anna));
    let bad = s.set_settings(SettingsPatch {
        home_person: Some(Some(Uuid::now_v7())),
        ..SettingsPatch::default()
    });
    assert_eq!(bad.unwrap_err().code, ErrorCode::NotFound);
    s.close();
    let s = Session::open(
        &dir.path().join("project"),
        SessionClock::fixed(today(), NOW_MS),
    )
    .unwrap();
    assert_eq!(s.settings().living_max_age, 80);
    assert_eq!(s.info().home_person, Some(anna));
}

#[test]
fn note_and_place_hits_carry_the_owning_person() {
    let recs = vec![
        indi(
            1,
            "Anna /Berg/",
            "1 NOTE Emigrated in 1891 to Ohio\n1 BIRT\n2 DATE 1850\n2 PLAC Zanzibarton\n",
        ),
        indi(
            2,
            "Karl /Berg/",
            "1 DEAT\n2 DATE 1900\n2 NOTE Buried at sea near Quuxville\n",
        ),
    ];
    let (_d, s) = open_ged(&ged(&recs));
    let anna = s.search("Berg", 10).unwrap();
    let anna_id = anna
        .iter()
        .find(|h| h.label.starts_with("Anna"))
        .unwrap()
        .id;
    let karl_id = anna
        .iter()
        .find(|h| h.label.starts_with("Karl"))
        .unwrap()
        .id;

    let notes = s.search("emigrated", 10).unwrap();
    let hit = notes.iter().find(|h| h.kind == HitKindDto::Note).unwrap();
    assert_eq!(hit.person.as_ref().unwrap().id, anna_id);

    let event_note = s.search("quuxville", 10).unwrap();
    let hit = event_note
        .iter()
        .find(|h| h.kind == HitKindDto::Note)
        .unwrap();
    assert_eq!(hit.person.as_ref().unwrap().id, karl_id);

    let places = s.search("Zanzibarton", 10).unwrap();
    let hit = places.iter().find(|h| h.kind == HitKindDto::Place).unwrap();
    assert_eq!(hit.person.as_ref().unwrap().id, anna_id);
}
