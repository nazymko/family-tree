#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! An agent merges a duplicate person into another, in one undoable step: names stay as
//! alternative names, family links, events and godparent links move, nobody is made to count as
//! alive, and the refusals say why. Invented people only.

mod common;

use common::*;
use gt_core::model::{EventKind, ParticipantRef, Role, Sex};
use gt_session::*;
use uuid::Uuid;

/// Hryhorii and Olha Bondar have two sons in the tree, Ivan and Ioan (also written Yoan): the
/// same man, entered twice. Ivan and Ioan both have a family with Maria Koval. Petro is Ivan's
/// child, Stepan Ioan's. Ioan's death year differs by one.
const TREE: &str = "\
0 @I1@ INDI\n1 NAME Hryhorii /Bondar/\n1 SEX M\n1 BIRT\n2 DATE 1800\n2 PLAC Verbka\n1 DEAT\n2 DATE 1870\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Olha /Bondar/\n1 SEX F\n1 BIRT\n2 DATE 1805\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Ivan /Bondar/\n1 SEX M\n1 BIRT\n2 DATE 1830\n1 DEAT\n2 DATE 1900\n1 FAMC @F1@\n1 FAMS @F2@\n1 NOTE Ivan lived by the mill.\n\
0 @I4@ INDI\n1 NAME Ioan /Bondar/\n1 NAME Yoan /Bondar/\n1 SEX M\n1 BIRT\n2 DATE 1830\n1 DEAT\n2 DATE 1901\n1 FAMC @F1@\n1 FAMS @F3@\n1 NOTE Ioan sang in the choir.\n1 SOUR @S1@\n\
0 @I5@ INDI\n1 NAME Maria /Koval/\n1 SEX F\n1 BIRT\n2 DATE 1835\n1 FAMS @F2@\n1 FAMS @F3@\n\
0 @I6@ INDI\n1 NAME Petro /Bondar/\n1 SEX M\n1 BIRT\n2 DATE 1860\n1 FAMC @F2@\n\
0 @I7@ INDI\n1 NAME Stepan /Bondar/\n1 SEX M\n1 BIRT\n2 DATE 1862\n1 FAMC @F3@\n\
0 @I8@ INDI\n1 NAME Dmytro /Shevchuk/\n1 SEX M\n1 BIRT\n2 DATE 1840\n\
0 @I9@ INDI\n1 NAME Halyna /Kravets/\n1 SEX F\n1 BIRT\n2 DATE 1835\n\
0 @I10@ INDI\n1 NAME Taras /Kravets/\n1 SEX M\n1 BIRT\n2 DATE 1835\n\
0 @I11@ INDI\n1 NAME Yarema /Tkach/\n1 SEX M\n\
0 @I12@ INDI\n1 NAME Yaroslav /Tkach/\n1 SEX M\n1 DEAT\n2 DATE 1950\n\
0 @I13@ INDI\n1 NAME Zoriana /Tkach/\n1 SEX U\n1 BIRT\n2 DATE 1836\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n1 CHIL @I4@\n\
0 @F2@ FAM\n1 HUSB @I3@\n1 WIFE @I5@\n1 CHIL @I6@\n\
0 @F3@ FAM\n1 HUSB @I4@\n1 WIFE @I5@\n1 CHIL @I7@\n\
0 @S1@ SOUR\n1 TITL Parish book of Verbka\n";

fn me() -> AgentCtx {
    AgentCtx {
        id: Uuid::from_u128(11),
        name: "People keeper".into(),
    }
}

struct World {
    _dir: tempfile::TempDir,
    s: Session,
    batch: Uuid,
    ivan: Uuid,
    ioan: Uuid,
    maria: Uuid,
    petro: Uuid,
    stepan: Uuid,
    dmytro: Uuid,
}

fn person_owner(id: Uuid) -> EventOwner {
    EventOwner {
        kind: EventOwnerKind::Person,
        id,
    }
}

fn baptism(godparent: Uuid) -> EventInput {
    EventInput {
        id: None,
        kind: "baptism".into(),
        date_raw: Some("1 Jan 1860".into()),
        place_text: None,
        place_id: None,
        description: None,
        age: None,
        cause: None,
        negative: false,
        private: false,
        preferred: true,
        participants: None,
        godparents: Some(vec![GodparentInput {
            id: None,
            person: Some(godparent),
            name: None,
            role: "godfather".into(),
            note: None,
        }]),
    }
}

