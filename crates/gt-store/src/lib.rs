#![forbid(unsafe_code)]
//! SQLite project store: project folder, schema, migrations, save/load, search, backups, media.

mod backup;
mod codec;
mod collation;
mod load;
mod location;
mod media;
pub mod migrate;
mod project;
mod save;
mod search;

pub use backup::BackupReason;
pub use collation::COLLATION_NAME;
pub use location::{LocationWarning, check_location};
pub use project::{Project, SCHEMA_VERSION, StoreError};
pub use search::{HitKind, SearchHit};
