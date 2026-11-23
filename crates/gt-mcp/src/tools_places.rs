//! Events of a place: reading the one list of what happened in a place (people of the tree and
//! place records about others), and adding place records from church books and the like.

use crate::tools_read::{ToolResult, answer, invalid, str_arg, uuid_arg};
use crate::tools_write::{batch_arg, degrees, long, optional_uuid, short, with_batch};
use crate::{AgentCtx, Host, Tool};
use gt_session::{
    AgentWrite, ApiError, PLACE_EVENTS_PAGE_MAX, PLACE_MERGE_MAX, PLACE_RECORD_PEOPLE_MAX,
    PLACE_RECORDS_BATCH_MAX, PlaceEventsQuery, PlaceEventsWho, Session, place_record_kinds,
};
use gt_store::PlaceRecordInput;
use serde_json::{Value, json};
use uuid::Uuid;

pub(crate) fn tools<H: Host>() -> Vec<Tool<H>> {
    let kinds = json!(place_record_kinds());
    let text = |what: &str| json!({"type": "string", "description": what, "maxLength": 400});
    let person = json!({
        "type": "object",
        "properties": {
            "role": text("The role in the record: child, father, mother, groom, bride, godparent, witness, head, wife, son..."),
            "given": text("Given name."),
            "patronymic": text("Patronymic."),
            "surname": text("Surname."),
            "as_written": text("The name exactly as the book writes it, when it differs from the parts."),
            "estate": text("Social estate or occupation as the record gives it."),
            "age": text("Age as written: 40, 3 months..."),
            "note": text("A note on this person."),
            "person": {"type": "string", "description": "The id of the person in the tree this is, only when sure: a relative of the tree. Leave out for everyone else."},
        },
        "additionalProperties": false,
    });
    let record = json!({
        "type": "object",
        "properties": {
            "kind": {"type": "string", "enum": kinds, "description": "What the record is."},
            "date": text("The date as the book gives it; a partial date is fine (1850, 03.1850, 12.03.1850)."),
            "year": {"type": "integer", "minimum": 1000, "maximum": 2100, "description": "The sortable year; read from date when left out."},
            "book": text("The book's code."),
            "page": text("The page or folio."),
            "rec_id": text("The record's id in the book's index. With book it makes the record unique: adding it again adds nothing."),
            "note": {"type": "string", "maxLength": 4000},
            "unsure": {"type": "boolean", "description": "True when the reading or the identification is doubtful."},
            "people": {"type": "array", "maxItems": PLACE_RECORD_PEOPLE_MAX, "items": person},
        },
        "required": ["kind"],
        "additionalProperties": false,
    });
    vec![
        Tool {
            name: "add_place_records",
            description: "Adds records about people who are NOT in the tree, from a church book, confession list, revision or census, to a place, so the place can show every event that happened there. One record is one entry of the book with everyone it names (child, parents, godparents; the household of a revision). Give book and rec_id so that sending the same record again adds nothing. Up to 1000 records a call. Put a person in the tree in `person` only when you are sure it is the same person; otherwise leave it out. Never invent a date, name or age: write what the book says, and mark doubtful readings unsure. The answer counts what was added and what was already there.",
            schema: with_batch(
                &["place", "records"],
                json!({
                    "place": {"type": "string", "description": "The id of the place (from list_places) where the events happened."},
                    "records": {"type": "array", "minItems": 1, "maxItems": PLACE_RECORDS_BATCH_MAX, "items": record},
                }),
            ),
            call: add_place_records::<H>,
        },
        Tool {
            name: "list_place_events",
            description: "Every event that happened in a place and the places under it, oldest first, undated last: births, marriages, deaths and other events of people in the tree, and place records about other people. Filter by kind, years, whether it concerns relatives (ours), other people (others) or all, and by part of a name. Each row says where it comes from (tree or record), the date, the people with their roles, and for a record its book, page and record id, and whether the reading is unsure. The answer's byKind and byDecade count what matches the who and name filters, whatever kinds and years were asked for. Read on with offset, as nextOffset says.",
            schema: json!({
                "type": "object",
                "properties": {
                    "place": {"type": "string", "description": "The place's id (from list_places)."},
                    "kinds": {"type": "array", "items": {"type": "string", "enum": kinds}},
                    "kind": {"type": "string", "enum": kinds, "description": "One kind; the same as kinds with one name. Give kind or kinds, not both."},
                    "year_from": {"type": "integer"},
                    "year_to": {"type": "integer"},
                    "who": {"type": "string", "enum": ["all", "ours", "others"], "default": "all", "description": "ours: events of people in the tree and records that name one of them; others: records about other people."},
                    "query": {"type": "string", "maxLength": 200, "description": "Part of a name."},
                    "offset": {"type": "integer", "minimum": 0},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 200, "default": 50},
                },
                "required": ["place"],
                "additionalProperties": false,
            }),
            call: list_place_events::<H>,
        },
        Tool {
            name: "add_place",
            description: "Makes a place that is missing, such as a village the records name that list_places does not have. Give its name, optionally its type (village, town, district, region, country...), its parent place (the district or region it lies in, from list_places; leave out for a top-level place), coordinates with their source, and a note. A place of the same name (or alternative name, ignoring case) under the same parent is never made twice: the answer then has alreadyThere and the existing id, which you use. Search list_places first, and make the parents first when they are missing too. The change is in the log under your name and the owner can undo it with the batch.",
            schema: with_batch(
                &["name"],
                json!({
                    "name": text("The place's name as it is written now, such as Verbka."),
                    "type": text("What it is: village, town, district, region, country..."),
                    "parent": {"type": "string", "description": "The id of the place it lies in (from list_places or an earlier add_place)."},
                    "lat": {"type": "number", "minimum": -90, "maximum": 90, "description": "Latitude in decimal degrees, south negative. Needs lon and source."},
                    "lon": {"type": "number", "minimum": -180, "maximum": 180, "description": "Longitude in decimal degrees, west negative."},
                    "source": text("Where the coordinates (or the place) come from, briefly: GeoNames 702550, Wikipedia: ..."),
                    "note": {"type": "string", "maxLength": 4000, "description": "Optional: how you know it is this place, such as the district and parish the records name."},
                }),
            ),
            call: add_place::<H>,
        },
        Tool {
            name: "merge_places",
            description: "Merges duplicate places into one: everything that names a place in merge_ids (events of people, pictures, place records from books, places under it) then names keep_id, the names, notes and citations of the merged places go to keep_id, and the merged places are removed, all in one step the owner can undo with the batch (the log has one change group, Merge places). A research note on keep_id lists what was merged. Only for places that are the very same place: the same village in the same district, not namesakes elsewhere; when unsure, leave them and say so in your report. Refused when keep_id is among merge_ids, or when a merged place is an ancestor of keep_id. Merge the child places first when two duplicates each have a child of the same name. The answer counts what was repointed.",
            schema: with_batch(
                &["keep_id", "merge_ids"],
                json!({
                    "keep_id": {"type": "string", "description": "The id of the place that stays."},
                    "merge_ids": {"type": "array", "minItems": 1, "maxItems": PLACE_MERGE_MAX, "items": {"type": "string"}, "description": "The ids of the duplicates to merge into it."},
                }),
            ),
            call: merge_places::<H>,
        },
    ]
}

