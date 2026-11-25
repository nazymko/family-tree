//! Verified corrections: changing what someone else wrote, when a record the agent actually read
//! shows the right value. Every tool takes `evidence` (source, page, the record's words) and a
//! reason; the session cites the record on what changed and keeps the old value in the history.

use crate::tools_read::{ToolResult, invalid, str_arg, uuid_arg};
use crate::tools_write::{
    apply, batch_arg, date_arg, long, note_holder, optional_uuid, short, surnames_arg, with_batch,
    word_arg,
};
use crate::{AgentCtx, Host, Tool};
use gt_session::dto::NameKindDto;
use gt_session::{ApiError, EventCorrection, NameCorrection, Session, VerifiedEvidence};
use serde_json::{Value, json};

const NAME_KINDS: &str =
    "birth, married, aka, religious, nickname, immigrant, professional, as_recorded, other";

fn evidence_schema() -> Value {
    json!({
        "type": "object",
        "description": "The record you actually read that shows the right value. Without one, do not correct: leave a note.",
        "properties": {
            "source": {"type": "string", "description": "The id of the source (book, document) that holds the record; add_source first when it is not in the tree."},
            "page": {"type": "string", "maxLength": 1000, "description": "Where in the source: page, entry or number."},
            "quote": {"type": "string", "description": "The record's own words that show it, as written."},
            "quality": {"type": "integer", "minimum": 0, "maximum": 3, "description": "How reliable the record is: 3 confirmed, 2 said in a recording, 1 doubtful spelling or identity, 0 unclear."},
        },
        "required": ["source", "page", "quote"],
        "additionalProperties": false,
    })
}

fn reason_schema() -> Value {
    json!({"type": "string", "description": "What the record shows that the tree has wrong; goes in the history beside the old value."})
}

fn evidence_arg(args: &Value) -> Result<VerifiedEvidence, ApiError> {
    let given = args
        .get("evidence")
        .filter(|v| v.is_object())
        .ok_or_else(|| {
            invalid(
                "evidence must have source, page and quote: the record you actually read. \
             Without one, leave a note instead."
                    .into(),
            )
        })?;
    let quality = match given.get("quality") {
        None | Some(Value::Null) => None,
        Some(q) => Some(
            q.as_u64()
                .and_then(|q| u8::try_from(q).ok())
                .filter(|q| *q <= 3)
                .ok_or_else(|| {
                    invalid("evidence.quality must be a whole number from 0 to 3".into())
                })?,
        ),
    };
    Ok(VerifiedEvidence {
        source: uuid_arg(given, "source")
            .map_err(|_| invalid("evidence.source must be the id of a source".into()))?,
        page: str_arg(given, "page")
            .map_err(|_| invalid("evidence.page must be text".into()))?
            .to_string(),
        quote: long(given, "quote")?
            .ok_or_else(|| invalid("evidence.quote must be the record's own words".into()))?,
        quality,
    })
}

fn reason_arg(args: &Value) -> Result<String, ApiError> {
    long(args, "reason")?
        .filter(|r| !r.trim().is_empty())
        .ok_or_else(|| {
            invalid("reason must say what the record shows that the tree has wrong.".into())
        })
}

fn flag(args: &Value, name: &str) -> Result<bool, ApiError> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(invalid(format!("{name} must be true or false"))),
    }
}

