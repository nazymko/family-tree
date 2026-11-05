//! The write tools. Each one only turns its arguments into a command for [`write`]: what an agent
//! may change is decided by the gate in `gt_session::agent`, and its refusals go to the agent as
//! they are, so that it can adjust. No tool takes whether someone is alive or dead.
//!
//! What a command is built from in the tree is read under the same lock as the write, so that
//! nothing can change between the two.

use crate::tools_read::{
    TEXT_ARG_MAX, ToolResult, answer, checked_text, invalid, optional_str, str_arg, uuid_arg,
};
use crate::{AgentCtx, Host, Tool};
use gt_core::model::{Common, Event, Name, NameKind, NoteKind, ParticipantRef};
use gt_session::person_detail::{event_kind_id, rel_id};
use gt_session::{
    AgentWrite, ApiError, CitationArg, CitationInput, CitationPatch, DateStatus, DayOrder,
    EditCommand, EditResult, EventInput, EventOwner, EventOwnerKind, LivingPatch, NameInput,
    NameKindDto, NoteInput, NoteKindDto, OwnerKind, OwnerRef, ParentRelId, PersonDraft, Relation,
    Session, SexKind, SourceInput, SourceRef, UnionKindId, parse_date_preview,
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
const SOURCE_DATE: &str = "Only as the source gives it, like 12 MAR 1901, ABT 1880 or BET 1880 AND 1890. Words of the source go in brackets, like (after the war). A date like 03/04/1901 can be read two ways and is refused: write the month as a word.";
/// Bytes of a note, an event's description or a citation's text.
const TEXT_MAX: usize = 64 * 1024;
/// Bytes of a source's text, which can be a whole transcript. Searching one this long is safe:
/// a long text shows no snippet (see `gt_store::search`).
const SOURCE_TEXT_MAX: usize = 1024 * 1024;
/// Bytes of the report that closes a batch.
const REPORT_MAX: usize = 64 * 1024;

pub(crate) fn tools<H: Host>() -> Vec<Tool<H>> {
    let short =
        |what: &str| json!({"type": "string", "description": what, "maxLength": TEXT_ARG_MAX});
    let id = |what: &str| json!({"type": "string", "description": what});
    let date = || json!({"type": "string", "description": SOURCE_DATE, "maxLength": TEXT_ARG_MAX});
    let writing = |what: &str| {
        let what = format!(
            "{what} Up to 64 KiB: split a longer text, or put it on a source with add_source and cite it."
        );
        json!({"type": "string", "description": what})
    };
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
            "kind": {"type": "string", "description": "birth, death, burial, baptism, marriage, residence, occupation and so on, or custom:<your words>. A family takes what happens to a couple, like marriage, engagement, divorce, or residence."},
            "date": date(),
            "place": short("As the source writes it, smallest place first, with commas. Other readings of it go in a note."),
            "description": writing("What the source says of it."),
            "age": short("Their age at the time, as the source gives it."),
            "cause": short("The cause, as the source gives it."),
        });
        merge(&mut properties, more);
        with_batch(required, properties)
    };
    vec![
        Tool {
            name: "begin_batch",
            description: "Opens a batch: call this first, then pass its id to every write. The answer is {\"batch\": id, \"id\": id}, the same value twice: batch is kept for older clients and id matches what the other tools return. A batch groups what you add, so the owner can review it. Give it a title, and the file you are working from if there is one.",
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
            description: "Closes a batch with your report: what was added, what was left unsettled, and questions for the family. What you added stays. The report states only what the server accepted: a write that was refused or never made is not in the tree, so check get_log for this batch before you write it, and do not count what you only planned.",
            schema: with_batch(
                &["report"],
                json!({"report": {"type": "string", "description": "Up to 64 KiB."}}),
            ),
            call: end_batch::<H>,
        },
        Tool {
            name: "add_person",
            description: "Adds someone new, on their own or related to someone. Search first: most people already exist, and if you cannot tell, do not add them. Unknown or unclear information goes in a note with add_note, never in a guessed name or date. Never create a nameless person (no given name, surname or patronymic): it is refused, and the same holds for a partner or parent made through relation. Someone known only by a role, such as somebody's father, goes in a note on the people around them. Brothers or sisters mentioned with no names are not people to add: use add_sibling_marker, with sibling_of for a person they belong beside. A birth year goes on the birth event: give birth_date here (or add_event with kind birth), not in a name or a note. Do not assume a parent this server created for a child: the answer's parents names who was set, so read it. Give source_file and number every time, from a numbered file or a label of your own that stays the same: a rerun then answers {id, alreadyThere: true} and adds nobody, where without them it adds the person again. As a child of someone with one partnership, the person also becomes the child by birth of that partner, and the answer's parents names everyone set. If that partner is not their parent, make a family with add_family and partner1 alone, and give it as relation.family. Make the person first and the family only when you are about to link, so that a refused link leaves no empty family; if one is left, remove_own_empty_family takes it away.",
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
                    "source_file": short("The file this person comes from, with number. Give it every time."),
                    "number": short("Their number in that file, with source_file. Give it every time."),
                }),
            ),
            call: add_person::<H>,
        },
        Tool {
            name: "link_people",
            description: "Connects two people already in the tree: with relation.to and relation.as, person is the father (or mother, partner, child, sibling) of to. To make someone a child of a family, give as: child and family without to. If a link is either-or or uncertain, make none: write a note on the people involved. A second parent by birth is refused: give both versions in a note, or link as a step, adoptive or foster parent if that is what the source says. A link already there answers alreadyThere. As a child of someone with one partnership, the person also becomes the child by birth of that partner, and the answer's parents names everyone set. If that partner is not their parent, make a family with add_family and partner1 alone, and give it as relation.family. Make the family only when you are about to link, so that a refused link leaves no empty family; if one is left, remove_own_empty_family takes it away.",
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
            description: "Adds another name to a person, such as a married name or another spelling. It never takes over as their main name. A name may hold only a nickname (kind nickname, nickname set): do not invent a given name for it. A spelling you are unsure of goes in a note instead.",
            schema: name_fields(&["person", "kind"], json!({})),
            call: add_name::<H>,
        },
        Tool {
            name: "remove_own_empty_family",
            description: "Removes a family you made yourself in this open batch, only when it has no children, events, notes or citations: for the empty family a refused link leaves behind. Anything else is refused and stays; list it in your report. It is logged like any write and the owner can undo it. It cannot touch anyone's alive or dead status.",
            schema: with_batch(
                &["family"],
                json!({"family": id("The id of the family you made.")}),
            ),
            call: remove_own_empty_family::<H>,
        },
        Tool {
            name: "add_event",
            description: "Adds an event to a person or a family. Give one of them. A birth year goes on the birth event (kind birth), with its date. Never invent a date: with no date in the source, leave it out and put what the source says in a note. Inferences go in a note too. A person has one birth, death, burial and cremation; a different version of one goes in a note. An event that would change whether someone counts as alive or dead is refused: only the owner can do that, so leave a note and put it in your report.",
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
                    "text": writing("The note."),
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
                    "text": {"type": "string", "description": "The source's text, up to 1 MiB."},
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
                    "text": writing("What the source says here."),
                    "quality": {"type": "integer", "minimum": 0, "maximum": 3},
                    "date": date(),
                }),
            ),
            call: add_citation::<H>,
        },
        Tool {
            name: "add_sibling_marker",
            description: "Adds the one entry to a family that stands for brothers and sisters mentioned with no names. It takes no names or dates. Named brothers and sisters are people: use add_person. Give family, or give sibling_of for a person whose family is what you know: their birth family is used, and if they have none in the tree, a family with unknown parents is made with them as its child. A second time answers alreadyThere.",
            schema: with_batch(
                &[],
                json!({
                    "family": id("The family's id."),
                    "sibling_of": id("The id of a person those brothers and sisters belong beside."),
                }),
            ),
            call: add_sibling_marker::<H>,
        },
        Tool {
            name: "set_number",
            description: "Records a person's number in a source file's own numbering, so that a rerun finds them again with find_by_number instead of adding them twice. A number points at one person within a file. For someone new, give the number to add_person instead.",
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
            description: "Changes the text of your own note. On a note someone else wrote or changed you can only fill it if it is empty; otherwise it stays as it is: add your own beside it. Saying again what it says answers alreadyThere.",
            schema: with_batch(
                &["note", "text"],
                json!({
                    "note": id("The note's id."),
                    "text": writing("The note."),
                    "kind": one_of(NOTE_KINDS),
                }),
            ),
            call: update_own_note::<H>,
        },
        Tool {
            name: "update_own_event",
            description: "Changes your own event. What you leave out stays as it is. On an event someone else wrote or changed you can only fill a field that is empty; the rest stays as it is: add a note instead. Saying again what it says answers alreadyThere.",
            schema: event_fields(&["event"], json!({"event": id("The event's id.")})),
            call: update_own_event::<H>,
        },
        Tool {
            name: "update_own_name",
            description: "Changes a name of your own. What you leave out stays as it is. On a name someone else wrote or changed you can only fill a field that is empty; the rest stays as it is: add a note instead. Saying again what it says answers alreadyThere.",
            schema: name_fields(&["person", "name"], json!({"name": id("The name's id.")})),
            call: update_own_name::<H>,
        },
        Tool {
            name: "update_own_citation",
            description: "Changes your own citation. What you leave out stays as it is. On a citation someone else wrote or changed you can only fill a field that is empty; the rest stays as it is: add your own beside it. Saying again what it says answers alreadyThere.",
            schema: with_batch(
                &["citation"],
                json!({
                    "citation": id("The citation's id."),
                    "page": short("Where in the source, such as a page or a time."),
                    "text": writing("What the source says here."),
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

/// What a write did: the gate's answer, whom it reached, and whose parents it may have set.
type Applied = (AgentWrite, Vec<Uuid>, Option<Uuid>);

/// A write of the command `build` makes, under one lock. `build` adds what the agent should hear
/// about its arguments to the list it is given.
fn write<H: Host>(
    host: &H,
    ctx: &AgentCtx,
    batch: Uuid,
    build: impl Fn(&Session, &mut Vec<String>) -> Result<EditCommand, ApiError>,
    new_id: impl Fn(&Session, &EditResult) -> Option<Uuid>,
) -> ToolResult {
    apply(
        host,
        ctx,
        |s, warnings| {
            let cmd = build(s, warnings)?;
            let (write, reached) = s.apply_as_agent_reaching(cmd.clone(), ctx, batch)?;
            let child = child_of_write(&cmd, &write);
            Ok((write, reached, child))
        },
        new_id,
    )
}

/// The child a command linked, whose parents the planner may have chosen by itself.
fn child_of_write(cmd: &EditCommand, write: &AgentWrite) -> Option<Uuid> {
    match (cmd, write) {
        (
            EditCommand::QuickAdd {
                relation: Relation::Child | Relation::Sibling,
                ..
            },
            AgentWrite::Done(result),
        ) => result.focus,
        (
            EditCommand::LinkExisting {
                relation: Relation::Child | Relation::Sibling,
                person,
                ..
            },
            AgentWrite::Done(_),
        ) => Some(*person),
        (EditCommand::LinkChild { child, .. }, AgentWrite::Done(_)) => Some(*child),
        _ => None,
    }
}

/// Who a write made parents of `child`, in the families it touched.
fn parents_set(s: &Session, child: Uuid, touched: &[Uuid]) -> Vec<Value> {
    let mut rows = Vec::new();
    for family in touched.iter().filter_map(|id| s.tree().families.get(id)) {
        let links = family.children.iter().filter(|l| l.person == child);
        for link in links {
            let slots = [(family.partner1, &link.rel1), (family.partner2, &link.rel2)];
            for (parent, rel) in slots {
                if let Some(parent) = parent {
                    let name = s.person_ref(parent).map(|p| p.name);
                    rows.push(json!({"id": parent, "name": name, "link": rel_id(rel)}));
                }
            }
        }
    }
    rows
}

/// The one way a tool writes, and the one place that learns what a write touched. It tells the app
/// only after the session's lock is let go, and a repeat tells it nothing. `new_id` runs under the
/// same lock as the write, so that it sees this write and no other.
fn apply<H: Host>(
    host: &H,
    ctx: &AgentCtx,
    run: impl Fn(&mut Session, &mut Vec<String>) -> Result<Applied, ApiError>,
    new_id: impl Fn(&Session, &EditResult) -> Option<Uuid>,
) -> ToolResult {
    let (answered, reached) = host.with_session(&mut |s| {
        let mut warnings = Vec::new();
        let (write, reached, child) = run(s, &mut warnings)?;
        let mut answered = match write {
            AgentWrite::Done(result) => {
                let mut fields = json!({"id": new_id(s, &result), "touched": result.touched});
                let family = result
                    .touched
                    .iter()
                    .find(|id| s.tree().families.contains_key(*id));
                if let Some(family) = family {
                    fields["family"] = json!(family);
                }
                let parents = child.map(|child| parents_set(s, child, &result.touched));
                if let Some(parents) = parents.filter(|rows| !rows.is_empty()) {
                    fields["parents"] = json!(parents);
                }
                fields
            }
            AgentWrite::AlreadyThere { record } => json!({"id": record, "alreadyThere": true}),
        };
        if !warnings.is_empty() {
            answered["warnings"] = json!(warnings);
        }
        Ok((answered, reached))
    })?;
    if !reached.is_empty() {
        host.changed(&reached, ctx);
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

/// Writing up to `max` bytes, as what `too_long` says when it is more.
fn writing(
    args: &Value,
    name: &str,
    max: usize,
    too_long: &str,
) -> Result<Option<String>, ApiError> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text.len() > max => {
            Err(invalid(format!("{name} is too long: {too_long}")))
        }
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(invalid(format!("{name} must be text"))),
    }
}

/// A note, an event's description or a citation's text.
fn long(args: &Value, name: &str) -> Result<Option<String>, ApiError> {
    writing(
        args,
        name,
        TEXT_MAX,
        "a record holds up to 64 KiB. Split it into several, or put the whole text on a source \
         with add_source and cite it.",
    )
}

/// `source_file` and `number`, both or neither. Blank ones are for the gate to refuse.
fn number_args(args: &Value) -> Result<Option<(String, String)>, ApiError> {
    match (short(args, "source_file")?, short(args, "number")?) {
        (None, None) => Ok(None),
        (Some(file), Some(number)) => Ok(Some((file, number))),
        _ => Err(invalid("Give both source_file and number.".into())),
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
/// date the owner would have to untangle, so that is refused here, as is one that reads as two
/// different dates: the owner's day order says nothing about how the source wrote it. Whatever
/// else the session has to say about a date goes in `warnings`. Blank is no date.
fn date_arg(
    s: &Session,
    args: &Value,
    name: &str,
    warnings: &mut Vec<String>,
) -> Result<Option<String>, ApiError> {
    let Some(raw) = optional_str(args, name)?
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
    else {
        return Ok(None);
    };
    let preview = parse_date_preview(raw, s.settings().day_order);
    if preview.status == DateStatus::Unparsed {
        return Err(invalid(format!(
            "\"{raw}\" is not understood as a date. {SOURCE_DATE} Never guess one: if the source \
             gives no date like that, leave it out and put what the source says in a note."
        )));
    }
    let (dmy, mdy) = (
        parse_date_preview(raw, DayOrder::Dmy),
        parse_date_preview(raw, DayOrder::Mdy),
    );
    let both = dmy.status == DateStatus::Ok && mdy.status == DateStatus::Ok;
    if both && dmy.understood_as != mdy.understood_as {
        return Err(invalid(format!(
            "\"{raw}\" can be read two ways, as {} or as {}. Write the month as a word, like 3 APR \
             1901. If the source does not say which, put what it says in a note.",
            dmy.understood_as, mdy.understood_as
        )));
    }
    warnings.extend(preview.warnings);
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
    answer(&json!({"batch": batch, "id": batch}))
}

fn end_batch<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let report = writing(
        args,
        "report",
        REPORT_MAX,
        "a report holds up to 64 KiB. Keep to what was added, what was left unsettled, and the \
         questions for the family.",
    )?
    .filter(|report| !report.trim().is_empty())
    .ok_or_else(|| {
        invalid(
            "report must say what was added, what was left unsettled, and the questions for the \
             family."
                .into(),
        )
    })?;
    host.with_session(&mut |s| s.end_batch(ctx, batch, &report))?;
    // Nothing in the tree changed, so nobody is named: this tells the app to look at its batches.
    host.changed(&[], ctx);
    answer(&json!({"batch": batch, "closed": true}))
}

fn add_person<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let number = number_args(args)?;
    let draft = PersonDraft {
        given: short(args, "given")?,
        surnames: surnames_arg(args)?.unwrap_or_default(),
        patronymic: short(args, "patronymic")?,
        sex: word_arg(args, "sex", "sex", SEXES)?.unwrap_or(SexKind::Unknown),
        birth_date_raw: None,
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
    apply(
        host,
        ctx,
        |s, warnings| {
            let mut cmd = cmd.clone();
            let date = date_arg(s, args, "birth_date", warnings)?;
            if let EditCommand::CreatePerson { draft } | EditCommand::QuickAdd { draft, .. } =
                &mut cmd
            {
                draft.birth_date_raw = date;
            }
            let (write, reached) = match &number {
                Some((file, number)) => {
                    s.apply_numbered_as_agent(cmd.clone(), file, number, ctx, batch)?
                }
                None => s.apply_as_agent_reaching(cmd.clone(), ctx, batch)?,
            };
            let child = child_of_write(&cmd, &write);
            Ok((write, reached, child))
        },
        focus,
    )
}

fn link_people<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let person = uuid_arg(args, "person")?;
    let given = relation_arg(args)?.ok_or_else(|| invalid("relation must have as".into()))?;
    if given.to.is_none() {
        if given.relation != Relation::Child || given.family.is_none() {
            return Err(invalid(
                "relation.to must be the id of a person. Only a child can go straight into a \
                 family, with as: child and family."
                    .into(),
            ));
        }
        if given.step {
            return Err(invalid(
                "A stepchild is the child of one partner only: give that partner as relation.to."
                    .into(),
            ));
        }
    }
    let rel = given.link.unwrap_or(ParentRelId::Birth);
    write(
        host,
        ctx,
        batch,
        |s, _| {
            Ok(match (given.to, given.family) {
                (Some(anchor), family) => EditCommand::LinkExisting {
                    anchor,
                    relation: given.relation,
                    person,
                    family,
                    new_parent_set: None,
                    parent_rel: given.link,
                },
                (None, family) => EditCommand::LinkChild {
                    family: family.ok_or_else(|| invalid("relation.family is needed".into()))?,
                    child: person,
                    rel: (rel, rel),
                    // A family the person already has stays the one that counts for them.
                    preferred: s.tree().families_as_child(person).next().is_none(),
                },
            })
        },
        |_, _| Some(person),
    )
}

fn add_family<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let cmd = EditCommand::SaveFamily {
        id: None,
        partner1: optional_uuid(args, "partner1")?,
        partner2: optional_uuid(args, "partner2")?,
        kind: word_arg(args, "kind", "kind", UNIONS)?.unwrap_or(UnionKindId::Unknown),
    };
    write(
        host,
        ctx,
        batch,
        |_, _| Ok(cmd.clone()),
        |s, result| written(result, |id| s.tree().families.contains_key(id)),
    )
}

fn remove_own_empty_family<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let family = uuid_arg(args, "family")?;
    apply(
        host,
        ctx,
        |s, _| {
            let (write, reached) = s.remove_own_empty_family(family, ctx, batch)?;
            Ok((write, reached, None))
        },
        |_, _| Some(family),
    )
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
fn name_input(
    s: &Session,
    args: &Value,
    old: Option<&Name>,
    warnings: &mut Vec<String>,
) -> Result<NameInput, ApiError> {
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
        date_raw: date_arg(s, args, "date", warnings)?
            .or_else(|| old.and_then(|o| o.date.as_ref()).map(|d| d.raw.clone())),
    })
}

fn add_name<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let person = uuid_arg(args, "person")?;
    write(
        host,
        ctx,
        batch,
        |s, warnings| {
            let name = name_input(s, args, None, warnings)?;
            Ok(EditCommand::SaveName { person, name })
        },
        |s, _| s.tree().persons.get(&person)?.names.last().map(|n| n.id),
    )
}

