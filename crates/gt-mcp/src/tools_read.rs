//! The read tools. Everything in the tree is readable, living people included: the agent runs on
//! the owner's machine with a key the owner made.

use crate::records;
use crate::{AgentCtx, Host, RULES, Tool};
use base64::Engine;
use gt_core::model::Person;
use gt_session::names::{BIRTH_KINDS, DEATH_KINDS, compose, pick_event};
use gt_session::{
    ApiError, ErrorCode, LogFilterDto, PersonQuery, Session, dates, index, render_px,
};
use gt_store::HitKind;
use serde::Serialize;
use serde_json::{Value, json};
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Mutex, PoisonError};
use uuid::Uuid;

const PAGE_DEFAULT: u64 = 50;
const PAGE_MAX: u64 = 200;
/// Each row of the log costs the session real time, and the session is the app's own.
const LOG_PAGE_MAX: u64 = 50;
/// The longest free text an argument may be. Searching costs far more than the length suggests.
pub(crate) const TEXT_ARG_MAX: usize = 1024;
/// Notes `get_person` returns unless asked for more.
const NOTES_DEFAULT: u64 = 20;
/// Bytes of note text after which `get_person` stops adding notes, so that a client's limit on a
/// tool's output is not reached by accident. One note is never more than this: a longer one comes
/// as a preview (`records::note_row`).
const NOTES_TEXT_BUDGET: usize = records::NOTE_TEXT_MAX;
/// Characters in a piece of text when `text_offset` is given without `text_limit`.
const TEXT_PIECE_DEFAULT: u64 = 20_000;
const PICTURE_DEFAULT_PX: u64 = 1600;
const PICTURE_MAX_PX: u64 = 4096;
const PICTURE_MIN_PX: u64 = 16;
/// A picture is decoded and shrunk, so its file may be bigger than what goes back.
const PICTURE_FILE_MAX: u64 = 256 * 1024 * 1024;
const BLOB_MAX: u64 = 8 * 1024 * 1024;
/// Decoding takes a lot of memory for a short while, so only one picture is decoded at a time.
static DECODING: Mutex<()> = Mutex::new(());

pub(crate) type ToolResult = Result<Vec<Value>, ApiError>;

