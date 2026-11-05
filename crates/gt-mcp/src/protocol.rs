//! JSON-RPC for both eras of MCP. Older clients (up to `2025-11-25`) open with `initialize`; newer
//! ones (`2026-07-28`) call `server/discover` and state their version on every request in `_meta`,
//! mirrored in HTTP headers.

use crate::{AgentCtx, Host, RULES, Tool, tools};
use base64::Engine;
use gt_session::{ApiError, ErrorCode};
use serde_json::{Map, Value, json};
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

const MODERN: &str = "2026-07-28";
const OLDER: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];
/// What an older client that names no version is taken to speak, as the transport spec says.
const OLDER_DEFAULT: &str = "2025-03-26";
const SERVER_NAME: &str = "gene-tree";

const META_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
const META_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
const META_SERVER: &str = "io.modelcontextprotocol/serverInfo";

/// How long a client may keep `server/discover` and `tools/list`: the tools only change with the
/// app itself.
const CACHE_TTL_MS: u64 = 3_600_000;

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const HEADER_MISMATCH: i64 = -32020;
const UNSUPPORTED_VERSION: i64 = -32022;

/// Tool calls one server runs at once. A tool holds the app's session, so a flood of them would
/// freeze the app.
const TOOLS_AT_ONCE: usize = 2;
/// How long a tool call waits its turn before it is told the server is busy.
const TOOL_WAIT: Duration = Duration::from_secs(2);
const BUSY: &str = "Gene Tree is busy with other tool calls. Try again in a moment.";

/// A semaphore for tool calls.
#[derive(Debug, Default)]
pub(crate) struct ToolGate {
    running: Mutex<usize>,
    freed: Condvar,
}

/// A place in the gate, given back however the tool call ends.
struct ToolTurn<'a>(&'a ToolGate);

impl ToolGate {
    fn wait_for_turn(&self, wait: Duration) -> Option<ToolTurn<'_>> {
        let give_up = Instant::now() + wait;
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        while *running >= TOOLS_AT_ONCE {
            let left = give_up.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            running = self
                .freed
                .wait_timeout(running, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        *running += 1;
        Some(ToolTurn(self))
    }
}

impl Drop for ToolTurn<'_> {
    fn drop(&mut self) {
        let mut running = self
            .0
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *running = running.saturating_sub(1);
        self.0.freed.notify_one();
    }
}

/// The mirrored headers of one HTTP request.
#[derive(Debug, Default)]
pub(crate) struct HttpMeta {
    pub protocol_version: Option<String>,
    pub method: Option<String>,
    pub name: Option<String>,
}

/// An answer and the HTTP status it goes out with. No body: nothing to send.
#[derive(Debug)]
pub(crate) struct Reply {
    pub status: u16,
    pub body: Option<Value>,
}

/// One message without a transport: `None` for a notification or a client's own response.
pub fn handle_message<H: Host>(host: &H, ctx: &AgentCtx, body: &Value) -> Option<Value> {
    dispatch(host, ctx, body, None, None).body
}

/// A body that is not JSON. There is no id to answer to.
pub(crate) fn parse_error() -> Reply {
    Reply {
        status: 400,
        body: Some(error_body(None, PARSE_ERROR, "Not valid JSON", None)),
    }
}

/// Two `MCP-Protocol-Version` headers that disagree. Which one is meant cannot be known.
pub(crate) fn conflicting_version() -> Reply {
    bad_request(
        None,
        INVALID_REQUEST,
        "MCP-Protocol-Version is sent twice, with different values",
    )
}

/// One message. `http` is `None` when there is no transport, and then no headers are expected.
/// `gate` limits how many tool calls run at once.
pub(crate) fn dispatch<H: Host>(
    host: &H,
    ctx: &AgentCtx,
    body: &Value,
    http: Option<&HttpMeta>,
    gate: Option<&ToolGate>,
) -> Reply {
    let Some(message) = body.as_object() else {
        return bad_request(
            None,
            INVALID_REQUEST,
            "Send one JSON-RPC message as an object",
        );
    };
    let id = message.get("id");
    let id_ok = id.is_some_and(|i| i.is_string() || i.is_number());
    let echo = id.filter(|_| id_ok);
    if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return bad_request(echo, INVALID_REQUEST, "Only JSON-RPC 2.0 is spoken");
    }
    let Some(method) = message.get("method") else {
        // A client's response to a request. This server asks nothing, so nothing waits for it.
        if id_ok && (message.contains_key("result") || message.contains_key("error")) {
            return accepted();
        }
        return bad_request(echo, INVALID_REQUEST, "The message has no method");
    };
    let Some(method) = method.as_str() else {
        return bad_request(echo, INVALID_REQUEST, "The method must be text");
    };
    if id.is_none() {
        return notification(http);
    }
    let Some(id) = echo else {
        return bad_request(None, INVALID_REQUEST, "The id must be text or a number");
    };
    let empty = Map::new();
    let params = match message.get("params") {
        None => &empty,
        Some(Value::Object(p)) => p,
        Some(_) => return bad_request(Some(id), INVALID_PARAMS, "The params must be an object"),
    };
    if method == "initialize" {
        if let Some(version) = http.and_then(|h| h.protocol_version.as_deref())
            && !is_supported(version)
        {
            return unsupported(Some(id), version);
        }
        return initialize(id, params);
    }
    let era = match negotiate(id, method, params, http) {
        Ok(era) => era,
        Err(reply) => return reply,
    };
    route(host, ctx, id, method, params, era, gate)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Era {
    Older,
    Modern,
}

