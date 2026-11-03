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
const MAX_XML_BYTES: u64 = 256 << 20;
const MAX_NODES: u32 = 10_000_000;
/// Output past this size must also stay under `MAX_RATIO` times the compressed size.
const RATIO_FLOOR: usize = 16 << 20;
const MAX_RATIO: usize = 200;

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

/// The XML bytes: inflated (streamed, refused past the size cap or an absurd ratio) or borrowed.
fn decode(bytes: &[u8]) -> Result<std::borrow::Cow<'_, [u8]>, GrampsError> {
    if !is_gzip(bytes) {
        return if bytes.len() as u64 > MAX_XML_BYTES {
            Err(GrampsError::TooLarge)
        } else {
            Ok(std::borrow::Cow::Borrowed(bytes))
        };
    }
    let mut dec = flate2::read::MultiGzDecoder::new(bytes);
    let mut out: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = dec
            .read(&mut buf)
            .map_err(|e| GrampsError::Gzip(e.to_string()))?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(buf.get(..n).unwrap_or(&[]));
        if out.len() as u64 > MAX_XML_BYTES
            || (out.len() > RATIO_FLOOR && out.len() / MAX_RATIO > bytes.len())
        {
            return Err(GrampsError::TooLarge);
        }
    }
    Ok(std::borrow::Cow::Owned(out))
}

/// Import a Gramps XML database. Problems that do not stop the import are reported, never dropped.
pub fn import_gramps(bytes: &[u8], opts: &ImportOptions) -> Result<ImportOutcome, GrampsError> {
    let raw = decode(bytes)?;
    let text = std::str::from_utf8(&raw).map_err(|_| GrampsError::NotUtf8)?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let doc = Document::parse_with_options(
        text,
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
    // `report.version` is the GEDCOM version (the app shows "GEDCOM <v>"); the Gramps XML format
    // version goes in the producer line instead.
    let xml_version = root
        .tag_name()
        .namespace()
        .and_then(|ns| ns.trim_end_matches('/').rsplit('/').next())
        .map(str::to_string);
    ctx.report = ImportReport {
        encoding: "UTF-8".to_string(),
        ..ImportReport::default()
    };
    ctx.run(root);
    let (tree, mut report, resolved_media): (Tree, ImportReport, _) = ctx.finish();
    if let Some(v) = xml_version {
        let p = report
            .producer
            .take()
            .unwrap_or_else(|| "Gramps".to_string());
        report.producer = Some(format!("{p} (XML format {v})"));
    }
    Ok(ImportOutcome {
        tree,
        report,
        resolved_media,
    })
}
