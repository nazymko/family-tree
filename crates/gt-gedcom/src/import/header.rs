//! HEAD handling: version, producer, unknown header data.

use super::{Ctx, push_unknown, val};
use crate::node::GedNode;
use gt_core::date::GedcomVersion;

/// The version declared in `HEAD.GEDC.VERS`.
pub(crate) fn detect_version(head: Option<&GedNode>) -> (GedcomVersion, Option<String>) {
    let vers = head
        .and_then(|h| h.children.iter().find(|c| c.tag == "GEDC"))
        .and_then(|g| g.children.iter().find(|c| c.tag == "VERS"))
        .and_then(val)
        .map(str::to_string);
    let v = match &vers {
        Some(s) if s.trim_start().starts_with('7') => GedcomVersion::V70,
        _ => GedcomVersion::V551,
    };
    (v, vers)
}

/// Our own submitter record, written by the exporter when the source had none.
pub(crate) const OWN_SUBM: &str = "@GTSUBM@";

pub(crate) fn read_head(ctx: &mut Ctx, head: &GedNode) {
    for c in &head.children {
        match c.tag.as_str() {
            "SOUR" => {
                let producer = val(c).map(str::to_string);
                let version = c.children.iter().find(|x| x.tag == "VERS").and_then(val);
                ctx.report.producer = producer.clone();
                if producer.as_deref() == Some("GENE-TREE") {
                    // Our own file: the original producer travels in SOUR.DATA.
                    if let Some(d) = c.children.iter().find(|x| x.tag == "DATA") {
                        ctx.tree.header.producer = val(d).map(str::to_string);
                        ctx.tree.header.source_version = d
                            .children
                            .iter()
                            .find(|x| x.tag == "VERS")
                            .and_then(val)
                            .map(str::to_string);
                        ctx.report.producer = ctx.tree.header.producer.clone();
                    }
                } else {
                    ctx.tree.header.producer = producer;
                    ctx.tree.header.source_version = version.map(str::to_string);
                }
            }
            "GEDC" | "CHAR" | "SCHMA" => {}
            "SUBM" if val(c) == Some(OWN_SUBM) => {}
            _ => {
                let mut v = std::mem::take(&mut ctx.tree.header.unknown);
                // Position among the unknown children only: the known ones differ per version.
                let upos = v.len();
                push_unknown(ctx, &mut v, c, upos, 0);
                ctx.tree.header.unknown = v;
            }
        }
    }
}