fn world() -> World {
    let tree = tree_of(TREE);
    let (ivan, ioan, maria) = (
        find(&tree, "Ivan"),
        find(&tree, "Ioan"),
        find(&tree, "Maria"),
    );
    let (petro, stepan, dmytro) = (
        find(&tree, "Petro"),
        find(&tree, "Stepan"),
        find(&tree, "Dmytro"),
    );
    let (_dir, mut s) = open_tree(&tree);
    // Ioan stood godfather to Petro; Dmytro stood godfather at Ioan's own baptism.
    s.apply(EditCommand::SaveEvent {
        owner: person_owner(petro),
        event: baptism(ioan),
    })
    .unwrap();
    s.apply(EditCommand::SaveEvent {
        owner: person_owner(ioan),
        event: baptism(dmytro),
    })
    .unwrap();
    let batch = s.begin_batch(&me(), "Duplicates", None).unwrap();
    World {
        _dir,
        s,
        batch,
        ivan,
        ioan,
        maria,
        petro,
        stepan,
        dmytro,
    }
}

fn merge(w: &mut World, keep: Uuid, gone: Uuid) -> Result<PeopleMergeOutcome, ApiError> {
    w.s.merge_people_as_agent(
        keep,
        gone,
        "Same parents and the same wife; born 1830.",
        &me(),
        w.batch,
    )
    .map(|(o, _)| o)
}

/// The same records, whatever order the map keeps them in and whatever the `changed` stamps say
/// (an undo puts the content back and stamps the time).
fn same_records(a: &gt_core::tree::Tree, b: &gt_core::tree::Tree) -> bool {
    fn plain(t: &gt_core::tree::Tree) -> gt_core::tree::Tree {
        let mut t = t.clone();
        t.persons.values_mut().for_each(|r| r.common.changed = None);
        t.families
            .values_mut()
            .for_each(|r| r.common.changed = None);
        t.events.values_mut().for_each(|r| r.common.changed = None);
        t.notes.values_mut().for_each(|r| r.common.changed = None);
        t.citations
            .values_mut()
            .for_each(|r| r.common.changed = None);
        t
    }
    let (a, b) = (plain(a), plain(b));
    macro_rules! same {
        ($m:ident) => {
            a.$m.len() == b.$m.len() && a.$m.iter().all(|(k, v)| b.$m.get(k) == Some(v))
        };
    }
    same!(persons) && same!(families) && same!(events) && same!(notes) && same!(citations)
}

fn refusal(w: &mut World, keep: Uuid, gone: Uuid) -> String {
    let before = w.s.tree().clone();
    let err = merge(w, keep, gone).unwrap_err();
    assert!(
        same_records(w.s.tree(), &before),
        "a refusal changes nothing"
    );
    err.message
}

#[test]
fn a_merge_moves_everything_and_reports_what_to_look_at() {
    let mut w = world();
    let (ivan, ioan) = (w.ivan, w.ioan);
    let out = merge(&mut w, ivan, ioan).unwrap();
    let t = w.s.tree();
    assert!(!t.persons.contains_key(&ioan));
    assert_eq!(out.kept, Some(ivan));
    assert_eq!(out.removed, Some(ioan));
    let c = &out.counts;
    assert_eq!(c.families_as_partner, 1);
    assert_eq!(c.families_as_child, 1);
    assert_eq!(c.families_folded, 1, "Ivan and Maria had two families");
    assert_eq!(c.godparent_links, 1);
    assert_eq!(c.events_folded, 1, "the two births of 1830 are one");
    assert_eq!((c.notes, c.citations, c.numbers), (1, 1, 1), "{c:?}");

    // One family of Ivan and Maria, with both sons' children.
    let families: Vec<_> = t.families_as_partner(ivan).collect();
    assert_eq!(families.len(), 1);
    let kids: Vec<Uuid> = families[0].children.iter().map(|c| c.person).collect();
    assert_eq!(kids.len(), 2);
    assert!(kids.contains(&w.petro) && kids.contains(&w.stepan));
    assert_eq!(families[0].partner2, Some(w.maria));
    // A child once in the parents' family.
    let parents = t.families_as_child(ivan).collect::<Vec<_>>();
    assert_eq!(parents.len(), 1);
    assert_eq!(
        parents[0]
            .0
            .children
            .iter()
            .filter(|c| c.person == ivan)
            .count(),
        1
    );
    // Nothing points at the removed person.
    for f in t.families.values() {
        assert!(f.partner1 != Some(ioan) && f.partner2 != Some(ioan));
        assert!(f.children.iter().all(|c| c.person != ioan));
    }
    for e in t.events.values() {
        assert!(
            e.participants
                .iter()
                .all(|p| p.target != ParticipantRef::Person(ioan))
        );
        assert!(e.godparents.iter().all(|g| g.person != Some(ioan)));
    }
    // Two different death dates are kept as two events, and said so.
    let deaths = t
        .events_of(ivan)
        .filter(|(e, r)| e.kind == EventKind::Death && **r == Role::Principal)
        .count();
    assert_eq!(deaths, 2);
    assert!(
        out.conflicts.iter().any(|m| m.contains("Death")),
        "{:?}",
        out.conflicts
    );
    // The reason is in the log label and in a research note on the kept person.
    let p = &t.persons[&ivan];
    let notes: Vec<&str> = p
        .common
        .notes
        .iter()
        .map(|n| t.notes[n].text.as_str())
        .collect();
    assert!(
        notes
            .iter()
            .any(|n| n.contains("Same parents and the same wife"))
    );
    assert!(notes.iter().any(|n| n.contains("by the mill")));
    assert!(notes.iter().any(|n| n.contains("choir")));
    let log = w.s.log(LogFilterDto::default(), 0, 20).unwrap().items;
    assert!(
        log.iter()
            .any(|g| g.label.contains("Merge people") && g.label.contains("same wife")),
        "{:?}",
        log.iter().map(|g| &g.label).collect::<Vec<_>>()
    );
}

