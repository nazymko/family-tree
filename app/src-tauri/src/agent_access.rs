//! Agent access: the local MCP server of `gt-mcp`, run from the app, with its connections and keys.
//!
//! A plain key exists only in the answer to [`create_connection`]; only its SHA-256 is kept. Nothing
//! here derives `Debug` over a key or a hash, and nothing here logs.
//!
//! Connection threads of a stopped server can outlive it by up to the server's timeout, each holding
//! an [`AppHost`]. That is safe: a host holds no session, it asks [`AppState`] on every call, so a
//! late request finds no tree once it is closed, and no key once the server has stopped.

use crate::settings_file::{self, ConnectionDto};
use crate::state::{AppState, no_project};
use gt_mcp::{AgentCtx, Host, ServerHandle};
use gt_session::{ApiError, ErrorCode, Session};
use serde::Serialize;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use uuid::Uuid;

/// The event the page hears right after an agent's commit.
pub const CHANGED_EVENT: &str = "tree-changed-by-agent";
/// With the e2e config-dir override, hashes are in this file in that folder.
pub const HASH_FILE: &str = "agent-hashes.json";
const MIN_PORT: u32 = 1024;
const MAX_PORT: u32 = 65535;

type ApiResult<T> = Result<T, ApiError>;

/// What Settings shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAccessState {
    pub enabled: bool,
    pub port: u16,
    pub running: bool,
    pub error: Option<String>,
    pub connections: Vec<ConnectionDto>,
}

/// The one answer that holds the plain key. No `Debug`, so it cannot be printed by accident.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewConnectionDto {
    pub connection: ConnectionDto,
    pub key: String,
    pub snippet: String,
}

/// The payload of [`CHANGED_EVENT`].
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentChange<'a> {
    touched: &'a [Uuid],
    agent: &'a str,
}

// ------------------------------------------------------------ where hashes are kept

/// Where the SHA-256 of each key is kept. Errors say nothing of the hash.
pub trait HashStore: Send + Sync {
    fn get(&self, id: &Uuid) -> Option<[u8; 32]>;
    fn set(&self, id: &Uuid, hash: &[u8; 32]) -> Result<(), String>;
    fn remove(&self, id: &Uuid) -> Result<(), String>;
}

/// The OS keychain, one entry per connection under the app's service.
pub struct KeychainHashes;

impl KeychainHashes {
    pub fn user(id: &Uuid) -> String {
        format!("mcp-{id}")
    }

    fn entry(id: &Uuid) -> Result<keyring::Entry, String> {
        keyring::Entry::new(crate::ai::KEYCHAIN_SERVICE, &Self::user(id)).map_err(|e| e.to_string())
    }
}

