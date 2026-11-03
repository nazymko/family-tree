//! Notes: records (`NOTE`, `SNOTE`) and inline `NOTE` structures.

use super::{Ctx, Rec, child, common_child, new_common, parse_uuid, push_unknown, resolve, val};
use crate::idgen::derive_id;
use crate::node::GedNode;
use gt_core::model::*;
use uuid::Uuid;

fn read_note_node(ctx: &mut Ctx, node: &GedNode, id: Uuid, shared: bool) -> Note {
    let mut n = Note {
        common: new_common(id),
        kind: NoteKind::General,
        text: node.value.clone().unwrap_or_default(),
        lang: None,
        shared,
    };
    if let Some(x) = &node.xref {
        n.common.ext_ids.push(ExternalId {
            kind: ExtIdKind::Xref,
            value: x.clone(),
        });
    }
    for (pos, c) in node.children.iter().enumerate() {
        if c.tag == "LANG" && val(c).is_some() && n.lang.is_none() {
            n.lang = val(c).map(str::to_string);
            continue;
        }
        let mut common = std::mem::replace(&mut n.common, new_common(id));
        if !common_child(ctx, c, &mut common) {
            push_unknown(ctx, &mut common.unknown, c, pos, node.children.len());
        }
        n.common = common;
    }
    n
}

pub(crate) fn read_note_record(ctx: &mut Ctx, node: &GedNode, id: Uuid) {
    let n = read_note_node(ctx, node, id, true);
    ctx.tree.notes.insert(id, n);
}

/// `NOTE`/`SNOTE` under a structure: a pointer links a shared note, text creates an inline note.
pub(crate) fn link_note(ctx: &mut Ctx, owner: Uuid, c: &GedNode, notes: &mut Vec<Uuid>) -> bool {
    if val(c).is_some() && c.ptr {
        if let Some(id) = resolve(ctx, c, Rec::Note)
            && !notes.contains(&id)
        {
            notes.push(id);
        }
        return true;
    }
    if c.tag == "SNOTE" {
        return false;
    }
    let id = child(c, "_GTID")
        .and_then(val)
        .and_then(parse_uuid)
        .unwrap_or_else(|| derive_id(owner, "note", notes.len()));
    let n = read_note_node(ctx, c, id, false);
    ctx.tree.notes.insert(id, n);
    notes.push(id);
    true
}
