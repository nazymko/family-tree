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
mod place_moves;
mod place_records;
mod project;
mod save;
mod search;

pub use authors::{Actor, Batch, FirstWrite, LogFilter, LogGroup, LogMeta, LogPerson, ReviewScope};
pub use backup::BackupReason;
pub use collation::COLLATION_NAME;
pub use edit::{Change, ChangeOp, ChangeSet, HistoryEntry, Record, RecordKind};
pub use location::{LocationWarning, check_location};
pub use media::{StoredMedia, add_media_file_as, add_media_file_in};
pub use merge::{LogRow, MergedGroup};
pub use place_moves::PlaceRecordAdjust;
pub use place_records::{
    AddOutcome, PLACE_RECORD_KINDS, PlaceRecordCounts, PlaceRecordFilter, PlaceRecordInput,
    PlaceRecordKey, PlaceRecordPerson, PlaceRecordRow, fold_search, kind_from_section,
    normalize_kind, year_from_text,
};
pub use project::{Project, SCHEMA_VERSION, StoreError};
pub use search::{HitKind, SearchHit};
