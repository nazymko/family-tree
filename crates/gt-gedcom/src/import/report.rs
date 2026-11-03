//! What the importer found, repaired and could not map.

use crate::node::TokenIssue;
use std::collections::BTreeMap;
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Repair {
    DanglingPointer { line: u32, xref: String },
    DuplicateXref { line: u32, xref: String },
    OneWayLink { xref_from: String, xref_to: String },
    MissingHeader,
    MissingTrailer,
    Token(TokenIssue),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub encoding: String,
    pub version: Option<String>,
    pub producer: Option<String>,
    pub counts: BTreeMap<String, usize>,
    pub repairs: Vec<Repair>,
    pub unmapped_tags: BTreeMap<String, usize>,
    pub unparsed_dates: Vec<(u32, String)>,
    pub missing_media: Vec<String>,
}

impl ImportReport {
    /// Human-readable summary.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "Encoding: {}", self.encoding);
        let _ = writeln!(
            s,
            "GEDCOM version: {}",
            self.version.as_deref().unwrap_or("?")
        );
        let _ = writeln!(s, "Producer: {}", self.producer.as_deref().unwrap_or("?"));
        for (k, v) in &self.counts {
            let _ = writeln!(s, "  {k}: {v}");
        }
        if !self.repairs.is_empty() {
            let _ = writeln!(s, "Repairs ({}):", self.repairs.len());
            for r in &self.repairs {
                let _ = writeln!(s, "  {r:?}");
            }
        }
        if !self.unmapped_tags.is_empty() {
            let _ = writeln!(s, "Kept as unmapped data:");
            for (k, v) in &self.unmapped_tags {
                let _ = writeln!(s, "  {k} x{v}");
            }
        }
        if !self.unparsed_dates.is_empty() {
            let _ = writeln!(
                s,
                "Dates kept as unparsed text ({}):",
                self.unparsed_dates.len()
            );
            for (l, d) in &self.unparsed_dates {
                let _ = writeln!(s, "  line {l}: {d}");
            }
        }
        if !self.missing_media.is_empty() {
            let _ = writeln!(s, "Media files not found ({}):", self.missing_media.len());
            for m in &self.missing_media {
                let _ = writeln!(s, "  {m}");
            }
        }
        s
    }
}
