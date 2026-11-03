#![forbid(unsafe_code)]
//! Format-neutral genealogy model.

pub mod date;
pub mod ids;
pub mod model;
pub mod phonetic;
pub mod raw;
pub mod tree;

pub use date::{DayOrder, GedcomVersion};
pub use uuid::Uuid;
