//! The write tools. Each one only turns its arguments into a command for [`write`]: what an agent
//! may change is decided by the gate in `gt_session::agent`, and its refusals go to the agent as
//! they are, so that it can adjust. No tool takes whether someone is alive or dead.

use crate::tools_read::{
    TEXT_ARG_MAX, ToolResult, answer, checked_text, invalid, optional_str, read, str_arg, uuid_arg,
};
use crate::{AgentCtx, Host, Tool};
use gt_core::model::{Common, Event, Name, NameKind, NoteKind, ParticipantRef};
use gt_session::person_detail::event_kind_id;
use gt_session::{
    AgentWrite, ApiError, CitationArg, CitationInput, CitationPatch, DateStatus, EditCommand,
    EditResult, EventInput, EventOwner, EventOwnerKind, LivingPatch, NameInput, NameKindDto,
    NoteInput, NoteKindDto, OwnerKind, OwnerRef, ParentRelId, PersonDraft, Relation, Session,
    SexKind, SourceInput, SourceRef, UnionKindId, parse_date_preview,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use uuid::Uuid;

const SEXES: &str = "male, female, intersex, unknown, other";
const RELATIONS: &str = "father, mother, partner, child, sibling";
const LINKS: &str = "birth, adopted, step, foster, guardian, sealed, unknown, other";
const UNIONS: &str = "married, civil_union, unmarried, unknown, other";
const NAME_KINDS: &str = "birth, married, aka, religious, nickname, immigrant, professional, other";
const NOTE_KINDS: &str = "general, story, research, transcription, conclusion, other";
const TARGET_KINDS: &str = "person, family, name, event, media, citation";
const SOURCE_DATE: &str = "Only as the source gives it, like 12 MAR 1901, ABT 1880 or BET 1880 AND 1890. Words of the source go in brackets, like (after the war).";

pub(crate) fn tools<H: Host>() -> Vec<Tool<H>> {
    let short =
        |what: &str| json!({"type": "string", "description": what, "maxLength": TEXT_ARG_MAX});
    let id = |what: &str| json!({"type": "string", "description": what});
    let date = || json!({"type": "string", "description": SOURCE_DATE, "maxLength": TEXT_ARG_MAX});
    let surnames =
        || json!({"type": "array", "items": {"type": "string", "maxLength": TEXT_ARG_MAX}});
    let one_of = |list: &str| -> Value {
        json!({"type": "string", "enum": list.split(", ").collect::<Vec<_>>()})
    };
    let relation = |required: Value| {
        json!({
            "type": "object",
            "description": "Who this person is to someone: with to and as, \"the father of to\".",
            "properties": {
                "to": {"type": "string", "description": "The id of the person they are related to."},
                "as": one_of(RELATIONS),
                "step": {"type": "boolean", "description": "A step-parent or stepchild."},
                "link": {"type": "string", "enum": LINKS.split(", ").collect::<Vec<_>>(), "description": "How a parent and child are linked, when it is not by birth."},
                "family": {"type": "string", "description": "The family, when the person is in more than one."},
            },
            "required": required,
            "additionalProperties": false,
        })
    };
    let target = || {
        json!({
            "type": "object",
            "properties": {"kind": one_of(TARGET_KINDS), "id": {"type": "string"}},
            "required": ["kind", "id"],
            "additionalProperties": false,
        })
    };
    let name_fields = |required: &[&str], more: Value| {
        let mut properties = json!({
            "kind": one_of(NAME_KINDS),
            "kind_text": short("What kind of name, when kind is other."),
            "prefix": short("Before the name, like Dr."),
            "given": short("The given name."),
            "call": short("The name they went by among their given names."),
            "surname_prefix": short("Before the surname, like von."),
            "surnames": surnames(),
            "patronymic": short("The patronymic."),
            "suffix": short("After the name, like Jr."),
            "nickname": short("The nickname."),
            "lang": short("The language of the name, like uk."),
            "date": date(),
            "person": id("The person's id."),
        });
        merge(&mut properties, more);
        with_batch(required, properties)
    };
    let event_fields = |required: &[&str], more: Value| {
        let mut properties = json!({
            "kind": {"type": "string", "description": "birth, death, burial, baptism, marriage, residence, occupation and so on, or custom:<your words>."},
            "date": date(),
            "place": short("As the source writes it, smallest place first, with commas. Other readings of it go in a note."),
            "description": {"type": "string"},
            "age": short("Their age at the time, as the source gives it."),
            "cause": short("The cause, as the source gives it."),
        });
        merge(&mut properties, more);
        with_batch(required, properties)
    };
    vec![
        Tool {
            name: "begin_batch",
            description: "Opens a batch: call this first, then pass its id to every write. A batch groups what you add, so the owner can review it. Give it a title, and the file you are working from if there is one.",
            schema: schema(
                &["title"],
                json!({
                    "title": short("What this work is, for example the name of the file."),
                    "source_file": short("The file you are working from."),
                }),
            ),
            call: begin_batch::<H>,
        },
        Tool {
            name: "end_batch",
            description: "Closes a batch with your report: what was added, what was left unsettled, and questions for the family. What you added stays.",
            schema: with_batch(&["report"], json!({"report": {"type": "string"}})),
            call: end_batch::<H>,
        },
        Tool {
            name: "add_person",
            description: "Adds someone new, on their own or related to someone. Search first: most people already exist, and if you cannot tell, do not add them. Unknown or unclear information goes in a note with add_note, never in a guessed name or date. Someone with a known role and no known name is a person with an empty name and a note saying so. Brothers or sisters mentioned with no names are not people to add: use add_sibling_marker. A rerun adds a person again unless you look for them first with find_by_number; record your numbers with set_number.",
            schema: with_batch(
                &[],
                json!({
                    "given": short("The given name."),
                    "surnames": surnames(),
                    "patronymic": short("The patronymic."),
                    "sex": one_of(SEXES),
                    "birth_date": date(),
                    "birth_place": short("As the source writes it, smallest place first, with commas."),
                    "relation": relation(json!(["to", "as"])),
                }),
            ),
            call: add_person::<H>,
        },
        Tool {
            name: "link_people",
            description: "Connects two people already in the tree: with relation.to and relation.as, person is the father (or mother, partner, child, sibling) of to. To make someone a child of a family, give as: child and family without to. If a link is either-or or uncertain, make none: write a note on the people involved. A second parent by birth is refused: give both versions in a note, or link as a step, adoptive or foster parent if that is what the source says.",
            schema: with_batch(
                &["person", "relation"],
                json!({
                    "person": id("The person being linked."),
                    "relation": relation(json!(["as"])),
                }),
            ),
            call: link_people::<H>,
        },
        Tool {
            name: "add_family",
            description: "Adds a family of one or two partners. add_person and link_people make the family they need, so this is for a couple with no children recorded, or when you need its id first.",
            schema: with_batch(
                &[],
                json!({
                    "partner1": id("A partner's id."),
                    "partner2": id("A partner's id."),
                    "kind": one_of(UNIONS),
                }),
            ),
            call: add_family::<H>,
        },
        Tool {
            name: "add_name",
            description: "Adds another name to a person, such as a married name or another spelling. It never takes over as their main name. A spelling you are unsure of goes in a note instead.",
            schema: name_fields(&["person", "kind"], json!({})),
            call: add_name::<H>,
        },
        Tool {
            name: "add_event",
            description: "Adds an event to a person or a family. Give one of them. Never invent a date: with no date in the source, leave it out and put what the source says in a note. Inferences go in a note too. A person has one birth, death, burial and cremation; a different version of one goes in a note. An event that would change whether someone counts as alive or dead is refused: only the owner can do that, so leave a note and put it in your report.",
            schema: event_fields(
                &["kind"],
                json!({
                    "person": id("The person's id."),
                    "family": id("The family's id."),
                }),
            ),
            call: add_event::<H>,
        },
        Tool {
            name: "add_note",
            description: "Adds a note to a person, family, name, event, media or citation. Unknown or unclear information belongs here, as does anything a source disagrees on: give both versions and the source. Say what is inference.",
            schema: with_batch(
                &["target", "text"],
                json!({
                    "target": target(),
                    "text": {"type": "string"},
                    "kind": one_of(NOTE_KINDS),
                }),
            ),
            call: add_note::<H>,
        },
        Tool {
            name: "add_source",
            description: "Adds a source. Look with list_sources first. To say a fact comes from it, use add_citation.",
            schema: with_batch(
                &["title"],
                json!({
                    "title": short("The source's title."),
                    "author": short("Its author."),
                    "publication": short("Where and when it was published."),
                    "abbreviation": short("A short name for it."),
                    "text": {"type": "string", "description": "The source's text."},
                }),
            ),
            call: add_source::<H>,
        },
        Tool {
            name: "add_citation",
            description: "Says which source a fact comes from and how reliable it is. For quality: 3 confirmed by the family directly, 2 said in a recording, 1 doubtful spelling or identity. An inference is a note, not a citation.",
            schema: with_batch(
                &["target", "source"],
                json!({
                    "target": target(),
                    "source": id("The source's id."),
                    "page": short("Where in the source, such as a page or a time."),
                    "text": {"type": "string", "description": "What the source says here."},
                    "quality": {"type": "integer", "minimum": 0, "maximum": 3},
                    "date": date(),
                }),
            ),
            call: add_citation::<H>,
        },
        Tool {
            name: "add_sibling_marker",
            description: "Adds the one entry to a family that stands for brothers and sisters mentioned with no names. It takes no names or dates. Named brothers and sisters are people: use add_person.",
            schema: with_batch(&["family"], json!({"family": id("The family's id.")})),
            call: add_sibling_marker::<H>,
        },
        Tool {
            name: "set_number",
            description: "Records a person's number in a source file's own numbering, so that a rerun finds them again with find_by_number instead of adding them twice. A number points at one person within a file.",
            schema: with_batch(
                &["person", "source_file", "number"],
                json!({
                    "person": id("The person's id."),
                    "source_file": short("The file the number belongs to."),
                    "number": short("The number."),
                }),
            ),
            call: set_number::<H>,
        },
        Tool {
            name: "set_sex",
            description: "Sets the sex of someone you added. On anyone else, a note says it instead. Only this can be set on a person; whether someone is alive or dead is the owner's alone to change, so leave a note and put it in your report.",
            schema: with_batch(
                &["person", "sex"],
                json!({
                    "person": id("The person's id."),
                    "sex": one_of(SEXES),
                    "sex_text": short("What it is, when sex is other."),
                }),
            ),
            call: set_sex::<H>,
        },
        Tool {
            name: "update_own_note",
            description: "Changes the text of your own note. A note someone else wrote or changed stays as it is: add your own beside it.",
            schema: with_batch(
                &["note", "text"],
                json!({
                    "note": id("The note's id."),
                    "text": {"type": "string"},
                    "kind": one_of(NOTE_KINDS),
                }),
            ),
            call: update_own_note::<H>,
        },
        Tool {
            name: "update_own_event",
            description: "Changes your own event. What you leave out stays as it is. An event someone else wrote or changed stays as it is: add a note instead.",
            schema: event_fields(&["event"], json!({"event": id("The event's id.")})),
            call: update_own_event::<H>,
        },
        Tool {
            name: "update_own_name",
            description: "Changes a name of your own. What you leave out stays as it is. A name someone else wrote or changed stays as it is: add a note instead.",
            schema: name_fields(&["person", "name"], json!({"name": id("The name's id.")})),
            call: update_own_name::<H>,
        },
        Tool {
            name: "update_own_citation",
            description: "Changes your own citation. What you leave out stays as it is. A citation someone else wrote or changed stays as it is: add your own beside it.",
            schema: with_batch(
                &["citation"],
                json!({
                    "citation": id("The citation's id."),
                    "page": short("Where in the source, such as a page or a time."),
                    "text": {"type": "string"},
                    "quality": {"type": "integer", "minimum": 0, "maximum": 3},
                    "date": date(),
                }),
            ),
            call: update_own_citation::<H>,
        },
    ]
}

fn schema(required: &[&str], properties: Value) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn merge(into: &mut Value, more: Value) {
    if let (Some(fields), Value::Object(more)) = (into.as_object_mut(), more) {
        fields.extend(more);
    }
}

fn with_batch(required: &[&str], mut properties: Value) -> Value {
    merge(
        &mut properties,
        json!({"batch": {"type": "string", "description": "The id begin_batch gave you."}}),
    );
    let mut all = vec!["batch"];
    all.extend(required);
    schema(&all, properties)
}

/// The one way a tool writes, and the one place that learns what a write touched. It tells the app
/// only after the session's lock is let go, and a repeat tells it nothing. `new_id` runs under the
/// same lock as the write, so that it sees this write and no other.
fn write<H: Host>(
    host: &H,
    cmd: EditCommand,
    ctx: &AgentCtx,
    batch: Uuid,
    new_id: impl Fn(&Session, &EditResult) -> Option<Uuid>,
) -> ToolResult {
    let (answered, touched) = host.with_session(&mut |s| {
        Ok(match s.apply_as_agent(cmd.clone(), ctx, batch)? {
            AgentWrite::Done(result) => {
                let mut fields = json!({"id": new_id(s, &result), "touched": result.touched});
                let family = result
                    .touched
                    .iter()
                    .find(|id| s.tree().families.contains_key(*id));
                if let Some(family) = family {
                    fields["family"] = json!(family);
                }
                (fields, result.touched)
            }
            AgentWrite::AlreadyThere { record } => {
                (json!({"id": record, "alreadyThere": true}), Vec::new())
            }
        })
    })?;
    if !touched.is_empty() {
        host.changed(&touched);
    }
    answer(&answered)
}

/// The person a write was about.
fn focus(_: &Session, result: &EditResult) -> Option<Uuid> {
    result.focus
}

/// The first of what was written that passes `is_it`.
fn written(result: &EditResult, is_it: impl Fn(&Uuid) -> bool) -> Option<Uuid> {
    result.touched.iter().copied().find(|id| is_it(id))
}

fn batch_arg(args: &Value) -> Result<Uuid, ApiError> {
    let given = args.get("batch").and_then(Value::as_str);
    match given.and_then(|text| Uuid::parse_str(text).ok()) {
        Some(batch) => Ok(batch),
        None => Err(invalid(
            "batch must be the id begin_batch gave you. Open a batch with begin_batch first."
                .into(),
        )),
    }
}

/// Short text from the agent, or `None`.
fn short(args: &Value, name: &str) -> Result<Option<String>, ApiError> {
    Ok(optional_str(args, name)?.map(str::to_string))
}

/// Writing of any length: only the size of a request limits it.
fn long(args: &Value, name: &str) -> Result<Option<String>, ApiError> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(invalid(format!("{name} must be text"))),
    }
}

