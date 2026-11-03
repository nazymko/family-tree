#![forbid(unsafe_code)]

pub mod ansel;
pub mod encoding;
pub mod export;
pub mod gedzip;
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
pub use gedzip::{GedzipContents, GedzipError, import_gedzip, read_gedzip};
pub use gt_core::date::GedcomVersion;
pub use import::{
    ImportOptions, ImportOutcome, ImportReport, MediaLocator, MissingMedia, MissingReason, Repair,
    import, is_lexically_under,
};
pub use node::{GedNode, TokenIssue, Tokenized};
pub use tokenizer::tokenize;
