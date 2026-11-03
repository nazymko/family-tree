#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use gt_core::DayOrder;
use gt_core::date::GenDate;
use gt_core::model::*;
use gt_gedcom::{ImportOptions, ImportOutcome, import};

fn opts() -> ImportOptions {
    ImportOptions {
        day_order: DayOrder::Dmy,
        media_roots: vec![],
        ..Default::default()
    }
}

fn imp(s: &str) -> ImportOutcome {
    import(s.as_bytes(), &opts())
}

fn basic() -> ImportOutcome {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/gedcom/basic551.ged"
    );
    import(&std::fs::read(p).unwrap(), &opts())
}

#[test]
fn basic_persons_events_places_names() {
    let o = basic();
    let t = &o.tree;
    assert_eq!(t.persons.len(), 2);
    assert_eq!(o.report.producer.as_deref(), Some("ANCESTRY"));
    let p = t.persons.values().next().unwrap();
    assert_eq!(p.sex, Sex::Female);
    let (ev, role) = t.events_of(p.common.id).next().unwrap();
    assert_eq!(*role, Role::Principal);
    assert_eq!(ev.kind, EventKind::Birth);
    let d = ev.date.as_ref().unwrap();
    assert_eq!(d.sort_min, GenDate::parse("1845", DayOrder::Dmy).sort_min);
    let leaf = &t.places[&ev.place.unwrap()];
    assert_eq!(leaf.raw.as_deref(), Some("Berlin, Brandenburg, Germany"));
    assert_eq!(leaf.lat, Some(52.5));
    assert_eq!(leaf.lon, Some(13.4));
    let mut depth = 1;
    let mut cur = leaf;
    while let Some(par) = cur.parent {
        cur = &t.places[&par];
        depth += 1;
    }
    assert_eq!(depth, 3);
    assert_eq!(cur.names[0].name, "Germany");
    assert_eq!(p.names.len(), 2);
    assert_eq!(p.names[1].kind, NameKind::Married);
    assert_eq!(p.names[1].surnames, vec!["Schmidt".to_string()]);
    assert!(p.names[0].assertion.preferred);
    assert!(
        p.common
            .ext_ids
            .iter()
            .any(|e| e.kind == ExtIdKind::Uid && e.value == "ABC123")
    );
}

#[test]
fn unknown_tag_kept_with_pos() {
    let o = basic();
    assert_eq!(o.report.unmapped_tags["_MTTAG"], 1);
    let p = o.tree.persons.values().next().unwrap();
    assert_eq!(p.common.unknown[0].tag, "_MTTAG");
    assert_eq!(p.common.unknown[0].pos, 1);
}

#[test]
fn shared_event_merges_by_evid() {
    let o = imp(
        "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n0 @I1@ INDI\n1 BAPM\n2 _EVID abc\n0 @I2@ INDI\n1 BAPM\n2 _EVID abc\n0 TRLR\n",
    );
    assert_eq!(o.tree.events.len(), 1);
    assert_eq!(o.tree.events.values().next().unwrap().participants.len(), 2);
}

#[test]
fn negative_assertion() {
    let o = imp("0 HEAD\n1 GEDC\n2 VERS 7.0\n0 @I1@ INDI\n1 NO MARR\n0 TRLR\n");
    let e = o.tree.events.values().next().unwrap();
    assert_eq!(e.kind, EventKind::Marriage);
    assert!(e.negative);
    let o = imp("0 HEAD\n1 GEDC\n2 VERS 5.5.1\n0 @I1@ INDI\n1 _NO MARR\n0 TRLR\n");
    assert!(o.tree.events.values().next().unwrap().negative);
}

#[test]
fn name_parts_and_custom_event() {
    let o = imp(
        "0 HEAD\n0 @I1@ INDI\n1 NAME Dr. Jan /van der Berg/ Jr\n2 SURN van, Berg\n2 NICK Jantje\n1 EVEN x\n2 TYPE Mission\n1 RESN PRIVACY\n0 TRLR\n",
    );
    let p = o.tree.persons.values().next().unwrap();
    let n = &p.names[0];
    assert_eq!(n.given.as_deref(), Some("Dr. Jan"));
    assert_eq!(n.suffix.as_deref(), Some("Jr"));
    assert_eq!(n.surnames, vec!["van".to_string(), "Berg".to_string()]);
    assert_eq!(n.nickname.as_deref(), Some("Jantje"));
    assert!(p.common.private);
    let e = o.tree.events.values().next().unwrap();
    assert_eq!(e.kind, EventKind::Custom("Mission".into()));
}

#[test]
fn unparsed_date_reported() {
    let o = imp("0 HEAD\n0 @I1@ INDI\n1 BIRT\n2 DATE sometime in spring\n0 TRLR\n");
    assert_eq!(o.report.unparsed_dates.len(), 1);
    assert!(o.report.to_text().contains("unparsed"));
}
