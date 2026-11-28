#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Cleaning up empty and parentless families: the list, the removal of a whole family and of one
//! child link, every refusal, and the undo. Invented people only.

mod common;

use common::*;
use gt_core::model::Family;
use gt_session::*;
use uuid::Uuid;

/// Hryhorii and Olha Bondar (F1) have Ivan, Maria and Petro. A parentless family (F2) holds the
/// same three children and a note. Stepan has a family with no children (F3, with a note), Yuriy
/// one with no note (F4), F5 has nobody. Orphan is in a parentless family (F6) and nowhere else.
/// Lesia and Bohdan are a real single-parent family (F8), Dmytro and Halyna a couple (F9), and
/// Taras is the one partner of a family with a marriage (F10).
const TREE: &str = "\
0 @I1@ INDI\n1 NAME Hryhorii /Bondar/\n1 SEX M\n1 BIRT\n2 DATE 1800\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Olha /Bondar/\n1 SEX F\n1 BIRT\n2 DATE 1805\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Ivan /Bondar/\n1 SEX M\n1 BIRT\n2 DATE 1830\n1 FAMC @F1@\n1 FAMC @F2@\n\
0 @I4@ INDI\n1 NAME Maria /Bondar/\n1 SEX F\n1 BIRT\n2 DATE 1832\n1 FAMC @F1@\n1 FAMC @F2@\n\
0 @I5@ INDI\n1 NAME Petro /Bondar/\n1 SEX M\n1 BIRT\n2 DATE 1835\n1 FAMC @F1@\n1 FAMC @F2@\n\
0 @I6@ INDI\n1 NAME Stepan /Koval/\n1 SEX M\n1 BIRT\n2 DATE 1840\n1 FAMS @F3@\n\
0 @I7@ INDI\n1 NAME Yuriy /Tkach/\n1 SEX M\n1 BIRT\n2 DATE 1841\n1 FAMS @F4@\n\
0 @I8@ INDI\n1 NAME Orphan /Shevchuk/\n1 SEX M\n1 BIRT\n2 DATE 1850\n1 FAMC @F6@\n\
0 @I9@ INDI\n1 NAME Lesia /Kravets/\n1 SEX F\n1 BIRT\n2 DATE 1860\n1 FAMS @F8@\n\
0 @I10@ INDI\n1 NAME Bohdan /Kravets/\n1 SEX M\n1 BIRT\n2 DATE 1885\n1 FAMC @F8@\n\
0 @I11@ INDI\n1 NAME Dmytro /Melnyk/\n1 SEX M\n1 BIRT\n2 DATE 1870\n1 FAMS @F9@\n\
0 @I12@ INDI\n1 NAME Halyna /Melnyk/\n1 SEX F\n1 BIRT\n2 DATE 1872\n1 FAMS @F9@\n\
0 @I13@ INDI\n1 NAME Taras /Lysenko/\n1 SEX M\n1 BIRT\n2 DATE 1870\n1 FAMS @F10@\n\
0 @I14@ INDI\n1 NAME Eva /Meyer/\n1 SEX F\n1 BIRT\n2 DATE 1870\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n1 CHIL @I4@\n1 CHIL @I5@\n\
0 @F2@ FAM\n1 CHIL @I3@\n1 CHIL @I4@\n1 CHIL @I5@\n1 NOTE The three are one sibling group.\n\
0 @F3@ FAM\n1 HUSB @I6@\n1 NOTE Stepan had a first wife, name unknown.\n\
0 @F4@ FAM\n1 HUSB @I7@\n\
0 @F5@ FAM\n1 NOTE Nobody in it.\n\
0 @F6@ FAM\n1 CHIL @I8@\n\
0 @F8@ FAM\n1 WIFE @I9@\n1 CHIL @I10@\n\
0 @F9@ FAM\n1 HUSB @I11@\n1 WIFE @I12@\n\
0 @F10@ FAM\n1 HUSB @I13@\n1 MARR\n2 DATE 1895\n";

fn me() -> AgentCtx {
    AgentCtx {
        id: Uuid::from_u128(21),
        name: "Family cleaner".into(),
    }
}

struct World {
    _dir: tempfile::TempDir,
    s: Session,
    batch: Uuid,
    f2: Uuid,
    f3: Uuid,
    f4: Uuid,
    f6: Uuid,
    f8: Uuid,
    f9: Uuid,
    f10: Uuid,
    ivan: Uuid,
    maria: Uuid,
    petro: Uuid,
    stepan: Uuid,
    orphan: Uuid,
    bohdan: Uuid,
    eva: Uuid,
}

