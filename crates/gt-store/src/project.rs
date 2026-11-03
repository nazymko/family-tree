use crate::migrate::{MIGRATIONS, migrate, user_version};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: i32 = 1;
const DB_FILE: &str = "tree.db";
const SUBDIRS: [&str; 3] = ["media", "thumbs", "backups"];

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("not a gene-tree project: {0}")]
    NotAProject(PathBuf),
    #[error("project schema {found} is newer than supported {supported}")]
    NewerSchema { found: i32, supported: i32 },
    #[error("corrupt project data: {0}")]
    Corrupt(String),
    #[error("a project already exists at {0}")]
    AlreadyExists(PathBuf),
}

/// An open project folder (`tree.db`, `media/`, `thumbs/`, `backups/`).
#[derive(Debug)]
pub struct Project {
    pub dir: PathBuf,
    pub(crate) conn: Connection,
    version: i32,
}

fn setup(conn: &Connection) -> Result<(), StoreError> {
    let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    crate::collation::register(conn)?;
    Ok(())
}

impl Project {
    pub fn create(dir: &Path) -> Result<Project, StoreError> {
        let db = dir.join(DB_FILE);
        if db.exists() {
            return Err(StoreError::AlreadyExists(dir.to_path_buf()));
        }
        std::fs::create_dir_all(dir)?;
        for sub in SUBDIRS {
            std::fs::create_dir_all(dir.join(sub))?;
        }
        let mut conn = Connection::open(&db)?;
        setup(&conn)?;
        let version = migrate(&mut conn, MIGRATIONS)?;
        let created_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO meta(key, value) VALUES ('app_version', ?1), ('created_ms', ?2)",
            (env!("CARGO_PKG_VERSION"), created_ms.to_string()),
        )?;
        Ok(Project {
            dir: dir.to_path_buf(),
            conn,
            version,
        })
    }

    pub fn open(dir: &Path) -> Result<Project, StoreError> {
        let db = dir.join(DB_FILE);
        if !db.is_file() {
            return Err(StoreError::NotAProject(dir.to_path_buf()));
        }
        let mut conn = Connection::open(&db)?;
        // Check the version before touching anything else (WAL setup writes).
        let found = user_version(&conn)?;
        if found > SCHEMA_VERSION {
            return Err(StoreError::NewerSchema {
                found,
                supported: SCHEMA_VERSION,
            });
        }
        setup(&conn)?;
        let version = migrate(&mut conn, MIGRATIONS)?;
        for sub in SUBDIRS {
            std::fs::create_dir_all(dir.join(sub))?;
        }
        Ok(Project {
            dir: dir.to_path_buf(),
            conn,
            version,
        })
    }

    pub fn schema_version(&self) -> i32 {
        self.version
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}
