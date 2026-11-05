#![allow(clippy::unwrap_used)]
//! Adding a step-parent (or an adoptive parent, or a stepchild): the birth links stay as they are.
mod common;

use common::*;
use gt_core::date::DayOrder;
use gt_core::model::*;
use gt_core::tree::Tree;
use gt_session::ops::{apply_to_tree, plan};
use gt_session::*;
use uuid::Uuid;

const NOW: i64 = 1_793_664_000_000;

/// Petro and Maria are Ivan's and Anna's parents; Olha raised Dmytro alone; Stepan, Nina and Taras have no links;
/// Halyna is already Stepan's wife.
const TREE: &str = "0 HEAD
1 GEDC
2 VERS 5.5.1
1 CHAR UTF-8
0 @I1@ INDI
1 NAME Petro /Koval/
1 SEX M
1 FAMS @F1@
0 @I2@ INDI
1 NAME Maria /Koval/
1 SEX F
1 FAMS @F1@
0 @I3@ INDI
1 NAME Ivan /Koval/
1 SEX M
1 FAMC @F1@
0 @I4@ INDI
1 NAME Anna /Koval/
1 SEX F
1 FAMC @F1@
0 @I5@ INDI
1 NAME Olha /Bondar/
1 SEX F
1 FAMS @F2@
1 FAMS @F3@
0 @I6@ INDI
1 NAME Dmytro /Bondar/
1 SEX M
1 FAMC @F2@
0 @I7@ INDI
1 NAME Stepan /Melnyk/
1 SEX M
1 FAMS @F3@
0 @I8@ INDI
1 NAME Nina /Shevchenko/
1 SEX F
0 @I9@ INDI
1 NAME Taras /Lysenko/
1 SEX M
0 @F1@ FAM
1 HUSB @I1@
1 WIFE @I2@
1 CHIL @I3@
1 CHIL @I4@
0 @F2@ FAM
1 WIFE @I5@
1 CHIL @I6@
0 @F3@ FAM
1 HUSB @I7@
1 WIFE @I5@
0 TRLR
";

fn link(anchor: Uuid, relation: Relation, person: Uuid, rel: ParentRelId) -> EditCommand {
    EditCommand::LinkExisting {
        anchor,
        relation,
        person,
        family: None,
        new_parent_set: None,
        parent_rel: Some(rel),
    }
}

fn add(anchor: Uuid, relation: Relation, rel: ParentRelId) -> EditCommand {
    EditCommand::QuickAdd {
        anchor,
        relation,
        draft: draft("New", SexKind::Unknown),
        family: None,
        new_parent_set: None,
        parent_rel: Some(rel),
    }
}

fn apply(t: &mut Tree, cmd: &EditCommand) -> Option<Uuid> {
    let (cs, focus) = plan(t, cmd, NOW, DayOrder::Dmy).unwrap();
    apply_to_tree(t, &cs);
    focus
}

/// The child's link to `parent`, and whether it is the preferred set.
fn rel_to(t: &Tree, child: Uuid, parent: Uuid) -> Option<(ParentRel, bool)> {
    t.families_as_child(child).find_map(|(f, l)| {
        if f.partner1 == Some(parent) {
            Some((l.rel1.clone(), l.preferred))
        } else if f.partner2 == Some(parent) {
            Some((l.rel2.clone(), l.preferred))
        } else {
            None
        }
    })
}

#[test]
fn a_stepfather_joins_the_mother_as_a_second_set_and_the_birth_parents_stay() {
    let mut t = tree_of(TREE);
    let (petro, maria, ivan, anna, taras) = (
        find(&t, "Petro"),
        find(&t, "Maria"),
        find(&t, "Ivan"),
        find(&t, "Anna"),
        find(&t, "Taras"),
    );
    apply(
        &mut t,
        &link(ivan, Relation::Father, taras, ParentRelId::Step),
    );
    let (rel, preferred) = rel_to(&t, ivan, taras).unwrap();
    assert_eq!(rel, ParentRel::Step);
    assert!(!preferred, "the birth parents stay the shown set");
    assert_eq!(rel_to(&t, ivan, petro).unwrap().0, ParentRel::Birth);
    assert!(rel_to(&t, ivan, petro).unwrap().1);
    // He is the mother's partner, and she is still Ivan's birth mother in that family too.
    let couple = t.families_as_partner(taras).next().unwrap();
    assert!(couple.partner1 == Some(maria) || couple.partner2 == Some(maria));
    assert_eq!(t.families_as_child(ivan).count(), 2);
    assert!(rel_to(&t, anna, taras).is_none(), "a sister is not assumed");
}

#[test]
fn a_stepmother_joins_the_father() {
    let mut t = tree_of(TREE);
    let (petro, ivan, nina) = (find(&t, "Petro"), find(&t, "Ivan"), find(&t, "Nina"));
    apply(
        &mut t,
        &link(ivan, Relation::Mother, nina, ParentRelId::Step),
    );
    assert_eq!(rel_to(&t, ivan, nina).unwrap().0, ParentRel::Step);
    let f = t.families_as_partner(nina).next().unwrap();
    assert!(f.partner1 == Some(petro) || f.partner2 == Some(petro));
}

