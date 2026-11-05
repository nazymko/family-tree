#![forbid(unsafe_code)]
//! SQLite project store: project folder, schema, migrations, save/load, search, backups, media.

mod authors;
mod backup;
mod codec;
pub mod collation;
mod edit;
mod load;
mod location;
mod media;
pub mod migrate;
mod project;
mod save;
mod search;

pub use authors::{Actor, Batch, LogFilter, LogGroup, LogMeta, LogPerson, ReviewScope};
pub use backup::BackupReason;
pub use collation::COLLATION_NAME;
pub use edit::{Change, ChangeOp, ChangeSet, HistoryEntry, Record, RecordKind};
pub use location::{LocationWarning, check_location};
pub use project::{Project, SCHEMA_VERSION, StoreError};
pub use search::{HitKind, SearchHit};
