#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Where a record of the change log is shown: the page to open for a person, a note, a citation.

mod common;

use common::*;
use gt_session::*;
use uuid::Uuid;

fn add_note(target: OwnerRef, text: &str) -> EditCommand {
    EditCommand::SaveNote {
        target,
        note: NoteInput {
            id: None,
            kind: NoteKindDto::General,
            text: text.into(),
        },
    }
}

fn birth(owner: EventOwner) -> EditCommand {
    EditCommand::SaveEvent {
        owner,
        event: EventInput {
            id: None,
            kind: "birth".into(),
            date_raw: Some("1 Jan 1900".into()),
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
    }
}

#[test]
fn records_are_located_on_the_page_that_shows_them() {
    let t = tree_of(SAMPLE);
    let eva = find(&t, "Eva");
    let (_d, mut s) = open_tree(&t);
    s.apply(EditCommand::QuickAdd {
        anchor: eva,
        relation: Relation::Partner,
        draft: draft("Marta", SexKind::Female),
        family: None,
        new_parent_set: None,
        parent_rel: None,
    })
    .unwrap();
    let family = *s.tree().families.keys().last().unwrap();
    let on_family = OwnerRef {
        kind: OwnerKind::Family,
        id: family,
    };
    s.apply(add_note(on_family, "On the family")).unwrap();
    let family_note = *s.tree().notes.keys().last().unwrap();
    s.apply(birth(EventOwner {
        kind: EventOwnerKind::Person,
        id: eva,
    }))
    .unwrap();
    let ev = *s.tree().events.keys().last().unwrap();
    let on_event = OwnerRef {
        kind: OwnerKind::Event,
        id: ev,
    };
    s.apply(add_note(on_event, "On the birth")).unwrap();
    let event_note = *s.tree().notes.keys().last().unwrap();
    s.apply(EditCommand::AddCitation {
        target: on_event,
        citation: CitationArg::New(CitationInput {
            source: SourceRef::New(NewSource {
                title: "Register".into(),
            }),
            page: None,
            date_raw: None,
            text: None,
            quality: None,
            evidence: None,
        }),
    })
    .unwrap();
    let cite = *s.tree().citations.keys().last().unwrap();
    let source = *s.tree().sources.keys().last().unwrap();
    s.apply(add_note(
        OwnerRef {
            kind: OwnerKind::Citation,
            id: cite,
        },
        "On the citation",
    ))
    .unwrap();
    let cite_note = *s.tree().notes.keys().last().unwrap();

    let r = |kind: &str, id: Uuid| RecordRefDto {
        kind: kind.into(),
        id,
    };
    let at =
        |page: LocationPage, id: Uuid, note: Option<Uuid>| Some(LocationDto { page, id, note });
    let gone = Uuid::from_u128(99);
    let got = s
        .locate_records(&[
            r("person", eva),
            r("family", family),
            r("event", ev),
            r("note", family_note),
            r("note", event_note),
            r("citation", cite),
            r("note", cite_note),
            r("source", source),
            r("note", gone),
            r("person", gone),
            r("place", eva),
        ])
        .unwrap();
    assert_eq!(
        got,
        vec![
            at(LocationPage::Person, eva, None),
            at(LocationPage::Family, family, None),
            at(LocationPage::Person, eva, None),
            at(LocationPage::Family, family, Some(family_note)),
            at(LocationPage::Person, eva, Some(event_note)),
            at(LocationPage::Source, source, None),
            at(LocationPage::Source, source, Some(cite_note)),
            at(LocationPage::Source, source, None),
            None,
            None,
            None,
        ]
    );
}
