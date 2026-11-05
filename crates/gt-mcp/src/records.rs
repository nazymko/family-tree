//! Every field of every record as JSON. The patterns name each field, with no `..`, so a new field
//! in the model stops this file compiling until a tool shows it.
//!
//! Records link to one another, even in circles. So a record read by id resolves its links one
//! level, and a linked row names what it links to by id, to be read with its own tool.

use gt_core::model::{
    AGENT_NUMBER_SCHEME, Assertion, ChildLink, Citation, Common, Event, EventKind, Evidence,
    ExtIdKind, ExternalId, Family, Media, MediaLink, Name, Note, NoteKind, Participant,
    ParticipantRef, Person, Place, PlaceName, Proof, RepoRef, Repository, Role, Source, UnionKind,
};
use gt_session::dates::date_dto;
use gt_session::person_detail::{event_kind_id, event_label, is_fact, rel_id, union_id};
use gt_session::{ApiError, Session};
use serde_json::{Map, Value, json};
use uuid::Uuid;

/// Bytes of a note's text past which a row shows only its start. Agents' clients limit what one
/// tool's answer can hold, and a note this long is better read by itself.
pub(crate) const NOTE_TEXT_MAX: usize = 16 * 1024;
/// Characters of such a note's text a row does show.
const NOTE_PREVIEW_CHARS: usize = 500;
/// Characters to ask `get_note` for at a time.
const NOTE_PIECE_CHARS: usize = 20_000;

type Fields = Map<String, Value>;

fn fields_of(value: Value) -> Fields {
    match value {
        Value::Object(fields) => fields,
        _ => Fields::new(),
    }
}

/// Every external id with its scheme, and for the agent's own numbers the file and number too.
fn numbers(ext_ids: &[ExternalId]) -> Vec<Value> {
    ext_ids
        .iter()
        .map(|ExternalId { kind, value }| match kind {
            ExtIdKind::Xref => json!({"scheme": "Xref", "value": value}),
            ExtIdKind::Uid => json!({"scheme": "Uid", "value": value}),
            ExtIdKind::FamilySearch => json!({"scheme": "FamilySearch", "value": value}),
            ExtIdKind::AncestryApid => json!({"scheme": "AncestryApid", "value": value}),
            ExtIdKind::Other(name) => json!({"scheme": "Other", "name": name, "value": value}),
            ExtIdKind::Exid { scheme } => match scheme.strip_prefix(AGENT_NUMBER_SCHEME) {
                Some(file) => json!({
                    "scheme": "Exid",
                    "name": scheme,
                    "value": value,
                    "sourceFile": file,
                    "number": value,
                }),
                None => json!({"scheme": "Exid", "name": scheme, "value": value}),
            },
        })
        .collect()
}

fn media_link_fields(link: &MediaLink) -> Fields {
    let MediaLink {
        media,
        region,
        primary,
    } = link;
    fields_of(json!({"media": media, "region": region, "primary": primary}))
}

/// What a linked row says about its record: links by id, and nothing empty.
fn common_by_id(common: &Common) -> Fields {
    let Common {
        id,
        private,
        changed,
        citations,
        notes,
        media,
        ext_ids,
        unknown,
    } = common;
    let links: Vec<Fields> = media.iter().map(media_link_fields).collect();
    let mut out = fields_of(json!({
        "id": id,
        "private": private,
        "changed": changed,
        "citationIds": citations,
        "noteIds": notes,
        "mediaLinks": links,
        "numbers": numbers(ext_ids),
        "unknown": unknown,
    }));
    out.retain(|_, v| v.as_array().is_none_or(|list| !list.is_empty()));
    out
}

/// What a record read by id says: its links resolved, each list present even when empty.
fn common_resolved(s: &Session, common: &Common) -> Fields {
    let Common {
        id,
        private,
        changed,
        citations,
        notes,
        media,
        ext_ids,
        unknown,
    } = common;
    fields_of(json!({
        "id": id,
        "private": private,
        "changed": changed,
        "citations": citation_rows(s, citations),
        "notes": note_rows(s, notes),
        "media": media_rows(s, media),
        "numbers": numbers(ext_ids),
        "unknown": unknown,
    }))
}

