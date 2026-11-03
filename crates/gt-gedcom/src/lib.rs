#![forbid(unsafe_code)]

pub mod ansel;
pub mod encoding;
pub mod export;
pub(crate) mod idgen;
pub mod import;
pub mod maps;
pub mod node;
#[cfg(feature = "synthetic")]
pub mod synthetic;
pub mod tags;
pub mod tokenizer;

pub use encoding::{DetectedEncoding, decode_payload, detect, to_line_bytes};
pub use export::{ExportOptions, export};
pub use gt_core::date::GedcomVersion;
pub use import::{
    ImportOptions, ImportOutcome, ImportReport, MissingMedia, MissingReason, Repair, import,
};
pub use node::{GedNode, TokenIssue, Tokenized};
pub use tokenizer::tokenize;