pub(crate) fn tools<H: Host>() -> Vec<Tool<H>> {
    let id = |what: &str| json!({"type": "string", "description": what});
    let text = |what: &str| json!({"type": "string", "description": what, "maxLength": 1000});
    vec![
        Tool {
            name: "correct_name",
            description: "Verified correction of a person's name, whoever wrote it, when a record you read shows the right spelling or kind: change given, patronymic, surnames and/or kind of one name (name; the main name when left out), and/or make_preferred: true to make an existing name the main one (for a right spelling that is only an alternative name now). When the main name's text changes, its old text stays automatically as a non-preferred alternative name (as_recorded), so nothing is lost; the citation of your record goes on the corrected name and the old value is in the history with the reason. Needs evidence (source, page, the record's own words) and a reason. Disagreement without a decisive record is a note, not a correction; an inference is never evidence. It never changes who counts as alive or dead, and never deletes anything; the owner can undo it with the batch.",
            schema: with_batch(
                &["person", "evidence", "reason"],
                json!({
                    "person": id("The person."),
                    "name": id("The name's id, from get_person; the main name when left out."),
                    "given": text("The corrected given name; empty text clears it."),
                    "patronymic": text("The corrected patronymic; empty text clears it."),
                    "surnames": {"type": "array", "items": {"type": "string", "maxLength": 1000}, "description": "The corrected surnames, all of them."},
                    "kind": {"type": "string", "description": format!("The corrected kind: {NAME_KINDS}.")},
                    "make_preferred": {"type": "boolean", "description": "Make this name the main one; the old main name stays as an alternative."},
                    "evidence": evidence_schema(),
                    "reason": reason_schema(),
                }),
            ),
            call: correct_name::<H>,
        },
        Tool {
            name: "correct_event",
            description: "Verified correction of an event, whoever wrote it, when a record you read contradicts it: date, place, description, age and/or cause of any event of a person or family (what you leave out stays; empty text clears a description, age or cause). Needs evidence (source, page, the record's own words) and a reason. The citation of your record goes on the event, the old values are in the history with the reason, and the owner can undo it with the batch. Disagreement without a decisive record is a note, not a correction; an inference is never evidence. A date may make someone count as dead only as the rules allow, never alive. correct_event_date stays for a date the family confirmed.",
            schema: with_batch(
                &["event", "evidence", "reason"],
                json!({
                    "event": id("The event's id."),
                    "date": {"type": "string", "description": "The corrected date, as the record gives it, like 12 MAR 1901."},
                    "place": text("The corrected place as the record writes it, smallest place first, comma separated."),
                    "place_id": id("An existing place instead of place text."),
                    "description": {"type": "string", "description": "The corrected description; empty text clears it."},
                    "age": text("The corrected age as written; empty text clears it."),
                    "cause": text("The corrected cause; empty text clears it."),
                    "evidence": evidence_schema(),
                    "reason": reason_schema(),
                }),
            ),
            call: correct_event::<H>,
        },
        Tool {
            name: "move_child",
            description: "Verified move of a child's parent-child link from one family to another that shares at least one parent with it, when a record you read names the other parent: from a family of the father alone to the family of the father and the mother. The kind of link stays, the child's other data stays, and a research note on the child records the move and the reason. The record's words are cited on the new link. A family you made in this batch that the move leaves empty is removed; any other stays. Families that share no parent are refused: that is another child's parents, not the same ones found more fully; leave a note. Needs evidence and a reason; the owner can undo it with the batch.",
            schema: with_batch(
                &["person", "from_family", "to_family", "evidence", "reason"],
                json!({
                    "person": id("The child."),
                    "from_family": id("The family the child is in now."),
                    "to_family": id("The family to move the child to; it must share a parent with the first."),
                    "evidence": evidence_schema(),
                    "reason": reason_schema(),
                }),
            ),
            call: move_child::<H>,
        },
        Tool {
            name: "correct_note",
            description: "Verified correction of a note's text, whoever wrote it (the owner too), when a record you read shows that the note's reading was wrong. Replaces the text; the old text stays in the history with the reason, and the owner can undo it with the batch. Keep every fact the record does not contradict. The citation of your record goes on what the note is attached to. When the note has a story (a readable retelling), give the new story too, or clear_story: true to take it away. rewrite_note stays for a rewording the owner asked for. Disagreement without a decisive record is a new note, not a correction.",
            schema: with_batch(
                &["note", "text", "evidence", "reason"],
                json!({
                    "note": id("The note's id."),
                    "text": {"type": "string", "description": "The corrected text of the note."},
                    "story": {"type": "string", "description": "The new readable retelling, when the note has a story."},
                    "clear_story": {"type": "boolean", "description": "Take the story away instead."},
                    "evidence": evidence_schema(),
                    "reason": reason_schema(),
                }),
            ),
            call: correct_note::<H>,
        },
    ]
}

