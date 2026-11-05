#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The unnamed-siblings marker: one entry per family standing for brothers and sisters nobody named.

mod common;

use common::*;
use gt_core::living::{LivingRule, compute_living};
use gt_session::*;
use uuid::Uuid;

/// Anna + Karl -> Hans (see `SAMPLE`); Hans has a sister Liese; Eva is alone.
const WITH_SISTER: &str = "0 @I1@ INDI\n1 NAME Anna /Schmidt/\n1 SEX F\n1 BIRT\n2 DATE 1850\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Karl /Schmidt/\n1 SEX M\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Hans /Schmidt/\n1 SEX M\n1 FAMC @F1@\n\
0 @I4@ INDI\n1 NAME Eva /Meyer/\n1 SEX F\n\
0 @I5@ INDI\n1 NAME Liese /Schmidt/\n1 SEX F\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 HUSB @I2@\n1 WIFE @I1@\n1 CHIL @I3@\n1 CHIL @I5@\n";

fn family_of(s: &Session, child: Uuid) -> Uuid {
    s.tree()
        .families
        .values()
        .find(|f| f.children.iter().any(|c| c.person == child))
        .unwrap()
        .common
        .id
}

fn add_marker(s: &mut Session, family: Uuid) -> Uuid {
    s.apply(EditCommand::AddSiblingMarker { family })
        .unwrap()
        .focus
        .unwrap()
}

fn markers(s: &Session) -> Vec<Uuid> {
    s.tree()
        .persons
        .values()
        .filter(|p| p.sibling_marker)
        .map(|p| p.common.id)
        .collect()
}

#[test]
fn marker_not_counted() {
    let (_d, mut s) = open_tree(&tree_of(WITH_SISTER));
    let before = s.info().person_count;
    let all = s.list_persons(&PersonQuery::default(), 0, 200);
    let fam = family_of(&s, find(s.tree(), "Hans"));
    let m = add_marker(&mut s, fam);
    assert_eq!(s.info().person_count, before);
    let page = s.list_persons(&PersonQuery::default(), 0, 200);
    assert_eq!(page.total, all.total);
    assert!(page.items.iter().all(|r| r.person.id != m));
    s.verify_derived_state().unwrap();
    // Filters that skip the plain fast path leave it out as well.
    let q = PersonQuery {
        no_source: true,
        ..PersonQuery::default()
    };
    assert!(
        s.list_persons(&q, 0, 200)
            .items
            .iter()
            .all(|r| r.person.id != m)
    );
}

#[test]
fn marker_survives_reload() {
    // A marker has no name, so no query could ever match it: what matters is that the saved
    // flag comes back (the store tests cover the search table itself).
    let (dir, mut s) = open_tree(&tree_of(WITH_SISTER));
    let fam = family_of(&s, find(s.tree(), "Hans"));
    let m = add_marker(&mut s, fam);
    s.close();
    let s = Session::open(
        &dir.path().join("project"),
        SessionClock::fixed(today(), NOW_MS),
    )
    .unwrap();
    assert_eq!(markers(&s), vec![m]);
    assert!(s.person_ref(m).unwrap().sibling_marker);
    s.verify_derived_state().unwrap();
}