fn parse_records(args: &Value) -> Result<Vec<PlaceRecordInput>, ApiError> {
    let list = args
        .get("records")
        .and_then(Value::as_array)
        .filter(|l| !l.is_empty())
        .ok_or_else(|| invalid("records must be a list of at least one record.".into()))?;
    if list.len() > PLACE_RECORDS_BATCH_MAX {
        return Err(invalid(format!(
            "Send at most {PLACE_RECORDS_BATCH_MAX} records in one call."
        )));
    }
    let mut out = Vec::with_capacity(list.len());
    for (i, v) in list.iter().enumerate() {
        let rec: PlaceRecordInput = serde_json::from_value(v.clone())
            .map_err(|e| invalid(format!("Record {} is not valid: {e}", i + 1)))?;
        if !place_record_kinds().contains(&rec.kind.as_str()) {
            return Err(invalid(format!(
                "Record {} has a kind that is none of {}.",
                i + 1,
                place_record_kinds().join(", ")
            )));
        }
        if rec.people.len() > PLACE_RECORD_PEOPLE_MAX {
            return Err(invalid(format!("Record {} names too many people.", i + 1)));
        }
        if rec.people.is_empty() && rec.note.trim().is_empty() {
            return Err(invalid(format!(
                "Record {} names nobody and has no note.",
                i + 1
            )));
        }
        out.push(rec);
    }
    Ok(out)
}

fn add_place_records<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let place = uuid_arg(args, "place")?;
    let records = parse_records(args)?;
    let outcome = host.with_session(&mut |s: &mut Session| {
        s.add_place_records_as_agent(ctx, batch, place, &records)
    })?;
    let linked = Session::place_record_people(&records);
    host.changed(&linked, ctx);
    answer(&json!({
        "added": outcome.added,
        "duplicates": outcome.duplicates,
        "alreadyThere": outcome.added == 0 && outcome.duplicates > 0,
    }))
}