pub(crate) fn tools<H: Host>() -> Vec<Tool<H>> {
    let id = |what: &str| {
        json!({
            "type": "object",
            "properties": {"id": {"type": "string", "description": what}},
            "required": ["id"],
            "additionalProperties": false,
        })
    };
    let piece_schema = |what: &str| {
        json!({
            "type": "object",
            "properties": {
                "id": {"type": "string", "description": what},
                "text_offset": {"type": "integer", "minimum": 0},
                "text_limit": {"type": "integer", "minimum": 1},
            },
            "required": ["id"],
            "additionalProperties": false,
        })
    };
    let paged_query = |query: &str, required: bool| {
        json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": query, "maxLength": TEXT_ARG_MAX},
                "offset": {"type": "integer", "minimum": 0},
                "limit": {"type": "integer", "minimum": 1, "maximum": PAGE_MAX, "default": PAGE_DEFAULT},
            },
            "required": if required { json!(["query"]) } else { json!([]) },
            "additionalProperties": false,
        })
    };
    vec![
        Tool {
            name: "get_rules",
            description: "The rules for working in this tree. Read them before anything else.",
            schema: no_args(),
            call: |_, _, _| Ok(vec![text(RULES)]),
        },
        Tool {
            name: "tree_summary",
            description: "The tree's name, how many records of each kind it has, and its home person.",
            schema: no_args(),
            call: tree_summary::<H>,
        },
        Tool {
            name: "search_people",
            description: "Finds people by any of their names, including similar spellings and sounds. Each row has years, parents and partners, to tell namesakes apart.",
            schema: paged_query("A name, or part of one.", true),
            call: search_people::<H>,
        },
        Tool {
            name: "list_people",
            description: "Everyone in the tree, one page at a time, with the same rows as search_people.",
            schema: json!({
                "type": "object",
                "properties": {
                    "offset": {"type": "integer", "minimum": 0},
                    "limit": {"type": "integer", "minimum": 1, "maximum": PAGE_MAX, "default": PAGE_DEFAULT},
                },
                "additionalProperties": false,
            }),
            call: list_people::<H>,
        },
        Tool {
            name: "get_person",
            description: "One person in full: all names, events, citations, media details, numbers and living status, and the first notes. A person with many notes gets some at a time: read on with notes_offset, as notesNextOffset says. A very long note comes as its start, with textTruncated and textHint: read it in pieces with get_note.",
            schema: json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "The person's id."},
                    "notes_offset": {"type": "integer", "minimum": 0},
                    "notes_limit": {"type": "integer", "minimum": 1, "maximum": PAGE_MAX, "default": NOTES_DEFAULT},
                },
                "required": ["id"],
                "additionalProperties": false,
            }),
            call: get_person::<H>,
        },
        Tool {
            name: "get_relatives",
            description: "A person's parents, partners, children and siblings as rows, with the id of the family linking each. A row with siblingMarker true is no person: it stands for brothers and sisters nobody named, and has no details.",
            schema: id("The person's id."),
            call: get_relatives::<H>,
        },
        Tool {
            name: "get_family",
            description: "One family: partners, children with how each is linked, events, notes, citations and media. A child with siblingMarker true is no person: it stands for brothers and sisters nobody named. It has no pages; a very long note in it comes as its start, to read in pieces with get_note.",
            schema: id("The family's id."),
            call: get_family::<H>,
        },
        Tool {
            name: "get_source",
            description: "One source in full, with its text and repositories. A long text can pass what your client shows at once: read it in pieces with text_offset and text_limit, in characters, as nextTextOffset says.",
            schema: piece_schema("The source's id."),
            call: get_source::<H>,
        },
        Tool {
            name: "get_note",
            description: "One note in full, with its links. A long text can pass what your client shows at once: read it in pieces with text_offset and text_limit, in characters, as nextTextOffset says.",
            schema: piece_schema("The note's id."),
            call: get_note::<H>,
        },
        Tool {
            name: "get_citation",
            description: "One citation with its links and its source. The source's text is left out: get_source has it. A long text of its own can pass what your client shows at once: read it in pieces with text_offset and text_limit, in characters, as nextTextOffset says. A very long note in it comes as its start, to read in pieces with get_note.",
            schema: piece_schema("The citation's id."),
            call: get_citation::<H>,
        },
        Tool {
            name: "get_event",
            description: "One event in full: date, place, everyone taking part, notes, citations and media. A long description can pass what your client shows at once: read it in pieces with text_offset and text_limit, in characters, as nextTextOffset says. A very long note in it comes as its start, to read in pieces with get_note.",
            schema: piece_schema("The event's id."),
            call: get_event::<H>,
        },
        Tool {
            name: "get_place",
            description: "One place in full: its names with their languages and dates, parent, coordinates, notes, citations and media. It has no pages; a very long note in it comes as its start, to read in pieces with get_note.",
            schema: id("The place's id."),
            call: get_place::<H>,
        },
        Tool {
            name: "get_repository",
            description: "One repository in full, with the sources held there and their call numbers. It has no pages; a very long note in it comes as its start, to read in pieces with get_note.",
            schema: id("The repository's id."),
            call: get_repository::<H>,
        },
        Tool {
            name: "list_sources",
            description: "Sources one page at a time, optionally those whose title, abbreviation or author contains the query. Read on with offset, as nextOffset says. The text of each is in get_source.",
            schema: paged_query("Text to look for.", false),
            call: list_sources::<H>,
        },
        Tool {
            name: "list_places",
            description: "Places one page at a time, optionally those whose name contains the query. Read on with offset, as nextOffset says.",
            schema: paged_query("Text to look for.", false),
            call: list_places::<H>,
        },
        Tool {
            name: "list_notes",
            description: "Notes one page at a time, attached to anything or to nothing, optionally those whose text contains the query. Read on with offset, as nextOffset says. Each row has the start of the text; get_note has the rest.",
            schema: paged_query("Text to look for.", false),
            call: list_notes::<H>,
        },
        Tool {
            name: "list_media",
            description: "Media records one page at a time, attached to anything or to nothing, optionally those whose title, file name, description or type contains the query. Read on with offset, as nextOffset says.",
            schema: paged_query("Text to look for.", false),
            call: list_media::<H>,
        },
        Tool {
            name: "list_families",
            description: "Families one page at a time, optionally those with a partner whose name contains the query. Read on with offset, as nextOffset says. unnamedSiblings says there are more children nobody named, beyond childCount.",
            schema: paged_query("Text to look for.", false),
            call: list_families::<H>,
        },
        Tool {
            name: "list_repositories",
            description: "Repositories one page at a time, optionally those whose name or address contains the query. Read on with offset, as nextOffset says.",
            schema: paged_query("Text to look for.", false),
            call: list_repositories::<H>,
        },
        Tool {
            name: "list_events",
            description: "Events one page at a time, attached to anyone or to no one, optionally those whose kind, description, cause or place contains the query. Read on with offset, as nextOffset says.",
            schema: paged_query("Text to look for.", false),
            call: list_events::<H>,
        },
        Tool {
            name: "list_citations",
            description: "Citations one page at a time, attached to anything or to nothing, optionally those whose page, text or source title contains the query. Read on with offset, as nextOffset says.",
            schema: paged_query("Text to look for.", false),
            call: list_citations::<H>,
        },
        Tool {
            name: "get_media",
            description: "A media file's details and the file itself: a picture as an image, shrunk to max_size pixels on its longest side, so a smaller max_size means a smaller answer; anything else, or a picture that cannot be decoded, as a blob of up to 8 MiB.",
            schema: json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "The media's id."},
                    "max_size": {"type": "integer", "minimum": PICTURE_MIN_PX, "maximum": PICTURE_MAX_PX, "default": PICTURE_DEFAULT_PX},
                },
                "required": ["id"],
                "additionalProperties": false,
            }),
            call: get_media::<H>,
        },
        Tool {
            name: "get_log",
            description: "The log of who changed what, newest first, up to 50 groups a page. Read on with offset, as nextOffset says. Filters, all optional: actorId, actorKind (user or agent), batch, person, unreviewedOnly.",
            schema: json!({
                "type": "object",
                "properties": {
                    "filters": {
                        "type": "object",
                        "properties": {
                            "actorId": {"type": "string"},
                            "actorKind": {"type": "string", "enum": ["user", "agent"]},
                            "batch": {"type": "string"},
                            "person": {"type": "string"},
                            "unreviewedOnly": {"type": "boolean"},
                        },
                        "additionalProperties": false,
                    },
                    "offset": {"type": "integer", "minimum": 0},
                    "limit": {"type": "integer", "minimum": 1, "maximum": LOG_PAGE_MAX, "default": LOG_PAGE_MAX},
                },
                "additionalProperties": false,
            }),
            call: get_log::<H>,
        },
        Tool {
            name: "get_history",
            description: "The changes made to one record, newest first, up to 50 a page. Read on with offset, as nextOffset says.",
            schema: json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "The record's id."},
                    "offset": {"type": "integer", "minimum": 0},
                    "limit": {"type": "integer", "minimum": 1, "maximum": LOG_PAGE_MAX, "default": LOG_PAGE_MAX},
                },
                "required": ["id"],
                "additionalProperties": false,
            }),
            call: get_history::<H>,
        },
        Tool {
            name: "find_by_number",
            description: "The person holding a number from a source file's own numbering, or null. A number is unique within one file only.",
            schema: json!({
                "type": "object",
                "properties": {
                    "source_file": {"type": "string", "maxLength": TEXT_ARG_MAX},
                    "number": {"type": "string", "maxLength": TEXT_ARG_MAX},
                },
                "required": ["source_file", "number"],
                "additionalProperties": false,
            }),
            call: find_by_number::<H>,
        },
    ]
}