fn notification(http: Option<&HttpMeta>) -> Reply {
    if let Some(version) = http.and_then(|h| h.protocol_version.as_deref())
        && !is_supported(version)
    {
        return unsupported(None, version);
    }
    accepted()
}

/// Older clients negotiate here: their version when it is known, else the newest older one.
fn initialize(id: &Value, params: &Map<String, Value>) -> Reply {
    let Some(asked) = params.get("protocolVersion").and_then(Value::as_str) else {
        return bad_request(Some(id), INVALID_PARAMS, "protocolVersion is required");
    };
    let version = OLDER
        .iter()
        .find(|v| **v == asked)
        .copied()
        .unwrap_or(OLDER[0]);
    success(
        id,
        json!({
            "protocolVersion": version,
            "capabilities": {"tools": {}},
            "serverInfo": server_info(),
            "instructions": RULES,
        }),
    )
}

/// Which era a request belongs to, after every version and header check.
fn negotiate(
    id: &Value,
    method: &str,
    params: &Map<String, Value>,
    http: Option<&HttpMeta>,
) -> Result<Era, Reply> {
    let meta = params.get("_meta").and_then(Value::as_object);
    let stated = meta
        .and_then(|m| m.get(META_VERSION))
        .and_then(Value::as_str);
    let header = http.and_then(|h| h.protocol_version.as_deref());
    if let (Some(s), Some(h)) = (stated, header)
        && s != h
    {
        return Err(mismatch(
            id,
            "MCP-Protocol-Version differs from the request",
        ));
    }
    let version = match stated.or(header) {
        Some(v) => v,
        None if method == "server/discover" => {
            return Err(bad_request(
                Some(id),
                INVALID_PARAMS,
                "server/discover needs the protocol version in _meta",
            ));
        }
        None => OLDER_DEFAULT,
    };
    if !is_supported(version) {
        return Err(unsupported(Some(id), version));
    }
    if version != MODERN {
        return Ok(Era::Older);
    }
    if let Some(http) = http {
        check_mirrors(id, method, params, http)?;
    }
    let has_capabilities = meta
        .and_then(|m| m.get(META_CAPABILITIES))
        .is_some_and(Value::is_object);
    if stated.is_none() || !has_capabilities {
        return Err(bad_request(
            Some(id),
            INVALID_PARAMS,
            "_meta needs the protocol version and the client capabilities",
        ));
    }
    Ok(Era::Modern)
}

/// A newer client's headers must say what its body says.
fn check_mirrors(
    id: &Value,
    method: &str,
    params: &Map<String, Value>,
    http: &HttpMeta,
) -> Result<(), Reply> {
    if http.protocol_version.is_none() {
        return Err(mismatch(id, "MCP-Protocol-Version is missing"));
    }
    if http.method.as_deref() != Some(method) {
        return Err(mismatch(
            id,
            "Mcp-Method is missing or differs from the request",
        ));
    }
    if method == "tools/call" {
        let named = http.name.as_deref().and_then(decode_mirrored);
        if named.as_deref() != params.get("name").and_then(Value::as_str) {
            return Err(mismatch(
                id,
                "Mcp-Name is missing or differs from the request",
            ));
        }
    }
    Ok(())
}

