#![forbid(unsafe_code)]
//! SQLite project store: project folder, schema and migrations.

pub mod migrate;
mod project;

pub use project::{Project, SCHEMA_VERSION, StoreError};
