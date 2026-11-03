//! Media: `OBJE` records and inline/pointer `OBJE` links.

use super::media_paths::{file_name, is_url, locate, normalize};
use super::{
    Ctx, Rec, child, common_child, is_pointer, new_common, parse_uuid, push_unknown, resolve, val,
};
use crate::idgen::derive_id;
use crate::node::GedNode;
use gt_core::model::*;
use uuid::Uuid;

fn parse_region(v: &str) -> Option<Region> {
    let mut it = v.split_whitespace().map(|s| s.parse::<f32>().ok());
    let (x, y, w, h) = (it.next()??, it.next()??, it.next()??, it.next()??);
    if it.next().is_some() {
        return None;
    }
    let ok = |n: f32| n.is_finite() && (0.0..=1.0).contains(&n);
    (ok(x) && ok(y) && ok(w) && ok(h)).then_some(Region { x, y, w, h })
}

/// Link properties found on the node that references the media.
fn link_props(node: &GedNode) -> (bool, Option<Region>) {
    let mut primary = false;
    let mut region = None;
    for c in &node.children {
        match c.tag.as_str() {
            "_PRIM" | "_PHOTO" if val(c).is_some_and(|v| v.trim().eq_ignore_ascii_case("Y")) => {
                primary = true;
            }
            "_REGION" if region.is_none() => region = val(c).and_then(parse_region),
            _ => {}
        }
    }
    (primary, region)
}

fn is_link_prop(c: &GedNode) -> bool {
    matches!(c.tag.as_str(), "_PRIM" | "_PHOTO" | "_REGION")
}

fn read_media_node(ctx: &mut Ctx, node: &GedNode, id: Uuid, inline: bool) -> Option<Media> {
    let file = child(node, "FILE")?;
    let path = normalize(val(file)?);
    let mut m = Media {
        common: new_common(id),
        path: path.clone(),
        original_name: None,
        mime: None,
        title: None,
        date: None,
        description: None,
        sha256: None,
    };
    if let Some(x) = &node.xref {
        m.common.ext_ids.push(ExternalId {
            kind: ExtIdKind::Xref,
            value: x.clone(),
        });
    }
    if let Some(f) = child(file, "FORM") {
        m.mime = val(f).map(str::to_string);
    }
    if let Some(t) = child(file, "TITL") {
        m.title = val(t).map(str::to_string);
    }
    let mut file_seen = false;
    for (pos, c) in node.children.iter().enumerate() {
        match c.tag.as_str() {
            "FILE" if !file_seen => file_seen = true,
            "FORM" if m.mime.is_none() && val(c).is_some() => m.mime = val(c).map(str::to_string),
            "TITL" if m.title.is_none() && val(c).is_some() => m.title = val(c).map(str::to_string),
            _ if inline && is_link_prop(c) => {}
            _ => {
                let mut common = std::mem::replace(&mut m.common, new_common(id));
                if !common_child(ctx, c, &mut common) {
                    push_unknown(ctx, &mut common.unknown, c, pos, node.children.len());
                }
                m.common = common;
            }
        }
    }
    if !is_url(&path) {
        match locate(ctx, &path) {
            Some(found) => {
                m.original_name = file_name(&path);
                ctx.resolved_media.push((id, found));
            }
            None => ctx.report.missing_media.push(path),
        }
    }
    Some(m)
}

pub(crate) fn read_media_record(ctx: &mut Ctx, node: &GedNode, id: Uuid) {
    match read_media_node(ctx, node, id, false) {
        Some(m) => {
            ctx.tree.media.insert(id, m);
        }
        None => {
            // No FILE: keep the whole record verbatim.
            let mut v = std::mem::take(&mut ctx.tree.header.unknown);
            push_unknown(ctx, &mut v, node, usize::MAX, 0);
            ctx.tree.header.unknown = v;
        }
    }
}

/// `OBJE` under a structure: a pointer to a record, or an inline media definition.
pub(crate) fn link_media(
    ctx: &mut Ctx,
    owner: Uuid,
    c: &GedNode,
    media: &mut Vec<MediaLink>,
) -> bool {
    let (primary, region) = link_props(c);
    if let Some(p) = val(c)
        && is_pointer(p)
    {
        if let Some(id) = resolve(ctx, c, Rec::Obje) {
            media.push(MediaLink {
                media: id,
                region,
                primary,
            });
        }
        return true;
    }
    let id = child(c, "_GTID")
        .and_then(val)
        .and_then(parse_uuid)
        .unwrap_or_else(|| derive_id(owner, "obje", media.len()));
    match read_media_node(ctx, c, id, true) {
        Some(m) => {
            ctx.tree.media.insert(id, m);
            media.push(MediaLink {
                media: id,
                region,
                primary,
            });
            true
        }
        None => false,
    }
}