#[test]
fn marker_not_exported_and_gedcom_valid() {
    let (dir, mut s) = open_tree(&tree_of(WITH_SISTER));
    let fam = family_of(&s, find(s.tree(), "Hans"));
    let opts = ExportOptions {
        version: ExportVersion::V551,
        living: LivingExport::All,
        include_private: false,
    };
    let export = |s: &Session, name: &str| {
        let p = s.preview_export(&opts).unwrap();
        let out = dir.path().join(name);
        s.export_gedcom(&opts, &out, &p.token).unwrap();
        (p, std::fs::read_to_string(out).unwrap())
    };
    let (p0, before) = export(&s, "before.ged");
    add_marker(&mut s, fam);
    let (p1, after) = export(&s, "after.ged");
    assert_eq!(p0.exported_persons, p1.exported_persons);
    assert_eq!(p0.hidden_persons, p1.hidden_persons);
    assert_eq!(
        before.matches(" INDI").count(),
        after.matches(" INDI").count()
    );
    assert_eq!(
        before.matches("1 CHIL").count(),
        after.matches("1 CHIL").count()
    );
    // Every pointer used in the file names a record that is in the file.
    let defined: Vec<&str> = after
        .lines()
        .filter(|l| l.starts_with("0 @"))
        .filter_map(|l| l.split_whitespace().nth(1))
        .collect();
    for l in after.lines() {
        if let Some(ptr) = l
            .strip_prefix("1 CHIL ")
            .or_else(|| l.strip_prefix("1 HUSB "))
            .or_else(|| l.strip_prefix("1 WIFE "))
        {
            assert!(defined.contains(&ptr.trim()), "dangling {ptr}");
        }
    }
    // And it reads back as the same people.
    let back = gt_gedcom::import(after.as_bytes(), &gt_gedcom::ImportOptions::default());
    assert_eq!(back.tree.persons.len(), s.info().person_count as usize);
}

#[test]
fn marker_not_in_kinship_or_living() {
    let (_d, mut s) = open_tree(&tree_of(WITH_SISTER));
    let hans = find(s.tree(), "Hans");
    let fam = family_of(&s, hans);
    let m = add_marker(&mut s, fam);
    assert!(s.kinship_between(hans, m).is_none());
    assert!(s.kinship_between(m, hans).is_none());
    assert!(s.kinship_between(hans, find(s.tree(), "Liese")).is_some());
    let living = compute_living(s.tree(), &LivingRule { max_age_years: 100 }, today());
    assert!(!living.contains_key(&m));
    assert_eq!(living.len(), s.tree().persons.len() - 1);
    // No dates and no name on the card the chart draws.
    let r = s.person_ref(m).unwrap();
    assert!(r.sibling_marker);
    assert_eq!(r.name, "");
    assert_eq!(r.lifespan, "");
}

#[test]
fn one_marker_per_family() {
    let (_d, mut s) = open_tree(&tree_of(WITH_SISTER));
    let fam = family_of(&s, find(s.tree(), "Hans"));
    add_marker(&mut s, fam);
    let e = s
        .apply(EditCommand::AddSiblingMarker { family: fam })
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Conflict);
    assert!(
        e.message.contains("unnamed brothers and sisters"),
        "{}",
        e.message
    );
    assert_eq!(markers(&s).len(), 1);
    let e = s
        .apply(EditCommand::AddSiblingMarker {
            family: Uuid::nil(),
        })
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn marker_undo_redo() {
    let (_d, mut s) = open_tree(&tree_of(WITH_SISTER));
    let fam = family_of(&s, find(s.tree(), "Hans"));
    let kids = s.tree().families[&fam].children.len();
    let r = s
        .apply(EditCommand::AddSiblingMarker { family: fam })
        .unwrap();
    assert_eq!(r.label, "Add unnamed brothers and sisters");
    let m = r.focus.unwrap();
    s.verify_derived_state().unwrap();
    assert_eq!(s.tree().families[&fam].children.len(), kids + 1);
    s.undo().unwrap().unwrap();
    assert!(markers(&s).is_empty());
    assert_eq!(s.tree().families[&fam].children.len(), kids);
    s.verify_derived_state().unwrap();
    s.redo().unwrap().unwrap();
    assert_eq!(markers(&s), vec![m]);
    s.verify_derived_state().unwrap();
    // The owner deletes it like anyone else; the family keeps its other children.
    s.apply(EditCommand::DeletePerson { id: m }).unwrap();
    assert!(markers(&s).is_empty());
    assert_eq!(s.tree().families[&fam].children.len(), kids);
    s.verify_derived_state().unwrap();
    s.undo().unwrap().unwrap();
    assert_eq!(markers(&s), vec![m]);
}