fn optional_uuid(args: &Value, name: &str) -> Result<Option<Uuid>, ApiError> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => uuid_arg(args, name).map(Some),
    }
}

fn surnames_arg(args: &Value) -> Result<Option<Vec<String>>, ApiError> {
    let list = match args.get("surnames") {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::Array(list)) => list,
        Some(_) => return Err(invalid("surnames must be a list of text".into())),
    };
    list.iter()
        .map(|surname| {
            let text = surname
                .as_str()
                .ok_or_else(|| invalid("surnames must be a list of text".into()))?;
            Ok(checked_text("A surname", text)?.to_string())
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

/// One of a closed list of words, read by the type the session takes it as.
fn word_arg<T: DeserializeOwned>(
    args: &Value,
    name: &str,
    shown_as: &str,
    list: &str,
) -> Result<Option<T>, ApiError> {
    let Some(given) = args.get(name).filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    serde_json::from_value(given.clone())
        .map(Some)
        .map_err(|_| invalid(format!("{shown_as} must be one of: {list}")))
}

/// A date the session understands. The planner stores anything, and a date it cannot read is a
/// date the owner would have to untangle, so that is refused here. Blank is no date.
fn date_arg<H: Host>(host: &H, args: &Value, name: &str) -> Result<Option<String>, ApiError> {
    let Some(raw) = optional_str(args, name)?
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
    else {
        return Ok(None);
    };
    let order = read(host, |s| Ok(s.settings().day_order))?;
    if parse_date_preview(raw, order).status == DateStatus::Unparsed {
        return Err(invalid(format!(
            "\"{raw}\" is not understood as a date. {SOURCE_DATE} Never guess one: if the source \
             gives no date like that, leave it out and put what the source says in a note."
        )));
    }
    Ok(Some(raw.to_string()))
}

fn quality_arg(args: &Value) -> Result<Option<u8>, ApiError> {
    let wrong = || invalid("quality must be 0, 1, 2 or 3".into());
    match args.get("quality") {
        None | Some(Value::Null) => Ok(None),
        Some(given) => {
            let number = given.as_u64().filter(|q| *q <= 3).ok_or_else(wrong)?;
            u8::try_from(number).map(Some).map_err(|_| wrong())
        }
    }
}

fn target_arg(args: &Value) -> Result<OwnerRef, ApiError> {
    let target = args
        .get("target")
        .filter(|v| v.is_object())
        .ok_or_else(|| invalid("target must have a kind and an id".into()))?;
    Ok(OwnerRef {
        kind: word_arg(target, "kind", "target.kind", TARGET_KINDS)?
            .ok_or_else(|| invalid(format!("target.kind must be one of: {TARGET_KINDS}")))?,
        id: uuid_arg(target, "id")
            .map_err(|_| invalid("target.id must be a record's id".into()))?,
    })
}

/// `relation`, as the session takes it.
struct RelationArgs {
    to: Option<Uuid>,
    relation: Relation,
    link: Option<ParentRelId>,
    step: bool,
    family: Option<Uuid>,
}

fn relation_arg(args: &Value) -> Result<Option<RelationArgs>, ApiError> {
    let given = match args.get("relation") {
        None | Some(Value::Null) => return Ok(None),
        Some(given) if given.is_object() => given,
        Some(_) => return Err(invalid("relation must have to and as".into())),
    };
    let optional_id = |name: &str, shown_as: &str| {
        optional_uuid(given, name).map_err(|_| invalid(format!("{shown_as} must be a record's id")))
    };
    let relation = word_arg(given, "as", "relation.as", RELATIONS)?
        .ok_or_else(|| invalid(format!("relation.as must be one of: {RELATIONS}")))?;
    let step = given.get("step").and_then(Value::as_bool).unwrap_or(false);
    let named: Option<ParentRelId> = word_arg(given, "link", "relation.link", LINKS)?;
    if step && named.is_some_and(|link| link != ParentRelId::Step) {
        return Err(invalid(
            "relation has step, and a link that says something else. Give one of them.".into(),
        ));
    }
    Ok(Some(RelationArgs {
        to: optional_id("to", "relation.to")?,
        relation,
        link: if step { Some(ParentRelId::Step) } else { named },
        step: step || named == Some(ParentRelId::Step),
        family: optional_id("family", "relation.family")?,
    }))
}

fn begin_batch<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let title = str_arg(args, "title")?;
    let file = optional_str(args, "source_file")?;
    let batch = host.with_session(&mut |s| s.begin_batch(ctx, title, file))?;
    answer(&json!({"batch": batch}))
}

fn end_batch<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let report = long(args, "report")?.ok_or_else(|| invalid("report must be text".into()))?;
    host.with_session(&mut |s| s.end_batch(ctx, batch, &report))?;
    answer(&json!({"batch": batch, "closed": true}))
}

