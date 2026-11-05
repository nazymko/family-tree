#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use common::*;
use gt_core::date::{DateValue, DayOrder};
use gt_core::model::*;
use gt_core::tree::Tree;
use gt_session::ops::{apply_to_tree, places, plan};
use gt_session::*;
use gt_store::{ChangeSet, RecordKind};
use uuid::Uuid;

const NOW: i64 = 1_793_664_000_000;

fn run(tree: &Tree, cmd: &EditCommand) -> (ChangeSet, Option<Uuid>) {
    plan(tree, cmd, NOW, DayOrder::Dmy).unwrap()
}

fn run_apply(tree: &mut Tree, cmd: &EditCommand) -> ChangeSet {
    let (cs, _) = run(tree, cmd);
    apply_to_tree(tree, &cs);
    cs
}

fn err(tree: &Tree, cmd: &EditCommand) -> ApiError {
    plan(tree, cmd, NOW, DayOrder::Dmy).unwrap_err()
}

fn quick(anchor: Uuid, relation: Relation, name: &str) -> EditCommand {
    EditCommand::QuickAdd {
        anchor,
        relation,
        draft: draft(name, SexKind::Unknown),
        family: None,
        new_parent_set: None,
        parent_rel: None,
    }
}

fn ev_input(kind: &str, date: Option<&str>) -> EventInput {
    EventInput {
        id: None,
        kind: kind.into(),
        date_raw: date.map(str::to_string),
        place_text: None,
        place_id: None,
        description: None,
        age: None,
        cause: None,
        negative: false,
        private: false,
        preferred: true,
        participants: None,
    }
}

fn person_owner(id: Uuid) -> EventOwner {
    EventOwner {
        kind: EventOwnerKind::Person,
        id,
    }
}

fn owner(kind: OwnerKind, id: Uuid) -> OwnerRef {
    OwnerRef { kind, id }
}

fn sample() -> Tree {
    tree_of(SAMPLE)
}

#[test]
fn create_person_has_name_and_id() {
    let t = sample();
    let (cs, focus) = run(
        &t,
        &EditCommand::CreatePerson {
            draft: draft("Otto", SexKind::Male),
        },
    );
    let id = focus.unwrap();
    assert!(!id.is_nil());
    let mut t2 = t.clone();
    apply_to_tree(&mut t2, &cs);
    let p = &t2.persons[&id];
    assert_eq!(p.sex, Sex::Male);
    assert_eq!(p.names.len(), 1);
    assert_eq!(p.names[0].given.as_deref(), Some("Otto"));
    assert!(p.names[0].assertion.preferred);
    assert_eq!(cs.label, "Add person");
}

#[test]
fn quick_add_father_fills_empty_slot() {
    let mut t = sample();
    let hans = find(&t, "Hans");
    let fam = *t.families.keys().next().unwrap();
    t.families.get_mut(&fam).unwrap().partner1 = None;
    let (cs, focus) = run(&t, &quick(hans, Relation::Father, "Ludwig"));
    let new = focus.unwrap();
    apply_to_tree(&mut t, &cs);
    assert_eq!(t.families[&fam].partner1, Some(new));
    assert_eq!(t.persons[&new].sex, Sex::Male);
    assert_eq!(t.families.len(), 1);
}

#[test]
fn quick_add_father_without_parents_creates_family_and_preferred_link() {
    let mut t = sample();
    let eva = find(&t, "Eva");
    let (cs, focus) = run(&t, &quick(eva, Relation::Father, "Ludwig"));
    let new = focus.unwrap();
    apply_to_tree(&mut t, &cs);
    let (f, link) = t.families_as_child(eva).next().unwrap();
    assert_eq!(f.partner1, Some(new));
    assert_eq!(f.partner2, None);
    assert!(link.preferred);
    assert_eq!(
        (&link.rel1, &link.rel2),
        (&ParentRel::Birth, &ParentRel::Birth)
    );
}

#[test]
fn quick_add_father_when_full_conflicts_unless_new_parent_set() {
    let mut t = sample();
    let hans = find(&t, "Hans");
    assert_eq!(
        err(&t, &quick(hans, Relation::Father, "X")).code,
        ErrorCode::Conflict
    );
    let cmd = EditCommand::QuickAdd {
        anchor: hans,
        relation: Relation::Father,
        draft: draft("Y", SexKind::Unknown),
        family: None,
        new_parent_set: Some(true),
        parent_rel: None,
    };
    run_apply(&mut t, &cmd);
    let sets: Vec<_> = t.families_as_child(hans).collect();
    assert_eq!(sets.len(), 2);
    // The existing link stays the preferred one.
    assert_eq!(sets.iter().filter(|(_, l)| l.preferred).count(), 1);
}