/// The schema of a tool that takes nothing.
fn no_args() -> Value {
    json!({"type": "object", "additionalProperties": false})
}

pub(crate) fn read<H: Host, T>(
    host: &H,
    mut f: impl FnMut(&Session) -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    host.with_session(&mut |s| f(s))
}

fn text(s: &str) -> Value {
    json!({"type": "text", "text": s})
}

fn json_block(v: &impl Serialize) -> Result<Value, ApiError> {
    Ok(text(&serde_json::to_string(v).map_err(internal)?))
}

pub(crate) fn answer(v: &impl Serialize) -> ToolResult {
    Ok(vec![json_block(v)?])
}

fn internal(e: serde_json::Error) -> ApiError {
    ApiError::new(ErrorCode::Internal, e.to_string())
}

fn to_value(v: &impl Serialize) -> Result<Value, ApiError> {
    serde_json::to_value(v).map_err(internal)
}

pub(crate) fn invalid(message: String) -> ApiError {
    ApiError::new(ErrorCode::Invalid, message)
}

pub(crate) fn uuid_arg(args: &Value, name: &str) -> Result<Uuid, ApiError> {
    args.get(name)
        .and_then(Value::as_str)
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| invalid(format!("{name} must be the id of a record")))
}

/// Free text from the agent, which costs little to refuse and can cost a lot to search for.
pub(crate) fn checked_text<'a>(name: &str, text: &'a str) -> Result<&'a str, ApiError> {
    if text.len() > TEXT_ARG_MAX {
        return Err(invalid(format!("{name} is longer than 1 KiB. Shorten it.")));
    }
    Ok(text)
}