fn add_person<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let draft = PersonDraft {
        given: short(args, "given")?,
        surnames: surnames_arg(args)?.unwrap_or_default(),
        patronymic: short(args, "patronymic")?,
        sex: word_arg(args, "sex", "sex", SEXES)?.unwrap_or(SexKind::Unknown),
        birth_date_raw: date_arg(host, args, "birth_date")?,
        birth_place_text: short(args, "birth_place")?,
    };
    let cmd = match relation_arg(args)? {
        None => EditCommand::CreatePerson { draft },
        Some(given) => EditCommand::QuickAdd {
            anchor: given
                .to
                .ok_or_else(|| invalid("relation.to must be the id of a person".into()))?,
            relation: given.relation,
            draft,
            family: given.family,
            new_parent_set: None,
            parent_rel: given.link,
        },
    };
    write(host, cmd, ctx, batch, focus)
}

fn link_people<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let person = uuid_arg(args, "person")?;
    let given = relation_arg(args)?.ok_or_else(|| invalid("relation must have as".into()))?;
    let cmd = match (given.to, given.relation, given.family) {
        (Some(anchor), relation, family) => EditCommand::LinkExisting {
            anchor,
            relation,
            person,
            family,
            new_parent_set: None,
            parent_rel: given.link,
        },
        (None, Relation::Child, Some(family)) => {
            if given.step {
                return Err(invalid(
                    "A stepchild is the child of one partner only: give that partner as \
                     relation.to."
                        .into(),
                ));
            }
            let rel = given.link.unwrap_or(ParentRelId::Birth);
            // A family the person already has stays the one that counts for them.
            let first = read(host, |s| {
                Ok(s.tree().families_as_child(person).next().is_none())
            })?;
            EditCommand::LinkChild {
                family,
                child: person,
                rel: (rel, rel),
                preferred: first,
            }
        }
        _ => {
            return Err(invalid(
                "relation.to must be the id of a person. Only a child can go straight into a \
                 family, with as: child and family."
                    .into(),
            ));
        }
    };
    write(host, cmd, ctx, batch, |_, _| Some(person))
}

