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
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// The agent behind a request, known from its key.
pub use gt_session::AgentCtx;
pub use http::{ServerHandle, ServerLimits, serve, serve_with};
pub use protocol::handle_message;

/// What the agent must follow: the first thing it gets, and the `get_rules` tool.
pub const RULES: &str = include_str!("rules.md");

/// What the server tells a client when it connects: the rules, then how to work with these tools.
pub const INSTRUCTIONS: &str = concat!(
    include_str!("rules.md"),
    "\n\nHow to work with these tools: give add_person your source_file and the person's number \
     every time, from a numbered file or a label of your own that stays the same. A second run \
     over the same file then gets {\"id\", \"alreadyThere\": true} and adds nobody; without the \
     number it adds them twice. A date a source writes as 03/04/1901 can be read two ways and is \
     refused: write the month as a word, or put the date in a note.\n\n\
     This server refuses a person with no name at all (no given name, surname or patronymic). \
     A birth year goes on the birth event, not in a name or a note. Do not assume a parent that an \
     earlier write created for a child: read the parents the answer lists. The report you close \
     a batch with states only what the server accepted: check get_log first.\n"
);

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

    /// The built-in agent, for a request with no key at all, while the owner has not required
    /// one: see [`keyless_agent`]. `None` (the default) means every request needs a key. Asked
    /// again right before a tool runs, so turning the requirement on stops a request already
    /// past its head. A request with a key, even a bad one, never gets here.
    fn authenticate_keyless(&self) -> Option<AgentCtx> {
        None
    }

    /// Records changed: the app refreshes its views. A write tool passes on what
    /// `Session::apply_as_agent_reaching` returned, after `with_session` has returned, and does not
    /// call this for a repeat that wrote nothing.
    fn changed(&self, touched: &[Uuid], agent: &AgentCtx);
}

/// The fixed id of the built-in agent that works when no key is required. It cannot be revoked or
/// deleted, and no connection can have it.
pub const KEYLESS_AGENT_ID: Uuid = Uuid::from_u128(0x01927a00_0000_7000_8000_6e6f6b657900);

/// The name stored in the log for the built-in agent. The app shows a translated label by id.
pub const KEYLESS_AGENT_NAME: &str = "Agent without a key";

/// The built-in agent of a request with no key.
pub fn keyless_agent() -> AgentCtx {
    AgentCtx {
        id: KEYLESS_AGENT_ID,
        name: KEYLESS_AGENT_NAME.to_string(),
    }
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
    [tools_read::tools::<H>(), tools_write::tools::<H>()]
        .into_iter()
        .flatten()
        .collect()
}
