use crate::migrate::{MIGRATIONS, migrate, user_version};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: i32 = 3;
const DB_FILE: &str = "tree.db";
const LOCK_FILE: &str = ".gene-tree.lock";
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
    #[error("This tree is open in another Gene Tree window.")]
    Locked(PathBuf),
}

/// An open project folder (`tree.db`, `media/`, `thumbs/`, `backups/`).
#[derive(Debug)]
pub struct Project {
    pub dir: PathBuf,
    pub(crate) conn: Connection,
    version: i32,
    /// Holds the exclusive OS lock for as long as the project is open; released on drop.
    _lock: std::fs::File,
}

/// Takes the exclusive lock file in `dir`. A second holder gets `StoreError::Locked`.
fn acquire_lock(dir: &Path) -> Result<std::fs::File, StoreError> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(LOCK_FILE))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(std::fs::TryLockError::WouldBlock) => Err(StoreError::Locked(dir.to_path_buf())),
        Err(std::fs::TryLockError::Error(e)) => Err(StoreError::Io(e)),
    }
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
        let lock = acquire_lock(dir)?;
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
        conn.execute(
            "INSERT INTO meta(key, value) VALUES ('fts_scheme', '2')",
            [],
        )?;
        Ok(Project {
            dir: dir.to_path_buf(),
            conn,
            version,
            _lock: lock,
        })
    }

    pub fn open(dir: &Path) -> Result<Project, StoreError> {
        let db = dir.join(DB_FILE);
        if !db.is_file() {
            return Err(StoreError::NotAProject(dir.to_path_buf()));
        }
        let lock = acquire_lock(dir)?;
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
        if found < SCHEMA_VERSION {
            // Cheap insurance: a migration rewrites the file in place.
            crate::backup::vacuum_backup(&conn, dir, crate::BackupReason::BeforeMigration)?;
        }
        let version = migrate(&mut conn, MIGRATIONS)?;
        for sub in SUBDIRS {
            std::fs::create_dir_all(dir.join(sub))?;
        }
        let mut project = Project {
            dir: dir.to_path_buf(),
            conn,
            version,
            _lock: lock,
        };
        // v1 full-text rows have arbitrary row ids; v2 edits address them by seq. The key is
        // written by the rebuild itself, so an interrupted upgrade is redone on the next open.
        if project.meta_get("fts_scheme")?.is_none() {
            project.rebuild_search()?;
        }
        Ok(project)
    }

    pub fn schema_version(&self) -> i32 {
        self.version
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn open_backs_up_before_migrating() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("old");
        std::fs::create_dir_all(&dir).unwrap();
        // A project as the previous app version left it: only migration 1 applied.
        let c = Connection::open(dir.join(DB_FILE)).unwrap();
        c.execute_batch(MIGRATIONS[0].1).unwrap();
        c.pragma_update(None, "user_version", 1).unwrap();
        drop(c);

        let p = Project::open(&dir).unwrap();
        assert_eq!(p.schema_version(), SCHEMA_VERSION);
        let backups: Vec<_> = std::fs::read_dir(dir.join("backups"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(backups.len(), 1, "{backups:?}");
        assert!(backups[0].ends_with("-pre-migration.db"), "{backups:?}");
        let b = Connection::open(dir.join("backups").join(&backups[0])).unwrap();
        assert_eq!(user_version(&b).unwrap(), 1);
        drop(p);

        // Already current: no new backup.
        let p = Project::open(&dir).unwrap();
        drop(p);
        assert_eq!(std::fs::read_dir(dir.join("backups")).unwrap().count(), 1);
    }

    #[test]
    fn dropping_the_project_checkpoints_the_wal() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("p");
        let mut p = Project::create(&dir).unwrap();
        p.meta_set("k", "v").unwrap();
        assert!(dir.join("tree.db-wal").metadata().unwrap().len() > 0);
        drop(p);
        let wal = dir.join("tree.db-wal");
        assert!(!wal.exists() || wal.metadata().unwrap().len() == 0);
        let p = Project::open(&dir).unwrap();
        assert_eq!(p.meta_get("k").unwrap().as_deref(), Some("v"));
    }
}
