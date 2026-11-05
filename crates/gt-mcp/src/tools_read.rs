//! The read tools. Everything in the tree is readable, living people included: the agent runs on
//! the owner's machine with a key the owner made.

use crate::{AgentCtx, Host, RULES, Tool};
use base64::Engine;
use gt_core::model::{Common, ExtIdKind, MediaLink, Person};
use gt_session::names::{BIRTH_KINDS, DEATH_KINDS, compose, pick_event};
use gt_session::{
    ApiError, ErrorCode, LogFilterDto, PersonQuery, Session, dates, index, render_px,
};
use gt_store::HitKind;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::Path;
use uuid::Uuid;

const PAGE_DEFAULT: u64 = 50;
const PAGE_MAX: u64 = 200;
const HISTORY_MAX: usize = 200;
const PICTURE_DEFAULT_PX: u64 = 1600;
const PICTURE_MAX_PX: u64 = 4096;
const PICTURE_MIN_PX: u64 = 16;
/// A picture is decoded and shrunk, so its file may be bigger than what goes back.
const PICTURE_FILE_MAX: u64 = 256 * 1024 * 1024;
const BLOB_MAX: u64 = 8 * 1024 * 1024;
/// How the agent's numbering of a source file is stored in `Common.ext_ids`.
const NUMBER_SCHEME_PREFIX: &str = "agent:";

type ToolResult = Result<Vec<Value>, ApiError>;