fn add_family<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let cmd = EditCommand::SaveFamily {
        id: None,
        partner1: optional_uuid(args, "partner1")?,
        partner2: optional_uuid(args, "partner2")?,
        kind: word_arg(args, "kind", "kind", UNIONS)?.unwrap_or(UnionKindId::Unknown),
    };
    write(host, cmd, ctx, batch, |s, result| {
        written(result, |id| s.tree().families.contains_key(id))
    })
}

fn name_kind(kind: &NameKind) -> NameKindDto {
    match kind {
        NameKind::Birth => NameKindDto::Birth,
        NameKind::Married => NameKindDto::Married,
        NameKind::Aka => NameKindDto::Aka,
        NameKind::Religious => NameKindDto::Religious,
        NameKind::Nickname => NameKindDto::Nickname,
        NameKind::Immigrant => NameKindDto::Immigrant,
        NameKind::Professional => NameKindDto::Professional,
        NameKind::Other(_) => NameKindDto::Other,
    }
}

/// A name from the arguments, on top of what `old` says.
fn name_input<H: Host>(host: &H, args: &Value, old: Option<&Name>) -> Result<NameInput, ApiError> {
    let text = |name: &str, was: fn(&Name) -> &Option<String>| -> Result<_, ApiError> {
        Ok(short(args, name)?.or_else(|| old.and_then(|o| was(o).clone())))
    };
    let kind = word_arg(args, "kind", "kind", NAME_KINDS)?
        .or_else(|| old.map(|o| name_kind(&o.kind)))
        .ok_or_else(|| invalid(format!("kind must be one of: {NAME_KINDS}")))?;
    Ok(NameInput {
        id: old.map(|o| o.id),
        kind,
        kind_text: short(args, "kind_text")?,
        // The gate decides this one.
        preferred: false,
        private: old.is_some_and(|o| o.private),
        prefix: text("prefix", |o| &o.prefix)?,
        given: text("given", |o| &o.given)?,
        call: text("call", |o| &o.call)?,
        surname_prefix: text("surname_prefix", |o| &o.surname_prefix)?,
        surnames: surnames_arg(args)?
            .or_else(|| old.map(|o| o.surnames.clone()))
            .unwrap_or_default(),
        patronymic: text("patronymic", |o| &o.patronymic)?,
        suffix: text("suffix", |o| &o.suffix)?,
        nickname: text("nickname", |o| &o.nickname)?,
        lang: text("lang", |o| &o.lang)?,
        date_raw: date_arg(host, args, "date")?
            .or_else(|| old.and_then(|o| o.date.as_ref()).map(|d| d.raw.clone())),
    })
}

