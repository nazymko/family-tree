#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The Map view's payload and the place-coordinates edit. Invented people and villages only.

use gt_core::date::calendar::{Calendar, first_jdn};
use gt_gedcom::{ImportOptions, import};
use gt_session::*;
use gt_store::Project;
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
0 @I1@ INDI\n1 NAME Ivan Petrovych /Koval/\n1 SEX M\n1 FAMC @F1@\n1 FAMS @F3@\n\
1 BIRT\n2 DATE 1900\n2 PLAC Berezivka, Zelena oblast\n3 MAP\n4 LATI N50.5\n4 LONG E30.25\n\
1 RESI\n2 DATE 1925\n2 PLAC Dubky, Zelena oblast\n\
1 DEAT\n2 DATE 12 MAR 1970\n2 PLAC Kyiv\n3 MAP\n4 LATI N50.45\n4 LONG E30.52\n\
0 @I2@ INDI\n1 NAME Petro /Koval/\n1 SEX M\n1 FAMC @F2@\n1 FAMS @F1@\n1 BIRT\n2 DATE 1870\n2 PLAC Dubky, Zelena oblast\n\
0 @I3@ INDI\n1 NAME Maria /Koval/\n1 SEX F\n1 FAMS @F1@\n1 BIRT\n2 DATE 1875\n2 PLAC Hlukhiv\n\
0 @I4@ INDI\n1 NAME Hryhorii /Koval/\n1 SEX M\n1 FAMS @F2@\n1 BIRT\n2 DATE 1840\n2 PLAC Dubky, Zelena oblast\n\
0 @I5@ INDI\n1 NAME Olena /Koval/\n1 SEX F\n1 FAMC @F3@\n1 BIRT\n2 DATE 1930\n2 PLAC Kyiv\n\
0 @I6@ INDI\n1 NAME Nadiia /Koval/\n1 SEX F\n1 FAMS @F3@\n1 BIRT\n2 PLAC Hlukhiv\n\
0 @F1@ FAM\n1 HUSB @I2@\n1 WIFE @I3@\n1 CHIL @I1@\n1 MARR\n2 DATE 1895\n2 PLAC Hlukhiv\n\
0 @F2@ FAM\n1 HUSB @I4@\n1 CHIL @I2@\n\
0 @F3@ FAM\n1 HUSB @I1@\n1 WIFE @I6@\n1 CHIL @I5@\n\
0 TRLR\n";

