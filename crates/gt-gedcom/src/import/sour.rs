//! Source and repository records, and citations (`SOUR` under any structure).

use super::{
    Ctx, Rec, child, common_child, count_dropped, is_pointer, new_common, parse_uuid, push_unknown,
    read_date, resolve, val,
};
use crate::idgen::derive_id;
use crate::node::GedNode;
use gt_core::model::*;
use uuid::Uuid;

fn xref_ext(node: &GedNode, common: &mut Common) {
    if let Some(x) = &node.xref {
        common.ext_ids.push(ExternalId {
            kind: ExtIdKind::Xref,
            value: x.clone(),
        });
    }
}

fn blank_source(id: Uuid) -> Source {
    Source {
        common: new_common(id),
        title: None,
        author: None,
        publication: None,
        abbreviation: None,
        text: None,
        repositories: vec![],
    }
}

/// A `SOUR` under a structure: a new `Citation` linked into `citations`. Returns true when consumed.
pub(crate) fn link_citation(
    ctx: &mut Ctx,
    owner: Uuid,
    c: &GedNode,
    citations: &mut Vec<Uuid>,
) -> bool {
    let id = child(c, "_GTID")
        .and_then(val)
        .and_then(parse_uuid)
        .unwrap_or_else(|| derive_id(owner, "cit", citations.len()));
    let mut cit = Citation {
        common: new_common(id),
        source: None,
        page: None,
        date: None,
        text: None,
        quality: None,
        evidence: Evidence {
            original: None,
            primary_info: None,
            direct: None,
        },
    };
    match val(c) {
        Some(p) if is_pointer(p) => {
            if !p.trim().eq_ignore_ascii_case("@VOID@") {
                match resolve(ctx, c, Rec::Sour) {
                    Some(s) => cit.source = Some(s),
                    None => return true, // dangling: the link is dropped
                }
            }
        }
        Some(text) => {
            let sid = derive_id(id, "src", 0);
            let mut s = blank_source(sid);
            s.text = Some(text.to_string());
            ctx.tree.sources.insert(sid, s);
            cit.source = Some(sid);
        }
        None => {}
    }
    for (pos, k) in c.children.iter().enumerate() {
        let v = val(k).map(str::to_string);
        match (k.tag.as_str(), v) {
            ("PAGE", Some(v)) => cit.page = Some(v),
            ("TEXT", Some(v)) if cit.text.is_none() => cit.text = Some(v),
            ("DATA", _) => {
                for d in &k.children {
                    match (d.tag.as_str(), val(d)) {
                        ("DATE", _) if cit.date.is_none() => cit.date = read_date(ctx, d),
                        ("TEXT", Some(t)) if cit.text.is_none() => cit.text = Some(t.to_string()),
                        _ => count_dropped(ctx, "DATA.child"),
                    }
                }
            }
            ("QUAY", Some(q)) if matches!(q.trim(), "0" | "1" | "2" | "3") => {
                cit.quality = q.trim().parse().ok();
            }
            ("_APID", Some(v)) => cit.common.ext_ids.push(ExternalId {
                kind: ExtIdKind::AncestryApid,
                value: v,
            }),
            _ => {
                let mut common = std::mem::replace(&mut cit.common, new_common(id));
                if !common_child(ctx, k, &mut common) {
                    push_unknown(ctx, &mut common.unknown, k, pos);
                }
                cit.common = common;
            }
        }
    }
    ctx.tree.citations.insert(id, cit);
    citations.push(id);
    true
}

pub(crate) fn read_source(ctx: &mut Ctx, node: &GedNode, id: Uuid) {
    let mut s = blank_source(id);
    xref_ext(node, &mut s.common);
    for (pos, c) in node.children.iter().enumerate() {
        let v = val(c).map(str::to_string);
        match (c.tag.as_str(), v) {
            ("TITL", Some(v)) => s.title = Some(v),
            ("AUTH", Some(v)) => s.author = Some(v),
            ("PUBL", Some(v)) => s.publication = Some(v),
            ("ABBR", Some(v)) => s.abbreviation = Some(v),
            ("TEXT", Some(v)) => s.text = Some(v),
            ("REPO", Some(p)) if is_pointer(&p) => {
                if let Some(r) = resolve(ctx, c, Rec::Repo) {
                    s.repositories.push(RepoRef {
                        repository: r,
                        call_number: child(c, "CALN").and_then(val).map(str::to_string),
                    });
                }
            }
            _ => {
                let mut common = std::mem::replace(&mut s.common, new_common(id));
                if !common_child(ctx, c, &mut common) {
                    push_unknown(ctx, &mut common.unknown, c, pos);
                }
                s.common = common;
            }
        }
    }
    ctx.tree.sources.insert(id, s);
}

pub(crate) fn read_repository(ctx: &mut Ctx, node: &GedNode, id: Uuid) {
    let mut r = Repository {
        common: new_common(id),
        name: String::new(),
        address: None,
        urls: vec![],
    };
    xref_ext(node, &mut r.common);
    for (pos, c) in node.children.iter().enumerate() {
        match c.tag.as_str() {
            "NAME" => r.name = val(c).unwrap_or("").to_string(),
            "ADDR" if c.children.is_empty() && r.address.is_none() && val(c).is_some() => {
                r.address = val(c).map(str::to_string);
            }
            "WWW" if val(c).is_some() => r.urls.extend(val(c).map(str::to_string)),
            _ => {
                let mut common = std::mem::replace(&mut r.common, new_common(id));
                if !common_child(ctx, c, &mut common) {
                    push_unknown(ctx, &mut common.unknown, c, pos);
                }
                r.common = common;
            }
        }
    }
    ctx.tree.repositories.insert(id, r);
}