#[test]
fn quick_add_child_single_family_default() {
    let mut t = sample();
    let anna = find(&t, "Anna");
    let (cs, focus) = run(&t, &quick(anna, Relation::Child, "Greta"));
    apply_to_tree(&mut t, &cs);
    let fam = t.families.values().next().unwrap();
    assert_eq!(fam.children.len(), 2);
    assert_eq!(fam.children.last().unwrap().person, focus.unwrap());
    assert_eq!(t.families.len(), 1);
}

#[test]
fn quick_add_child_multiple_families_requires_choice() {
    let mut t = sample();
    let anna = find(&t, "Anna");
    run_apply(&mut t, &quick(anna, Relation::Partner, "Fritz"));
    assert_eq!(t.families_as_partner(anna).count(), 2);
    let e = err(&t, &quick(anna, Relation::Child, "Greta"));
    assert_eq!(e.code, ErrorCode::Invalid);
    // Naming the family works.
    let second = t.families_as_partner(anna).nth(1).unwrap().common.id;
    let cmd = EditCommand::QuickAdd {
        anchor: anna,
        relation: Relation::Child,
        draft: draft("Greta", SexKind::Female),
        family: Some(second),
        new_parent_set: None,
        parent_rel: None,
    };
    run_apply(&mut t, &cmd);
    assert_eq!(t.families[&second].children.len(), 1);
}

#[test]
fn quick_add_child_without_family_creates_one() {
    let mut t = sample();
    let eva = find(&t, "Eva");
    run_apply(&mut t, &quick(eva, Relation::Child, "Kid"));
    let f = t.families_as_partner(eva).next().unwrap();
    assert_eq!(f.partner1, Some(eva));
    assert_eq!(f.partner2, None);
    assert_eq!(f.children.len(), 1);
}

#[test]
fn quick_add_partner_makes_family() {
    let mut t = sample();
    let eva = find(&t, "Eva");
    let (cs, focus) = run(&t, &quick(eva, Relation::Partner, "Fritz"));
    apply_to_tree(&mut t, &cs);
    let f = t.families_as_partner(eva).next().unwrap();
    assert_eq!((f.partner1, f.partner2), (Some(eva), focus));
    assert_eq!(f.kind, UnionKind::Unknown);
}

#[test]
fn quick_add_sibling_without_parents_creates_empty_family_with_both_children() {
    let mut t = sample();
    let eva = find(&t, "Eva");
    let (cs, focus) = run(&t, &quick(eva, Relation::Sibling, "Bruno"));
    apply_to_tree(&mut t, &cs);
    let (f, _) = t.families_as_child(eva).next().unwrap();
    assert_eq!((f.partner1, f.partner2), (None, None));
    let kids: Vec<_> = f.children.iter().map(|c| c.person).collect();
    assert_eq!(kids, vec![eva, focus.unwrap()]);
}

#[test]
fn quick_add_creates_birth_event_with_place_chain() {
    let mut t = sample();
    let eva = find(&t, "Eva");
    let mut d = draft("Kid", SexKind::Female);
    d.birth_date_raw = Some("3 Apr 1900".into());
    d.birth_place_text = Some("Berlin, Brandenburg, Germany".into());
    let cmd = |d: PersonDraft| EditCommand::QuickAdd {
        anchor: eva,
        relation: Relation::Child,
        draft: d,
        family: None,
        new_parent_set: None,
        parent_rel: None,
    };
    let before = t.places.len();
    let (cs, focus) = run(&t, &cmd(d.clone()));
    apply_to_tree(&mut t, &cs);
    assert_eq!(t.places.len(), before + 3);
    let kid = focus.unwrap();
    let (e, role) = t.events_of(kid).next().unwrap();
    assert_eq!(e.kind, EventKind::Birth);
    assert_eq!(*role, Role::Principal);
    let leaf = e.place.unwrap();
    assert_eq!(
        places::full_text(&t, leaf).unwrap(),
        "Berlin, Brandenburg, Germany"
    );
    // A second add reuses the chain.
    run_apply(&mut t, &cmd(d));
    assert_eq!(t.places.len(), before + 3);
}