pub(crate) fn str_arg<'a>(args: &'a Value, name: &str) -> Result<&'a str, ApiError> {
    let text = args
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{name} must be text")))?;
    checked_text(name, text)
}

pub(crate) fn optional_str<'a>(args: &'a Value, name: &str) -> Result<Option<&'a str>, ApiError> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => str_arg(args, name).map(Some),
    }
}

fn optional_u64(args: &Value, name: &str) -> Result<Option<u64>, ApiError> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .map(Some)
            .ok_or_else(|| invalid(format!("{name} must be a whole number, 0 or more"))),
    }
}

#[derive(Clone, Copy)]
struct PageArgs {
    offset: usize,
    limit: usize,
}

fn page_args(args: &Value, most: u64) -> Result<PageArgs, ApiError> {
    let offset = optional_u64(args, "offset")?.unwrap_or(0);
    let limit = optional_u64(args, "limit")?
        .unwrap_or(PAGE_DEFAULT)
        .clamp(1, most);
    Ok(PageArgs {
        offset: usize::try_from(offset).unwrap_or(usize::MAX),
        limit: usize::try_from(limit).unwrap_or(1),
    })
}

fn paged(total: usize, page: PageArgs, items: Vec<Value>) -> Value {
    let end = page.offset.saturating_add(items.len());
    let mut out = json!({
        "total": total,
        "offset": page.offset,
        "limit": page.limit,
        "items": items,
    });
    if end < total
        && let Some(fields) = out.as_object_mut()
    {
        fields.insert("nextOffset".into(), json!(end));
    }
    out
}

fn slice<T>(all: Vec<T>, page: PageArgs) -> impl Iterator<Item = T> {
    all.into_iter().skip(page.offset).take(page.limit)
}

fn person(s: &Session, id: Uuid) -> Result<&Person, ApiError> {
    s.tree()
        .persons
        .get(&id)
        .ok_or_else(|| ApiError::not_found("person", id))
}

/// A stated year as the record has it ("c. 1880"), null when there is none.
fn event_year(s: &Session, id: Uuid, kinds: &[gt_core::model::EventKind]) -> Option<String> {
    pick_event(s.tree().events_of(id).map(|(e, _)| e), id, kinds)
        .and_then(|e| e.date.as_ref())
        .map(dates::stated_year)
        .filter(|y| y != "?")
}

/// The row that tells one person from another of the same name. The marker for unnamed brothers
/// and sisters has nothing to tell, and says what it is.
fn person_row(s: &Session, id: Uuid) -> Result<Value, ApiError> {
    let p = person(s, id)?;
    if p.sibling_marker {
        return Ok(json!({"id": id, "siblingMarker": true}));
    }
    let person_ref = s
        .person_ref(id)
        .ok_or_else(|| ApiError::not_found("person", id))?;
    let order = s.settings().name_order;
    let mut names: Vec<String> = Vec::new();
    for name in &p.names {
        let composed = compose(name, order, true);
        if !composed.is_empty() && !names.contains(&composed) {
            names.push(composed);
        }
    }
    let view = s.family_view(id)?;
    let parents: Vec<&str> = view
        .parent_sets
        .iter()
        .flat_map(|set| [&set.parents.0, &set.parents.1])
        .flatten()
        .map(|r| r.name.as_str())
        .collect();
    let partners: Vec<&str> = view
        .partnerships
        .iter()
        .filter_map(|f| f.partner.as_ref())
        .map(|r| r.name.as_str())
        .collect();
    Ok(json!({
        "id": id,
        "name": person_ref.name,
        "names": names,
        "birthYear": event_year(s, id, &BIRTH_KINDS),
        "deathYear": event_year(s, id, &DEATH_KINDS),
        "lifespan": person_ref.lifespan,
        "sex": person_ref.sex,
        "living": person_ref.living,
        "parents": parents,
        "partners": partners,
    }))
}

/// `person_row` plus the family that links this person to the one asked about.
fn relative_row(s: &Session, id: Uuid, family: Uuid) -> Result<Value, ApiError> {
    let mut row = person_row(s, id)?;
    if let Some(fields) = row.as_object_mut() {
        fields.insert("familyId".into(), json!(family));
    }
    Ok(row)
}