fn person(s: &Session, given: &str) -> Uuid {
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

fn scope(kind: MapScopeKind, person: Option<Uuid>, generations: Option<u32>) -> MapScope {
    MapScope {
        kind,
        person,
        generations,
    }
}

#[test]
fn whole_tree_has_every_event_with_a_place() {
    let (_d, s) = open_ged(GED);
    let m = s.map_data(&scope(MapScopeKind::All, None, None));
    assert_eq!(m.people.len(), 6);
    // Ivan: birth, residence, death; Petro, Maria, Hryhorii, Olena, Nadiia: one birth each; the marriage once for
    // each of the two partners.
    let kinds: Vec<&str> = m.events.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(kinds.iter().filter(|k| **k == "birth").count(), 6);
    assert_eq!(kinds.iter().filter(|k| **k == "marriage").count(), 2);
    assert_eq!(m.events.len(), 10);
    let berezivka = m.places.iter().find(|p| p.name == "Berezivka").unwrap();
    assert_eq!(berezivka.label, "Berezivka, Zelena oblast");
    assert!((berezivka.lat.unwrap() - 50.5).abs() < 1e-9);
    assert!(
        m.places
            .iter()
            .find(|p| p.name == "Dubky")
            .unwrap()
            .lat
            .is_none()
    );
    // Events are in year order, undated first.
    let years: Vec<Option<i32>> = m.events.iter().map(|e| e.year).collect();
    assert_eq!(years.first(), Some(&None));
    assert_eq!(years.get(1), Some(&Some(1840)));
    assert_eq!(m.events_without_place, 0);
}

#[test]
fn ancestors_walk_generations_and_link_births() {
    let (_d, s) = open_ged(GED);
    let ivan = person(&s, "Ivan");
    let all = s.map_data(&scope(MapScopeKind::Ancestors, Some(ivan), None));
    let gens: Vec<(String, i32)> = {
        let mut v: Vec<_> = all
            .people
            .iter()
            .map(|p| (p.name.clone(), p.generation))
            .collect();
        v.sort();
        v
    };
    assert_eq!(
        gens.iter().find(|(n, _)| n.starts_with("Ivan")).unwrap().1,
        0
    );
    assert_eq!(
        gens.iter().find(|(n, _)| n.starts_with("Petro")).unwrap().1,
        1
    );
    assert_eq!(
        gens.iter().find(|(n, _)| n.starts_with("Maria")).unwrap().1,
        1
    );
    assert_eq!(
        gens.iter()
            .find(|(n, _)| n.starts_with("Hryhorii"))
            .unwrap()
            .1,
        2
    );
    assert_eq!(all.people.len(), 4);
    assert_eq!(all.links.len(), 3);
    let one = s.map_data(&scope(MapScopeKind::Ancestors, Some(ivan), Some(1)));
    assert_eq!(one.people.len(), 3);
    assert!(one.people.iter().all(|p| p.generation <= 1));
    assert_eq!(one.links.len(), 2);
}

#[test]
fn descendants_and_one_person() {
    let (_d, s) = open_ged(GED);
    let ivan = person(&s, "Ivan");
    let d = s.map_data(&scope(MapScopeKind::Descendants, Some(ivan), None));
    let mut names: Vec<_> = d.people.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Ivan Petrovych Koval", "Olena Koval"]);
    assert_eq!(d.links.len(), 1);
    let one = s.map_data(&scope(MapScopeKind::Person, Some(ivan), None));
    assert_eq!(one.people.len(), 1);
    assert_eq!(one.events.len(), 3);
    assert!(one.links.is_empty());
    let none = s.map_data(&scope(MapScopeKind::Person, None, None));
    assert!(none.people.is_empty() && none.events.is_empty());
}

#[test]
fn private_people_and_events_stay_out() {
    let (_d, mut s) = open_ged(GED);
    let petro = person(&s, "Petro");
    s.apply(EditCommand::UpdatePerson {
        id: petro,
        sex: None,
        sex_text: None,
        living_override: LivingPatch::Keep,
        private: Some(true),
    })
    .unwrap();
    let m = s.map_data(&scope(
        MapScopeKind::Ancestors,
        Some(person(&s, "Ivan")),
        None,
    ));
    assert!(m.people.iter().all(|p| p.id != petro));
    // The walk does not pass through a private person: his father is not reached either.
    assert_eq!(m.people.len(), 2);
}

#[test]
fn set_and_clear_coordinates_are_undoable() {
    let (_d, mut s) = open_ged(GED);
    let m = s.map_data(&scope(MapScopeKind::All, None, None));
    let dubky = m.places.iter().find(|p| p.name == "Dubky").unwrap().id;
    let r = s
        .apply(EditCommand::SetPlaceCoordinates {
            place: dubky,
            lat_e6: Some(50_123_456),
            lon_e6: Some(30_654_321),
        })
        .unwrap();
    assert_eq!(r.label, "Set place coordinates");
    let p = &s.tree().places[&dubky];
    assert!((p.lat.unwrap() - 50.123456).abs() < 1e-9 && (p.lon.unwrap() - 30.654321).abs() < 1e-9);
    s.undo().unwrap();
    assert!(s.tree().places[&dubky].lat.is_none());
    s.redo().unwrap();
    let r = s
        .apply(EditCommand::SetPlaceCoordinates {
            place: dubky,
            lat_e6: None,
            lon_e6: None,
        })
        .unwrap();
    assert_eq!(r.label, "Clear place coordinates");
    assert!(s.tree().places[&dubky].lon.is_none());
}

#[test]
fn bad_coordinates_are_refused() {
    let (_d, mut s) = open_ged(GED);
    let m = s.map_data(&scope(MapScopeKind::All, None, None));
    let id = m.places[0].id;
    for (la, lo) in [
        (Some(91_000_000), Some(0)),
        (Some(0), Some(181_000_000)),
        (Some(1), None),
        (None, Some(1)),
    ] {
        let e = s
            .apply(EditCommand::SetPlaceCoordinates {
                place: id,
                lat_e6: la,
                lon_e6: lo,
            })
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::Invalid);
    }
    let e = s
        .apply(EditCommand::SetPlaceCoordinates {
            place: Uuid::nil(),
            lat_e6: Some(1),
            lon_e6: Some(1),
        })
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn ten_thousand_people_build_quickly() {
    let mut ged = String::from("0 HEAD\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n");
    for i in 0..10_000 {
        ged.push_str(&format!(
            "0 @I{i}@ INDI\n1 NAME P{i} /Koval/\n1 BIRT\n2 DATE {}\n2 PLAC Village{}, Zelena oblast\n",
            1800 + i % 200,
            i % 300
        ));
    }
    ged.push_str("0 TRLR\n");
    let (_d, s) = open_ged(&ged);
    let t = std::time::Instant::now();
    let m = s.map_data(&scope(MapScopeKind::All, None, None));
    let took = t.elapsed();
    assert_eq!(m.events.len(), 10_000);
    assert_eq!(m.places.len(), 300);
    eprintln!("map_data for 10k events: {took:?}");
    assert!(took.as_millis() < 5000, "{took:?}");
}