fn family_where(t: &gt_core::tree::Tree, f: impl Fn(&Family) -> bool) -> Uuid {
    t.families.values().find(|x| f(x)).unwrap().common.id
}

fn world() -> World {
    let tree = tree_of(TREE);
    let id = |given: &str| find(&tree, given);
    let (ivan, maria, petro, stepan) = (id("Ivan"), id("Maria"), id("Petro"), id("Stepan"));
    let (orphan, bohdan, eva) = (id("Orphan"), id("Bohdan"), id("Eva"));
    let (yuriy, dmytro, taras) = (id("Yuriy"), id("Dmytro"), id("Taras"));
    let lesia = id("Lesia");
    let no_partners = |x: &Family| x.partner1.is_none() && x.partner2.is_none();
    let f2 = family_where(&tree, |x| no_partners(x) && x.children.len() == 3);
    let f3 = family_where(&tree, |x| x.partner1 == Some(stepan));
    let f4 = family_where(&tree, |x| x.partner1 == Some(yuriy));
    let f6 = family_where(&tree, |x| x.children.iter().any(|c| c.person == orphan));
    let f8 = family_where(&tree, |x| x.partner2 == Some(lesia));
    let f9 = family_where(&tree, |x| x.partner1 == Some(dmytro));
    let f10 = family_where(&tree, |x| x.partner1 == Some(taras));
    let (_dir, mut s) = open_tree(&tree);
    let batch = s.begin_batch(&me(), "Cleanup", None).unwrap();
    World {
        _dir,
        s,
        batch,
        f2,
        f3,
        f4,
        f6,
        f8,
        f9,
        f10,
        ivan,
        maria,
        petro,
        stepan,
        orphan,
        bohdan,
        eva,
    }
}

fn remove(
    w: &mut World,
    family: Uuid,
    notes_to: Option<Uuid>,
) -> Result<EmptyFamilyRemoval, ApiError> {
    w.s.remove_empty_family_as_agent(family, "Empty, nobody in it.", notes_to, &me(), w.batch)
        .map(|(r, _)| r)
}

fn refusal(w: &mut World, family: Uuid, notes_to: Option<Uuid>) -> String {
    let before = w.s.tree().clone();
    let err = remove(w, family, notes_to).unwrap_err();
    assert_eq!(err.code, ErrorCode::AgentRefused, "{err:?}");
    assert_eq!(*w.s.tree(), before, "a refusal changes nothing");
    err.message
}

#[test]
fn the_list_sorts_families_by_kind_and_says_what_blocks_them() {
    let w = world();
    let rows = w.s.list_empty_families(None).unwrap();
    // F1 (couple with children), F8 (single parent with a child) and F9 (couple) are no candidates.
    assert_eq!(
        rows.len(),
        6,
        "{:#?}",
        rows.iter().map(|r| r.id).collect::<Vec<_>>()
    );
    let row = |id: Uuid| rows.iter().find(|r| r.id == id).unwrap();

    let f2 = row(w.f2);
    assert_eq!(f2.kind, EmptyFamilyKind::Parentless);
    assert_eq!(f2.children.len(), 3);
    assert!(
        f2.children
            .iter()
            .all(|c| c.has_other_family && c.other_parent_families == 1)
    );
    assert_eq!(f2.notes, 1);
    assert!(f2.needs_notes_to && f2.removable, "{:?}", f2.blockers);
    assert_eq!(f2.suggested_notes_to, Some(w.ivan));
    assert!(f2.origin.is_none(), "an imported family has no log row");

    assert_eq!(row(w.f3).kind, EmptyFamilyKind::SinglePartner);
    assert_eq!(row(w.f3).suggested_notes_to, Some(w.stepan));
    assert!(!row(w.f4).needs_notes_to && row(w.f4).removable);

    let f6 = row(w.f6);
    assert!(!f6.removable);
    assert!(!f6.children[0].has_other_family);
    assert!(
        f6.blockers[0].contains("no parent family"),
        "{:?}",
        f6.blockers
    );

    let f10 = row(w.f10);
    assert_eq!(f10.events.len(), 1);
    assert!(!f10.removable && f10.blockers[0].contains("events"));

    let empty = rows
        .iter()
        .find(|r| r.kind == EmptyFamilyKind::Empty)
        .unwrap();
    assert!(
        !empty.removable,
        "a note with nobody to take it: {:?}",
        empty.blockers
    );

    let only =
        w.s.list_empty_families(Some(EmptyFamilyKind::SinglePartner))
            .unwrap();
    assert_eq!(only.len(), 3, "F3, F4 and F10");
    assert!(
        only.iter()
            .all(|r| r.kind == EmptyFamilyKind::SinglePartner)
    );
}