fn extend(mut base: Fields, more: Fields) -> Value {
    base.extend(more);
    Value::Object(base)
}

fn assertion_fields(assertion: &Assertion) -> Fields {
    let Assertion {
        preferred,
        proof,
        confidence,
    } = assertion;
    let proof = proof.as_ref().map(|p| match p {
        Proof::Proven => "proven",
        Proof::Disproven => "disproven",
    });
    fields_of(json!({"preferred": preferred, "proof": proof, "confidence": confidence}))
}

/// `(kind, kindText)` of a note.
fn note_kind(kind: &NoteKind) -> (&'static str, Option<&str>) {
    match kind {
        NoteKind::General => ("general", None),
        NoteKind::Story => ("story", None),
        NoteKind::Research => ("research", None),
        NoteKind::Transcription => ("transcription", None),
        NoteKind::Conclusion => ("conclusion", None),
        NoteKind::Other(text) => ("other", Some(text.as_str())),
    }
}

/// The sound recordings attached to a note: what they are, never their bytes.
fn voice_samples(s: &Session, common: &Common) -> Vec<Value> {
    common
        .media
        .iter()
        .filter_map(|link| s.tree().media.get(&link.media))
        .filter(|m| m.mime.as_deref().is_some_and(|t| t.starts_with("audio/")))
        .map(|m| {
            let file = s.media_original_path(m.common.id).ok();
            json!({
                "id": m.common.id,
                "title": m.title,
                "originalName": m.original_name,
                "mime": m.mime,
                "sizeBytes": file.and_then(|f| std::fs::metadata(f).ok()).map(|f| f.len()),
                "resolved": s.media_dtos(&[MediaLink { media: m.common.id, region: None, primary: false }])
                    .first()
                    .is_some_and(|d| d.media.resolved),
            })
        })
        .collect()
}

fn note_fields(s: &Session, note: &Note) -> Fields {
    let samples = voice_samples(s, &note.common);
    let Note {
        common: _,
        kind,
        text,
        lang,
        shared,
    } = note;
    let (kind, kind_text) = note_kind(kind);
    let mut out = fields_of(json!({
        "kind": kind,
        "kindText": kind_text,
        "text": text,
        "textLength": text.chars().count(),
        "lang": lang,
        "shared": shared,
    }));
    if !samples.is_empty() {
        out.insert("voiceSamples".into(), Value::Array(samples));
    }
    out
}

/// Leaves only the start of a very long text in `field` of a row, and how to read the rest.
fn cut_long_text(fields: &mut Fields, field: &str, what: &str, tool: &str, id: Uuid) {
    let long = fields
        .get(field)
        .and_then(Value::as_str)
        .is_some_and(|text| text.len() > NOTE_TEXT_MAX);
    if long {
        cut_text(fields, field, what, tool, id);
    }
}

/// Leaves only the start of the text in `field`, whatever its length, and how to read the rest.
pub(crate) fn cut_text(fields: &mut Fields, field: &str, what: &str, tool: &str, id: Uuid) {
    let Some(text) = fields.get(field).and_then(Value::as_str) else {
        return;
    };
    let preview: String = text.chars().take(NOTE_PREVIEW_CHARS).collect();
    fields.insert(field.into(), json!(preview));
    fields.insert(format!("{field}Truncated"), json!(true));
    fields.insert(
        format!("{field}Hint"),
        json!(format!(
            "Only the start of this {what}. Read the rest with {tool}, id {id}, text_offset \
             {NOTE_PREVIEW_CHARS}, text_limit {NOTE_PIECE_CHARS}."
        )),
    );
}

/// A note as a row in another record. A very long one comes as its start and how to read the rest.
pub(crate) fn note_row(s: &Session, note: &Note) -> Value {
    let mut fields = note_fields(s, note);
    cut_long_text(&mut fields, "text", "note", "get_note", note.common.id);
    extend(common_by_id(&note.common), fields)
}

/// A note read by id.
pub(crate) fn note_full(s: &Session, note: &Note) -> Value {
    extend(common_resolved(s, &note.common), note_fields(s, note))
}