fn tree_summary<H: Host>(host: &H, _: &AgentCtx, _: &Value) -> ToolResult {
    let summary = read(host, |s| {
        let info = s.info();
        let tree = s.tree();
        Ok(json!({
            "name": info.name,
            "counts": {
                "people": tree.persons.values().filter(|p| !p.sibling_marker).count(),
                "families": tree.families.len(),
                "events": tree.events.len(),
                "places": tree.places.len(),
                "sources": tree.sources.len(),
                "repositories": tree.repositories.len(),
                "citations": tree.citations.len(),
                "media": tree.media.len(),
                "notes": tree.notes.len(),
            },
            "homePerson": info.home_person.and_then(|id| s.person_ref(id)),
        }))
    })?;
    answer(&summary)
}

fn search_people<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let query = str_arg(args, "query")?;
    let page = page_args(args, PAGE_MAX)?;
    let result = read(host, |s| {
        // The session's own search stops at 50 hits; here every hit can be paged to. The limit
        // is per kind of match, so this one reaches them all.
        let tree = s.tree();
        let everything = tree.persons.len()
            + tree.families.len()
            + tree.places.len()
            + tree.sources.len()
            + tree.notes.len()
            + tree.media.len()
            + 1;
        let hits = s
            .project()
            .search(query, everything)
            .map_err(ApiError::from)?;
        let mut seen = HashSet::new();
        let found: Vec<Uuid> = hits
            .into_iter()
            .filter(|h| h.kind == HitKind::Person && tree.persons.contains_key(&h.id))
            .map(|h| h.id)
            .filter(|id| seen.insert(*id))
            .collect();
        let total = found.len();
        let items = slice(found, page)
            .map(|id| person_row(s, id))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(paged(total, page, items))
    })?;
    answer(&result)
}

fn list_people<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let page = page_args(args, PAGE_MAX)?;
    let result = read(host, |s| {
        let rows = s.list_persons(&PersonQuery::default(), page.offset, page.limit);
        let items = rows
            .items
            .iter()
            .map(|r| person_row(s, r.person.id))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(paged(rows.total as usize, page, items))
    })?;
    answer(&result)
}

fn get_person<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let id = uuid_arg(args, "id")?;
    let offset = optional_u64(args, "notes_offset")?.unwrap_or(0);
    let limit = optional_u64(args, "notes_limit")?
        .unwrap_or(NOTES_DEFAULT)
        .clamp(1, PAGE_MAX);
    let offset = usize::try_from(offset).unwrap_or(usize::MAX);
    let limit = usize::try_from(limit).unwrap_or(1);
    let result = read(host, |s| {
        let p = person(s, id)?;
        let mut value = records::person_full(s, p)?;
        bound_notes(&mut value, offset, limit);
        Ok(value)
    })?;
    answer(&result)
}

/// Leaves one page of a person's notes, ended early by size, and says how to read the rest.
fn bound_notes(person: &mut Value, offset: usize, limit: usize) {
    let Some(fields) = person.as_object_mut() else {
        return;
    };
    let all = match fields.remove("notes") {
        Some(Value::Array(all)) => all,
        _ => Vec::new(),
    };
    let total = all.len();
    let mut page = Vec::new();
    let mut size = 0;
    for note in all.into_iter().skip(offset).take(limit) {
        if !page.is_empty() && size >= NOTES_TEXT_BUDGET {
            break;
        }
        size += note.get("text").and_then(Value::as_str).map_or(0, str::len);
        page.push(note);
    }
    let end = offset.saturating_add(page.len());
    fields.insert("notes".into(), json!(page));
    fields.insert("notesTotal".into(), json!(total));
    if end < total {
        fields.insert("notesNextOffset".into(), json!(end));
        fields.insert(
            "notesHint".into(),
            json!(format!(
                "{} more notes: call get_person with notes_offset {end}.",
                total - end
            )),
        );
    }
}

fn get_relatives<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let id = uuid_arg(args, "id")?;
    let result = read(host, |s| {
        let view = s.family_view(id)?;
        let mut parents = Vec::new();
        for set in &view.parent_sets {
            for parent in [&set.parents.0, &set.parents.1].into_iter().flatten() {
                parents.push(relative_row(s, parent.id, set.family_id)?);
            }
        }
        let mut partners = Vec::new();
        let mut children = Vec::new();
        for family in &view.partnerships {
            if let Some(partner) = &family.partner {
                partners.push(relative_row(s, partner.id, family.family_id)?);
            }
            for child in &family.children {
                children.push(relative_row(s, child.person.id, family.family_id)?);
            }
        }
        let mut siblings = Vec::new();
        for sibling in &view.siblings {
            // The first of this person's parent families that the sibling also belongs to.
            let shared = view.parent_sets.iter().map(|set| set.family_id).find(|f| {
                s.tree()
                    .families
                    .get(f)
                    .is_some_and(|fam| fam.children.iter().any(|c| c.person == sibling.id))
            });
            let mut row = person_row(s, sibling.id)?;
            row["familyId"] = json!(shared);
            siblings.push(row);
        }
        Ok(json!({
            "parents": parents,
            "partners": partners,
            "children": children,
            "siblings": siblings,
        }))
    })?;
    answer(&result)
}