#[test]
fn the_list_names_who_made_a_family() {
    let mut w = world();
    let made =
        w.s.apply(EditCommand::SaveFamily {
            id: None,
            partner1: None,
            partner2: None,
            kind: UnionKindId::Unknown,
        })
        .unwrap()
        .touched[0];
    let rows =
        w.s.list_empty_families(Some(EmptyFamilyKind::Empty))
            .unwrap();
    let o = rows
        .iter()
        .find(|r| r.id == made)
        .unwrap()
        .origin
        .clone()
        .unwrap();
    assert_eq!(o.by, "owner");
    assert!(o.created);
    assert_eq!(o.at, NOW_MS);
}

#[test]
fn a_family_with_one_partner_and_nothing_else_goes() {
    let mut w = world();
    let f4 = w.f4;
    let out = remove(&mut w, f4, None).unwrap();
    assert_eq!(out.removed, f4);
    assert!(out.children_unlinked.is_empty());
    assert!(!w.s.tree().families.contains_key(&f4));
}

#[test]
fn notes_move_to_a_member_and_a_missing_or_foreign_one_is_refused() {
    let mut w = world();
    let (f3, stepan, eva) = (w.f3, w.stepan, w.eva);
    let note = w.s.tree().families[&f3].common.notes[0];
    let msg = refusal(&mut w, f3, None);
    assert!(msg.contains("notes_to"), "{msg}");
    let msg = refusal(&mut w, f3, Some(eva));
    assert!(msg.contains("partner or a child"), "{msg}");

    let out = remove(&mut w, f3, Some(stepan)).unwrap();
    assert_eq!((out.notes, out.moved_to), (1, Some(stepan)));
    let t = w.s.tree();
    assert!(!t.families.contains_key(&f3));
    assert!(
        t.persons[&stepan].common.notes.contains(&note),
        "the note is Stepan's now"
    );
    assert!(t.notes.contains_key(&note));
    assert!(
        t.persons[&stepan]
            .common
            .notes
            .iter()
            .any(|n| t.notes[n].text.contains("Empty, nobody in it."))
    );
}

#[test]
fn a_parentless_family_of_children_with_parents_goes_and_the_undo_brings_it_back() {
    let mut w = world();
    let before = w.s.tree().clone();
    let (f2, ivan) = (w.f2, w.ivan);
    let note = before.families[&f2].common.notes[0];
    let out = remove(&mut w, f2, Some(ivan)).unwrap();
    assert_eq!(out.children_unlinked.len(), 3);
    let t = w.s.tree();
    assert!(!t.families.contains_key(&f2));
    for child in [w.ivan, w.maria, w.petro] {
        assert_eq!(
            t.families_as_child(child).count(),
            1,
            "still in the parents' family"
        );
        assert!(t.persons[&child].common.notes.iter().any(|n| {
            t.notes[n]
                .text
                .contains("Removed from a family with no partners")
        }));
    }
    assert!(t.persons[&ivan].common.notes.contains(&note));

    let filter = LogFilterDto {
        batch: Some(w.batch),
        ..Default::default()
    };
    let rows = w.s.log(filter, 0, 10).unwrap();
    assert!(
        rows.items
            .iter()
            .any(|r| r.label == "Remove empty family: Empty, nobody in it."),
        "the reason is in the log label"
    );

    let report = w.s.undo_batch(w.batch).unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    let t = w.s.tree();
    assert_eq!(t.families[&f2].children.len(), 3, "the links are back");
    assert_eq!(t.families[&f2].common.notes, vec![note]);
    assert_eq!(
        t.persons[&ivan].common.notes,
        before.persons[&ivan].common.notes
    );
    assert_eq!(t.families.len(), before.families.len());
    assert_eq!(t.notes.len(), before.notes.len());
}

#[test]
fn every_real_family_is_refused() {
    let mut w = world();
    let (f6, f8, f9, f10) = (w.f6, w.f8, w.f9, w.f10);
    assert!(refusal(&mut w, f6, None).contains("no parent family"));
    assert!(refusal(&mut w, f8, None).contains("single-parent"));
    assert!(refusal(&mut w, f9, None).contains("two partners"));
    assert!(refusal(&mut w, f10, None).contains("events"));
    // F1, the real parents of three, has two partners.
    let f1 = family_where(w.s.tree(), |x| {
        x.partner1.is_some() && x.partner2.is_some() && x.children.len() == 3
    });
    assert!(refusal(&mut w, f1, None).contains("two partners"));
}