fn update_own_name<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let person = uuid_arg(args, "person")?;
    let id = uuid_arg(args, "name")?;
    write(
        host,
        ctx,
        batch,
        |s, warnings| {
            let old = s
                .tree()
                .persons
                .get(&person)
                .and_then(|p| p.names.iter().find(|n| n.id == id))
                .ok_or_else(|| ApiError::not_found("name", id))?;
            let name = name_input(s, args, Some(old), warnings)?;
            Ok(EditCommand::SaveName { person, name })
        },
        |_, _| Some(id),
    )
}

/// An event from the arguments, on top of what `old` says.
fn event_input(
    s: &Session,
    args: &Value,
    old: Option<&Event>,
    warnings: &mut Vec<String>,
) -> Result<EventInput, ApiError> {
    let kind = short(args, "kind")?
        .or_else(|| old.map(|o| event_kind_id(&o.kind)))
        .ok_or_else(|| invalid("kind must be a kind of event, or custom:<your words>".into()))?;
    let was = |field: fn(&Event) -> &Option<String>| old.and_then(|o| field(o).clone());
    Ok(EventInput {
        id: old.map(|o| o.common.id),
        kind,
        date_raw: date_arg(s, args, "date", warnings)?
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
    write(
        host,
        ctx,
        batch,
        |s, warnings| {
            let event = event_input(s, args, None, warnings)?;
            Ok(EditCommand::SaveEvent { owner, event })
        },
        |s, result| written(result, |id| s.tree().events.contains_key(id)),
    )
}

fn update_own_event<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let id = uuid_arg(args, "event")?;
    write(
        host,
        ctx,
        batch,
        |s, warnings| {
            let old = s
                .tree()
                .events
                .get(&id)
                .ok_or_else(|| ApiError::not_found("event", id))?;
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
            let event = event_input(s, args, Some(old), warnings)?;
            Ok(EditCommand::SaveEvent { owner, event })
        },
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
        ctx,
        batch,
        |_, _| {
            Ok(EditCommand::SaveNote {
                target,
                note: note.clone(),
            })
        },
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
    write(
        host,
        ctx,
        batch,
        |s, _| {
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
            let note = NoteInput {
                id: Some(id),
                kind: kind.unwrap_or(was),
                text: text.clone(),
            };
            Ok(EditCommand::SaveNote { target, note })
        },
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
        text: writing(
            args,
            "text",
            SOURCE_TEXT_MAX,
            "a source holds up to 1 MiB. Split it across several sources.",
        )?,
    };
    write(
        host,
        ctx,
        batch,
        |_, _| {
            Ok(EditCommand::SaveSource {
                source: source.clone(),
            })
        },
        |s, result| written(result, |id| s.tree().sources.contains_key(id)),
    )
}

fn add_citation<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let target = target_arg(args)?;
    let source = uuid_arg(args, "source")?;
    let page = short(args, "page")?;
    let text = long(args, "text")?;
    let quality = quality_arg(args)?;
    write(
        host,
        ctx,
        batch,
        |s, warnings| {
            let citation = CitationArg::New(CitationInput {
                source: SourceRef::Id(source),
                page: page.clone(),
                date_raw: date_arg(s, args, "date", warnings)?,
                text: text.clone(),
                quality,
                evidence: None,
            });
            Ok(EditCommand::AddCitation { target, citation })
        },
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
    let page = short(args, "page")?;
    let text = long(args, "text")?;
    let quality = quality_arg(args)?;
    write(
        host,
        ctx,
        batch,
        |s, warnings| {
            let citation = CitationPatch {
                source: None,
                page: page.clone(),
                date_raw: date_arg(s, args, "date", warnings)?,
                text: text.clone(),
                quality,
                evidence: None,
            };
            Ok(EditCommand::UpdateCitation { id, citation })
        },
        |_, _| Some(id),
    )
}

fn add_sibling_marker<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    match (
        optional_uuid(args, "family")?,
        optional_uuid(args, "sibling_of")?,
    ) {
        (Some(family), None) => write(
            host,
            ctx,
            batch,
            |_, _| Ok(EditCommand::AddSiblingMarker { family }),
            focus,
        ),
        (None, Some(person)) => apply(
            host,
            ctx,
            |s, _| {
                let (write, reached) = s.add_sibling_marker_beside(person, ctx, batch)?;
                Ok((write, reached, None))
            },
            focus,
        ),
        _ => Err(invalid(
            "Give family or sibling_of, one of them: sibling_of is a person those brothers and \
             sisters belong beside."
                .into(),
        )),
    }
}

fn set_number<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let cmd = EditCommand::AddNumber {
        person: uuid_arg(args, "person")?,
        source_file: str_arg(args, "source_file")?.to_string(),
        number: str_arg(args, "number")?.to_string(),
    };
    write(host, ctx, batch, |_, _| Ok(cmd.clone()), focus)
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
    write(host, ctx, batch, |_, _| Ok(cmd.clone()), focus)
}