fn get_family<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let id = uuid_arg(args, "id")?;
    let result = read(host, |s| {
        let family = s
            .tree()
            .families
            .get(&id)
            .ok_or_else(|| ApiError::not_found("family", id))?;
        records::family_full(s, family)
    })?;
    answer(&result)
}

/// Cuts a record's `text` to the piece asked for. Asked for nothing, the whole text stays.
fn text_piece(record: &mut Value, args: &Value) -> Result<(), ApiError> {
    piece_of(record, "text", args)
}

/// [`text_piece`], for the field that holds the record's long text.
fn piece_of(record: &mut Value, field: &str, args: &Value) -> Result<(), ApiError> {
    let offset = optional_u64(args, "text_offset")?;
    let limit = optional_u64(args, "text_limit")?;
    if offset.is_none() && limit.is_none() {
        return Ok(());
    }
    let offset = usize::try_from(offset.unwrap_or(0)).unwrap_or(usize::MAX);
    let limit = limit.unwrap_or(TEXT_PIECE_DEFAULT).max(1);
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let Some(text) = record.get(field).and_then(Value::as_str) else {
        return Ok(());
    };
    let piece: String = text.chars().skip(offset).take(limit).collect();
    let end = offset.saturating_add(limit);
    let more = text.chars().nth(end).is_some();
    if let Some(fields) = record.as_object_mut() {
        fields.insert(field.into(), json!(piece));
        fields.insert("textOffset".into(), json!(offset));
        if more {
            fields.insert("nextTextOffset".into(), json!(end));
        }
    }
    Ok(())
}

fn get_source<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let id = uuid_arg(args, "id")?;
    let mut result = read(host, |s| {
        s.tree()
            .sources
            .get(&id)
            .map(|source| records::source_full(s, source))
            .ok_or_else(|| ApiError::not_found("source", id))
    })?;
    text_piece(&mut result, args)?;
    answer(&result)
}

fn get_note<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let id = uuid_arg(args, "id")?;
    let mut result = read(host, |s| {
        s.tree()
            .notes
            .get(&id)
            .map(|note| records::note_full(s, note))
            .ok_or_else(|| ApiError::not_found("note", id))
    })?;
    text_piece(&mut result, args)?;
    answer(&result)
}

fn get_citation<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let id = uuid_arg(args, "id")?;
    let mut result = read(host, |s| {
        s.tree()
            .citations
            .get(&id)
            .map(|citation| records::citation_full(s, citation))
            .ok_or_else(|| ApiError::not_found("citation", id))
    })?;
    text_piece(&mut result, args)?;
    answer(&result)
}

fn get_event<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let id = uuid_arg(args, "id")?;
    let mut result = read(host, |s| {
        s.tree()
            .events
            .get(&id)
            .map(|event| records::event_full(s, event))
            .ok_or_else(|| ApiError::not_found("event", id))
    })?;
    piece_of(&mut result, "description", args)?;
    answer(&result)
}

fn get_place<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let id = uuid_arg(args, "id")?;
    let result = read(host, |s| {
        s.tree()
            .places
            .get(&id)
            .map(|place| records::place_full(s, place))
            .ok_or_else(|| ApiError::not_found("place", id))
    })?;
    answer(&result)
}

fn get_repository<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let id = uuid_arg(args, "id")?;
    let result = read(host, |s| {
        s.tree()
            .repositories
            .get(&id)
            .map(|repository| records::repository_full(s, repository))
            .ok_or_else(|| ApiError::not_found("repository", id))
    })?;
    answer(&result)
}

/// Text a list searches, its first entry also ordering the list.
type Terms<'a> = Vec<Cow<'a, str>>;

fn texts<'a>(fields: impl IntoIterator<Item = Option<&'a String>>) -> Terms<'a> {
    fields
        .into_iter()
        .map(|f| Cow::Borrowed(f.map_or("", String::as_str)))
        .collect()
}

/// Characters of a list's first term that order it, so that long notes cost little to sort.
const SORT_CHARS: usize = 64;

