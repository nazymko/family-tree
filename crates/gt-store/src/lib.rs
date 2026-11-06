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
mod merge;
pub mod migrate;
mod project;
mod save;
mod search;

pub use authors::{Actor, Batch, LogFilter, LogGroup, LogMeta, LogPerson, ReviewScope};
pub use backup::BackupReason;
pub use collation::COLLATION_NAME;
pub use edit::{Change, ChangeOp, ChangeSet, HistoryEntry, Record, RecordKind};
pub use merge::{LogRow, MergedGroup};
pub use location::{LocationWarning, check_location};
pub use media::add_media_file_in;
pub use project::{Project, SCHEMA_VERSION, StoreError};
pub use search::{HitKind, SearchHit};
