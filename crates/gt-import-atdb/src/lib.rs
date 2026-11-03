//! Import of a Древо Жизни (Genery) `.atdb` database: a SQLite file in an entity-attribute-value
//! layout. The result is the same `ImportOutcome` the GEDCOM importer returns, so the session
//! builds the project through its one project-building path.
//!
//! The file is opened read-only and immutable; nothing is ever written to it. What the importer
//! does not understand is kept as unknown data on the record it belongs to and counted in the
//! report's `unmapped_tags`, never dropped silently.

#![forbid(unsafe_code)]

mod convert;
mod dates;
mod db;
#[cfg(feature = "fixture")]
pub mod fixture;

use gt_gedcom::{ImportOptions, ImportOutcome};
use std::path::{Path, PathBuf};

pub use db::SQLITE_MAGIC;

#[derive(thiserror::Error, Debug)]
pub enum AtdbError {
    #[error("Cannot open {}: {reason}", .path.display())]
    Open { path: PathBuf, reason: String },
    #[error("This is not a Древо Жизни database: {0}")]
    NotAtdb(String),
    #[error("Cannot read the Древо Жизни database: {0}")]
    Sql(#[from] rusqlite::Error),
}

/// True when `path` is a SQLite file that has the Древо Жизни tables (`Persons`, `Events`,
/// `EventDetails`). Opens the file read-only; any failure is simply `false`.
pub fn is_atdb(path: &Path) -> bool {
    db::open(path).is_ok_and(|c| db::has_atdb_tables(&c))
}

/// Reads a `.atdb` file. Relative media paths resolve under `opts.gedcom_dir` (default: the folder
/// of the `.atdb`) and `opts.media_roots`.
pub fn import_atdb(path: &Path, opts: &ImportOptions) -> Result<ImportOutcome, AtdbError> {
    let conn = db::open(path)?;
    if !db::has_atdb_tables(&conn) {
        return Err(AtdbError::NotAtdb(
            "the file has no Persons, Events and EventDetails tables".into(),
        ));
    }
    let mut opts = opts.clone();
    if opts.gedcom_dir.is_none() {
        opts.gedcom_dir = path
            .parent()
            .map(Path::to_path_buf)
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(|| Some(PathBuf::from(".")));
    }
    convert::convert(&conn, &opts)
}