fn add_name<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let person = uuid_arg(args, "person")?;
    let name = name_input(host, args, None)?;
    write(
        host,
        EditCommand::SaveName { person, name },
        ctx,
        batch,
        |s, _| s.tree().persons.get(&person)?.names.last().map(|n| n.id),
    )
}

fn update_own_name<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let person = uuid_arg(args, "person")?;
    let id = uuid_arg(args, "name")?;
    let old = read(host, |s| {
        s.tree()
            .persons
            .get(&person)
            .and_then(|p| p.names.iter().find(|n| n.id == id))
            .cloned()
            .ok_or_else(|| ApiError::not_found("name", id))
    })?;
    let name = name_input(host, args, Some(&old))?;
    write(
        host,
        EditCommand::SaveName { person, name },
        ctx,
        batch,
        |_, _| Some(id),
    )
}

/// An event from the arguments, on top of what `old` says.
fn event_input<H: Host>(
    host: &H,
    args: &Value,
    old: Option<&Event>,
) -> Result<EventInput, ApiError> {
    let kind = short(args, "kind")?
        .or_else(|| old.map(|o| event_kind_id(&o.kind)))
        .ok_or_else(|| invalid("kind must be a kind of event, or custom:<your words>".into()))?;
    let was = |field: fn(&Event) -> &Option<String>| old.and_then(|o| field(o).clone());
    Ok(EventInput {
        id: old.map(|o| o.common.id),
        kind,
        date_raw: date_arg(host, args, "date")?
            .or_else(|| old.and_then(|o| o.date.as_ref()).map(|d| d.raw.clone())),
        // Left out, the planner keeps the place an event has.
        place_text: short(args, "place")?,
        place_id: None,
        description: long(args, "description")?.or_else(|| was(|o| &o.description)),
        age: short(args, "age")?.or_else(|| was(|o| &o.age)),
        cause: short(args, "cause")?.or_else(|| was(|o| &o.cause)),
        negative: old.is_some_and(|o| o.negative),
        private: old.is_some_and(|o| o.common.private),
        preferred: old.is_some_and(|o| o.assertion.preferred),
        participants: None,
    })
}

