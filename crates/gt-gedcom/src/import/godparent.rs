//! Godparents of an event, as the exporter writes them (see `export::build::godparent_node`):
//! `ASSO` to a person with a godparent's role, a 7.0 `ASSO @VOID@` with the name as its
//! `PHRASE`, and the 5.5.1 `NOTE Godfather: <name>` (the godparent's note on the next lines).

use super::{Ctx, Rec, child, resolve, val};
use crate::maps::godparent_role_from_text;
use crate::node::GedNode;
use gt_core::model::{Godparent, GodparentRole};
use uuid::Uuid;

/// The godparent's role an `ASSO` gives, or `None` when it is no godparent.
fn asso_role(c: &GedNode) -> Option<GodparentRole> {
    if let Some(rn) = child(c, "ROLE")
        && let Some(r) = val(rn)
    {
        let phrase = child(rn, "PHRASE").and_then(val);
        if r.eq_ignore_ascii_case("GODP") {
            return Some(
                phrase
                    .and_then(godparent_role_from_text)
                    .unwrap_or(GodparentRole::Godparent),
            );
        }
        if r.eq_ignore_ascii_case("OTHER") {
            return phrase.and_then(godparent_role_from_text);
        }
        return None;
    }
    child(c, "RELA")
        .and_then(val)
        .and_then(godparent_role_from_text)
}

/// `Some(godparent)` for an `ASSO` that names a godparent, `Some(None)` for one that is a
/// godparent nobody can be found for (a pointer to nothing), `None` for any other `ASSO`.
pub(crate) fn read_godparent_asso(ctx: &mut Ctx, c: &GedNode) -> Option<Option<Godparent>> {
    let role = asso_role(c)?;
    let note = child(c, "NOTE").and_then(val).map(str::to_string);
    let void = val(c).is_some_and(|p| p.trim().eq_ignore_ascii_case("@VOID@"));
    let (person, name) = if void {
        match child(c, "PHRASE").and_then(val) {
            Some(n) if !n.trim().is_empty() => (None, Some(n.trim().to_string())),
            _ => return Some(None),
        }
    } else {
        match resolve(ctx, c, Rec::Indi) {
            Some(id) => (Some(id), None),
            None => return Some(None),
        }
    };
    Some(Some(Godparent {
        id: Uuid::nil(),
        person,
        name,
        role,
        note,
    }))
}

/// The 5.5.1 form of a godparent known only by name: `NOTE Godfather: <name>`.
pub(crate) fn read_godparent_note(c: &GedNode) -> Option<Godparent> {
    let text = val(c)?;
    let (first, rest) = text.split_once('\n').unwrap_or((text, ""));
    let (word, name) = first.split_once(':')?;
    let role = godparent_role_from_text(word)?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let note = rest.trim();
    Some(Godparent {
        id: Uuid::nil(),
        person: None,
        name: Some(name.to_string()),
        role,
        note: (!note.is_empty()).then(|| note.to_string()),
    })
}

/// Adds `g` unless the same godparent is there (a shared event is written under each principal).
pub(crate) fn add_godparent(event: Uuid, list: &mut Vec<Godparent>, mut g: Godparent) {
    let same = list.iter().any(|o| {
        o.role == g.role
            && match (o.person, g.person) {
                (Some(a), Some(b)) => a == b,
                (None, None) => o.name == g.name,
                _ => false,
            }
    });
    if !same {
        g.id = crate::idgen::derive_id(event, "godparent", list.len());
        list.push(g);
    }
}
