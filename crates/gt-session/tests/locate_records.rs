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
    let at = |page: LocationPage, id: Uuid, kind: &str, shown: Uuid| {
        Some(LocationDto {
            page,
            id,
            target: RevealTarget {
                kind: kind.into(),
                id: shown,
            },
        })
    };
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
            at(LocationPage::Person, eva, "person", eva),
            at(LocationPage::Family, family, "family", family),
            at(LocationPage::Person, eva, "event", ev),
            at(LocationPage::Family, family, "note", family_note),
            // The page does not draw a note on an event: the event row carries it.
            at(LocationPage::Person, eva, "event", ev),
            // The source page lists no citations: the source it cites is the target.
            at(LocationPage::Source, source, "source", source),
            at(LocationPage::Source, source, "source", source),
            at(LocationPage::Source, source, "source", source),
            None,
            None,
            None,
        ]
    );
}

fn wav(ms: u32) -> Vec<u8> {
    let data = 16 * ms;
    let mut out = b"RIFF".to_vec();
    out.extend((36 + data).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(8000u32.to_le_bytes());
    out.extend(16_000u32.to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(data.to_le_bytes());
    out.extend((0..data / 2).flat_map(|i| (((i % 64) as i16 - 32) * 8).to_le_bytes()));
    out
}

fn on(kind: OwnerKind, id: Uuid) -> OwnerRef {
    OwnerRef { kind, id }
}

#[test]
fn the_target_is_the_nearest_record_the_page_draws() {
    let t = tree_of(SAMPLE);
    let eva = find(&t, "Eva");
    let (dir, mut s) = open_tree(&t);
    // A second name with a note.
    s.apply(EditCommand::SaveName {
        person: eva,
        name: NameInput {
            id: None,
            kind: NameKindDto::Aka,
            kind_text: None,
            preferred: false,
            private: false,
            prefix: None,
            given: Some("Evie".into()),
            call: None,
            surname_prefix: None,
            surnames: vec![],
            patronymic: None,
            suffix: None,
            nickname: None,
            lang: None,
            date_raw: None,
        },
    })
    .unwrap();
    let name = s.tree().persons[&eva].names.last().unwrap().id;
    s.apply(add_note(on(OwnerKind::Name, name), "On the name"))
        .unwrap();
    let name_note = *s.tree().notes.keys().last().unwrap();
    // A family with an event, and a recording attached to it whole.
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
    s.apply(birth(EventOwner {
        kind: EventOwnerKind::Family,
        id: family,
    }))
    .unwrap();
    let family_event = *s.tree().events.keys().last().unwrap();
    // A person's event carrying a recording.
    s.apply(birth(EventOwner {
        kind: EventOwnerKind::Person,
        id: eva,
    }))
    .unwrap();
    let event = *s.tree().events.keys().last().unwrap();
    // Each recording is made on a shelf source, linked where it is wanted, and taken off the shelf.
    let shelf = s
        .apply(EditCommand::SaveSource {
            source: SourceInput {
                id: None,
                title: Some("Shelf".into()),
                author: None,
                publication: None,
                abbreviation: None,
                text: None,
                ..Default::default()
            },
        })
        .unwrap()
        .touched[0];
    let mut length = 1000;
    let mut sound = |s: &mut Session, owner: Option<OwnerRef>| {
        length += 100;
        let file = dir.path().join(format!("talk{length}.wav"));
        std::fs::write(&file, wav(length)).unwrap();
        let attached = s
            .attach_files(on(OwnerKind::Source, shelf), &[file])
            .unwrap()
            .attached;
        let media = attached[0].id;
        if let Some(target) = owner {
            s.apply(EditCommand::AttachMedia {
                target,
                media: vec![media],
            })
            .unwrap();
            s.apply(EditCommand::DetachMedia {
                target: on(OwnerKind::Source, shelf),
                media,
            })
            .unwrap();
        }
        media
    };
    let whole_on_event = sound(&mut s, Some(on(OwnerKind::Event, event)));
    let whole_on_person = sound(&mut s, Some(on(OwnerKind::Person, eva)));
    let whole_on_family = sound(&mut s, Some(on(OwnerKind::Family, family)));
    // A recording of a source, with a stretch of it told on the person.
    let interview = s
        .apply(EditCommand::SaveSource {
            source: SourceInput {
                id: None,
                title: Some("Interview".into()),
                author: None,
                publication: None,
                abbreviation: None,
                text: None,
                ..Default::default()
            },
        })
        .unwrap()
        .touched[0];
    let clip_on_person = sound(&mut s, Some(on(OwnerKind::Source, interview)));
    s.apply(EditCommand::AddAudioClip {
        target: on(OwnerKind::Person, eva),
        media: clip_on_person,
        start_ms: 100,
        end_ms: 600,
        label: None,
    })
    .unwrap();

    let r = |kind: &str, id: Uuid| RecordRefDto {
        kind: kind.into(),
        id,
    };
    let got = s
        .locate_records(&[
            r("note", name_note),
            r("event", family_event),
            r("media", whole_on_event),
            r("media", whole_on_person),
            r("media", whole_on_family),
            r("media", clip_on_person),
        ])
        .unwrap();
    let shows = |page: LocationPage, id: Uuid, kind: &str, shown: Uuid| {
        Some(LocationDto {
            page,
            id,
            target: RevealTarget {
                kind: kind.into(),
                id: shown,
            },
        })
    };
    assert_eq!(
        got,
        vec![
            shows(LocationPage::Person, eva, "name", name),
            shows(LocationPage::Family, family, "event", family_event),
            // The event row shows what is attached to it.
            shows(LocationPage::Person, eva, "event", event),
            shows(LocationPage::Person, eva, "media", whole_on_person),
            // The family page draws only clips of recordings, not whole ones.
            shows(LocationPage::Family, family, "family", family),
            shows(LocationPage::Person, eva, "media", clip_on_person),
        ]
    );
}