/// A note in a list: the start of its text only.
pub(crate) fn note_list_row(s: &Session, note: &Note) -> Value {
    const PREVIEW_CHARS: usize = 200;
    let mut fields = note_fields(s, note);
    fields.insert(
        "text".into(),
        json!(note.text.chars().take(PREVIEW_CHARS).collect::<String>()),
    );
    extend(common_by_id(&note.common), fields)
}

fn note_rows(s: &Session, ids: &[Uuid]) -> Vec<Value> {
    ids.iter()
        .filter_map(|id| s.tree().notes.get(id).map(|n| note_row(s, n)))
        .collect()
}

fn source_fields(s: &Session, source: &Source) -> Fields {
    let Source {
        common,
        title,
        author,
        publication,
        abbreviation,
        text,
        repositories,
    } = source;
    let refs: Vec<Value> = repositories
        .iter()
        .map(
            |RepoRef {
                 repository,
                 call_number,
             }| {
                let name = s.tree().repositories.get(repository).map(|r| &r.name);
                json!({"id": repository, "name": name, "callNumber": call_number})
            },
        )
        .collect();
    let citation_count = s.source_citation_count(common.id);
    fields_of(json!({
        "title": title,
        "author": author,
        "publication": publication,
        "abbreviation": abbreviation,
        "textLength": text.as_ref().map(|t| t.chars().count()),
        "repositories": refs,
        "citationCount": citation_count,
    }))
}

/// A source without its text, which can be as long as a whole transcript. `get_source` has it.
pub(crate) fn source_row(s: &Session, source: &Source) -> Value {
    extend(common_by_id(&source.common), source_fields(s, source))
}

/// A source read by id, with its text.
pub(crate) fn source_full(s: &Session, source: &Source) -> Value {
    let mut fields = source_fields(s, source);
    fields.insert("text".into(), json!(source.text));
    extend(common_resolved(s, &source.common), fields)
}

fn citation_fields(s: &Session, citation: &Citation) -> Fields {
    let Citation {
        common: _,
        source,
        page,
        date,
        text,
        quality,
        evidence,
    } = citation;
    let Evidence {
        original,
        primary_info,
        direct,
    } = evidence;
    let source = source
        .and_then(|id| s.tree().sources.get(&id))
        .map(|found| source_row(s, found));
    fields_of(json!({
        "source": source,
        "page": page,
        "date": date.as_ref().map(date_dto),
        "text": text,
        "quality": quality,
        "evidence": {"original": original, "primaryInfo": primary_info, "direct": direct},
    }))
}

/// A citation as a row in another record or a list.
pub(crate) fn citation_row(s: &Session, citation: &Citation) -> Value {
    let mut fields = citation_fields(s, citation);
    let id = citation.common.id;
    cut_long_text(&mut fields, "text", "citation", "get_citation", id);
    extend(common_by_id(&citation.common), fields)
}

/// A citation read by id.
pub(crate) fn citation_full(s: &Session, citation: &Citation) -> Value {
    extend(
        common_resolved(s, &citation.common),
        citation_fields(s, citation),
    )
}

fn citation_rows(s: &Session, ids: &[Uuid]) -> Vec<Value> {
    ids.iter()
        .filter_map(|id| s.tree().citations.get(id))
        .map(|c| citation_row(s, c))
        .collect()
}

fn media_fields(s: &Session, media: &Media) -> Fields {
    let Media {
        common,
        path,
        original_name,
        mime,
        title,
        date,
        description,
        sha256,
    } = media;
    let link = MediaLink {
        media: common.id,
        region: None,
        primary: false,
    };
    let file = s.media_dtos(&[link]).pop().map(|dto| dto.media);
    fields_of(json!({
        "path": path,
        "originalName": original_name,
        "mime": mime,
        "title": title,
        "date": date.as_ref().map(date_dto),
        "description": description,
        "sha256": sha256,
        "resolved": file.as_ref().is_some_and(|f| f.resolved),
        "width": file.as_ref().and_then(|f| f.width),
        "height": file.as_ref().and_then(|f| f.height),
    }))
}