#[test]
fn quick_add_is_one_changeset() {
    let t = sample();
    let eva = find(&t, "Eva");
    let mut d = draft("Kid", SexKind::Female);
    d.birth_place_text = Some("Hamburg".into());
    let (cs, _) = run(
        &t,
        &EditCommand::QuickAdd {
            anchor: eva,
            relation: Relation::Child,
            draft: d,
            family: None,
            new_parent_set: None,
            parent_rel: None,
        },
    );
    assert_eq!(cs.label, "Add child");
    let kinds: Vec<_> = cs.changes.iter().map(|c| c.kind).collect();
    for k in [
        RecordKind::Person,
        RecordKind::Family,
        RecordKind::Event,
        RecordKind::Place,
    ] {
        assert!(kinds.contains(&k), "{k:?} missing in {kinds:?}");
    }
}

#[test]
fn save_event_unparsed_date_kept_verbatim() {
    let mut t = sample();
    let eva = find(&t, "Eva");
    let cmd = EditCommand::SaveEvent {
        owner: person_owner(eva),
        event: ev_input("residence", Some("sometime in the 1850s?")),
    };
    run_apply(&mut t, &cmd);
    let (e, _) = t.events_of(eva).next().unwrap();
    let d = e.date.as_ref().unwrap();
    assert_eq!(d.raw, "sometime in the 1850s?");
    assert_eq!(d.value, DateValue::Unparsed);
    assert_eq!(e.kind, EventKind::Residence);
}

#[test]
fn save_event_mdy_vs_dmy() {
    let t = sample();
    let eva = find(&t, "Eva");
    let cmd = EditCommand::SaveEvent {
        owner: person_owner(eva),
        event: ev_input("birth", Some("03/04/1900")),
    };
    let key = |order| {
        let (cs, _) = plan(&t, &cmd, NOW, order).unwrap();
        let mut t2 = t.clone();
        apply_to_tree(&mut t2, &cs);
        let (e, _) = t2.events_of(eva).next().unwrap();
        e.date.as_ref().unwrap().sort_key().unwrap()
    };
    // 3 April versus 4 March: 30 days apart.
    assert_eq!(key(DayOrder::Dmy) - key(DayOrder::Mdy), 30);
}

#[test]
fn save_event_custom_kind_and_place_and_edit() {
    let mut t = sample();
    let eva = find(&t, "Eva");
    let mut input = ev_input("custom:apprenticeship", Some("1865"));
    input.place_text = Some("Berlin, Germany".into());
    let cs = run_apply(
        &mut t,
        &EditCommand::SaveEvent {
            owner: person_owner(eva),
            event: input,
        },
    );
    let ev_id = cs
        .changes
        .iter()
        .find(|c| c.kind == RecordKind::Event)
        .unwrap()
        .id;
    assert_eq!(
        t.events[&ev_id].kind,
        EventKind::Custom("apprenticeship".into())
    );
    // Editing without place keeps it; the date changes.
    let mut edit = ev_input("custom:apprenticeship", Some("1866"));
    edit.id = Some(ev_id);
    run_apply(
        &mut t,
        &EditCommand::SaveEvent {
            owner: person_owner(eva),
            event: edit,
        },
    );
    assert!(t.events[&ev_id].place.is_some());
    assert_eq!(t.events[&ev_id].date.as_ref().unwrap().raw, "1866");
    assert_eq!(t.events.len(), 2); // Anna's birth and this one
    // Unknown kinds are rejected.
    let bad = EditCommand::SaveEvent {
        owner: person_owner(eva),
        event: ev_input("zorp", None),
    };
    assert_eq!(err(&t, &bad).code, ErrorCode::Invalid);
}

#[test]
fn negative_event_flag() {
    let mut t = sample();
    let eva = find(&t, "Eva");
    let mut input = ev_input("death", None);
    input.negative = true;
    run_apply(
        &mut t,
        &EditCommand::SaveEvent {
            owner: person_owner(eva),
            event: input,
        },
    );
    let (e, _) = t.events_of(eva).next().unwrap();
    assert!(e.negative);
}

#[test]
fn family_event_uses_family_participant() {
    let mut t = sample();
    let fam = *t.families.keys().next().unwrap();
    run_apply(
        &mut t,
        &EditCommand::SaveEvent {
            owner: EventOwner {
                kind: EventOwnerKind::Family,
                id: fam,
            },
            event: ev_input("marriage", Some("1872")),
        },
    );
    let e = t
        .events
        .values()
        .find(|e| e.kind == EventKind::Marriage)
        .unwrap();
    assert_eq!(
        e.participants,
        vec![Participant {
            target: ParticipantRef::Family(fam),
            role: Role::Principal
        }]
    );
}