pub(crate) fn tools<H: Host>() -> Vec<Tool<H>> {
    let id = |what: &str| {
        json!({
            "type": "object",
            "properties": {"id": {"type": "string", "description": what}},
            "required": ["id"],
            "additionalProperties": false,
        })
    };
    let paged_query = |query: &str, required: bool| {
        json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": query},
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
            description: "One person in full: all names, events, notes, citations with their sources, media details, numbers and living status.",
            schema: id("The person's id."),
            call: get_person::<H>,
        },
        Tool {
            name: "get_relatives",
            description: "A person's parents, partners, children and siblings as rows, with the id of the family linking each.",
            schema: id("The person's id."),
            call: get_relatives::<H>,
        },
        Tool {
            name: "get_family",
            description: "One family: partners, children, events, notes and citations.",
            schema: id("The family's id."),
            call: get_family::<H>,
        },
        Tool {
            name: "get_source",
            description: "One source in full, with its text.",
            schema: id("The source's id."),
            call: get_source::<H>,
        },
        Tool {
            name: "get_note",
            description: "One note in full.",
            schema: id("The note's id."),
            call: get_note::<H>,
        },
        Tool {
            name: "get_citation",
            description: "One citation with its source.",
            schema: id("The citation's id."),
            call: get_citation::<H>,
        },
        Tool {
            name: "list_sources",
            description: "Sources one page at a time, optionally those whose title, abbreviation or author contains the query.",
            schema: paged_query("Text to look for.", false),
            call: list_sources::<H>,
        },
        Tool {
            name: "list_places",
            description: "Places one page at a time, optionally those whose name contains the query.",
            schema: paged_query("Text to look for.", false),
            call: list_places::<H>,
        },
        Tool {
            name: "get_media",
            description: "A media file's details and the file itself: a picture as an image, shrunk to max_size pixels on its longest side; anything else as a blob of up to 8 MiB.",
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
            description: "The log of who changed what, newest first. Filters, all optional: actorId, actorKind (user or agent), batch, person, unreviewedOnly.",
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
                    "limit": {"type": "integer", "minimum": 1, "maximum": PAGE_MAX, "default": PAGE_DEFAULT},
                },
                "additionalProperties": false,
            }),
            call: get_log::<H>,
        },
        Tool {
            name: "get_history",
            description: "The changes made to one record, newest first.",
            schema: id("The record's id."),
            call: get_history::<H>,
        },
        Tool {
            name: "find_by_number",
            description: "The person holding a number from a source file's own numbering, or null. A number is unique within one file only.",
            schema: json!({
                "type": "object",
                "properties": {
                    "source_file": {"type": "string"},
                    "number": {"type": "string"},
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

fn read<H: Host, T>(
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

fn answer(v: &impl Serialize) -> ToolResult {
    Ok(vec![json_block(v)?])
}

fn internal(e: serde_json::Error) -> ApiError {
    ApiError::new(ErrorCode::Internal, e.to_string())
}

fn to_value(v: &impl Serialize) -> Result<Value, ApiError> {
    serde_json::to_value(v).map_err(internal)
}

fn invalid(message: String) -> ApiError {
    ApiError::new(ErrorCode::Invalid, message)
}

fn uuid_arg(args: &Value, name: &str) -> Result<Uuid, ApiError> {
    args.get(name)
        .and_then(Value::as_str)
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| invalid(format!("{name} must be the id of a record")))
}

fn str_arg<'a>(args: &'a Value, name: &str) -> Result<&'a str, ApiError> {
    args.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{name} must be text")))
}

fn optional_str<'a>(args: &'a Value, name: &str) -> Result<Option<&'a str>, ApiError> {
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

fn page_args(args: &Value) -> Result<PageArgs, ApiError> {
    let offset = optional_u64(args, "offset")?.unwrap_or(0);
    let limit = optional_u64(args, "limit")?
        .unwrap_or(PAGE_DEFAULT)
        .clamp(1, PAGE_MAX);
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

/// The row that tells one person from another of the same name.
fn person_row(s: &Session, id: Uuid) -> Result<Value, ApiError> {
    let p = person(s, id)?;
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

/// The numbers a record has in source files' own numberings.
fn numbers(common: &Common) -> Vec<Value> {
    common
        .ext_ids
        .iter()
        .filter_map(|e| match &e.kind {
            ExtIdKind::Exid { scheme } => scheme
                .strip_prefix(NUMBER_SCHEME_PREFIX)
                .map(|file| json!({"sourceFile": file, "number": e.value})),
            _ => None,
        })
        .collect()
}

/// Adds what a record has beyond its DTO: notes, citations, media and numbers.
fn with_common(s: &Session, mut value: Value, common: &Common) -> Result<Value, ApiError> {
    if let Some(fields) = value.as_object_mut() {
        fields.insert("notes".into(), to_value(&s.note_dtos(&common.notes))?);
        fields.insert(
            "citations".into(),
            to_value(&s.citations_dto(&common.citations))?,
        );
        fields.insert("media".into(), to_value(&s.media_dtos(&common.media))?);
        fields.insert("numbers".into(), json!(numbers(common)));
    }
    Ok(value)
}

fn tree_summary<H: Host>(host: &H, _: &AgentCtx, _: &Value) -> ToolResult {
    let summary = read(host, |s| {
        let info = s.info();
        let tree = s.tree();
        Ok(json!({
            "name": info.name,
            "counts": {
                "people": tree.persons.len(),
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
    let page = page_args(args)?;
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
    let page = page_args(args)?;
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
    let result = read(host, |s| {
        let p = person(s, id)?;
        let mut value = to_value(&s.get_person(id)?)?;
        // The detail view leaves out what is noted on a name.
        if let Some(rows) = value.get_mut("names").and_then(Value::as_array_mut) {
            for row in rows {
                let name_id = row.get("id").and_then(Value::as_str).unwrap_or_default();
                let notes = p
                    .names
                    .iter()
                    .find(|n| n.id.to_string() == name_id)
                    .map(|n| s.note_dtos(&n.notes))
                    .unwrap_or_default();
                row["notes"] = to_value(&notes)?;
            }
        }
        if let Some(fields) = value.as_object_mut() {
            fields.insert("living".into(), json!(s.person_ref(id).map(|r| r.living)));
            fields.insert("numbers".into(), json!(numbers(&p.common)));
            fields.insert("siblingMarker".into(), json!(is_sibling_marker(p)));
        }
        Ok(value)
    })?;
    answer(&result)
}

/// Whether this is the marker entry for unnamed brothers or sisters. The tree cannot hold one yet;
/// this reads `Person.sibling_marker` once that field exists.
fn is_sibling_marker(_: &Person) -> bool {
    false
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
        with_common(s, to_value(&s.get_family(id)?)?, &family.common)
    })?;
    answer(&result)
}

fn get_source<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let id = uuid_arg(args, "id")?;
    let result = read(host, |s| {
        let source = s
            .tree()
            .sources
            .get(&id)
            .ok_or_else(|| ApiError::not_found("source", id))?;
        let dto = s
            .source_dto(id)
            .ok_or_else(|| ApiError::not_found("source", id))?;
        with_common(s, to_value(&dto)?, &source.common)
    })?;
    answer(&result)
}

fn get_note<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let id = uuid_arg(args, "id")?;
    let result = read(host, |s| {
        s.note_dtos(&[id])
            .pop()
            .ok_or_else(|| ApiError::not_found("note", id))
    })?;
    answer(&result)
}

fn get_citation<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let id = uuid_arg(args, "id")?;
    let result = read(host, |s| {
        let citation = s
            .tree()
            .citations
            .get(&id)
            .ok_or_else(|| ApiError::not_found("citation", id))?;
        let dto = s
            .citation_dto(id)
            .ok_or_else(|| ApiError::not_found("citation", id))?;
        let mut value = to_value(&dto)?;
        value["notes"] = to_value(&s.note_dtos(&citation.common.notes))?;
        Ok(value)
    })?;
    answer(&result)
}

/// Whether any of `fields` contains the folded query. An absent query takes everything.
fn has_query(query: Option<&str>, fields: &[Option<&str>]) -> bool {
    query.is_none_or(|q| fields.iter().flatten().any(|f| index::fold(f).contains(q)))
}

fn list_sources<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let query = optional_str(args, "query")?.map(index::fold);
    let page = page_args(args)?;
    let result = read(host, |s| {
        let mut found: Vec<(String, Uuid)> = s
            .tree()
            .sources
            .iter()
            .filter(|(_, src)| {
                has_query(
                    query.as_deref(),
                    &[
                        src.title.as_deref(),
                        src.abbreviation.as_deref(),
                        src.author.as_deref(),
                    ],
                )
            })
            .map(|(id, src)| (index::fold(src.title.as_deref().unwrap_or_default()), *id))
            .collect();
        found.sort();
        let total = found.len();
        let mut items = Vec::new();
        for (_, id) in slice(found, page) {
            let mut row = to_value(&s.source_dto(id))?;
            // A list stays light: get_source has the text.
            if let Some(fields) = row.as_object_mut() {
                fields.remove("text");
            }
            items.push(row);
        }
        Ok(paged(total, page, items))
    })?;
    answer(&result)
}

fn list_places<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let query = optional_str(args, "query")?.map(index::fold);
    let page = page_args(args)?;
    let result = read(host, |s| {
        let mut found: Vec<(String, Uuid)> = Vec::new();
        for (id, place) in &s.tree().places {
            let Some(option) = s.place_option(*id) else {
                continue;
            };
            let mut fields = vec![Some(option.full.as_str()), place.raw.as_deref()];
            fields.extend(place.names.iter().map(|n| Some(n.name.as_str())));
            if has_query(query.as_deref(), &fields) {
                found.push((index::fold(&option.full), *id));
            }
        }
        found.sort();
        let total = found.len();
        let mut items = Vec::new();
        for (_, id) in slice(found, page) {
            let mut row = to_value(&s.place_option(id))?;
            if let (Some(place), Some(fields)) = (s.tree().places.get(&id), row.as_object_mut()) {
                let names: Vec<&str> = place.names.iter().map(|n| n.name.as_str()).collect();
                fields.insert("names".into(), json!(names));
                fields.insert("lat".into(), json!(place.lat));
                fields.insert("lon".into(), json!(place.lon));
            }
            items.push(row);
        }
        Ok(paged(total, page, items))
    })?;
    answer(&result)
}

fn get_media<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let id = uuid_arg(args, "id")?;
    let max_size = optional_u64(args, "max_size")?
        .unwrap_or(PICTURE_DEFAULT_PX)
        .clamp(PICTURE_MIN_PX, PICTURE_MAX_PX);
    // The file is read and decoded after the session is let go, so the app stays responsive.
    let (details, mime, path) = read(host, |s| {
        let link = MediaLink {
            media: id,
            region: None,
            primary: false,
        };
        let dto = s
            .media_dtos(&[link])
            .pop()
            .ok_or_else(|| ApiError::not_found("media", id))?;
        let mime = dto.media.mime.clone().unwrap_or_default();
        Ok((dto.media, mime, s.media_original_path(id).ok()))
    })?;
    let block = match path {
        None => text("The file is missing from the project folder."),
        Some(path) if mime.starts_with("image/") => picture_block(&path, max_size),
        Some(path) => blob_block(&path, id, &mime),
    };
    Ok(vec![json_block(&details)?, block])
}

fn picture_block(path: &Path, max_size: u64) -> Value {
    let too_big = std::fs::metadata(path).is_ok_and(|m| m.len() > PICTURE_FILE_MAX);
    if too_big {
        return text("The picture is too large to read here.");
    }
    let shrunk = std::fs::read(path)
        .map_err(|_| ())
        .and_then(|bytes| render_px(&bytes, max_size as u32).map_err(|_| ()));
    match shrunk {
        Ok(jpeg) => json!({
            "type": "image",
            "data": base64::engine::general_purpose::STANDARD.encode(jpeg),
            "mimeType": "image/jpeg",
        }),
        Err(()) => text("The picture could not be read."),
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
    let page = page_args(args)?;
    let filter: LogFilterDto = match args.get("filters") {
        None | Some(Value::Null) => LogFilterDto::default(),
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|e| invalid(format!("filters are not valid: {e}")))?,
    };
    let result = read(host, |s| {
        let rows = s.log(filter.clone(), page.offset, page.limit)?;
        let items = rows.items.iter().map(to_value).collect::<Result<_, _>>()?;
        Ok(paged(rows.total as usize, page, items))
    })?;
    answer(&result)
}

fn get_history<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let id = uuid_arg(args, "id")?;
    let items = read(host, |s| s.history(id, HISTORY_MAX))?;
    answer(&json!({"items": items}))
}

fn find_by_number<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let file = str_arg(args, "source_file")?;
    let number = str_arg(args, "number")?;
    let scheme = format!("{NUMBER_SCHEME_PREFIX}{file}");
    let person = read(host, |s| {
        Ok(s.tree()
            .persons
            .iter()
            .find(|(_, p)| {
                p.common.ext_ids.iter().any(|e| {
                    e.value == number
                        && matches!(&e.kind, ExtIdKind::Exid { scheme: found } if *found == scheme)
                })
            })
            .map(|(id, _)| *id))
    })?;
    answer(&json!({"person": person}))
}
