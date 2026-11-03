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
    pub missing_media: Vec<MissingMedia>,
    /// `FILE` values that are URLs (http, data, ...): kept as-is, never fetched.
    pub media_urls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissingReason {
    NotFound,
    /// An absolute path outside the GEDCOM's folder and the media roots; the user must confirm.
    OutsideRoots,
    /// Several files under the media roots share the basename.
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingMedia {
    pub path: String,
    pub reason: MissingReason,
    pub candidates: Vec<std::path::PathBuf>,
}

impl ImportReport {
    /// Human-readable summary.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "Encoding: {}", self.encoding);
        // Converters from other formats (Gramps, .atdb) leave the version unset.
        if let Some(v) = self.version.as_deref() {
            let _ = writeln!(s, "GEDCOM version: {v}");
        }
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
                let _ = writeln!(s, "  {} ({:?})", m.path, m.reason);
                for c in &m.candidates {
                    let _ = writeln!(s, "    candidate: {}", c.display());
                }
            }
        }
        if !self.media_urls.is_empty() {
            let _ = writeln!(s, "Media given as URLs ({}):", self.media_urls.len());
            for u in &self.media_urls {
                let _ = writeln!(s, "  {u}");
            }
        }
        s
    }
}