/// A media record as a row in a list.
pub(crate) fn media_row(s: &Session, media: &Media) -> Value {
    extend(common_by_id(&media.common), media_fields(s, media))
}

/// A media record read by id.
pub(crate) fn media_full(s: &Session, media: &Media) -> Value {
    extend(common_resolved(s, &media.common), media_fields(s, media))
}

/// The media a record links to, with what the link itself says.
fn media_rows(s: &Session, links: &[MediaLink]) -> Vec<Value> {
    links
        .iter()
        .filter_map(|link| {
            let record = s.tree().media.get(&link.media)?;
            let mut row = media_link_fields(link);
            row.insert("media".into(), media_row(s, record));
            Some(Value::Object(row))
        })
        .collect()
}

fn role_text(role: &Role) -> String {
    match role {
        Role::Principal => "principal".into(),
        Role::Witness => "witness".into(),
        Role::Godparent => "godparent".into(),
        Role::Clergy => "clergy".into(),
        Role::Officiator => "officiator".into(),
        Role::Informant => "informant".into(),
        Role::Parent => "parent".into(),
        Role::Father => "father".into(),
        Role::Mother => "mother".into(),
        Role::Child => "child".into(),
        Role::Spouse => "spouse".into(),
        Role::Husband => "husband".into(),
        Role::Wife => "wife".into(),
        Role::Friend => "friend".into(),
        Role::Neighbor => "neighbor".into(),
        Role::Multiple => "multiple".into(),
        Role::Other(text) => text.clone(),
    }
}

fn event_fields(s: &Session, event: &Event) -> Fields {
    let Event {
        common: _,
        kind,
        date,
        place,
        description,
        age,
        cause,
        negative,
        assertion,
        participants,
    } = event;
    let parties: Vec<Value> = participants
        .iter()
        .map(|Participant { target, role }| match target {
            ParticipantRef::Person(id) => {
                json!({"person": s.person_ref(*id), "personId": id, "role": role_text(role)})
            }
            ParticipantRef::Family(id) => json!({"family": id, "role": role_text(role)}),
        })
        .collect();
    let mut out = fields_of(json!({
        "kind": event_kind_id(kind),
        "label": event_label(kind),
        "isFact": is_fact(kind),
        "date": date.as_ref().map(date_dto),
        "place": place.and_then(|p| s.place_option(p)),
        "description": description,
        "age": age,
        "cause": cause,
        "negative": negative,
        "participants": parties,
    }));
    out.extend(assertion_fields(assertion));
    out
}

fn cut_description(fields: &mut Fields, id: Uuid) {
    cut_long_text(fields, "description", "description", "get_event", id);
}

/// An event in a list.
pub(crate) fn event_row(s: &Session, event: &Event) -> Value {
    let mut fields = event_fields(s, event);
    cut_description(&mut fields, event.common.id);
    extend(common_by_id(&event.common), fields)
}

/// An event as part of its person or family: all of it, but a very long description.
fn event_part(s: &Session, event: &Event) -> Value {
    let mut fields = event_fields(s, event);
    cut_description(&mut fields, event.common.id);
    extend(common_resolved(s, &event.common), fields)
}

/// An event read by id.
pub(crate) fn event_full(s: &Session, event: &Event) -> Value {
    extend(common_resolved(s, &event.common), event_fields(s, event))
}

pub(crate) fn event_terms(s: &Session, event: &Event) -> Vec<String> {
    let mut terms = vec![event_label(&event.kind)];
    if let EventKind::Custom(text) = &event.kind {
        terms.push(text.clone());
    }
    terms.extend(event.description.clone());
    terms.extend(event.cause.clone());
    terms.extend(
        event
            .place
            .and_then(|p| s.place_option(p))
            .map(|place| place.full),
    );
    terms
}