fn add_event<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let owner = match (
        optional_uuid(args, "person")?,
        optional_uuid(args, "family")?,
    ) {
        (Some(id), None) => EventOwner {
            kind: EventOwnerKind::Person,
            id,
        },
        (None, Some(id)) => EventOwner {
            kind: EventOwnerKind::Family,
            id,
        },
        _ => return Err(invalid("Give a person or a family, one of them.".into())),
    };
    let event = event_input(host, args, None)?;
    write(
        host,
        EditCommand::SaveEvent { owner, event },
        ctx,
        batch,
        |s, result| written(result, |id| s.tree().events.contains_key(id)),
    )
}

fn update_own_event<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let id = uuid_arg(args, "event")?;
    let old = read(host, |s| {
        s.tree()
            .events
            .get(&id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("event", id))
    })?;
    let owner = old
        .participants
        .first()
        .map(|taking_part| match taking_part.target {
            ParticipantRef::Person(id) => EventOwner {
                kind: EventOwnerKind::Person,
                id,
            },
            ParticipantRef::Family(id) => EventOwner {
                kind: EventOwnerKind::Family,
                id,
            },
        })
        .ok_or_else(|| ApiError::not_found("event", id))?;
    let event = event_input(host, args, Some(&old))?;
    write(
        host,
        EditCommand::SaveEvent { owner, event },
        ctx,
        batch,
        |_, _| Some(id),
    )
}