#[test]
fn marker_under_family_with_unknown_parents() {
    let ged = "0 @I1@ INDI\n1 NAME Hans /Schmidt/\n1 SEX M\n1 FAMC @F1@\n\
0 @I2@ INDI\n1 NAME Liese /Schmidt/\n1 SEX F\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 CHIL @I1@\n1 CHIL @I2@\n";
    let (_d, mut s) = open_tree(&tree_of(ged));
    let fam = family_of(&s, find(s.tree(), "Hans"));
    assert!(s.tree().families[&fam].partner1.is_none());
    let m = add_marker(&mut s, fam);
    let view = s.get_family(fam).unwrap();
    assert!(
        view.children
            .iter()
            .any(|c| c.child.person.id == m && c.child.person.sibling_marker)
    );
    assert_eq!(view.children.len(), 3);
    // The siblings of a child list it, last, flagged.
    let fv = s.family_view(find(s.tree(), "Hans")).unwrap();
    let last = fv.siblings.last().unwrap();
    assert_eq!(last.id, m);
    assert!(last.sibling_marker);
    // Deleting the marker leaves the parentless family in place.
    s.apply(EditCommand::DeletePerson { id: m }).unwrap();
    assert_eq!(s.tree().families[&fam].children.len(), 2);
    // A marker has no person page and no details to edit.
    let m = add_marker(&mut s, fam);
    assert_eq!(s.get_person(m).unwrap_err().code, ErrorCode::NotFound);
    let named = s
        .apply(EditCommand::SaveName {
            person: m,
            name: NameInput {
                id: None,
                kind: NameKindDto::Birth,
                kind_text: None,
                prefix: None,
                given: Some("Ghost".into()),
                call: None,
                surname_prefix: None,
                surnames: vec![],
                patronymic: None,
                suffix: None,
                nickname: None,
                lang: None,
                date_raw: None,
                preferred: true,
                private: false,
            },
        })
        .unwrap_err();
    assert_eq!(named.code, ErrorCode::Invalid);
    // Removing the child link removes the marker too, and undo brings both back.
    s.apply(EditCommand::UnlinkChild {
        family: fam,
        child: m,
    })
    .unwrap();
    assert!(markers(&s).is_empty());
    s.verify_derived_state().unwrap();
    s.undo().unwrap().unwrap();
    assert_eq!(markers(&s), vec![m]);
    s.apply(EditCommand::DeleteFamily { id: fam }).unwrap();
    assert!(markers(&s).is_empty());
}

#[test]
fn marker_belongs_to_its_one_family() {
    let (_d, mut s) = open_tree(&tree_of(SAMPLE));
    let fam = family_of(&s, find(s.tree(), "Hans"));
    let m = add_marker(&mut s, fam);
    let eva = find(s.tree(), "Eva");
    s.apply(EditCommand::SaveFamily {
        id: None,
        partner1: Some(eva),
        partner2: None,
        kind: UnionKindId::Married,
    })
    .unwrap();
    let eva_fam = s
        .tree()
        .families
        .values()
        .find(|f| f.partner1 == Some(eva))
        .unwrap()
        .common
        .id;
    let link = EditCommand::LinkChild {
        family: eva_fam,
        child: m,
        rel: (ParentRelId::Birth, ParentRelId::Birth),
        preferred: false,
    };
    assert_eq!(s.apply(link).unwrap_err().code, ErrorCode::Invalid);
    let partner = EditCommand::SaveFamily {
        id: Some(eva_fam),
        partner1: Some(eva),
        partner2: Some(m),
        kind: UnionKindId::Married,
    };
    assert_eq!(s.apply(partner).unwrap_err().code, ErrorCode::Invalid);
    let add = EditCommand::QuickAdd {
        anchor: m,
        relation: Relation::Child,
        draft: draft("Zed", SexKind::Unknown),
        family: None,
        new_parent_set: None,
        parent_rel: None,
    };
    assert_eq!(s.apply(add).unwrap_err().code, ErrorCode::Invalid);
    s.verify_derived_state().unwrap();
}

