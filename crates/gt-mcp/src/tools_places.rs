//! Events of a place: reading the one list of what happened in a place (people of the tree and
//! place records about others), and adding place records from church books and the like.

use crate::tools_read::{ToolResult, answer, invalid, str_arg, uuid_arg};
use crate::tools_write::{batch_arg, with_batch};
use crate::{AgentCtx, Host, Tool};
use gt_session::{
    ApiError, PLACE_EVENTS_PAGE_MAX, PLACE_RECORD_PEOPLE_MAX, PLACE_RECORDS_BATCH_MAX,
    PlaceEventsQuery, PlaceEventsWho, Session, place_record_kinds,
};
use gt_store::PlaceRecordInput;
use serde_json::{Value, json};

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
    let kinds: Option<Vec<String>> = match args.get("kinds") {
        None | Some(Value::Null) => None,
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