fn add_note<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let target = target_arg(args)?;
    let note = NoteInput {
        id: None,
        kind: word_arg(args, "kind", "kind", NOTE_KINDS)?.unwrap_or(NoteKindDto::General),
        text: long(args, "text")?.ok_or_else(|| invalid("text must be text".into()))?,
    };
    write(
        host,
        EditCommand::SaveNote { target, note },
        ctx,
        batch,
        |s, result| written(result, |id| s.tree().notes.contains_key(id)),
    )
}

/// What holds a note, among what a note can be added to.
fn note_holder(s: &Session, note: Uuid) -> Option<OwnerRef> {
    let tree = s.tree();
    let holds = |common: &Common| common.notes.contains(&note);
    let found = |kind, id| Some(OwnerRef { kind, id });
    for person in tree.persons.values() {
        if holds(&person.common) {
            return found(OwnerKind::Person, person.common.id);
        }
        if let Some(name) = person.names.iter().find(|n| n.notes.contains(&note)) {
            return found(OwnerKind::Name, name.id);
        }
    }
    let of_family = tree.families.values().find(|r| holds(&r.common));
    let of_event = tree.events.values().find(|r| holds(&r.common));
    let of_media = tree.media.values().find(|r| holds(&r.common));
    let of_citation = tree.citations.values().find(|r| holds(&r.common));
    let held = [
        of_family.map(|r| (OwnerKind::Family, r.common.id)),
        of_event.map(|r| (OwnerKind::Event, r.common.id)),
        of_media.map(|r| (OwnerKind::Media, r.common.id)),
        of_citation.map(|r| (OwnerKind::Citation, r.common.id)),
    ];
    held.into_iter()
        .flatten()
        .next()
        .and_then(|(kind, id)| found(kind, id))
}

