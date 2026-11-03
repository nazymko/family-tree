//! Placeholder; replaced by Task 3.

pub mod calendar;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GenDate {
    pub raw: String,
}