#[test]
fn every_name_of_the_merged_person_stays_as_an_alternative_name() {
    let mut w = world();
    let (ivan, ioan) = (w.ivan, w.ioan);
    let primary = w.s.tree().persons[&ivan].names[0].id;
    merge(&mut w, ivan, ioan).unwrap();
    let p = &w.s.tree().persons[&ivan];
    let givens: Vec<&str> = p.names.iter().filter_map(|n| n.given.as_deref()).collect();
    assert_eq!(givens, ["Ivan", "Ioan", "Yoan"]);
    assert_eq!(p.names[0].id, primary);
    assert!(p.names[0].assertion.preferred);
    assert!(p.names[1..].iter().all(|n| !n.assertion.preferred));
}

#[test]
fn godparent_links_move_in_both_directions() {
    let mut w = world();
    let (ivan, ioan, petro, dmytro) = (w.ivan, w.ioan, w.petro, w.dmytro);
    merge(&mut w, ivan, ioan).unwrap();
    let t = w.s.tree();
    // Ioan as godfather at Petro's baptism is now Ivan.
    let at_petro = t
        .events_of(petro)
        .find(|(e, _)| e.kind == EventKind::Baptism)
        .unwrap()
        .0;
    assert_eq!(at_petro.godparents.len(), 1);
    assert_eq!(at_petro.godparents[0].person, Some(ivan));
    // Ioan's own baptism, with Dmytro as godfather, is Ivan's now.
    let own = t
        .events_of(ivan)
        .find(|(e, _)| e.kind == EventKind::Baptism)
        .unwrap()
        .0;
    assert_eq!(own.godparents[0].person, Some(dmytro));
}

#[test]
fn living_and_dead_are_respected() {
    let mut w = world();
    // The dead stay dead: nobody in the family flipped.
    let (ivan, ioan) = (w.ivan, w.ioan);
    merge(&mut w, ivan, ioan).unwrap();
    assert!(!w.s.person_ref(ivan).unwrap().living);
    assert!(!w.s.person_ref(w.petro).unwrap().living);

    // Alive Yarema (no dates) and dead Yaroslav (died 1950): the result counts as dead, whichever
    // of the two is kept.
    let tree = w.s.tree().clone();
    let (yarema, yaroslav) = (find(&tree, "Yarema"), find(&tree, "Yaroslav"));
    assert!(w.s.person_ref(yarema).unwrap().living);
    assert!(!w.s.person_ref(yaroslav).unwrap().living);
    merge(&mut w, yarema, yaroslav).unwrap();
    assert!(!w.s.person_ref(yarema).unwrap().living);

    // The other way round, in a fresh tree.
    let mut w = world();
    let tree = w.s.tree().clone();
    let (yarema, yaroslav) = (find(&tree, "Yarema"), find(&tree, "Yaroslav"));
    merge(&mut w, yaroslav, yarema).unwrap();
    assert!(!w.s.person_ref(yaroslav).unwrap().living);

    // The owner marked the kept person as alive; the merged one is dead: refused, nothing changes.
    let mut w = world();
    let tree = w.s.tree().clone();
    let (yarema, yaroslav) = (find(&tree, "Yarema"), find(&tree, "Yaroslav"));
    w.s.apply(EditCommand::UpdatePerson {
        id: yarema,
        sex: None,
        sex_text: None,
        living_override: LivingPatch::Set(LivingOverrideDto::Living),
        private: None,
    })
    .unwrap();
    let msg = refusal(&mut w, yarema, yaroslav);
    assert!(msg.contains("alive"), "{msg}");
}