fn place_fields(s: &Session, place: &Place) -> Fields {
    let Place {
        common,
        names,
        parent,
        kind,
        lat,
        lon,
        raw,
    } = place;
    let names: Vec<Value> = names
        .iter()
        .map(|PlaceName { name, lang, date }| {
            json!({"name": name, "lang": lang, "date": date.as_ref().map(date_dto)})
        })
        .collect();
    let option = s.place_option(common.id);
    fields_of(json!({
        "name": option.as_ref().map(|o| &o.name),
        "full": option.as_ref().map(|o| &o.full),
        "names": names,
        "parent": parent.and_then(|p| s.place_option(p)),
        "kind": kind,
        "lat": lat,
        "lon": lon,
        "raw": raw,
    }))
}

/// A place in a list.
pub(crate) fn place_row(s: &Session, place: &Place) -> Value {
    extend(common_by_id(&place.common), place_fields(s, place))
}

/// A place read by id.
pub(crate) fn place_full(s: &Session, place: &Place) -> Value {
    extend(common_resolved(s, &place.common), place_fields(s, place))
}

fn repository_fields(repository: &Repository) -> Fields {
    let Repository {
        common: _,
        name,
        address,
        urls,
    } = repository;
    fields_of(json!({"name": name, "address": address, "urls": urls}))
}

/// A repository in a list.
pub(crate) fn repository_row(repository: &Repository) -> Value {
    extend(
        common_by_id(&repository.common),
        repository_fields(repository),
    )
}

/// A repository read by id, with the sources held there.
pub(crate) fn repository_full(s: &Session, repository: &Repository) -> Value {
    let id = repository.common.id;
    let sources: Vec<Value> = s
        .tree()
        .sources
        .values()
        .filter_map(|source| {
            let held = source.repositories.iter().find(|r| r.repository == id)?;
            Some(json!({
                "id": source.common.id,
                "title": source.title,
                "callNumber": held.call_number,
            }))
        })
        .collect();
    let mut fields = repository_fields(repository);
    fields.insert("sources".into(), json!(sources));
    extend(common_resolved(s, &repository.common), fields)
}

fn union_kind_text(kind: &UnionKind) -> Option<&str> {
    match kind {
        UnionKind::Married | UnionKind::CivilUnion | UnionKind::Unmarried | UnionKind::Unknown => {
            None
        }
        UnionKind::Other(text) => Some(text),
    }
}

/// What the family's `ChildLink` says beyond the row the session already makes.
fn child_link_fields(s: &Session, link: &ChildLink) -> Fields {
    let ChildLink {
        person: _,
        rel1,
        rel2,
        preferred,
        assertion,
        citations,
        unknown,
    } = link;
    let rel_text = |rel: &gt_core::model::ParentRel| match rel {
        gt_core::model::ParentRel::Other(text) => Some(text.clone()),
        _ => None,
    };
    fields_of(json!({
        "rel": [rel_id(rel1), rel_id(rel2)],
        "relText": [rel_text(rel1), rel_text(rel2)],
        "preferred": preferred,
        "assertion": assertion_fields(assertion),
        "citations": citation_rows(s, citations),
        "unknown": unknown,
    }))
}

fn is_marker(s: &Session, person: Uuid) -> bool {
    s.tree()
        .persons
        .get(&person)
        .is_some_and(|p| p.sibling_marker)
}

/// A family in a list.
pub(crate) fn family_row(s: &Session, family: &Family) -> Value {
    let Family {
        common,
        partner1,
        partner2,
        kind,
        children,
    } = family;
    let fields = fields_of(json!({
        "partners": [
            partner1.and_then(|p| s.person_ref(p)),
            partner2.and_then(|p| s.person_ref(p)),
        ],
        "kind": union_id(kind),
        "kindText": union_kind_text(kind),
        // The marker is no child: it says there are more that nobody named.
        "childCount": children.iter().filter(|c| !is_marker(s, c.person)).count(),
        "unnamedSiblings": children.iter().any(|c| is_marker(s, c.person)),
    }));
    extend(common_by_id(common), fields)
}

pub(crate) fn family_terms(s: &Session, family: &Family) -> Vec<String> {
    [family.partner1, family.partner2]
        .into_iter()
        .flatten()
        .filter_map(|p| s.person_ref(p).map(|r| r.name))
        .collect()
}

