//! Shared state behind every command.

use crate::agent_access::{AgentAccess, HashStore, KeychainHashes};
use crate::ai::{KeyStore, Keychain};
use crate::settings_file::{self, AppSettings};
use gt_session::{ApiError, ErrorCode, ImportReportDto, Session, SessionClock};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard, PoisonError};

pub struct AppState {
    pub session: Mutex<Option<Session>>,
    pub last_import: Mutex<Option<ImportReportDto>>,
    pub settings: Mutex<AppSettings>,
    pub config_dir: PathBuf,
    /// Makes the clock a session opens with; tests pin the date.
    pub clock: Box<dyn Fn() -> SessionClock + Send + Sync>,
    pub agent: AgentAccess,
    /// Where the Anthropic key is kept: the OS keychain, or (test runs) a file in the config folder.
    pub keys: Arc<dyn KeyStore>,
}

impl AppState {
    /// The real app: the system clock, and key hashes in the OS keychain.
    pub fn new(config_dir: PathBuf) -> AppState {
        AppState::with_clock(
            config_dir,
            Box::new(SessionClock::system),
            Box::new(KeychainHashes),
        )
    }

    /// `hashes` is explicit so that a test run, of which the e2e suite is one, has to say where key
    /// hashes go, and can keep away from the owner's keychain with `FileHashes`.
    pub fn with_clock(
        config_dir: PathBuf,
        clock: Box<dyn Fn() -> SessionClock + Send + Sync>,
        hashes: Box<dyn HashStore>,
    ) -> AppState {
        let settings = settings_file::load_settings(&config_dir);
        AppState {
            session: Mutex::new(None),
            last_import: Mutex::new(None),
            settings: Mutex::new(settings),
            config_dir,
            clock,
            agent: AgentAccess::new(hashes),
            keys: Arc::new(Keychain),
        }
    }

    /// Replaces the key store; a test run uses it to stay away from the owner's keychain.
    pub fn with_keys(mut self, keys: Arc<dyn KeyStore>) -> AppState {
        self.keys = keys;
        self
    }

    /// A panic while a lock was held must not take every later command down with it.
    pub fn lock<'a, T>(m: &'a Mutex<T>) -> MutexGuard<'a, T> {
        m.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Runs `f` on the open session, or fails with `NoProject`. The lock is held only for `f`.
    pub fn with_session<T>(
        &self,
        f: impl FnOnce(&mut Session) -> Result<T, ApiError>,
    ) -> Result<T, ApiError> {
        let mut guard = AppState::lock(&self.session);
        let session = guard.as_mut().ok_or_else(no_project)?;
        f(session)
    }
}

pub fn no_project() -> ApiError {
    ApiError::new(
        ErrorCode::NoProject,
        "No project is open. Open or create a project first.",
    )
}