fn with_stuff() -> Tree {
    // Anna has a sole-principal birth (note + citation), a shared note, and shares a census with Karl.
    tree_of(
        "0 @S1@ SOUR\n1 TITL Register\n\
0 @N1@ NOTE Shared story\n\
0 @I1@ INDI\n1 NAME Anna /Schmidt/\n1 SEX F\n1 FAMS @F1@\n1 NOTE @N1@\n1 NOTE Only Anna\n\
1 BIRT\n2 DATE 1850\n2 SOUR @S1@\n3 PAGE 1\n\
0 @I2@ INDI\n1 NAME Karl /Schmidt/\n1 SEX M\n1 FAMS @F1@\n1 NOTE @N1@\n\
0 @I3@ INDI\n1 NAME Hans /Schmidt/\n1 SEX M\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 HUSB @I2@\n1 WIFE @I1@\n1 CHIL @I3@\n1 MARR\n2 DATE 1872\n",
    )
}

#[test]
fn delete_person_cleans_everything() {
    let mut t = with_stuff();
    let anna = find(&t, "Anna");
    let karl = find(&t, "Karl");
    let hans = find(&t, "Hans");
    let notes_before = t.notes.len();
    assert!(notes_before >= 2);
    assert_eq!(t.citations.len(), 1);
    // Make Karl a participant in Anna's birth too (not the principal).
    let birth = t
        .events
        .values()
        .find(|e| e.kind == EventKind::Birth)
        .unwrap()
        .common
        .id;
    t.events
        .get_mut(&birth)
        .unwrap()
        .participants
        .push(Participant {
            target: ParticipantRef::Person(karl),
            role: Role::Witness,
        });
    let shared = t.persons[&anna].common.notes[0];
    t.notes.get_mut(&shared).unwrap().shared = true;
    run_apply(&mut t, &EditCommand::DeletePerson { id: anna });
    assert!(!t.persons.contains_key(&anna));
    // Her birth (sole principal) is gone, with its citation and her private note.
    assert!(t.events.values().all(|e| e.kind != EventKind::Birth));
    assert!(t.citations.is_empty());
    assert_eq!(t.notes.len(), notes_before - 1);
    assert!(t.notes.contains_key(&shared), "shared note kept");
    // The family keeps Karl and Hans, with the slot emptied.
    let f = t.families.values().next().unwrap();
    assert_eq!(f.partner2, None);
    assert_eq!(f.partner1, Some(karl));
    assert_eq!(f.children.len(), 1);
    assert_eq!(f.children[0].person, hans);
    // Deleting the rest removes the family and its marriage event.
    run_apply(&mut t, &EditCommand::DeletePerson { id: karl });
    assert_eq!(t.families.len(), 1);
    run_apply(&mut t, &EditCommand::DeletePerson { id: hans });
    assert!(t.families.is_empty());
    assert!(t.events.is_empty());
}

#[test]
fn delete_person_removes_non_principal_participation() {
    let mut t = with_stuff();
    let anna = find(&t, "Anna");
    let karl = find(&t, "Karl");
    let birth = t
        .events
        .values()
        .find(|e| e.kind == EventKind::Birth)
        .unwrap()
        .common
        .id;
    t.events
        .get_mut(&birth)
        .unwrap()
        .participants
        .push(Participant {
            target: ParticipantRef::Person(karl),
            role: Role::Witness,
        });
    run_apply(&mut t, &EditCommand::DeletePerson { id: karl });
    let e = &t.events[&birth];
    assert_eq!(e.participants.len(), 1);
    assert_eq!(e.participants[0].target, ParticipantRef::Person(anna));
}

#[test]
fn delete_impact_counts() {
    let t = with_stuff();
    let (_dir, s) = open_tree(&t);
    let anna = find(&t, "Anna");
    let hans = find(&t, "Hans");
    let karl = find(&t, "Karl");
    // Anna: her birth event goes; the family survives.
    assert_eq!(
        s.delete_impact(anna).unwrap(),
        DeleteImpact {
            families: 0,
            events: 1
        }
    );
    assert_eq!(
        s.delete_impact(hans).unwrap(),
        DeleteImpact {
            families: 0,
            events: 0
        }
    );
    let _ = karl;
    assert!(s.delete_impact(Uuid::nil()).is_err());
}

