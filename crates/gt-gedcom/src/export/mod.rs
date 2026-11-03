//! `Tree` -> GEDCOM 5.5.1 / 7.0 bytes.

mod build;
mod write;

use gt_core::date::GedcomVersion;
use gt_core::tree::Tree;

#[derive(Debug, Clone, Copy)]
pub struct ExportOptions {
    pub version: GedcomVersion,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            version: GedcomVersion::V70,
        }
    }
}

/// Serialise `tree`. 5.5.1: CRLF, lines of at most 255 bytes (CONC/CONT), `CHAR UTF-8`.
/// 7.0: UTF-8, LF, CONT only, extension tags declared in `HEAD.SCHMA`.
pub fn export(tree: &Tree, opts: &ExportOptions) -> Vec<u8> {
    let nodes = build::build(tree, opts.version);
    write::write(&nodes, opts.version == GedcomVersion::V70)
}