/// One page of a kind of record, optionally those with a term containing the query. In a stable
/// order, so that pages cover everything exactly once.
fn list_records<H: Host>(
    host: &H,
    args: &Value,
    terms: for<'a> fn(&'a Session) -> Vec<(Uuid, Terms<'a>)>,
    row: fn(&Session, Uuid) -> Option<Value>,
) -> ToolResult {
    let query = optional_str(args, "query")?.map(index::fold);
    let page = page_args(args, PAGE_MAX)?;
    let result = read(host, |s| {
        let mut found: Vec<(String, Uuid)> = Vec::new();
        for (id, record_terms) in terms(s) {
            let hit = query.as_deref().is_none_or(|q| {
                record_terms
                    .iter()
                    .any(|term| index::fold(term).contains(q))
            });
            if hit {
                let start: String = record_terms
                    .first()
                    .map(|t| t.chars().take(SORT_CHARS).collect())
                    .unwrap_or_default();
                found.push((index::fold(&start), id));
            }
        }
        found.sort();
        let total = found.len();
        let items = slice(found, page)
            .filter_map(|(_, id)| row(s, id))
            .collect();
        Ok(paged(total, page, items))
    })?;
    answer(&result)
}

fn list_sources<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    list_records(
        host,
        args,
        |s| {
            s.tree()
                .sources
                .iter()
                .map(|(id, r)| {
                    let terms =
                        texts([r.title.as_ref(), r.abbreviation.as_ref(), r.author.as_ref()]);
                    (*id, terms)
                })
                .collect()
        },
        |s, id| s.tree().sources.get(&id).map(|r| records::source_row(s, r)),
    )
}

fn list_places<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    list_records(
        host,
        args,
        |s| {
            s.tree()
                .places
                .iter()
                .map(|(id, r)| {
                    let full = s.place_option(*id).map(|o| o.full).unwrap_or_default();
                    let mut terms: Terms<'_> = vec![Cow::Owned(full)];
                    terms.extend(r.raw.as_deref().map(Cow::Borrowed));
                    terms.extend(r.names.iter().map(|n| Cow::Borrowed(n.name.as_str())));
                    (*id, terms)
                })
                .collect()
        },
        |s, id| s.tree().places.get(&id).map(|r| records::place_row(s, r)),
    )
}

fn list_notes<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    list_records(
        host,
        args,
        |s| {
            s.tree()
                .notes
                .iter()
                .map(|(id, r)| (*id, vec![Cow::Borrowed(r.text.as_str())]))
                .collect()
        },
        |s, id| s.tree().notes.get(&id).map(records::note_list_row),
    )
}

fn list_media<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    list_records(
        host,
        args,
        |s| {
            s.tree()
                .media
                .iter()
                .map(|(id, r)| {
                    let terms = texts([
                        r.title.as_ref(),
                        r.original_name.as_ref(),
                        r.description.as_ref(),
                        r.mime.as_ref(),
                    ]);
                    (*id, terms)
                })
                .collect()
        },
        |s, id| s.tree().media.get(&id).map(|r| records::media_row(s, r)),
    )
}

fn list_families<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    list_records(
        host,
        args,
        |s| {
            s.tree()
                .families
                .iter()
                .map(|(id, r)| {
                    let names = records::family_terms(s, r);
                    (*id, names.into_iter().map(Cow::Owned).collect())
                })
                .collect()
        },
        |s, id| {
            s.tree()
                .families
                .get(&id)
                .map(|r| records::family_row(s, r))
        },
    )
}

fn list_repositories<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    list_records(
        host,
        args,
        |s| {
            s.tree()
                .repositories
                .iter()
                .map(|(id, r)| (*id, texts([Some(&r.name), r.address.as_ref()])))
                .collect()
        },
        |s, id| s.tree().repositories.get(&id).map(records::repository_row),
    )
}

fn list_events<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    list_records(
        host,
        args,
        |s| {
            s.tree()
                .events
                .iter()
                .map(|(id, r)| {
                    let terms = records::event_terms(s, r);
                    (*id, terms.into_iter().map(Cow::Owned).collect())
                })
                .collect()
        },
        |s, id| s.tree().events.get(&id).map(|r| records::event_row(s, r)),
    )
}

fn list_citations<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    list_records(
        host,
        args,
        |s| {
            s.tree()
                .citations
                .iter()
                .map(|(id, r)| {
                    let title = r
                        .source
                        .and_then(|source| s.tree().sources.get(&source))
                        .and_then(|source| source.title.as_ref());
                    (*id, texts([title, r.page.as_ref(), r.text.as_ref()]))
                })
                .collect()
        },
        |s, id| {
            s.tree()
                .citations
                .get(&id)
                .map(|r| records::citation_row(s, r))
        },
    )
}

