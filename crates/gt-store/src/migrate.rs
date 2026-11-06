//! Ordered, forward-only schema migrations. `PRAGMA user_version` = number applied.

use crate::StoreError;
use rusqlite::Connection;

/// `(name, sql)`; entry `i` takes the schema from version `i` to `i + 1`.
pub const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init", include_str!("migrations/0001_init.sql")),
    ("0002_edit", include_str!("migrations/0002_edit.sql")),
    ("0003_authors", include_str!("migrations/0003_authors.sql")),
    (
        "0004_log_index",
        include_str!("migrations/0004_log_index.sql"),
    ),
    ("0005_merge", include_str!("migrations/0005_merge.sql")),
];

pub(crate) fn user_version(conn: &Connection) -> Result<i32, StoreError> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

/// Apply every migration above the current version, each in its own transaction.
/// Refuses a database written by a newer app.
pub(crate) fn migrate(
    conn: &mut Connection,
    migrations: &[(&str, &str)],
) -> Result<i32, StoreError> {
    let current = user_version(conn)?;
    let supported = i32::try_from(migrations.len()).unwrap_or(i32::MAX);
    if current > supported {
        return Err(StoreError::NewerSchema {
            found: current,
            supported,
        });
    }
    for (i, (_name, sql)) in migrations.iter().enumerate().skip(current.max(0) as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i as i32 + 1)?;
        tx.commit()?;
    }
    user_version(conn)
}