#[test]
fn the_reason_is_required_and_the_batch_must_be_open_and_the_agents_own() {
    let mut w = world();
    let (f4, batch) = (w.f4, w.batch);
    let err =
        w.s.remove_empty_family_as_agent(f4, "  ", None, &me(), batch)
            .unwrap_err();
    assert!(err.message.contains("reason"), "{err:?}");
    assert!(w.s.tree().families.contains_key(&f4));
    let other = AgentCtx {
        id: Uuid::from_u128(99),
        name: "Somebody else".into(),
    };
    assert!(
        w.s.remove_empty_family_as_agent(f4, "x", None, &other, batch)
            .is_err()
    );
    assert!(
        w.s.remove_empty_family_as_agent(f4, "x", None, &me(), Uuid::from_u128(5))
            .is_err()
    );
    assert!(
        w.s.remove_empty_family_as_agent(Uuid::from_u128(5), "x", None, &me(), batch)
            .is_err()
    );
}

#[test]
fn a_child_link_goes_only_when_the_child_keeps_a_family_with_a_parent() {
    let mut w = world();
    let (f2, f6, f8) = (w.f2, w.f6, w.f8);
    let (ivan, orphan, bohdan, petro) = (w.ivan, w.orphan, w.bohdan, w.petro);
    let go = |w: &mut World, f, c| {
        w.s.remove_child_from_family_as_agent(f, c, "Wrong group.", &me(), w.batch)
    };

    // The orphan and Bohdan have no other family; Ivan in F1 would keep only the parentless F2.
    for (f, c) in [(f6, orphan), (f8, bohdan)] {
        let err = go(&mut w, f, c).unwrap_err();
        assert_eq!(err.code, ErrorCode::AgentRefused);
        assert!(err.message.contains("no other family"), "{err:?}");
    }
    let f1 = family_where(w.s.tree(), |x| {
        x.partner1.is_some() && x.partner2.is_some() && x.children.len() == 3
    });
    assert!(go(&mut w, f1, ivan).is_err());
    // A person who is no child of the family.
    assert!(go(&mut w, f2, bohdan).is_err());
    let err =
        w.s.remove_child_from_family_as_agent(f2, ivan, "", &me(), w.batch)
            .unwrap_err();
    assert!(err.message.contains("reason"));

    go(&mut w, f2, ivan).unwrap();
    let t = w.s.tree();
    assert_eq!(t.families[&f2].children.len(), 2);
    assert_eq!(t.families_as_child(ivan).count(), 1);
    assert!(
        t.persons[&ivan]
            .common
            .notes
            .iter()
            .any(|n| t.notes[n].text.contains("Wrong group."))
    );
    assert!(t.persons.contains_key(&petro));

    w.s.undo_batch(w.batch).unwrap();
    assert_eq!(w.s.tree().families[&f2].children.len(), 3);
}

#[test]
fn the_owner_removes_one_and_ctrl_z_brings_it_back() {
    let mut w = world();
    let before = w.s.tree().clone();
    let (f2, ivan) = (w.f2, w.ivan);
    assert!(w.s.remove_empty_family_as_owner(w.f6, None).is_err());
    w.s.remove_empty_family_as_owner(f2, Some(ivan)).unwrap();
    assert!(!w.s.tree().families.contains_key(&f2));
    assert!(w.s.undo_state().can_undo);
    w.s.undo().unwrap().unwrap();
    assert_eq!(w.s.tree().families[&f2].children.len(), 3);
    assert_eq!(w.s.tree().families.len(), before.families.len());
    assert_eq!(w.s.tree().notes.len(), before.notes.len());
}

#[test]
fn an_empty_family_of_the_owner_goes_in_the_agents_batch() {
    let mut w = world();
    let made =
        w.s.apply(EditCommand::SaveFamily {
            id: None,
            partner1: None,
            partner2: None,
            kind: UnionKindId::Unknown,
        })
        .unwrap()
        .touched[0];
    let before = w.s.tree().clone();
    let out = remove(&mut w, made, None).unwrap();
    assert!(out.children_unlinked.is_empty() && out.moved_to.is_none());
    assert!(!w.s.tree().families.contains_key(&made));
    w.s.undo_batch(w.batch).unwrap();
    assert_eq!(w.s.tree().families.len(), before.families.len());
}