#[test]
fn refusals_say_why() {
    let mut w = world();
    let tree = w.s.tree().clone();
    let (ivan, ioan, petro) = (w.ivan, w.ioan, w.petro);
    let hryhorii = find(&tree, "Hryhorii");
    let (halyna, taras, zoriana) = (
        find(&tree, "Halyna"),
        find(&tree, "Taras"),
        find(&tree, "Zoriana"),
    );

    let msg = refusal(&mut w, ivan, ivan);
    assert!(msg.contains("same person"), "{msg}");
    let nobody = Uuid::from_u128(5);
    refusal(&mut w, ivan, nobody);
    refusal(&mut w, nobody, ivan);
    // A parent and a child, in either order.
    let msg = refusal(&mut w, ivan, petro);
    assert!(msg.contains("ancestor"), "{msg}");
    let msg = refusal(&mut w, petro, hryhorii);
    assert!(msg.contains("ancestor"), "{msg}");
    // Partners of one family.
    let maria = w.maria;
    let msg = refusal(&mut w, ivan, maria);
    assert!(msg.contains("partners") || msg.contains("sexes"), "{msg}");
    // Different recorded sexes.
    let msg = refusal(&mut w, halyna, taras);
    assert!(msg.contains("sexes"), "{msg}");
    assert_eq!(w.s.tree().persons[&halyna].sex, Sex::Female);
    // An empty reason.
    let err =
        w.s.merge_people_as_agent(ivan, ioan, "  ", &me(), w.batch)
            .unwrap_err();
    assert!(err.message.contains("reason"), "{}", err.message);
    // An unknown sex may be merged with either.
    merge(&mut w, halyna, zoriana).unwrap();
}

#[test]
fn it_needs_an_open_batch() {
    let mut w = world();
    let (ivan, ioan) = (w.ivan, w.ioan);
    let err =
        w.s.merge_people_as_agent(ivan, ioan, "Same man.", &me(), Uuid::from_u128(1))
            .unwrap_err();
    assert!(err.message.contains("batch"), "{}", err.message);
    assert!(w.s.tree().persons.contains_key(&ioan));
}

#[test]
fn one_undo_of_the_batch_restores_everything() {
    let mut w = world();
    let before = w.s.tree().clone();
    let people = w.s.person_ref(w.ioan).unwrap();
    let (ivan, ioan) = (w.ivan, w.ioan);
    merge(&mut w, ivan, ioan).unwrap();
    assert_ne!(*w.s.tree(), before);
    let report = w.s.undo_batch(w.batch).unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert!(
        same_records(w.s.tree(), &before),
        "persons, families, events, notes: all back"
    );
    assert_eq!(w.s.person_ref(w.ioan).unwrap(), people);
    assert_eq!(w.s.tree().persons.len(), before.persons.len());
    assert_eq!(w.s.tree().families.len(), before.families.len());
    assert_eq!(w.s.tree().events.len(), before.events.len());
}

fn record_people(w: &World, place: Uuid) -> Vec<Option<Uuid>> {
    w.s.place_events(&PlaceEventsQuery {
        place,
        ..Default::default()
    })
    .unwrap()
    .items
    .iter()
    .filter(|e| e.source == "record")
    .flat_map(|e| e.people.iter().map(|p| p.person))
    .collect()
}

#[test]
fn place_record_links_follow_the_merged_person_and_undo_brings_them_back() {
    let mut w = world();
    let (ivan, ioan) = (w.ivan, w.ioan);
    let place = *w.s.tree().places.keys().next().unwrap();
    let person = |given: &str, who: Uuid| gt_store::PlaceRecordPerson {
        role: "child".into(),
        given: given.into(),
        surname: "Bondar".into(),
        person: Some(who),
        ..Default::default()
    };
    // One record names Ioan, another names both men (two entries of one book page).
    let rec = |id: &str, people: Vec<gt_store::PlaceRecordPerson>| gt_store::PlaceRecordInput {
        kind: "birth".into(),
        date: "1830".into(),
        book: "BK-1".into(),
        page: "1".into(),
        rec_id: id.into(),
        people,
        ..Default::default()
    };
    w.s.add_place_records(
        place,
        &[
            rec("r:1", vec![person("Ioan", ioan)]),
            rec("r:2", vec![person("Ivan", ivan), person("Ioan", ioan)]),
        ],
        None,
    )
    .unwrap();
    assert_eq!(
        record_people(&w, place)
            .iter()
            .filter(|p| **p == Some(ioan))
            .count(),
        2
    );

    merge(&mut w, ivan, ioan).unwrap();
    let after = record_people(&w, place);
    assert_eq!(after.len(), 3);
    assert!(after.iter().all(|p| *p == Some(ivan)), "{after:?}");

    w.s.undo_batch(w.batch).unwrap();
    let back = record_people(&w, place);
    assert_eq!(
        back.iter().filter(|p| **p == Some(ioan)).count(),
        2,
        "{back:?}"
    );
    assert_eq!(back.iter().filter(|p| **p == Some(ivan)).count(), 1);
}