fn update_own_note<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let id = uuid_arg(args, "note")?;
    let text = long(args, "text")?.ok_or_else(|| invalid("text must be text".into()))?;
    let kind: Option<NoteKindDto> = word_arg(args, "kind", "kind", NOTE_KINDS)?;
    let (target, was) = read(host, |s| {
        let note = s
            .tree()
            .notes
            .get(&id)
            .ok_or_else(|| ApiError::not_found("note", id))?;
        let target = note_holder(s, id).ok_or_else(|| {
            invalid("This note is on nothing an agent can change. Add your own instead.".into())
        })?;
        let was = match &note.kind {
            NoteKind::General => NoteKindDto::General,
            NoteKind::Story => NoteKindDto::Story,
            NoteKind::Research => NoteKindDto::Research,
            NoteKind::Transcription => NoteKindDto::Transcription,
            NoteKind::Conclusion => NoteKindDto::Conclusion,
            NoteKind::Other(_) => NoteKindDto::Other,
        };
        Ok((target, was))
    })?;
    let note = NoteInput {
        id: Some(id),
        kind: kind.unwrap_or(was),
        text,
    };
    write(
        host,
        EditCommand::SaveNote { target, note },
        ctx,
        batch,
        |_, _| Some(id),
    )
}

fn add_source<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let source = SourceInput {
        id: None,
        title: short(args, "title")?,
        author: short(args, "author")?,
        publication: short(args, "publication")?,
        abbreviation: short(args, "abbreviation")?,
        text: long(args, "text")?,
    };
    write(
        host,
        EditCommand::SaveSource { source },
        ctx,
        batch,
        |s, result| written(result, |id| s.tree().sources.contains_key(id)),
    )
}

fn add_citation<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let target = target_arg(args)?;
    let citation = CitationArg::New(CitationInput {
        source: SourceRef::Id(uuid_arg(args, "source")?),
        page: short(args, "page")?,
        date_raw: date_arg(host, args, "date")?,
        text: long(args, "text")?,
        quality: quality_arg(args)?,
        evidence: None,
    });
    write(
        host,
        EditCommand::AddCitation { target, citation },
        ctx,
        batch,
        // A citation can be cited: the new one is not the one it went on.
        |s, result| {
            written(result, |id| {
                *id != target.id && s.tree().citations.contains_key(id)
            })
        },
    )
}

fn update_own_citation<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let id = uuid_arg(args, "citation")?;
    let citation = CitationPatch {
        source: None,
        page: short(args, "page")?,
        date_raw: date_arg(host, args, "date")?,
        text: long(args, "text")?,
        quality: quality_arg(args)?,
        evidence: None,
    };
    write(
        host,
        EditCommand::UpdateCitation { id, citation },
        ctx,
        batch,
        |_, _| Some(id),
    )
}

fn add_sibling_marker<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let family = uuid_arg(args, "family")?;
    write(
        host,
        EditCommand::AddSiblingMarker { family },
        ctx,
        batch,
        focus,
    )
}

fn set_number<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let cmd = EditCommand::AddNumber {
        person: uuid_arg(args, "person")?,
        source_file: str_arg(args, "source_file")?.to_string(),
        number: str_arg(args, "number")?.to_string(),
    };
    write(host, cmd, ctx, batch, focus)
}

fn set_sex<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let cmd = EditCommand::UpdatePerson {
        id: uuid_arg(args, "person")?,
        sex: Some(
            word_arg(args, "sex", "sex", SEXES)?
                .ok_or_else(|| invalid(format!("sex must be one of: {SEXES}")))?,
        ),
        sex_text: short(args, "sex_text")?,
        living_override: LivingPatch::Keep,
        private: None,
    };
    write(host, cmd, ctx, batch, focus)
}