impl HashStore for KeychainHashes {
    fn get(&self, id: &Uuid) -> Option<[u8; 32]> {
        parse_hex(&Self::entry(id).ok()?.get_password().ok()?)
    }
    fn set(&self, id: &Uuid, hash: &[u8; 32]) -> Result<(), String> {
        Self::entry(id)?
            .set_password(&to_hex(hash))
            .map_err(|e| e.to_string())
    }
    fn remove(&self, id: &Uuid) -> Result<(), String> {
        match Self::entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// A file in the config folder, for test runs that must never touch the owner's keychain.
pub struct FileHashes {
    path: PathBuf,
    write: Mutex<()>,
}

impl FileHashes {
    pub fn new(config_dir: &Path) -> Self {
        Self {
            path: config_dir.join(HASH_FILE),
            write: Mutex::new(()),
        }
    }

    fn load(&self) -> HashMap<Uuid, String> {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save(&self, map: &HashMap<Uuid, String>) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(map).map_err(|e| e.to_string())?;
        settings_file::write_atomic(&self.path, &json).map_err(|e| e.to_string())
    }
}

impl HashStore for FileHashes {
    fn get(&self, id: &Uuid) -> Option<[u8; 32]> {
        parse_hex(self.load().get(id)?)
    }
    fn set(&self, id: &Uuid, hash: &[u8; 32]) -> Result<(), String> {
        let _guard = AppState::lock(&self.write);
        let mut map = self.load();
        map.insert(*id, to_hex(hash));
        self.save(&map)
    }
    fn remove(&self, id: &Uuid) -> Result<(), String> {
        let _guard = AppState::lock(&self.write);
        let mut map = self.load();
        map.remove(id);
        self.save(&map)
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn parse_hex(text: &str) -> Option<[u8; 32]> {
    let mut out = [0u8; 32];
    if text.len() != 64 || !text.is_ascii() {
        return None;
    }
    for (slot, pair) in out.iter_mut().zip(text.as_bytes().chunks(2)) {
        *slot = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(out)
}

// ------------------------------------------------------------ the running part

/// A key the running server accepts. No `Debug`: it holds a hash.
struct KnownKey {
    hash: [u8; 32],
    ctx: AgentCtx,
}

#[derive(Default)]
struct Server {
    handle: Option<ServerHandle>,
    error: Option<String>,
}

/// In [`AppState`]. `server` also orders every change of switch, port and connections.
pub struct AgentAccess {
    server: Mutex<Server>,
    /// What the running server accepts: loaded when it starts, emptied when it stops.
    keys: Mutex<Vec<KnownKey>>,
    hashes: Box<dyn HashStore>,
}

impl AgentAccess {
    pub fn new(hashes: Box<dyn HashStore>) -> Self {
        Self {
            server: Mutex::new(Server::default()),
            keys: Mutex::new(Vec::new()),
            hashes,
        }
    }

    /// Looks at every key and every byte, whatever matches, so timing says nothing of a hash.
    fn authenticate(&self, hash: &[u8; 32]) -> Option<AgentCtx> {
        let keys = AppState::lock(&self.keys);
        let mut found = None;
        for known in keys.iter() {
            if same(&known.hash, hash) {
                found = Some(known.ctx.clone());
            }
        }
        found
    }
}

/// Equal in constant time.
fn same(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let diff = a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y));
    std::hint::black_box(diff) == 0
}

/// What the server needs from the app. Holds only the handle: see the module docs.
pub struct AppHost<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> AppHost<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> Host for AppHost<R> {
    fn with_session<T>(
        &self,
        f: &mut dyn FnMut(&mut Session) -> Result<T, ApiError>,
    ) -> Result<T, ApiError> {
        let state = self.app.try_state::<AppState>().ok_or_else(no_project)?;
        state.with_session(|s| f(s))
    }

    fn authenticate(&self, key_sha256: &[u8; 32]) -> Option<AgentCtx> {
        self.app
            .try_state::<AppState>()?
            .agent
            .authenticate(key_sha256)
    }

    fn changed(&self, touched: &[Uuid], agent: &AgentCtx) {
        // The page refreshes on its own at the next change; a lost event must not fail the write.
        let _ = self.app.emit(
            CHANGED_EVENT,
            AgentChange {
                touched,
                agent: &agent.name,
            },
        );
    }
}

// ------------------------------------------------------------ turning on and off

fn io_error(what: &str) -> ApiError {
    ApiError::new(ErrorCode::Io, what)
}

fn check_port(port: u32) -> ApiResult<u16> {
    u16::try_from(port)
        .ok()
        .filter(|p| (MIN_PORT..=MAX_PORT).contains(&u32::from(*p)))
        .ok_or_else(|| {
            ApiError::new(
                ErrorCode::Invalid,
                "The port must be a number from 1024 to 65535.",
            )
        })
}

fn bind_error(port: u16, e: &io::Error) -> String {
    match e.kind() {
        io::ErrorKind::AddrInUse => {
            format!("Port {port} is already used by another program. Choose another port.")
        }
        // Windows answers this way for ports it keeps for itself.
        io::ErrorKind::PermissionDenied => {
            format!("Port {port} cannot be used on this computer. Choose another port.")
        }
        _ => format!("Agent access could not start on port {port}: {e}"),
    }
}

/// Reads every connection's hash into memory. The error names those that could not be read.
fn load_keys(state: &AppState) -> Option<String> {
    let connections = AppState::lock(&state.settings)
        .agent_access
        .connections
        .clone();
    let mut known = Vec::with_capacity(connections.len());
    let mut missing = Vec::new();
    for c in connections {
        match state.agent.hashes.get(&c.id) {
            Some(hash) => known.push(KnownKey {
                hash,
                ctx: AgentCtx {
                    id: c.id,
                    name: c.name,
                },
            }),
            None => missing.push(c.name),
        }
    }
    *AppState::lock(&state.agent.keys) = known;
    (!missing.is_empty()).then(|| {
        format!(
            "The keys of these connections could not be read from the system keychain, so they cannot connect: {}. Check that the keychain is available, or revoke them and make new ones.",
            missing.join(", ")
        )
    })
}

fn start<R: Runtime>(app: &AppHandle<R>, state: &AppState, server: &mut Server, port: u16) {
    server.error = load_keys(state);
    match gt_mcp::serve(Arc::new(AppHost::new(app.clone())), port) {
        Ok(handle) => server.handle = Some(handle),
        Err(e) => {
            AppState::lock(&state.agent.keys).clear();
            server.error = Some(bind_error(port, &e));
        }
    }
}

/// Ends the accept loop and frees the port. Requests still on their way then find no key.
fn stop(state: &AppState, server: &mut Server) {
    if let Some(handle) = server.handle.take() {
        handle.stop();
    }
    AppState::lock(&state.agent.keys).clear();
    server.error = None;
}

fn snapshot(state: &AppState, server: &Server) -> AgentAccessState {
    let settings = AppState::lock(&state.settings);
    AgentAccessState {
        enabled: settings.agent_access.enabled,
        port: settings.agent_access.port,
        running: server.handle.is_some(),
        error: server.error.clone(),
        connections: settings.agent_access.connections.clone(),
    }
}

/// At startup: turns the server back on if it was on. A failure is kept for Settings to show.
pub fn resume<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let mut server = AppState::lock(&state.agent.server);
    let saved = AppState::lock(&state.settings).agent_access.clone();
    if !saved.enabled {
        return;
    }
    match check_port(u32::from(saved.port)) {
        Ok(port) => start(app, &state, &mut server, port),
        Err(e) => server.error = Some(e.message),
    }
}

/// At exit, before the tree closes: stops the server and leaves the switch as it is.
pub fn shutdown(state: &AppState) {
    stop(state, &mut AppState::lock(&state.agent.server));
}

pub fn access_state(state: &AppState) -> AgentAccessState {
    snapshot(state, &AppState::lock(&state.agent.server))
}

/// Turns the server on or off, or moves it. A taken port is an answer, not an error.
pub fn set_access<R: Runtime>(
    app: &AppHandle<R>,
    enabled: bool,
    port: u32,
) -> ApiResult<AgentAccessState> {
    let port = check_port(port)?;
    let state = app.state::<AppState>();
    let mut server = AppState::lock(&state.agent.server);
    {
        let mut settings = AppState::lock(&state.settings);
        let mut next = settings.clone();
        next.agent_access.enabled = enabled;
        next.agent_access.port = port;
        settings_file::save_settings(&state.config_dir, &next)?;
        *settings = next;
    }
    let same_server = server.handle.as_ref().is_some_and(|h| h.port() == port);
    if !enabled {
        stop(&state, &mut server);
    } else if same_server {
        server.error = load_keys(&state);
    } else {
        stop(&state, &mut server);
        start(app, &state, &mut server, port);
    }
    Ok(snapshot(&state, &server))
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

fn new_key() -> ApiResult<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| {
        ApiError::new(
            ErrorCode::Internal,
            "No secure random numbers are available.",
        )
    })?;
    Ok(format!("gt_{}", to_hex(&bytes)))
}

fn snippet(port: u16, key: &str) -> String {
    format!(
        "claude mcp add --transport http gene-tree http://127.0.0.1:{port}/mcp --header \"Authorization: Bearer {key}\""
    )
}

/// Makes a connection. The answer is the only place the plain key ever appears.
pub fn create_connection(state: &AppState, name: &str) -> ApiResult<NewConnectionDto> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ApiError::new(
            ErrorCode::Invalid,
            "Give the connection a name.",
        ));
    }
    let server = AppState::lock(&state.agent.server);
    let key = new_key()?;
    let hash = gt_mcp::key_hash(&key);
    let connection = ConnectionDto {
        id: Uuid::now_v7(),
        name: name.to_string(),
        created: now_millis(),
    };
    state.agent.hashes.set(&connection.id, &hash).map_err(|_| {
        io_error("The key could not be saved in the system keychain. Check that the keychain is available.")
    })?;
    let port = {
        let mut settings = AppState::lock(&state.settings);
        let mut next = settings.clone();
        next.agent_access.connections.push(connection.clone());
        if let Err(e) = settings_file::save_settings(&state.config_dir, &next) {
            let _ = state.agent.hashes.remove(&connection.id);
            return Err(e);
        }
        *settings = next;
        settings.agent_access.port
    };
    if server.handle.is_some() {
        AppState::lock(&state.agent.keys).push(KnownKey {
            hash,
            ctx: AgentCtx {
                id: connection.id,
                name: connection.name.clone(),
            },
        });
    }
    let snippet = snippet(port, &key);
    Ok(NewConnectionDto {
        connection,
        key,
        snippet,
    })
}

/// Stops a connection's access first of all: the next request with its key is refused, whatever
/// happens to the stored copies after that.
pub fn revoke_connection(state: &AppState, id: &str) -> ApiResult<()> {
    let not_found = || {
        ApiError::new(
            ErrorCode::NotFound,
            format!("No connection with id \"{id}\"."),
        )
    };
    let id: Uuid = id.parse().map_err(|_| not_found())?;
    let _server = AppState::lock(&state.agent.server);
    let saved = {
        let mut settings = AppState::lock(&state.settings);
        if !settings.agent_access.connections.iter().any(|c| c.id == id) {
            return Err(not_found());
        }
        AppState::lock(&state.agent.keys).retain(|k| k.ctx.id != id);
        settings.agent_access.connections.retain(|c| c.id != id);
        settings_file::save_settings(&state.config_dir, &settings)
    };
    let removed = state.agent.hashes.remove(&id);
    saved?;
    removed.map_err(|_| {
        io_error(
            "Access was stopped, but the key's record could not be removed from the system keychain.",
        )
    })
}
