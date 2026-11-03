//! Generic GEDCOM line tree.

use crate::encoding::DetectedEncoding;

#[derive(Debug, Clone, PartialEq)]
pub struct GedNode {
    pub line: u32,
    pub level: u8,
    pub xref: Option<String>,
    pub tag: String,
    pub value: Option<String>,
    pub children: Vec<GedNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenIssue {
    BadLine { line: u32, text: String },
    LevelJump { line: u32 },
    OrphanConc { line: u32 },
}

#[derive(Debug, Clone)]
pub struct Tokenized {
    pub encoding: DetectedEncoding,
    pub records: Vec<GedNode>,
    pub issues: Vec<TokenIssue>,
}

impl GedNode {
    /// Convert to a format-neutral raw node; `pos` is the index among source siblings.
    pub fn to_raw(&self, pos: u32) -> gt_core::raw::RawNode {
        gt_core::raw::RawNode {
            tag: self.tag.clone(),
            xref: self.xref.clone(),
            value: self.value.clone(),
            pos,
            children: self
                .children
                .iter()
                .zip(0u32..)
                .map(|(c, i)| c.to_raw(i))
                .collect(),
        }
    }
}