fn correct_name<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let person = uuid_arg(args, "person")?;
    let name = optional_uuid(args, "name")?;
    let evidence = evidence_arg(args)?;
    let reason = reason_arg(args)?;
    let kind: Option<NameKindDto> = word_arg(args, "kind", "kind", NAME_KINDS)?;
    let patch = NameCorrection {
        given: short(args, "given")?,
        patronymic: short(args, "patronymic")?,
        surnames: surnames_arg(args)?,
        kind,
        make_preferred: flag(args, "make_preferred")?,
    };
    if patch.given.is_none()
        && patch.patronymic.is_none()
        && patch.surnames.is_none()
        && patch.kind.is_none()
        && !patch.make_preferred
    {
        return Err(invalid(
            "Say what to correct: given, patronymic, surnames, kind or make_preferred.".into(),
        ));
    }
    apply(
        host,
        ctx,
        |s, _| {
            let (write, reached) =
                s.correct_name_as_agent(person, name, &patch, &evidence, &reason, ctx, batch)?;
            Ok((write, reached, None))
        },
        |_, _| Some(person),
    )
}

fn correct_event<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let event = uuid_arg(args, "event")?;
    let evidence = evidence_arg(args)?;
    let reason = reason_arg(args)?;
    let place_id = optional_uuid(args, "place_id")?;
    let place_text = short(args, "place")?;
    let description = long(args, "description")?;
    let age = short(args, "age")?;
    let cause = short(args, "cause")?;
    if args.get("date").is_none_or(Value::is_null)
        && place_id.is_none()
        && place_text.is_none()
        && description.is_none()
        && age.is_none()
        && cause.is_none()
    {
        return Err(invalid(
            "Say what to correct: date, place, description, age or cause.".into(),
        ));
    }
    apply(
        host,
        ctx,
        |s, warnings| {
            let patch = EventCorrection {
                date_raw: date_arg(s, args, "date", warnings)?,
                place_text: place_text.clone(),
                place_id,
                description: description.clone(),
                age: age.clone(),
                cause: cause.clone(),
            };
            let (write, reached) =
                s.correct_event_as_agent(event, &patch, &evidence, &reason, ctx, batch)?;
            Ok((write, reached, None))
        },
        |_, _| Some(event),
    )
}

fn move_child<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let person = uuid_arg(args, "person")?;
    let from = uuid_arg(args, "from_family")?;
    let to = uuid_arg(args, "to_family")?;
    let evidence = evidence_arg(args)?;
    let reason = reason_arg(args)?;
    apply(
        host,
        ctx,
        |s, _| {
            let (write, reached) =
                s.move_child_as_agent(person, from, to, &evidence, &reason, ctx, batch)?;
            Ok((write, reached, Some(person)))
        },
        |_, _| Some(person),
    )
}

fn correct_note<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let note = uuid_arg(args, "note")?;
    let evidence = evidence_arg(args)?;
    let reason = reason_arg(args)?;
    let text = long(args, "text")?
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| invalid("text must be the corrected text of the note.".into()))?;
    let story = long(args, "story")?;
    let clear_story = flag(args, "clear_story")?;
    apply(
        host,
        ctx,
        |s: &mut Session, _| {
            if !s.tree().notes.contains_key(&note) {
                return Err(ApiError::not_found("note", note));
            }
            let holder = note_holder(s, note).ok_or_else(|| {
                invalid("This note is on nothing an agent can change. Add your own instead.".into())
            })?;
            let (write, reached) = s.correct_note_as_agent(
                note,
                holder,
                &text,
                story.as_deref(),
                clear_story,
                &evidence,
                &reason,
                ctx,
                batch,
            )?;
            Ok((write, reached, None))
        },
        |_, _| Some(note),
    )
}