#[test]
fn same_sex_family_saved() {
    let mut t = sample();
    let anna = find(&t, "Anna");
    let eva = find(&t, "Eva");
    let cs = run_apply(
        &mut t,
        &EditCommand::SaveFamily {
            id: None,
            partner1: Some(anna),
            partner2: Some(eva),
            kind: UnionKindId::CivilUnion,
        },
    );
    let f = cs
        .changes
        .iter()
        .find_map(|c| match &c.after {
            Some(gt_store::Record::Family(f)) => Some(f.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(f.kind, UnionKind::CivilUnion);
    assert_eq!((f.partner1, f.partner2), (Some(anna), Some(eva)));
    // Same person twice is invalid.
    let bad = EditCommand::SaveFamily {
        id: None,
        partner1: Some(anna),
        partner2: Some(anna),
        kind: UnionKindId::Unknown,
    };
    assert_eq!(err(&t, &bad).code, ErrorCode::Invalid);
}

#[test]
fn self_child_rejected() {
    let t = sample();
    let anna = find(&t, "Anna");
    let fam = *t.families.keys().next().unwrap();
    let link = EditCommand::LinkChild {
        family: fam,
        child: anna,
        rel: (ParentRelId::Birth, ParentRelId::Birth),
        preferred: false,
    };
    assert_eq!(err(&t, &link).code, ErrorCode::Invalid);
    // A partner who is a child of the family cannot be saved either.
    let hans = find(&t, "Hans");
    let save = EditCommand::SaveFamily {
        id: Some(fam),
        partner1: Some(hans),
        partner2: None,
        kind: UnionKindId::Unknown,
    };
    assert_eq!(err(&t, &save).code, ErrorCode::Invalid);
    // Already a child -> conflict.
    let dup = EditCommand::LinkChild {
        family: fam,
        child: hans,
        rel: (ParentRelId::Birth, ParentRelId::Birth),
        preferred: false,
    };
    assert_eq!(err(&t, &dup).code, ErrorCode::Conflict);
}

#[test]
fn reorder_children_changes_only_order() {
    let mut t = sample();
    let anna = find(&t, "Anna");
    run_apply(&mut t, &quick(anna, Relation::Child, "Greta"));
    let fam = *t.families.keys().next().unwrap();
    let mut order: Vec<Uuid> = t.families[&fam].children.iter().map(|c| c.person).collect();
    order.reverse();
    let (cs, _) = run(
        &t,
        &EditCommand::ReorderChildren {
            family: fam,
            order: order.clone(),
        },
    );
    assert_eq!(cs.changes.len(), 1);
    let before = t.families[&fam].clone();
    apply_to_tree(&mut t, &cs);
    let after = &t.families[&fam];
    let mut expect = before.clone();
    expect.children.reverse();
    assert_eq!(after, &expect);
    // Same order again changes nothing; a wrong list is invalid.
    let (cs2, _) = run(
        &t,
        &EditCommand::ReorderChildren {
            family: fam,
            order: order.clone(),
        },
    );
    assert!(cs2.changes.is_empty());
    let bad = EditCommand::ReorderChildren {
        family: fam,
        order: vec![order[0]],
    };
    assert_eq!(err(&t, &bad).code, ErrorCode::Invalid);
}

#[test]
fn child_link_commands() {
    let mut t = sample();
    let eva = find(&t, "Eva");
    let fam = *t.families.keys().next().unwrap();
    run_apply(
        &mut t,
        &EditCommand::LinkChild {
            family: fam,
            child: eva,
            rel: (ParentRelId::Adopted, ParentRelId::Foster),
            preferred: true,
        },
    );
    let l = t.families[&fam]
        .children
        .iter()
        .find(|c| c.person == eva)
        .unwrap()
        .clone();
    assert_eq!((l.rel1, l.rel2), (ParentRel::Adopted, ParentRel::Foster));
    run_apply(
        &mut t,
        &EditCommand::UpdateChildLink {
            family: fam,
            child: eva,
            rel: (ParentRelId::Birth, ParentRelId::Birth),
            preferred: false,
        },
    );
    assert!(
        !t.families[&fam]
            .children
            .iter()
            .find(|c| c.person == eva)
            .unwrap()
            .preferred
    );
    run_apply(
        &mut t,
        &EditCommand::UnlinkChild {
            family: fam,
            child: eva,
        },
    );
    assert!(t.families[&fam].children.iter().all(|c| c.person != eva));
    let missing = EditCommand::UnlinkChild {
        family: fam,
        child: eva,
    };
    assert_eq!(err(&t, &missing).code, ErrorCode::NotFound);
}

#[test]
fn names_save_and_delete() {
    let mut t = sample();
    let anna = find(&t, "Anna");
    let input = NameInput {
        id: None,
        kind: NameKindDto::Married,
        kind_text: None,
        preferred: true,
        private: false,
        prefix: None,
        given: Some("Anna".into()),
        call: None,
        surname_prefix: None,
        surnames: vec!["Meyer".into()],
        patronymic: None,
        suffix: None,
        nickname: None,
        lang: None,
        date_raw: Some("1875".into()),
    };
    run_apply(
        &mut t,
        &EditCommand::SaveName {
            person: anna,
            name: input.clone(),
        },
    );
    let p = t.persons[&anna].clone();
    assert_eq!(p.names.len(), 2);
    assert_eq!(p.names.iter().filter(|n| n.assertion.preferred).count(), 1);
    assert!(p.names[1].assertion.preferred);
    let first = p.names[0].id;
    run_apply(
        &mut t,
        &EditCommand::DeleteName {
            person: anna,
            name: p.names[1].id,
        },
    );
    assert!(t.persons[&anna].names[0].assertion.preferred);
    let last = EditCommand::DeleteName {
        person: anna,
        name: first,
    };
    assert_eq!(err(&t, &last).code, ErrorCode::Invalid);
    let empty = NameInput {
        given: None,
        surnames: vec![" ".into()],
        ..input
    };
    let bad = EditCommand::SaveName {
        person: anna,
        name: empty,
    };
    assert_eq!(err(&t, &bad).code, ErrorCode::Invalid);
}

#[test]
fn update_person_fields() {
    let mut t = sample();
    let eva = find(&t, "Eva");
    run_apply(
        &mut t,
        &EditCommand::UpdatePerson {
            id: eva,
            sex: Some(SexKind::Other),
            sex_text: Some("non-binary".into()),
            living_override: LivingPatch::Set(LivingOverrideDto::Deceased),
            private: Some(true),
        },
    );
    let p = &t.persons[&eva];
    assert_eq!(p.sex, Sex::Other("non-binary".into()));
    assert_eq!(p.living_override, Some(LivingOverride::Deceased));
    assert!(p.common.private);
    run_apply(
        &mut t,
        &EditCommand::UpdatePerson {
            id: eva,
            sex: None,
            sex_text: None,
            living_override: LivingPatch::Clear,
            private: None,
        },
    );
    assert_eq!(t.persons[&eva].living_override, None);
    assert!(t.persons[&eva].common.private);
}

fn add_cit(target: OwnerRef, arg: CitationArg) -> EditCommand {
    EditCommand::AddCitation {
        target,
        citation: arg,
    }
}

fn new_cit(title: &str, page: &str) -> CitationArg {
    CitationArg::New(CitationInput {
        source: SourceRef::New(NewSource {
            title: title.into(),
        }),
        page: Some(page.into()),
        date_raw: None,
        text: None,
        quality: Some(2),
        evidence: None,
    })
}

#[test]
fn reuse_last_citation_copies() {
    let mut t = sample();
    let eva = find(&t, "Eva");
    let anna = find(&t, "Anna");
    let cs = run_apply(
        &mut t,
        &add_cit(owner(OwnerKind::Person, eva), new_cit("Census", "p. 1")),
    );
    let first = cs
        .changes
        .iter()
        .find_map(|c| match &c.after {
            Some(gt_store::Record::Citation(x)) => Some(x.clone()),
            _ => None,
        })
        .unwrap();
    // Without a remembered citation the command is invalid.
    let reuse = add_cit(
        owner(OwnerKind::Person, anna),
        CitationArg::Reuse(ReuseLast { reuse_last: true }),
    );
    assert_eq!(err(&t, &reuse).code, ErrorCode::Invalid);
    let EditCommand::AddCitation { target, citation } = reuse else {
        unreachable!()
    };
    let (cs2, _) = gt_session::ops::plan_with(
        &t,
        &add_cit(target, citation),
        NOW,
        DayOrder::Dmy,
        Some(&first),
    )
    .unwrap();
    apply_to_tree(&mut t, &cs2);
    assert_eq!(t.citations.len(), 2);
    let copy = t
        .citations
        .values()
        .find(|c| c.common.id != first.common.id)
        .unwrap();
    assert_eq!(copy.page, first.page);
    assert_eq!(copy.source, first.source);
    assert_eq!(t.persons[&anna].common.citations, vec![copy.common.id]);
    // The source was created once.
    assert_eq!(t.sources.len(), 1);
}

#[test]
fn clone_citation_new_id() {
    let mut t = sample();
    let eva = find(&t, "Eva");
    let anna = find(&t, "Anna");
    run_apply(
        &mut t,
        &add_cit(owner(OwnerKind::Person, eva), new_cit("Census", "p. 1")),
    );
    let orig = *t.citations.keys().next().unwrap();
    run_apply(
        &mut t,
        &add_cit(
            owner(OwnerKind::Person, anna),
            CitationArg::Clone(CloneOf { clone_of: orig }),
        ),
    );
    assert_eq!(t.citations.len(), 2);
    let copy = t.persons[&anna].common.citations[0];
    assert_ne!(copy, orig);
    let (a, b) = (&t.citations[&copy], &t.citations[&orig]);
    assert_eq!(
        (&a.page, &a.source, a.quality),
        (&b.page, &b.source, b.quality)
    );
    // Removing the clone from Anna drops the now unreferenced record.
    run_apply(
        &mut t,
        &EditCommand::RemoveCitation {
            target: owner(OwnerKind::Person, anna),
            citation: copy,
        },
    );
    assert_eq!(t.citations.len(), 1);
}

#[test]
fn notes_sources_and_media() {
    let mut t = sample();
    let eva = find(&t, "Eva");
    let target = owner(OwnerKind::Person, eva);
    run_apply(
        &mut t,
        &EditCommand::SaveNote {
            target,
            note: NoteInput {
                id: None,
                kind: NoteKindDto::Story,
                text: "Hello".into(),
            },
        },
    );
    let nid = t.persons[&eva].common.notes[0];
    run_apply(
        &mut t,
        &EditCommand::SaveNote {
            target,
            note: NoteInput {
                id: Some(nid),
                kind: NoteKindDto::Research,
                text: "Changed".into(),
            },
        },
    );
    assert_eq!(t.notes[&nid].text, "Changed");
    assert_eq!(t.notes[&nid].kind, NoteKind::Research);
    let blank = EditCommand::SaveNote {
        target,
        note: NoteInput {
            id: None,
            kind: NoteKindDto::General,
            text: "  ".into(),
        },
    };
    assert_eq!(err(&t, &blank).code, ErrorCode::Invalid);
    run_apply(&mut t, &EditCommand::DeleteNote { target, note: nid });
    assert!(t.notes.is_empty());

    run_apply(
        &mut t,
        &EditCommand::SaveSource {
            source: SourceInput {
                id: None,
                title: Some("Parish book".into()),
                author: None,
                publication: None,
                abbreviation: Some("PB".into()),
                text: None,
            },
        },
    );
    assert_eq!(
        t.sources.values().next().unwrap().abbreviation.as_deref(),
        Some("PB")
    );

    // Media: attach (first is primary), set primary, detach.
    let mut m = |name: &str| {
        let media = Media {
            common: gt_core::model::Common {
                id: Uuid::now_v7(),
                private: false,
                changed: None,
                citations: vec![],
                notes: vec![],
                media: vec![],
                ext_ids: vec![],
                unknown: vec![],
            },
            path: name.into(),
            original_name: None,
            mime: None,
            title: None,
            date: None,
            description: None,
            sha256: None,
        };
        let id = media.common.id;
        t.media.insert(id, media);
        id
    };
    let (m1, m2) = (m("a.jpg"), m("b.jpg"));
    run_apply(
        &mut t,
        &EditCommand::AttachMedia {
            target,
            media: vec![m1, m2],
        },
    );
    let links = &t.persons[&eva].common.media;
    assert_eq!((links[0].primary, links[1].primary), (true, false));
    run_apply(
        &mut t,
        &EditCommand::SetPrimaryMedia {
            person: eva,
            media: m2,
        },
    );
    assert!(t.persons[&eva].common.media[1].primary);
    run_apply(&mut t, &EditCommand::DetachMedia { target, media: m2 });
    assert!(t.persons[&eva].common.media[0].primary);
    run_apply(
        &mut t,
        &EditCommand::UpdateMedia {
            id: m1,
            title: Some("Portrait".into()),
            description: None,
            date_raw: Some("abt 1878".into()),
        },
    );
    assert_eq!(t.media[&m1].title.as_deref(), Some("Portrait"));
    assert!(t.media[&m1].date.is_some());
}

#[test]
fn apply_to_tree_matches_plan_inverse() {
    let t = with_stuff();
    let anna = find(&t, "Anna");
    for cmd in [
        EditCommand::DeletePerson { id: anna },
        quick(anna, Relation::Child, "Greta"),
        EditCommand::CreatePerson {
            draft: draft("Otto", SexKind::Male),
        },
    ] {
        let (cs, _) = run(&t, &cmd);
        let mut t2 = t.clone();
        apply_to_tree(&mut t2, &cs);
        assert_ne!(t2, t);
        apply_to_tree(&mut t2, &cs.inverse());
        assert_eq!(t2, t, "{cmd:?}");
    }
}

#[test]
fn plan_does_not_mutate() {
    let t = sample();
    let copy = t.clone();
    let _ = run(
        &t,
        &EditCommand::DeletePerson {
            id: find(&t, "Anna"),
        },
    );
    assert_eq!(t, copy);
}

#[test]
fn edit_command_json_matches_ts_fixtures() {
    let json = include_str!("../../../app/src/lib/api/__fixtures__/commands.json");
    let cmds: Vec<EditCommand> = serde_json::from_str(json).unwrap();
    assert_eq!(cmds.len(), 26);
    // Absent and null differ for livingOverride.
    let EditCommand::UpdatePerson {
        living_override, ..
    } = &cmds[1]
    else {
        panic!()
    };
    assert_eq!(*living_override, LivingPatch::Clear);
    let absent: EditCommand = serde_json::from_str(
        r#"{"type":"updatePerson","id":"018f3a6e-0000-7000-8000-000000000001"}"#,
    )
    .unwrap();
    let EditCommand::UpdatePerson {
        living_override, ..
    } = absent
    else {
        panic!()
    };
    assert_eq!(living_override, LivingPatch::Keep);
    // Round trip through our own serialisation.
    let again: Vec<EditCommand> =
        serde_json::from_str(&serde_json::to_string(&cmds).unwrap()).unwrap();
    assert_eq!(again, cmds);
    assert!(matches!(
        &cmds[17],
        EditCommand::AddCitation {
            citation: CitationArg::New(_),
            ..
        }
    ));
}

fn koval(patronymic: Option<&str>) -> PersonDraft {
    PersonDraft {
        given: Some("Ivan".into()),
        surnames: vec!["Koval".into()],
        patronymic: patronymic.map(str::to_string),
        sex: SexKind::Male,
        birth_date_raw: None,
        birth_place_text: None,
    }
}

#[test]
fn create_person_stores_the_patronymic_as_a_name_part() {
    let mut t = sample();
    let cmd = EditCommand::CreatePerson {
        draft: koval(Some(" Petrovych ")),
    };
    let (cs, focus) = run(&t, &cmd);
    apply_to_tree(&mut t, &cs);
    let p = &t.persons[&focus.unwrap()];
    assert_eq!(p.names[0].given.as_deref(), Some("Ivan"));
    assert_eq!(p.names[0].patronymic.as_deref(), Some("Petrovych"));
    assert_eq!(p.names[0].surnames, vec!["Koval".to_string()]);
    assert_eq!(
        display_name(p, NameOrder::GivenFirst),
        "Ivan Petrovych Koval"
    );
}

#[test]
fn quick_add_stores_the_patronymic_and_a_blank_one_is_none() {
    let mut t = sample();
    let anchor = find(&t, "Hans");
    for (typed, stored) in [
        (Some("Petrovych"), Some("Petrovych")),
        (Some("  "), None),
        (None, None),
    ] {
        let cmd = EditCommand::QuickAdd {
            anchor,
            relation: Relation::Sibling,
            draft: koval(typed),
            family: None,
            new_parent_set: None,
            parent_rel: None,
        };
        let (cs, focus) = run(&t, &cmd);
        apply_to_tree(&mut t, &cs);
        assert_eq!(
            t.persons[&focus.unwrap()].names[0].patronymic.as_deref(),
            stored
        );
    }
}

#[test]
fn a_draft_without_the_patronymic_key_still_deserialises() {
    let d: PersonDraft = serde_json::from_str(r#"{"surnames":["Koval"],"sex":"male"}"#).unwrap();
    assert_eq!(d.patronymic, None);
}

#[test]
fn a_name_of_only_a_patronymic_can_be_saved() {
    // The add forms allow it, and the name editor has to be able to save what they made.
    let mut t = sample();
    let (cs, focus) = run(
        &t,
        &EditCommand::CreatePerson {
            draft: PersonDraft {
                given: None,
                surnames: vec![],
                patronymic: Some("Petrovych".into()),
                sex: SexKind::Unknown,
                birth_date_raw: None,
                birth_place_text: None,
            },
        },
    );
    apply_to_tree(&mut t, &cs);
    let id = focus.unwrap();
    let cmd = EditCommand::SaveName {
        person: id,
        name: NameInput {
            id: Some(t.persons[&id].names[0].id),
            kind: NameKindDto::Birth,
            kind_text: None,
            preferred: true,
            private: false,
            prefix: None,
            given: None,
            call: None,
            surname_prefix: None,
            surnames: vec![],
            patronymic: Some("Petrovych".into()),
            suffix: None,
            nickname: None,
            lang: None,
            date_raw: None,
        },
    };
    let (cs, _) = run(&t, &cmd);
    apply_to_tree(&mut t, &cs);
    assert_eq!(
        t.persons[&id].names[0].patronymic.as_deref(),
        Some("Petrovych")
    );
}
