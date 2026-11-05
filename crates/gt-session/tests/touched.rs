#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! What `tree-changed-by-agent` carries: the records changed and the people they belong to, so an
//! editor open on a person hears about a change to that person's note, event or citation.

mod common;

use common::*;
use gt_core::model::ParticipantRef;
use gt_core::tree::Tree;
use gt_session::*;
use std::collections::HashSet;
use uuid::Uuid;

fn person_named(tree: &Tree, given: &str) -> Uuid {
    tree.persons
        .values()
        .find(|p| p.names.iter().any(|n| n.given.as_deref() == Some(given)))
        .map(|p| p.common.id)
        .unwrap()
}

fn touched(s: &Session, changed: &[Uuid]) -> HashSet<Uuid> {
    let list = s.touched_with_people(changed);
    let set: HashSet<Uuid> = list.iter().copied().collect();
    assert_eq!(set.len(), list.len(), "no id twice");
    set
}

const SOURCED: &str = "0 @I1@ INDI\n1 NAME Olena /Koval/\n1 SEX F\n1 BIRT\n2 DATE 1850\n2 PLAC Kyiv\n1 NOTE Born in the old house\n1 SOUR @S1@\n2 PAGE 12\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Ivan /Koval/\n1 SEX M\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Petro /Koval/\n1 SEX M\n1 FAMC @F1@\n\
0 @I4@ INDI\n1 NAME Maria /Melnyk/\n1 SEX F\n\
0 @F1@ FAM\n1 HUSB @I2@\n1 WIFE @I1@\n1 CHIL @I3@\n1 MARR\n2 DATE 1870\n1 NOTE Wed in spring\n\
0 @S1@ SOUR\n1 TITL Parish book\n";

#[test]
fn a_person_stays_in_the_list_and_brings_no_relatives() {
    let tree = tree_of(SOURCED);
    let (_d, s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    assert_eq!(touched(&s, &[olena]), HashSet::from([olena]));
}

#[test]
fn a_note_brings_its_person() {
    let tree = tree_of(SOURCED);
    let (_d, s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    let note = tree.persons[&olena].common.notes[0];
    assert_eq!(touched(&s, &[note]), HashSet::from([note, olena]));
}

#[test]
fn a_citation_brings_its_person() {
    let tree = tree_of(SOURCED);
    let (_d, s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    let citation = tree.persons[&olena].common.citations[0];
    assert_eq!(touched(&s, &[citation]), HashSet::from([citation, olena]));
}

#[test]
fn an_event_brings_its_people() {
    let tree = tree_of(SOURCED);
    let (_d, s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    let birth = tree
        .events
        .values()
        .find(|e| {
            e.participants
                .iter()
                .any(|p| p.target == ParticipantRef::Person(olena))
        })
        .unwrap();
    assert_eq!(
        touched(&s, &[birth.common.id]),
        HashSet::from([birth.common.id, olena])
    );
}

#[test]
fn a_family_note_brings_partners_and_children_but_not_strangers() {
    let tree = tree_of(SOURCED);
    let (_d, s) = open_tree(&tree);
    let family = tree.families.values().next().unwrap();
    let note = family.common.notes[0];
    let got = touched(&s, &[note]);
    assert!(got.contains(&note));
    assert!(got.contains(&person_named(&tree, "Olena")));
    assert!(got.contains(&person_named(&tree, "Ivan")));
    assert!(got.contains(&person_named(&tree, "Petro")));
    assert!(!got.contains(&person_named(&tree, "Maria")));
}

#[test]
fn a_family_event_brings_the_partners() {
    let tree = tree_of(SOURCED);
    let (_d, s) = open_tree(&tree);
    let family = tree.families.values().next().unwrap().common.id;
    let marriage = tree
        .events
        .values()
        .find(|e| {
            e.participants
                .iter()
                .any(|p| p.target == ParticipantRef::Family(family))
        })
        .unwrap();
    let got = touched(&s, &[marriage.common.id]);
    assert!(got.contains(&person_named(&tree, "Olena")));
    assert!(got.contains(&person_named(&tree, "Ivan")));
}

#[test]
fn a_source_and_a_place_reach_the_people_through_what_uses_them() {
    let tree = tree_of(SOURCED);
    let (_d, s) = open_tree(&tree);
    let olena = person_named(&tree, "Olena");
    let source = tree.sources.keys().next().copied().unwrap();
    assert!(touched(&s, &[source]).contains(&olena));
    let place = tree.places.keys().next().copied().unwrap();
    assert!(touched(&s, &[place]).contains(&olena));
    assert!(!touched(&s, &[place]).contains(&person_named(&tree, "Maria")));
}

#[test]
fn an_unknown_id_is_kept_and_adds_nobody() {
    let tree = tree_of(SOURCED);
    let (_d, s) = open_tree(&tree);
    let unknown = Uuid::from_u128(99);
    assert_eq!(touched(&s, &[unknown]), HashSet::from([unknown]));
    assert!(s.touched_with_people(&[]).is_empty());
}