fn marker_of_hans(s: &mut Session) -> (Uuid, Uuid) {
    let hans = find(s.tree(), "Hans");
    let fam = family_of(s, hans);
    (fam, add_marker(s, fam))
}

fn owner(id: Uuid) -> OwnerRef {
    OwnerRef {
        kind: OwnerKind::Person,
        id,
    }
}

fn refused(s: &mut Session, cmd: EditCommand) {
    let before = s.tree().clone();
    let err = s.apply(cmd).unwrap_err();
    assert_eq!(err.code, ErrorCode::Invalid, "{}", err.message);
    assert_eq!(*s.tree(), before);
    s.verify_derived_state().unwrap();
}

#[test]
fn marker_is_not_a_chart_root_family_view_or_person() {
    let (_d, mut s) = open_tree(&tree_of(WITH_SISTER));
    let (_, m) = marker_of_hans(&mut s);
    for kind in [
        ChartKind::Pedigree,
        ChartKind::Descendants,
        ChartKind::Relatives,
    ] {
        let err = s
            .chart_open(&ChartRequest {
                kind,
                root: m,
                generations: 3,
                aunts_uncles: None,
            })
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        assert!(err.message.contains("unnamed brothers and sisters"));
    }
    let err = s.family_view(m).unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
    assert!(err.message.contains("unnamed brothers and sisters"));
    let err = s.get_person(m).unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
    assert!(err.message.contains("no details"));
    // Anyone else still opens.
    let hans = find(s.tree(), "Hans");
    s.family_view(hans).unwrap();
}

#[test]
fn whole_tree_count_leaves_markers_out() {
    let (_d, mut s) = open_tree(&tree_of(WITH_SISTER));
    let before = s.chart_open_whole().unwrap();
    let people: u32 = before.blocks.iter().map(|b| b.people).sum();
    marker_of_hans(&mut s);
    let after = s.chart_open_whole().unwrap();
    assert_eq!(after.meta.node_count, before.meta.node_count);
    assert_eq!(after.blocks.iter().map(|b| b.people).sum::<u32>(), people);
    assert_eq!(
        after.blocks.iter().map(|b| &b.title).collect::<Vec<_>>(),
        before.blocks.iter().map(|b| &b.title).collect::<Vec<_>>()
    );
    // The marker is still drawn: the layout has one more node than the people.
    let win = s
        .chart_window(
            after.meta.chart_id,
            &Rect {
                x: -1e6,
                y: -1e6,
                w: 2e6,
                h: 2e6,
            },
            1.0,
        )
        .unwrap();
    assert_eq!(win.nodes.len() as u32, after.meta.node_count + 1);
}

#[test]
fn marker_living_flag_is_false() {
    let (_d, mut s) = open_tree(&tree_of(WITH_SISTER));
    let (_, m) = marker_of_hans(&mut s);
    assert!(!s.person_ref(m).unwrap().living);
    // A person with no dates is living, so the flag is not simply false everywhere.
    assert!(s.person_ref(find(s.tree(), "Eva")).unwrap().living);
}

#[test]
fn marker_refuses_note() {
    let (_d, mut s) = open_tree(&tree_of(WITH_SISTER));
    let (_, m) = marker_of_hans(&mut s);
    refused(
        &mut s,
        EditCommand::SaveNote {
            target: owner(m),
            note: NoteInput {
                id: None,
                kind: NoteKindDto::General,
                text: "two more daughters".into(),
            },
        },
    );
}

#[test]
fn marker_refuses_citation() {
    let (_d, mut s) = open_tree(&tree_of(WITH_SISTER));
    let (_, m) = marker_of_hans(&mut s);
    refused(
        &mut s,
        EditCommand::AddCitation {
            target: owner(m),
            citation: CitationArg::New(CitationInput {
                source: SourceRef::New(NewSource {
                    title: "Parish book".into(),
                }),
                page: None,
                date_raw: None,
                text: None,
                quality: None,
                evidence: None,
            }),
        },
    );
}

