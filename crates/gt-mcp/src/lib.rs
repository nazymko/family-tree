//! A local MCP server: AI agents on this machine read and add to the open family tree over
//! loopback HTTP.
//!
//! JSON-RPC by hand in [`protocol`], a small `std::net` server in [`http`], tools in `tools_*`.
//! No sessions, tools only. Bodies and keys are never logged.

mod http;
mod protocol;
mod records;
mod tools_read;
mod tools_write;

use gt_session::{ApiError, Session};
use serde_json::Value;
use uuid::Uuid;

pub use gt_session::AgentCtx;
pub use http::{ServerHandle, ServerLimits, serve, serve_with};
pub use protocol::handle_message;

/// What the agent must follow: the first thing it gets, and the `get_rules` tool.
pub const RULES: &str = include_str!("rules.md");

/// What the server needs from the app. Used as a generic bound only.
///
/// A connection can outlive the [`ServerHandle`] and hold its `Arc` until its last deadline, so an
/// implementor must not own the `Session` itself. Keep the session behind shared state the app can
/// empty, so that closing the tree works whatever a connection is doing. After that, `with_session`
/// fails with `NoProject` for whoever is still there.
pub trait Host: Send + Sync + 'static {
    /// Runs `f` with the open session, or fails with `ErrorCode::NoProject` when no tree is open.
    fn with_session<T>(
        &self,
        f: &mut dyn FnMut(&mut Session) -> Result<T, ApiError>,
    ) -> Result<T, ApiError>;

    /// The agent owning a key, by the SHA-256 of the key. `None` for an unknown or revoked key.
    fn authenticate(&self, key_sha256: &[u8; 32]) -> Option<AgentCtx>;

    /// Records changed: the app refreshes its views.
    fn changed(&self, touched: &[Uuid]);
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
    [tools_read::tools::<H>(), tools_write::tools::<H>()]
        .into_iter()
        .flatten()
        .collect()
}
