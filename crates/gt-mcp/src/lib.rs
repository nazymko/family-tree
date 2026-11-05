//! A local MCP server: AI agents on this machine read the open family tree over loopback HTTP.
//!
//! JSON-RPC by hand in [`protocol`], a small `std::net` server in [`http`], tools in `tools_*`.
//! No sessions, tools only. Bodies and keys are never logged.

mod http;
mod protocol;
mod tools_read;

use gt_session::{ApiError, Session};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// The agent behind a request, known from its key.
pub use gt_session::AgentCtx;
pub use http::{ServerHandle, ServerLimits, serve, serve_with};
pub use protocol::handle_message;

/// What the agent must follow: the first thing it gets, and the `get_rules` tool.
pub const RULES: &str = include_str!("rules.md");

/// What the server needs from the app. Used as a generic bound only.
pub trait Host: Send + Sync + 'static {
    /// Runs `f` with the open session, or fails with `ErrorCode::NoProject` when no tree is open.
    fn with_session<T>(
        &self,
        f: &mut dyn FnMut(&mut Session) -> Result<T, ApiError>,
    ) -> Result<T, ApiError>;

    /// The agent owning a key, by the SHA-256 of the key. `None` for an unknown or revoked key.
    fn authenticate(&self, key_sha256: &[u8; 32]) -> Option<AgentCtx>;

    /// Records changed: the app refreshes its views. A write tool passes on what
    /// `Session::apply_as_agent_reaching` returned, after `with_session` has returned, and does not
    /// call this for a repeat that wrote nothing.
    fn changed(&self, touched: &[Uuid], agent: &AgentCtx);
}

/// The SHA-256 of a key, as [`Host::authenticate`] receives it.
pub fn key_hash(key: &str) -> [u8; 32] {
    Sha256::digest(key.as_bytes()).into()
}

/// One tool, whichever module it comes from.
pub(crate) struct Tool<H> {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: Value,
    pub call: fn(&H, &AgentCtx, &Value) -> Result<Vec<Value>, ApiError>,
}

/// Every tool, in the order `tools/list` shows them. A new tool module is one more element.
pub(crate) fn tools<H: Host>() -> Vec<Tool<H>> {
    [tools_read::tools::<H>()].into_iter().flatten().collect()
}
