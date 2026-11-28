//! Cleaning up empty and parentless families: one read tool that lists them, and two writes that
//! remove a whole family or one child link, of any author, inside the agent's batch.

use crate::tools_read::{ToolResult, answer, invalid, uuid_arg};
use crate::tools_write::{batch_arg, long, optional_uuid, with_batch};
use crate::{AgentCtx, Host, Tool};
use gt_session::{EmptyFamilyKind, Session};
use serde_json::{Value, json};

const LIST_MAX: usize = 100;

pub(crate) fn tools<H: Host>() -> Vec<Tool<H>> {
    vec![
        Tool {
            name: "list_empty_families",
            description: "Lists the families that are empty or have no parents: kind parentless (no partners, with children), singlePartner (one partner, no children) and empty (no partners, no children); kind narrows the list to one. For each family: its id, partners, children (each with otherParentFamilies, the number of its other families that have a partner, and hasOtherFamily), counts of notes, citations and media, its events, who created it and when from the log (origin: owner, agent or copy; absent for a family that came with an imported file), removable, and blockers, the reasons remove_empty_family would refuse it. needsNotesTo says the family holds notes, citations or pictures: give notes_to (suggestedNotesTo is a member to use). Read only; one page at a time (offset, limit up to 100, nextOffset).",
            schema: json!({
                "type": "object",
                "properties": {
                    "kind": {"type": "string", "enum": ["parentless", "singlePartner", "empty"], "description": "Only families of this kind."},
                    "offset": {"type": "integer", "minimum": 0},
                    "limit": {"type": "integer", "minimum": 1, "maximum": LIST_MAX},
                },
                "additionalProperties": false,
            }),
            call: list_empty_families::<H>,
        },
        Tool {
            name: "remove_empty_family",
            description: "Removes an empty or parentless family of ANY author (the owner's, an import's, an earlier batch's) in your batch; the owner undoes it with the batch, which brings back the family, its child links and its notes. Give the reason (required: why the family is empty or wrong, and who or what showed it); it goes in the log and in research notes. With no partners the child links go with the family, each child gets a research note. The family's notes, citations and pictures move to notes_to, a partner or a child of the family; it is refused when the family has any of them and notes_to is missing. Refused, with the reason, when: the family has two partners; it has one partner AND children (a real single-parent family); any child would be left with no parent family (a child needs another family with at least one partner, else removing the link cuts the person off from the родовід: link them to their parents first); the family has events (marriage and the like); the reason is empty. It never changes whether anyone counts as alive or dead. list_empty_families shows what to remove and what would be refused.",
            schema: with_batch(
                &["family_id", "reason"],
                json!({
                    "family_id": {"type": "string", "description": "The id of the family to remove."},
                    "reason": {"type": "string", "maxLength": 1000, "description": "Why it is empty or wrong, and what showed it."},
                    "notes_to": {"type": "string", "description": "A partner or a child of the family who takes over its notes, citations and pictures. Required when the family has any."},
                }),
            ),
            call: remove_empty_family::<H>,
        },
        Tool {
            name: "remove_child_from_family",
            description: "Removes one child link in a family, whoever made it, in your batch; the owner undoes it with the batch. Only the link goes: the person, the family, its partners and its other children stay, and a family left with no partners and no children stays too (remove_empty_family takes it away). Allowed only when the child keeps at least one other family that has at least one partner; otherwise it is refused, because the person would be cut off from the родовід. Give the reason (required); a research note on the child records the family and the reason. It never changes whether anyone counts as alive or dead.",
            schema: with_batch(
                &["family_id", "child_id", "reason"],
                json!({
                    "family_id": {"type": "string", "description": "The family the child is to leave."},
                    "child_id": {"type": "string", "description": "The child whose link in that family goes."},
                    "reason": {"type": "string", "maxLength": 1000, "description": "Why the link is wrong, and what showed it."},
                }),
            ),
            call: remove_child_from_family::<H>,
        },
    ]
}

fn usize_arg(args: &Value, name: &str) -> Result<Option<usize>, gt_session::ApiError> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .map(Some)
            .ok_or_else(|| invalid(format!("{name} must be a whole number"))),
    }
}

fn list_empty_families<H: Host>(host: &H, _: &AgentCtx, args: &Value) -> ToolResult {
    let kind = match args.get("kind") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            serde_json::from_value::<EmptyFamilyKind>(v.clone()).map_err(|_| {
                invalid("kind must be one of: parentless, singlePartner, empty".into())
            })?,
        ),
    };
    let offset = usize_arg(args, "offset")?.unwrap_or(0);
    let limit = usize_arg(args, "limit")?
        .unwrap_or(LIST_MAX)
        .clamp(1, LIST_MAX);
    let rows = host.with_session(&mut |s: &mut Session| s.list_empty_families_as_agent(kind))?;
    let total = rows.len();
    let page: Vec<_> = rows.into_iter().skip(offset).take(limit).collect();
    let next = (offset + page.len() < total).then_some(offset + page.len());
    answer(&json!({"total": total, "families": page, "nextOffset": next}))
}

fn reason_arg(args: &Value, what: &str) -> Result<String, gt_session::ApiError> {
    long(args, "reason")?
        .filter(|r| !r.trim().is_empty())
        .ok_or_else(|| invalid(format!("reason must say {what}.")))
}

fn remove_empty_family<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let family = uuid_arg(args, "family_id")?;
    let notes_to = optional_uuid(args, "notes_to")?;
    let reason = reason_arg(args, "why this family is empty or wrong")?;
    let (done, reached) = host.with_session(&mut |s: &mut Session| {
        s.remove_empty_family_as_agent(family, &reason, notes_to, ctx, batch)
    })?;
    host.changed(&reached, ctx);
    answer(&json!({
        "removed": done.removed,
        "childrenUnlinked": done.children_unlinked,
        "movedTo": done.moved_to,
        "moved": {"notes": done.notes, "citations": done.citations, "media": done.media},
    }))
}

fn remove_child_from_family<H: Host>(host: &H, ctx: &AgentCtx, args: &Value) -> ToolResult {
    let batch = batch_arg(args)?;
    let family = uuid_arg(args, "family_id")?;
    let child = uuid_arg(args, "child_id")?;
    let reason = reason_arg(args, "why this child does not belong in this family")?;
    let reached = host.with_session(&mut |s: &mut Session| {
        s.remove_child_from_family_as_agent(family, child, &reason, ctx, batch)
    })?;
    host.changed(&reached, ctx);
    answer(&json!({"family": family, "removedChild": child}))
}
