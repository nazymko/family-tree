#![forbid(unsafe_code)]
//! SQLite project store: project folder, schema, migrations, save/load and search.

mod codec;
mod collation;
mod load;
pub mod migrate;
mod project;
mod save;
mod search;

pub use collation::COLLATION_NAME;
pub use project::{Project, SCHEMA_VERSION, StoreError};
pub use search::{HitKind, SearchHit};