#[test]
fn a_parent_recorded_alone_gets_the_step_parent_in_the_same_family_for_all_children() {
    let mut t = tree_of(TREE);
    let (olha, dmytro, taras) = (find(&t, "Olha"), find(&t, "Dmytro"), find(&t, "Taras"));
    let before = t.families_as_child(dmytro).count();
    apply(
        &mut t,
        &link(dmytro, Relation::Father, taras, ParentRelId::Step),
    );
    assert_eq!(t.families_as_child(dmytro).count(), before);
    assert_eq!(rel_to(&t, dmytro, taras).unwrap().0, ParentRel::Step);
    assert_eq!(rel_to(&t, dmytro, olha).unwrap().0, ParentRel::Birth);
}

#[test]
fn the_mothers_existing_husband_takes_the_child_into_that_couple() {
    let mut t = tree_of(TREE);
    let (olha, dmytro, stepan) = (find(&t, "Olha"), find(&t, "Dmytro"), find(&t, "Stepan"));
    let families = t.families.len();
    apply(
        &mut t,
        &link(dmytro, Relation::Father, stepan, ParentRelId::Step),
    );
    assert_eq!(t.families_as_child(dmytro).count(), 1);
    assert_eq!(rel_to(&t, dmytro, stepan).unwrap().0, ParentRel::Step);
    assert_eq!(rel_to(&t, dmytro, olha).unwrap().0, ParentRel::Birth);
    assert_eq!(t.families.len(), families - 1, "the family left empty goes");
}

#[test]
fn a_new_person_can_be_added_as_a_step_parent() {
    let mut t = tree_of(TREE);
    let ivan = find(&t, "Ivan");
    let new = apply(&mut t, &add(ivan, Relation::Father, ParentRelId::Step)).unwrap();
    assert_eq!(rel_to(&t, ivan, new).unwrap().0, ParentRel::Step);
    assert_eq!(t.persons[&new].sex, Sex::Male);
}

#[test]
fn adoptive_parents_form_their_own_set_and_the_second_joins_the_first() {
    let mut t = tree_of(TREE);
    let (ivan, taras, nina) = (find(&t, "Ivan"), find(&t, "Taras"), find(&t, "Nina"));
    apply(
        &mut t,
        &link(ivan, Relation::Father, taras, ParentRelId::Adopted),
    );
    assert_eq!(t.families_as_child(ivan).count(), 2);
    apply(
        &mut t,
        &link(ivan, Relation::Mother, nina, ParentRelId::Adopted),
    );
    assert_eq!(t.families_as_child(ivan).count(), 2, "she joins him");
    assert_eq!(rel_to(&t, ivan, nina).unwrap().0, ParentRel::Adopted);
    assert_eq!(rel_to(&t, ivan, taras).unwrap().0, ParentRel::Adopted);
}

#[test]
fn a_stepchild_is_the_step_child_of_the_anchor_and_the_birth_child_of_the_partner() {
    let mut t = tree_of(TREE);
    let (stepan, olha, taras) = (find(&t, "Stepan"), find(&t, "Olha"), find(&t, "Taras"));
    apply(
        &mut t,
        &link(stepan, Relation::Child, taras, ParentRelId::Step),
    );
    assert_eq!(rel_to(&t, taras, stepan).unwrap().0, ParentRel::Step);
    assert_eq!(rel_to(&t, taras, olha).unwrap().0, ParentRel::Birth);
}

#[test]
fn a_partner_or_sibling_cannot_be_linked_this_way() {
    let t = tree_of(TREE);
    let (ivan, nina) = (find(&t, "Ivan"), find(&t, "Nina"));
    let e = plan(
        &t,
        &link(ivan, Relation::Partner, nina, ParentRelId::Step),
        NOW,
        DayOrder::Dmy,
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::Invalid);
}

#[test]
fn someone_already_a_parent_is_refused() {
    let t = tree_of(TREE);
    let (ivan, petro) = (find(&t, "Ivan"), find(&t, "Petro"));
    let e = plan(
        &t,
        &link(ivan, Relation::Father, petro, ParentRelId::Step),
        NOW,
        DayOrder::Dmy,
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::Conflict);
}

#[test]
fn adding_a_stepfather_can_be_undone() {
    let t = tree_of(TREE);
    let (ivan, taras) = (find(&t, "Ivan"), find(&t, "Taras"));
    let (_dir, mut s) = open_tree(&t);
    s.apply(link(ivan, Relation::Father, taras, ParentRelId::Step))
        .unwrap();
    assert_eq!(s.tree().families_as_child(ivan).count(), 2);
    s.undo().unwrap();
    assert_eq!(s.tree().families_as_child(ivan).count(), 1);
    assert!(s.tree().families_as_partner(taras).next().is_none());
}