/// A mirrored value as sent, or decoded from its `=?base64?...?=` form. `None` if that is broken.
fn decode_mirrored(value: &str) -> Option<String> {
    let Some(inner) = value
        .strip_prefix("=?base64?")
        .and_then(|v| v.strip_suffix("?="))
    else {
        return Some(value.to_string());
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(inner)
        .ok()?;
    String::from_utf8(bytes).ok()
}

fn route<H: Host>(
    host: &H,
    ctx: &AgentCtx,
    id: &Value,
    method: &str,
    params: &Map<String, Value>,
    era: Era,
    gate: Option<&ToolGate>,
) -> Reply {
    let result = match (method, era) {
        ("ping", _) => json!({}),
        ("server/discover", Era::Modern) => cached(json!({
            "supportedVersions": supported_versions(),
            "capabilities": {"tools": {}},
            "instructions": RULES,
        })),
        ("tools/list", Era::Older) => list_tools::<H>(),
        ("tools/list", Era::Modern) => cached(list_tools::<H>()),
        ("tools/call", _) => match call_tool(host, ctx, params, gate) {
            Ok(result) => result,
            Err(message) => return error_reply(200, Some(id), INVALID_PARAMS, &message, None),
        },
        _ => {
            let status = if era == Era::Modern { 404 } else { 200 };
            let message = format!("Unknown method: {method}");
            return error_reply(status, Some(id), METHOD_NOT_FOUND, &message, None);
        }
    };
    let result = match era {
        Era::Older => result,
        Era::Modern => modern_result(result),
    };
    success(id, result)
}

fn list_tools<H: Host>() -> Value {
    let list: Vec<Value> = tools::<H>()
        .into_iter()
        .map(|t: Tool<H>| {
            json!({"name": t.name, "description": t.description, "inputSchema": t.schema})
        })
        .collect();
    json!({"tools": list})
}

/// A known tool's outcome, failures included, or the message for a tool that does not exist.
fn call_tool<H: Host>(
    host: &H,
    ctx: &AgentCtx,
    params: &Map<String, Value>,
    gate: Option<&ToolGate>,
) -> Result<Value, String> {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return Err("The tool name is required".to_string());
    };
    let Some(tool) = tools::<H>().into_iter().find(|t| t.name == name) else {
        return Err(format!("Unknown tool: {name}"));
    };
    let empty = json!({});
    let args = params.get("arguments").unwrap_or(&empty);
    let turn = gate.map(|g| g.wait_for_turn(TOOL_WAIT));
    let outcome = if matches!(turn, Some(None)) {
        Err(ApiError::new(ErrorCode::Conflict, BUSY))
    } else if args.is_object() {
        (tool.call)(host, ctx, args)
    } else {
        Err(ApiError::new(
            ErrorCode::Invalid,
            "arguments must be an object",
        ))
    };
    Ok(match outcome {
        Ok(content) => json!({"content": content}),
        Err(e) => json!({
            "content": [{"type": "text", "text": tool_error_text(&e)}],
            "isError": true,
        }),
    })
}

fn tool_error_text(e: &ApiError) -> String {
    if e.code == ErrorCode::NoProject {
        return "No tree is open. Ask the owner to open a tree in Gene Tree, then try again."
            .to_string();
    }
    e.message.clone()
}

fn cached(mut result: Value) -> Value {
    if let Some(fields) = result.as_object_mut() {
        fields.insert("ttlMs".into(), json!(CACHE_TTL_MS));
        fields.insert("cacheScope".into(), json!("private"));
    }
    result
}

fn modern_result(mut result: Value) -> Value {
    if let Some(fields) = result.as_object_mut() {
        fields.insert("resultType".into(), json!("complete"));
        fields.insert("_meta".into(), json!({META_SERVER: server_info()}));
    }
    result
}

fn server_info() -> Value {
    json!({"name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION")})
}

fn is_supported(version: &str) -> bool {
    version == MODERN || OLDER.contains(&version)
}

fn supported_versions() -> Vec<&'static str> {
    std::iter::once(MODERN).chain(OLDER).collect()
}

fn accepted() -> Reply {
    Reply {
        status: 202,
        body: None,
    }
}

fn success(id: &Value, result: Value) -> Reply {
    Reply {
        status: 200,
        body: Some(json!({"jsonrpc": "2.0", "id": id, "result": result})),
    }
}

fn bad_request(id: Option<&Value>, code: i64, message: &str) -> Reply {
    error_reply(400, id, code, message, None)
}

fn mismatch(id: &Value, message: &str) -> Reply {
    bad_request(Some(id), HEADER_MISMATCH, message)
}

fn unsupported(id: Option<&Value>, requested: &str) -> Reply {
    let data = json!({"supported": supported_versions(), "requested": requested});
    error_reply(
        400,
        id,
        UNSUPPORTED_VERSION,
        "Unsupported protocol version",
        Some(data),
    )
}

fn error_reply(
    status: u16,
    id: Option<&Value>,
    code: i64,
    message: &str,
    data: Option<Value>,
) -> Reply {
    Reply {
        status,
        body: Some(error_body(id, code, message, data)),
    }
}

fn error_body(id: Option<&Value>, code: i64, message: &str, data: Option<Value>) -> Value {
    let mut error = json!({"code": code, "message": message});
    if let (Some(data), Some(fields)) = (data, error.as_object_mut()) {
        fields.insert("data".into(), data);
    }
    let mut body = json!({"jsonrpc": "2.0", "error": error});
    if let (Some(id), Some(fields)) = (id, body.as_object_mut()) {
        fields.insert("id".into(), id.clone());
    }
    body
}