fn get_media<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let id = uuid_arg(args, "id")?;
    let max_size = optional_u64(args, "max_size")?
        .unwrap_or(PICTURE_DEFAULT_PX)
        .clamp(PICTURE_MIN_PX, PICTURE_MAX_PX);
    // The file is read and decoded after the session is let go, so the app stays responsive.
    let (details, mime, path) = read(host, |s| {
        let media = s
            .tree()
            .media
            .get(&id)
            .ok_or_else(|| ApiError::not_found("media", id))?;
        let mime = media.mime.clone().unwrap_or_default();
        Ok((
            records::media_full(s, media),
            mime,
            s.media_original_path(id).ok(),
        ))
    })?;
    let block = match path {
        None => text("The file is missing from the project folder."),
        Some(path) if mime.starts_with("image/") => picture_block(&path, max_size, id, &mime),
        Some(path) => blob_block(&path, id, &mime),
    };
    Ok(vec![json_block(&details)?, block])
}

/// Runs `f` while no other picture is being decoded.
fn one_decode_at_a_time<T>(f: impl FnOnce() -> T) -> T {
    let _one = DECODING.lock().unwrap_or_else(PoisonError::into_inner);
    f()
}

fn picture_block(path: &Path, max_size: u64, id: Uuid, mime: &str) -> Value {
    let too_big = std::fs::metadata(path).is_ok_and(|m| m.len() > PICTURE_FILE_MAX);
    if too_big {
        return text(&format!(
            "This picture's file is over {} MiB, too large to decode here. Its details are above.",
            PICTURE_FILE_MAX / (1024 * 1024)
        ));
    }
    let shrunk = one_decode_at_a_time(|| {
        std::fs::read(path)
            .map_err(|_| ())
            .and_then(|bytes| render_px(&bytes, max_size as u32).map_err(|_| ()))
    });
    match shrunk {
        Ok(jpeg) => json!({
            "type": "image",
            "data": base64::engine::general_purpose::STANDARD.encode(jpeg),
            "mimeType": "image/jpeg",
        }),
        // Not a picture after all, or one too demanding to decode: the agent gets the file as is.
        Err(()) => blob_block(path, id, mime),
    }
}

fn blob_block(path: &Path, id: Uuid, mime: &str) -> Value {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(u64::MAX);
    if size > BLOB_MAX {
        return text("The file is larger than 8 MiB, so it is not sent here.");
    }
    match std::fs::read(path) {
        Ok(bytes) => json!({
            "type": "resource",
            "resource": {
                "uri": format!("gene-tree://media/{id}"),
                "mimeType": if mime.is_empty() { "application/octet-stream" } else { mime },
                "blob": base64::engine::general_purpose::STANDARD.encode(bytes),
            },
        }),
        Err(_) => text("The file could not be read."),
    }
}

fn get_log<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let page = page_args(args, LOG_PAGE_MAX)?;
    let filter: LogFilterDto = match args.get("filters") {
        None | Some(Value::Null) => LogFilterDto::default(),
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|e| invalid(format!("filters are not valid: {e}")))?,
    };
    if let Some(kind) = &filter.actor_kind {
        checked_text("actorKind", kind)?;
    }
    let result = read(host, |s| {
        let rows = s.log(filter.clone(), page.offset, page.limit)?;
        let items = rows.items.iter().map(to_value).collect::<Result<_, _>>()?;
        Ok(paged(rows.total as usize, page, items))
    })?;
    answer(&result)
}

fn get_history<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let id = uuid_arg(args, "id")?;
    let page = page_args(args, LOG_PAGE_MAX)?;
    let result = read(host, |s| {
        // Only a person's rows carry field changes; those of every other kind carry none, so a
        // record's whole history stays cheap to read (the page below is cut after reading).
        let all = s.history(id, usize::MAX)?;
        let total = all.len();
        let items = slice(all, page)
            .map(|entry| to_value(&entry))
            .collect::<Result<_, _>>()?;
        Ok(paged(total, page, items))
    })?;
    answer(&result)
}

fn find_by_number<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let file = str_arg(args, "source_file")?;
    let number = str_arg(args, "number")?;
    let person = read(host, |s| Ok(s.find_by_number(file, number)))?;
    answer(&json!({"person": person}))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn pictures_are_decoded_one_at_a_time() {
        let running = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let threads: Vec<_> = (0..6)
            .map(|_| {
                let (running, peak) = (Arc::clone(&running), Arc::clone(&peak));
                std::thread::spawn(move || {
                    one_decode_at_a_time(|| {
                        let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(20));
                        running.fetch_sub(1, Ordering::SeqCst);
                    });
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), 1);
    }
}
