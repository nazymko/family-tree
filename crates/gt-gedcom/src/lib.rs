#![forbid(unsafe_code)]

pub mod ansel;
pub mod encoding;

pub use encoding::{DetectedEncoding, decode_payload, detect, to_line_bytes};
