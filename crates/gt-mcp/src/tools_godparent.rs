//! Godparents (хрещені) of a baptism or christening: adding one, taking back one's own, and how
//! they read on an event.

use crate::tools_read::{ToolResult, invalid, str_arg, uuid_arg};
use crate::tools_write::{apply, batch_arg, optional_uuid, short, with_batch, write};
use crate::{AgentCtx, Host, Tool};
use gt_core::model::Godparent;
use gt_session::ops::godparent::same_name;
use gt_session::{EditCommand, GodparentInput, Session};
use serde_json::{Value, json};
use uuid::Uuid;

pub(crate) fn tools<H: Host>() -> Vec<Tool<H>> {
    let text = |what: &str| json!({"type": "string", "description": what, "maxLength": 1000});
    let id = |what: &str| json!({"type": "string", "description": what});
    vec![
        Tool {
            name: "add_godparent",
            description: "Adds a godparent (хрещений батько or хрещена мати) to a baptism or christening event. Give the event (add_event with kind baptism makes one), role godfather or godmother, and exactly one of: person, the id of someone already in the tree, only when you are certain the godparent is that person; or name, the godparent as the record writes them, with what it adds (\"Ivan Petrovych Koval, a peasant of Ivanivka\"). Most godparents in metric books are nobody else in the tree: use name, and never create a person just to be a godparent. Take godparents only from the metric record (or the family) and cite it on the event with add_citation. The same person or name on the same event again answers alreadyThere. The answer's id is the godparent's, for remove_own_godparent.",
            schema: with_batch(
                &["event", "role"],
                json!({
                    "event": id("The baptism or christening event."),
                    "person": id("A person in the tree, when you are certain it is them."),
                    "name": text("The godparent as the record writes them."),
                    "role": {"type": "string", "enum": ["godfather", "godmother"]},
                    "note": text("What else the record says of them."),
                }),
            ),
            call: add_godparent::<H>,
        },
        Tool {
            name: "remove_own_godparent",
            description: "Takes a godparent you added yourself off its event, in this batch or an earlier one, when nobody has changed it since: when the record turned out to say something else. Godparents anyone else added stay: leave a note and list it in your report.",
            schema: with_batch(
                &["event", "godparent"],
                json!({
                    "event": id("The event."),
                    "godparent": id("The godparent's id, from add_godparent or get_event."),
                }),
            ),
            call: remove_own_godparent::<H>,
        },
    ]
}

/// How a godparent reads on an event.
pub(crate) fn godparent_rows(s: &Session, list: &[Godparent]) -> Vec<Value> {
    list.iter()
        .map(|g| {
            let mut row = json!({"id": g.id, "role": g.role.id()});
            if let Some(p) = g.person {
                row["personId"] = json!(p);
                row["person"] = json!(s.person_ref(p));
            }
            if let Some(n) = &g.name {
                row["name"] = json!(n);
            }
            if let Some(n) = &g.note {
                row["note"] = json!(n);
            }
            row
        })
        .collect()
}

fn add_godparent<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let event = uuid_arg(args, "event")?;
    let role = str_arg(args, "role")?.trim().to_ascii_lowercase();
    if !matches!(role.as_str(), "godfather" | "godmother") {
        return Err(invalid("role must be godfather or godmother.".into()));
    }
    let person = optional_uuid(args, "person")?;
    let name = short(args, "name")?.filter(|n| !n.trim().is_empty());
    if person.is_some() == name.is_some() {
        return Err(invalid(
            "Give exactly one of person (someone already in the tree) and name (the godparent as \
             the record writes them)."
                .into(),
        ));
    }
    let godparent = GodparentInput {
        id: None,
        person,
        name: name.clone(),
        role,
        note: short(args, "note")?.filter(|n| !n.trim().is_empty()),
    };
    write(
        host,
        ctx,
        batch,
        |_, _| {
            Ok(EditCommand::AddGodparent {
                event,
                godparent: godparent.clone(),
            })
        },
        |s, _| {
            let e = s.tree().events.get(&event)?;
            e.godparents
                .iter()
                .rev()
                .find(|g| match (person, name.as_deref()) {
                    (Some(p), _) => g.person == Some(p),
                    (None, Some(n)) => g.name.as_deref().is_some_and(|o| same_name(o, n)),
                    _ => false,
                })
                .map(|g| g.id)
        },
    )
}

fn remove_own_godparent<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let event = uuid_arg(args, "event")?;
    let godparent: Uuid = uuid_arg(args, "godparent")?;
    apply(
        host,
        ctx,
        |s, _| {
            let (write, reached) = s.remove_own_godparent(event, godparent, ctx, batch)?;
            Ok((write, reached, None))
        },
        move |_, _| Some(godparent),
    )
}
