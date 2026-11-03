#![forbid(unsafe_code)]
//! Gramps XML (`.gramps` = gzip, or plain XML; schema 1.x) -> `gt_gedcom::ImportOutcome`.
//!
//! Handles (`_abc`) are cross-references; they are resolved in two passes (all handles get an id
//! first). Ids are derived from the handle, so importing the same file twice gives the same ids.
//! What cannot be mapped is kept as `RawNode`s on the owning record and counted in the report.

mod date;
mod records;

use gt_core::tree::Tree;
use gt_gedcom::{ImportOptions, ImportOutcome, ImportReport};
use roxmltree::{Document, ParsingOptions};
use std::io::Read;

/// Largest XML accepted after decompression.
const MAX_XML_BYTES: u64 = 1 << 30;
const MAX_NODES: u32 = 50_000_000;

#[derive(Debug, thiserror::Error)]
pub enum GrampsError {
    #[error("the gzip data could not be read: {0}")]
    Gzip(String),
    #[error("the file is larger than {} MiB once decompressed", MAX_XML_BYTES >> 20)]
    TooLarge,
    #[error("the file is not valid UTF-8 text")]
    NotUtf8,
    #[error("the XML could not be parsed: {0}")]
    Xml(String),
    #[error("this is not a Gramps XML file (root element is `{0}`)")]
    NotGramps(String),
}

/// True when `bytes` start with the gzip magic.
pub fn is_gzip(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x1f, 0x8b])
}

fn decode(bytes: &[u8]) -> Result<String, GrampsError> {
    let raw: Vec<u8> = if is_gzip(bytes) {
        let mut out = Vec::new();
        flate2::read::MultiGzDecoder::new(bytes)
            .take(MAX_XML_BYTES + 1)
            .read_to_end(&mut out)
            .map_err(|e| GrampsError::Gzip(e.to_string()))?;
        if out.len() as u64 > MAX_XML_BYTES {
            return Err(GrampsError::TooLarge);
        }
        out
    } else {
        bytes.to_vec()
    };
    let text = String::from_utf8(raw).map_err(|_| GrampsError::NotUtf8)?;
    Ok(text
        .strip_prefix('\u{feff}')
        .map(str::to_string)
        .unwrap_or(text))
}

/// Import a Gramps XML database. Problems that do not stop the import are reported, never dropped.
pub fn import_gramps(bytes: &[u8], opts: &ImportOptions) -> Result<ImportOutcome, GrampsError> {
    let text = decode(bytes)?;
    let doc = Document::parse_with_options(
        &text,
        ParsingOptions {
            allow_dtd: true,
            nodes_limit: MAX_NODES,
            ..ParsingOptions::default()
        },
    )
    .map_err(|e| GrampsError::Xml(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "database" {
        return Err(GrampsError::NotGramps(root.tag_name().name().to_string()));
    }
    let mut ctx = records::Ctx::new(&doc, opts);
    ctx.report = ImportReport {
        encoding: "UTF-8".to_string(),
        version: root
            .tag_name()
            .namespace()
            .and_then(|ns| ns.trim_end_matches('/').rsplit('/').next())
            .map(str::to_string),
        ..ImportReport::default()
    };
    ctx.run(root);
    let (tree, report, resolved_media): (Tree, ImportReport, _) = ctx.finish();
    Ok(ImportOutcome {
        tree,
        report,
        resolved_media,
    })
}
