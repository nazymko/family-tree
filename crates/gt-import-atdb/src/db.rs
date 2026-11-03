//! Opening the file read-only and small query helpers.

use crate::AtdbError;
use rusqlite::{Connection, OpenFlags, Row};
use std::path::{Path, PathBuf};

/// First 16 bytes of every SQLite 3 database.
pub const SQLITE_MAGIC: &[u8; 16] = b"SQLite format 3\0";

/// `file:` URI for `path` with everything but unreserved characters and `/` `:` percent-encoded.
fn uri_for(path: &Path) -> Result<String, AtdbError> {
    let abs: PathBuf = std::path::absolute(path).map_err(|e| AtdbError::Open {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })?;
    let s = abs.to_string_lossy().replace('\\', "/");
    let mut enc = String::with_capacity(s.len() + 16);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'/' | b':' | b'-' | b'_' | b'.' | b'~') {
            enc.push(char::from(b));
        } else {
            enc.push_str(&format!("%{b:02X}"));
        }
    }
    // `file:///C:/x` for a drive path, `file:///home/x` for a POSIX path.
    if let Some(unc) = enc.strip_prefix("//") {
        return Ok(format!("file://{unc}?mode=ro&immutable=1"));
    }
    let enc = enc.trim_start_matches('/');
    Ok(format!("file:///{enc}?mode=ro&immutable=1"))
}

pub fn open(path: &Path) -> Result<Connection, AtdbError> {
    let uri = uri_for(path)?;
    Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| AtdbError::Open {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
}

pub fn table_exists(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |_| Ok(()),
    )
    .is_ok()
}

pub fn has_atdb_tables(conn: &Connection) -> bool {
    ["Persons", "Events", "EventDetails"]
        .iter()
        .all(|t| table_exists(conn, t))
}

/// All rows of `sql`, or none when `table` does not exist (older or trimmed databases).
pub fn rows<T>(
    conn: &Connection,
    table: &str,
    sql: &str,
    f: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>, AtdbError> {
    if !table_exists(conn, table) {
        return Ok(Vec::new());
    }
    let mut st = conn.prepare(sql)?;
    let out = st.query_map([], f)?.collect::<Result<Vec<_>, _>>()?;
    Ok(out)
}