/// A family read by id: the session's view of it, plus what that view leaves out.
pub(crate) fn family_full(s: &Session, family: &Family) -> Result<Value, ApiError> {
    let Family {
        common,
        partner1: _,
        partner2: _,
        kind,
        children,
    } = family;
    let mut value = fields_of(to_value(&s.get_family(common.id)?)?);
    let events: Vec<Value> = value_ids(value.get("events"))
        .into_iter()
        .filter_map(|id| s.tree().events.get(&id))
        .map(|event| event_part(s, event))
        .collect();
    value.insert("events".into(), json!(events));
    value.insert("kindText".into(), json!(union_kind_text(kind)));
    if let Some(rows) = value.get_mut("children").and_then(Value::as_array_mut) {
        for row in rows {
            let id = row["person"]["id"].as_str().unwrap_or_default().to_string();
            let link = children.iter().find(|c| c.person.to_string() == id);
            if let (Some(link), Some(fields)) = (link, row.as_object_mut()) {
                fields.extend(child_link_fields(s, link));
            }
        }
    }
    Ok(extend(value, common_resolved(s, common)))
}

fn to_value(v: &impl serde::Serialize) -> Result<Value, ApiError> {
    serde_json::to_value(v)
        .map_err(|e| ApiError::new(gt_session::ErrorCode::Internal, e.to_string()))
}

/// The ids of a list of rows.
fn value_ids(rows: Option<&Value>) -> Vec<Uuid> {
    rows.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| row.get("id").and_then(Value::as_str))
        .filter_map(|id| Uuid::parse_str(id).ok())
        .collect()
}

/// What the session's name row leaves out, and what it shows in a lighter form.
fn name_fields(s: &Session, name: &Name) -> Fields {
    let Name {
        id: _,
        kind: _,
        assertion,
        private: _,
        prefix: _,
        given: _,
        call: _,
        surname_prefix: _,
        surnames: _,
        patronymic: _,
        suffix: _,
        nickname: _,
        lang: _,
        date: _,
        raw,
        citations,
        notes,
        unknown,
    } = name;
    let mut out = fields_of(json!({
        "raw": raw,
        "citations": citation_rows(s, citations),
        "notes": note_rows(s, notes),
        "unknown": unknown,
    }));
    out.extend(assertion_fields(assertion));
    out
}

/// A person read by id: the session's view, with names, events and links in full. The notes are
/// left for the caller to page. Partnerships keep their events short: `get_family` has the rest.
pub(crate) fn person_full(s: &Session, person: &Person) -> Result<Value, ApiError> {
    let Person {
        common,
        sex: _,
        names,
        living_override: _,
        sibling_marker,
    } = person;
    let id = common.id;
    let mut value = fields_of(to_value(&s.get_person(id)?)?);
    if let Some(rows) = value.get_mut("names").and_then(Value::as_array_mut) {
        for (row, name) in rows.iter_mut().zip(names) {
            if let Some(fields) = row.as_object_mut() {
                fields.extend(name_fields(s, name));
            }
        }
    }
    let events: Vec<Value> = value_ids(value.get("events"))
        .into_iter()
        .filter_map(|event| s.tree().events.get(&event))
        .map(|event| event_part(s, event))
        .collect();
    value.insert("events".into(), json!(events));
    if let Some(rows) = value.get_mut("partnerships").and_then(Value::as_array_mut) {
        for row in rows {
            let events = row.get_mut("events").and_then(Value::as_array_mut);
            for event in events.into_iter().flatten() {
                if let Some(fields) = event.as_object_mut() {
                    for heavy in ["citations", "notes", "media", "participants"] {
                        fields.remove(heavy);
                    }
                    let id = fields.get("id").and_then(Value::as_str);
                    if let Some(id) = id.and_then(|id| Uuid::parse_str(id).ok()) {
                        cut_description(fields, id);
                    }
                }
            }
        }
    }
    value.insert("living".into(), json!(s.person_ref(id).map(|r| r.living)));
    value.insert("siblingMarker".into(), json!(sibling_marker));
    Ok(extend(value, common_resolved(s, common)))
}
