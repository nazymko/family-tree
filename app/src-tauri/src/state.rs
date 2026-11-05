//! Shared state behind every command.

use crate::agent_access::{AgentAccess, FileHashes, KeychainHashes};
use crate::settings_file::{self, AppSettings};
use gt_session::{ApiError, ErrorCode, ImportReportDto, Session, SessionClock};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

pub struct AppState {
    pub session: Mutex<Option<Session>>,
    pub last_import: Mutex<Option<ImportReportDto>>,
    pub settings: Mutex<AppSettings>,
    pub config_dir: PathBuf,
    /// Makes the clock a session opens with; tests pin the date.
    pub clock: Box<dyn Fn() -> SessionClock + Send + Sync>,
    pub agent: AgentAccess,
}

impl AppState {
    /// The real app: the system clock, and key hashes in the OS keychain.
    pub fn new(config_dir: PathBuf) -> AppState {
        let mut state = AppState::with_clock(config_dir, Box::new(SessionClock::system));
        state.agent = AgentAccess::new(Box::new(KeychainHashes));
        state
    }

    /// For test runs, of which the e2e suite is one: key hashes stay in a file in `config_dir`, so
    /// nothing built this way can reach the owner's keychain.
    pub fn with_clock(
        config_dir: PathBuf,
        clock: Box<dyn Fn() -> SessionClock + Send + Sync>,
    ) -> AppState {
        let settings = settings_file::load_settings(&config_dir);
        let hashes = FileHashes::new(&config_dir);
        AppState {
            session: Mutex::new(None),
            last_import: Mutex::new(None),
            settings: Mutex::new(settings),
            config_dir,
            clock,
            agent: AgentAccess::new(Box::new(hashes)),
        }
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