/// A media record to point commands at (attached to Eva, who is a real person).
fn some_media(dir: &tempfile::TempDir, s: &mut Session) -> Uuid {
    let eva = find(s.tree(), "Eva");
    let file = dir.path().join("note.pdf");
    std::fs::write(&file, b"%PDF-1.4\n%fake body\n").unwrap();
    let r = s.attach_files(owner(eva), &[file]).unwrap();
    assert_eq!(r.attached.len(), 1);
    s.tree().media.keys().next().copied().unwrap()
}

#[test]
fn marker_refuses_media() {
    let (d, mut s) = open_tree(&tree_of(WITH_SISTER));
    let media = some_media(&d, &mut s);
    let (_, m) = marker_of_hans(&mut s);
    refused(
        &mut s,
        EditCommand::AttachMedia {
            target: owner(m),
            media: vec![media],
        },
    );
    refused(&mut s, EditCommand::SetPrimaryMedia { person: m, media });
    let file = d.path().join("more.pdf");
    std::fs::write(&file, b"%PDF-1.4\n%another body\n").unwrap();
    let before = s.tree().clone();
    assert!(s.attach_files(owner(m), &[file]).is_err());
    assert_eq!(*s.tree(), before);
}

#[test]
fn marker_refuses_import_draft_over_it() {
    let (_d, mut s) = open_tree(&tree_of(WITH_SISTER));
    let (_, m) = marker_of_hans(&mut s);
    refused(
        &mut s,
        EditCommand::ImportDraft {
            persons: vec![DraftPerson {
                key: "a".into(),
                existing: Some(m),
                given: None,
                surname: None,
                patronymic: None,
                sex: SexKind::Unknown,
                birth_date_raw: Some("1900".into()),
                death_date_raw: None,
                notes: None,
            }],
            families: vec![],
            attach_picture: false,
        },
    );
}

#[test]
fn marker_refuses_edits_names_and_events() {
    let (_d, mut s) = open_tree(&tree_of(WITH_SISTER));
    let (_, m) = marker_of_hans(&mut s);
    refused(
        &mut s,
        EditCommand::UpdatePerson {
            id: m,
            sex: Some(SexKind::Female),
            sex_text: None,
            living_override: LivingPatch::Keep,
            private: None,
        },
    );
    refused(
        &mut s,
        EditCommand::SaveName {
            person: m,
            name: NameInput {
                id: None,
                kind: NameKindDto::Birth,
                kind_text: None,
                preferred: true,
                private: false,
                prefix: None,
                given: Some("Zed".into()),
                call: None,
                surname_prefix: None,
                surnames: vec![],
                patronymic: None,
                suffix: None,
                nickname: None,
                lang: None,
                date_raw: None,
            },
        },
    );
    refused(
        &mut s,
        EditCommand::SaveEvent {
            owner: EventOwner {
                kind: EventOwnerKind::Person,
                id: m,
            },
            event: EventInput {
                id: None,
                kind: "birth".into(),
                date_raw: Some("1900".into()),
                place_text: None,
                place_id: None,
                description: None,
                age: None,
                cause: None,
                negative: false,
                private: false,
                preferred: true,
                participants: None,
            },
        },
    );
}

#[test]
fn family_left_with_only_a_marker_goes_with_its_last_member() {
    let ged = "0 @I1@ INDI\n1 NAME Hans /Schmidt/\n1 SEX M\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 CHIL @I1@\n";
    let (_d, mut s) = open_tree(&tree_of(ged));
    let hans = find(s.tree(), "Hans");
    let fam = family_of(&s, hans);
    let m = add_marker(&mut s, fam);
    s.apply(EditCommand::DeletePerson { id: hans }).unwrap();
    assert!(s.tree().families.is_empty(), "the family is gone");
    assert!(s.tree().persons.is_empty(), "and the marker with it");
    s.verify_derived_state().unwrap();
    s.undo().unwrap().unwrap();
    assert_eq!(s.tree().families[&fam].children.len(), 2);
    assert_eq!(markers(&s), vec![m]);
    s.verify_derived_state().unwrap();
}
