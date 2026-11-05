#![allow(clippy::unwrap_used)]
//! linkExisting: placing someone already in the tree as a relative.
mod common;

use common::*;
use gt_core::date::DayOrder;
use gt_core::tree::Tree;
use gt_session::ops::{apply_to_tree, plan};
use gt_session::*;
use uuid::Uuid;

const NOW: i64 = 1_793_664_000_000;

/// Petro and Hanna are married with no child recorded; Ivan's birth names only Petro, Olena has no parents,
/// and Taras is Ivan's son.
const TREE: &str = "0 HEAD
1 GEDC
2 VERS 5.5.1
1 CHAR UTF-8
0 @I1@ INDI
1 NAME Petro /Koval/
1 SEX M
1 FAMS @F1@
1 FAMS @F2@
0 @I2@ INDI
1 NAME Hanna /Koval/
1 SEX F
1 FAMS @F1@
0 @I3@ INDI
1 NAME Ivan /Koval/
1 SEX M
1 FAMC @F2@
1 FAMS @F3@
0 @I4@ INDI
1 NAME Olena /Koval/
1 SEX F
0 @I5@ INDI
1 NAME Taras /Koval/
1 SEX M
1 FAMC @F3@
0 @I6@ INDI
1 NAME Oksana /Bondar/
1 SEX F
0 @F1@ FAM
1 HUSB @I1@
1 WIFE @I2@
1 MARR
0 @F2@ FAM
1 HUSB @I1@
1 CHIL @I3@
0 @F3@ FAM
1 HUSB @I3@
1 CHIL @I5@
0 TRLR
";

fn link(anchor: Uuid, relation: Relation, person: Uuid) -> EditCommand {
    EditCommand::LinkExisting {
        anchor,
        relation,
        person,
        family: None,
        new_parent_set: None,
        parent_rel: None,
    }
}

fn apply(t: &mut Tree, cmd: &EditCommand) {
    let (cs, _) = plan(t, cmd, NOW, DayOrder::Dmy).unwrap();
    apply_to_tree(t, &cs);
}

fn err(t: &Tree, cmd: &EditCommand) -> ErrorCode {
    plan(t, cmd, NOW, DayOrder::Dmy).unwrap_err().code
}

fn parents(t: &Tree, child: Uuid) -> Vec<(Option<Uuid>, Option<Uuid>)> {
    t.families_as_child(child)
        .map(|(f, _)| (f.partner1, f.partner2))
        .collect()
}

#[test]
fn the_fathers_wife_as_mother_moves_the_child_into_their_marriage() {
    let mut t = tree_of(TREE);
    let (petro, hanna, ivan) = (find(&t, "Petro"), find(&t, "Hanna"), find(&t, "Ivan"));
    let before = t.families.len();
    apply(&mut t, &link(ivan, Relation::Mother, hanna));
    assert_eq!(parents(&t, ivan), vec![(Some(petro), Some(hanna))]);
    // The father-only family is gone: the couple is recorded once.
    assert_eq!(t.families.len(), before - 1);
    assert_eq!(t.families_as_partner(hanna).count(), 1);
}

#[test]
fn someone_not_yet_a_partner_fills_the_free_parent_place() {
    let mut t = tree_of(TREE);
    let (petro, ivan, oksana) = (find(&t, "Petro"), find(&t, "Ivan"), find(&t, "Oksana"));
    let before = t.families.len();
    apply(&mut t, &link(ivan, Relation::Mother, oksana));
    assert_eq!(parents(&t, ivan), vec![(Some(petro), Some(oksana))]);
    assert_eq!(t.families.len(), before);
}

#[test]
fn a_parent_for_someone_with_none_starts_a_family() {
    let mut t = tree_of(TREE);
    let (olena, oksana) = (find(&t, "Olena"), find(&t, "Oksana"));
    apply(&mut t, &link(olena, Relation::Mother, oksana));
    assert_eq!(parents(&t, olena), vec![(Some(oksana), None)]);
    assert!(t.families_as_child(olena).next().unwrap().1.preferred);
}

#[test]
fn an_existing_person_as_child_partner_and_sibling() {
    let mut t = tree_of(TREE);
    let (petro, ivan, olena, oksana, taras) = (
        find(&t, "Petro"),
        find(&t, "Ivan"),
        find(&t, "Olena"),
        find(&t, "Oksana"),
        find(&t, "Taras"),
    );
    apply(&mut t, &link(ivan, Relation::Partner, oksana));
    assert_eq!(t.families_as_partner(oksana).count(), 1);
    // Ivan now has two families: the child must name one.
    let fam = t
        .families_as_partner(ivan)
        .find(|f| f.children.iter().any(|c| c.person == taras))
        .unwrap()
        .common
        .id;
    assert_eq!(
        err(&t, &link(ivan, Relation::Child, olena)),
        ErrorCode::Invalid
    );
    apply(
        &mut t,
        &EditCommand::LinkExisting {
            anchor: ivan,
            relation: Relation::Child,
            person: olena,
            family: Some(fam),
            new_parent_set: None,
            parent_rel: None,
        },
    );
    assert_eq!(parents(&t, olena), vec![(Some(ivan), None)]);
    let mut t2 = tree_of(TREE);
    let (ivan2, oksana2) = (find(&t2, "Ivan"), find(&t2, "Oksana"));
    apply(&mut t2, &link(ivan2, Relation::Sibling, oksana2));
    assert_eq!(
        parents(&t2, oksana2),
        vec![(Some(find(&t2, "Petro")), None)]
    );
    let _ = petro;
}

#[test]
fn impossible_links_are_refused() {
    let t = tree_of(TREE);
    let (petro, hanna, ivan, taras) = (
        find(&t, "Petro"),
        find(&t, "Hanna"),
        find(&t, "Ivan"),
        find(&t, "Taras"),
    );
    assert_eq!(
        err(&t, &link(ivan, Relation::Father, ivan)),
        ErrorCode::Invalid
    );
    assert_eq!(
        err(&t, &link(ivan, Relation::Mother, taras)),
        ErrorCode::Invalid
    );
    assert_eq!(
        err(&t, &link(ivan, Relation::Father, petro)),
        ErrorCode::Conflict
    );
    assert_eq!(
        err(&t, &link(petro, Relation::Partner, hanna)),
        ErrorCode::Conflict
    );
    assert_eq!(
        err(&t, &link(taras, Relation::Child, petro)),
        ErrorCode::Invalid
    );
}

#[test]
fn undo_puts_the_father_only_family_back() {
    let t0 = tree_of(TREE);
    let (_dir, mut s) = open_tree(&t0);
    let (hanna, ivan) = (find(&t0, "Hanna"), find(&t0, "Ivan"));
    let before = s.tree().families.len();
    s.apply(link(ivan, Relation::Mother, hanna)).unwrap();
    assert_eq!(s.tree().families.len(), before - 1);
    s.undo().unwrap();
    assert_eq!(s.tree().families.len(), before);
    assert_eq!(
        parents(s.tree(), ivan),
        vec![(Some(find(&t0, "Petro")), None)]
    );
}