fn list_place_events<H: Host>(host: &H, _ctx: &AgentCtx, args: &Value) -> ToolResult {
    let place = uuid_arg(args, "place")?;
    let who = match args.get("who").and_then(Value::as_str) {
        None | Some("all") => PlaceEventsWho::All,
        Some("ours") => PlaceEventsWho::Ours,
        Some("others") => PlaceEventsWho::Others,
        Some(_) => return Err(invalid("who is all, ours or others.".into())),
    };
    let one = match args.get("kind") {
        None | Some(Value::Null) => None,
        Some(Value::String(k)) if place_record_kinds().contains(&k.as_str()) => Some(k.clone()),
        Some(_) => {
            return Err(invalid(format!(
                "kind is one of {}. To ask for several, give kinds as a list.",
                place_record_kinds().join(", ")
            )));
        }
    };
    if one.is_some() && args.get("kinds").is_some_and(|k| !k.is_null()) {
        return Err(invalid("Give kind or kinds, not both.".into()));
    }
    let kinds: Option<Vec<String>> = match args.get("kinds") {
        None | Some(Value::Null) => one.map(|k| vec![k]),
        Some(Value::Array(a)) => Some(
            a.iter()
                .map(|k| {
                    k.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| invalid("kinds is a list of kind names.".into()))
                })
                .collect::<Result<_, _>>()?,
        ),
        Some(_) => return Err(invalid("kinds is a list of kind names.".into())),
    };
    let int = |name: &str| -> Result<Option<i64>, ApiError> {
        match args.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_i64()
                .map(Some)
                .ok_or_else(|| invalid(format!("{name} must be a whole number"))),
        }
    };
    let year = |name: &str| -> Result<Option<i32>, ApiError> {
        Ok(int(name)?.and_then(|v| i32::try_from(v).ok()))
    };
    let offset = int("offset")?.unwrap_or(0).max(0);
    let limit = int("limit")?.unwrap_or(50).clamp(1, 200);
    let query = match args.get("query") {
        None | Some(Value::Null) => None,
        Some(_) => Some(str_arg(args, "query")?.to_string()),
    };
    let q = PlaceEventsQuery {
        place,
        kinds,
        year_from: year("year_from")?,
        year_to: year("year_to")?,
        who: Some(who),
        query,
        offset: u32::try_from(offset).ok(),
        limit: u32::try_from(limit)
            .ok()
            .map(|l| l.min(PLACE_EVENTS_PAGE_MAX)),
    };
    let page = host.with_session(&mut |s: &mut Session| s.place_events(&q))?;
    let mut out = serde_json::to_value(&page)
        .map_err(|e| ApiError::new(gt_session::ErrorCode::Internal, e.to_string()))?;
    let end = u64::from(page.offset) + page.items.len() as u64;
    if end < u64::from(page.total)
        && let Some(fields) = out.as_object_mut()
    {
        fields.insert("nextOffset".into(), json!(end));
    }
    answer(&out)
}

fn add_place<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let name = str_arg(args, "name")?.to_string();
    let kind = short(args, "type")?;
    let parent = optional_uuid(args, "parent")?;
    let point = match (args.get("lat"), args.get("lon")) {
        (None | Some(Value::Null), None | Some(Value::Null)) => None,
        (Some(_), Some(_)) => Some((
            degrees(args, "lat", -90.0, 90.0)?,
            degrees(args, "lon", -180.0, 180.0)?,
        )),
        _ => return Err(invalid("Give both lat and lon, or neither.".into())),
    };
    let source = short(args, "source")?;
    let note = long(args, "note")?;
    let (write, reached) = host.with_session(&mut |s: &mut Session| {
        s.add_place_as_agent(
            &name,
            kind.as_deref(),
            parent,
            point,
            source.as_deref(),
            note.as_deref(),
            ctx,
            batch,
        )
    })?;
    match write {
        AgentWrite::AlreadyThere { record } => answer(&json!({"id": record, "alreadyThere": true})),
        AgentWrite::Done(result) => {
            if !reached.is_empty() {
                host.changed(&reached, ctx);
            }
            answer(&json!({"id": result.focus, "touched": result.touched}))
        }
    }
}

fn merge_places<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let keep = uuid_arg(args, "keep_id")?;
    let list = args
        .get("merge_ids")
        .and_then(Value::as_array)
        .filter(|l| !l.is_empty())
        .ok_or_else(|| invalid("merge_ids must be a list of at least one place id.".into()))?;
    let merge: Vec<Uuid> = list
        .iter()
        .map(|v| {
            v.as_str()
                .and_then(|t| Uuid::parse_str(t).ok())
                .ok_or_else(|| invalid("merge_ids must be place ids.".into()))
        })
        .collect::<Result<_, _>>()?;
    let (outcome, reached) = host
        .with_session(&mut |s: &mut Session| s.merge_places_as_agent(keep, &merge, ctx, batch))?;
    let mut touched = reached;
    touched.push(keep);
    host.changed(&touched, ctx);
    let c = &outcome.counts;
    answer(&json!({
        "id": keep,
        "removed": outcome.removed,
        "repointed": {
            "events": c.events,
            "media": c.media,
            "placeRecords": outcome.place_records,
            "childPlaces": c.child_places,
        },
        "keptFromMerged": {
            "names": c.names_added,
            "notes": c.notes,
            "citations": c.citations,
            "pictures": c.pictures,
        },
    }))
}
