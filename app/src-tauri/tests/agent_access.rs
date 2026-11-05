#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Agent access in the app: the five commands through real IPC, the server through a real socket.
//! Hashes are kept in a file in the config folder, so the OS keychain is never touched here.

use gt_app_lib::agent_access::{self, AppHost, FileHashes, HASH_FILE, HashStore, KeychainHashes};
use gt_app_lib::commands;
use gt_app_lib::settings_file::{self, SETTINGS_FILE};
use gt_app_lib::state::AppState;
use gt_mcp::{AgentCtx, Host, key_hash};
use gt_session::{ErrorCode, Session, SessionClock};
use gt_store::Project;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_context, noop_assets};
use tauri::webview::InvokeRequest;
use tauri::{App, Listener, Manager, WebviewWindow};
use uuid::Uuid;

fn state_in(root: &Path) -> AppState {
    let config = root.join("config");
    let hashes = FileHashes::new(&config);
    AppState::with_clock(config, Box::new(SessionClock::system), Box::new(hashes))
}

fn app_with(state: AppState) -> (App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let app = tauri::test::mock_builder()
        .manage(state)
        .invoke_handler(commands::handler())
        .build(mock_context(noop_assets()))
        .unwrap();
    let w = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    (app, w)
}

fn invoke(w: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
    let req = InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "http://tauri.localhost".parse().unwrap(),
        body: InvokeBody::Json(body),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    };
    get_ipc_response(w, req).map(|r| r.deserialize::<Value>().unwrap())
}

fn set(w: &WebviewWindow<MockRuntime>, enabled: bool, port: u16) -> Value {
    invoke(
        w,
        "agent_access_set",
        json!({ "enabled": enabled, "port": port }),
    )
    .unwrap()
}

fn create(w: &WebviewWindow<MockRuntime>, name: &str) -> (String, String, String) {
    let made = invoke(w, "agent_connection_create", json!({ "name": name })).unwrap();
    (
        made["connection"]["id"].as_str().unwrap().to_string(),
        made["key"].as_str().unwrap().to_string(),
        made["snippet"].as_str().unwrap().to_string(),
    )
}

/// A port nobody holds right now. Below the range the system picks from, because a port it picked
/// can be taken as the source of any test's next connection before the server binds it.
fn free_port() -> u16 {
    static NEXT: AtomicU16 = AtomicU16::new(0);
    let base = 20_000 + (std::process::id() % 2_000) as u16 * 10;
    loop {
        let port = base + NEXT.fetch_add(1, Ordering::Relaxed);
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
}

fn head(port: u16, key: &str, length: usize) -> String {
    format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {key}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {length}\r\n\r\n"
    )
}

fn read_answer(s: &mut TcpStream) -> (u16, String) {
    let mut all = Vec::new();
    let _ = s.read_to_end(&mut all);
    let text = String::from_utf8_lossy(&all).into_owned();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    (status, body)
}

fn connect(port: u16) -> TcpStream {
    let s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    s
}

/// One request over a real socket: the status and body of the answer.
fn post(port: u16, key: &str, body: &Value) -> (u16, String) {
    let text = body.to_string();
    let mut s = connect(port);
    s.write_all(head(port, key, text.len()).as_bytes()).unwrap();
    s.write_all(text.as_bytes()).unwrap();
    read_answer(&mut s)
}

/// The older protocol's request, which needs no version headers.
fn rpc(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })
}

fn ping(port: u16, key: &str) -> u16 {
    post(port, key, &rpc("ping", json!({}))).0
}

fn empty_project(root: &Path) -> std::path::PathBuf {
    let dir = root.join("project");
    drop(Project::create(&dir).unwrap());
    dir
}

fn open(state: &AppState, dir: &Path) {
    let session = Session::open(dir, SessionClock::system()).unwrap();
    *AppState::lock(&state.session) = Some(session);
}

// ---- the brief's tests ----

#[test]
fn create_then_connect_then_revoke() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    let on = set(&w, true, port);
    assert_eq!(on["running"], json!(true));

    let (id, key, snippet) = create(&w, "Claude on the laptop");
    assert_eq!(
        snippet,
        format!(
            "claude mcp add --transport http gene-tree http://127.0.0.1:{port}/mcp --header \"Authorization: Bearer {key}\""
        )
    );
    // Added while the server runs: it works at once, and a wrong key does not.
    assert_eq!(ping(port, &key), 200);
    assert_eq!(ping(port, "gt_00"), 401);

    invoke(&w, "agent_connection_revoke", json!({ "id": id })).unwrap();
    // The very next request.
    assert_eq!(ping(port, &key), 401);
    let state = invoke(&w, "agent_access_state", json!({})).unwrap();
    assert_eq!(state["connections"], json!([]));
    assert_eq!(state["running"], json!(true));
    set(&w, false, port);
}

#[test]
fn key_not_in_settings_file() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let (id, key, _) = create(&w, "Reader");

    assert!(key.starts_with("gt_"));
    assert_eq!(key.len(), 3 + 64);
    assert!(key[3..].bytes().all(|b| b.is_ascii_hexdigit()));

    let config = dir.path().join("config");
    let settings = std::fs::read_to_string(config.join(SETTINGS_FILE)).unwrap();
    assert!(settings.contains(&id), "the connection list is in settings");
    assert!(!settings.contains(&key), "never the key");
    assert!(!settings.contains(&key[3..]), "never the key's digits");
    let hash: String = key_hash(&key).iter().map(|b| format!("{b:02x}")).collect();
    assert!(!settings.contains(&hash), "the hash goes elsewhere");

    // With the e2e override the hash, and only the hash, is in a file.
    let hashes = std::fs::read_to_string(config.join(HASH_FILE)).unwrap();
    assert!(hashes.contains(&hash));
    assert!(!hashes.contains(&key));

    // Nothing later gives the key back.
    let state = invoke(&w, "agent_access_state", json!({})).unwrap();
    assert!(!state.to_string().contains(&key));

    // Revoking removes the hash too.
    invoke(&w, "agent_connection_revoke", json!({ "id": id })).unwrap();
    let hashes = std::fs::read_to_string(config.join(HASH_FILE)).unwrap();
    assert!(!hashes.contains(&hash));
}

#[test]
fn port_taken_reports_error() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let holder = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = holder.local_addr().unwrap().port();

    let state = set(&w, true, port);
    assert_eq!(state["enabled"], json!(true));
    assert_eq!(state["port"], json!(port));
    assert_eq!(state["running"], json!(false));
    assert_eq!(
        state["error"],
        json!(format!(
            "Port {port} is already used by another program. Choose another port."
        ))
    );
    assert_eq!(invoke(&w, "agent_access_state", json!({})).unwrap(), state);

    // Once the other program lets go, trying again works and the error goes away.
    drop(holder);
    let state = set(&w, true, port);
    assert_eq!(state["running"], json!(true));
    assert_eq!(state["error"], Value::Null);
    set(&w, false, port);
}

#[test]
fn enabled_resumes_on_start() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let key = {
        let (app, w) = app_with(state_in(dir.path()));
        set(&w, true, port);
        let (_, key, _) = create(&w, "Reader");
        agent_access::shutdown(&app.state::<AppState>());
        key
    };
    let (app, w) = app_with(state_in(dir.path()));
    let before = invoke(&w, "agent_access_state", json!({})).unwrap();
    assert_eq!(before["enabled"], json!(true));
    assert_eq!(before["running"], json!(false), "not until resumed");

    agent_access::resume(app.handle());
    let after = invoke(&w, "agent_access_state", json!({})).unwrap();
    assert_eq!(after["running"], json!(true));
    assert_eq!(after["error"], Value::Null);
    assert_eq!(ping(port, &key), 200);
    agent_access::shutdown(&app.state::<AppState>());
}

// ---- starting up ----

#[test]
fn off_stays_off_on_start() {
    let dir = tempfile::tempdir().unwrap();
    let (app, w) = app_with(state_in(dir.path()));
    agent_access::resume(app.handle());
    let state = invoke(&w, "agent_access_state", json!({})).unwrap();
    assert_eq!(
        state,
        json!({ "enabled": false, "port": 8765, "running": false, "error": null, "requireKey": false, "keylessCommand": "claude mcp add --transport http gene-tree http://127.0.0.1:8765/mcp", "connections": [] })
    );
}

#[test]
fn resume_failure_is_stored_and_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    let holder = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = holder.local_addr().unwrap().port();
    let config = dir.path().join("config");
    let mut saved = settings_file::load_settings(&config);
    saved.agent_access.enabled = true;
    saved.agent_access.port = port;
    settings_file::save_settings(&config, &saved).unwrap();

    let (app, w) = app_with(state_in(dir.path()));
    agent_access::resume(app.handle());
    let state = invoke(&w, "agent_access_state", json!({})).unwrap();
    assert_eq!(state["running"], json!(false));
    assert!(state["error"].as_str().unwrap().contains(&port.to_string()));
}

#[test]
fn a_port_out_of_range_in_the_file_does_not_start() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let mut saved = settings_file::load_settings(&config);
    saved.agent_access.enabled = true;
    saved.agent_access.port = 80;
    settings_file::save_settings(&config, &saved).unwrap();

    let (app, w) = app_with(state_in(dir.path()));
    agent_access::resume(app.handle());
    let state = invoke(&w, "agent_access_state", json!({})).unwrap();
    assert_eq!(state["running"], json!(false));
    assert!(state["error"].as_str().unwrap().contains("1024"));
}

#[test]
fn a_missing_hash_is_said_and_that_key_does_not_work() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let key = {
        let (app, w) = app_with(state_in(dir.path()));
        set(&w, true, port);
        let (_, key, _) = create(&w, "Reader");
        agent_access::shutdown(&app.state::<AppState>());
        key
    };
    std::fs::remove_file(dir.path().join("config").join(HASH_FILE)).unwrap();
    let (app, w) = app_with(state_in(dir.path()));
    agent_access::resume(app.handle());
    let state = invoke(&w, "agent_access_state", json!({})).unwrap();
    assert_eq!(state["running"], json!(true));
    assert!(state["error"].as_str().unwrap().contains("Reader"));
    assert_eq!(state["connections"][0]["name"], json!("Reader"));
    assert_eq!(ping(port, &key), 401);
    agent_access::shutdown(&app.state::<AppState>());
}

// ---- the port ----

#[test]
fn off_then_on_with_the_same_port_succeeds_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let (_, key, _) = create(&w, "Reader");
    assert_eq!(ping(port, &key), 200);

    for _ in 0..5 {
        let off = set(&w, false, port);
        assert_eq!(off["running"], json!(false));
        assert_eq!(off["error"], Value::Null);
        let refused = TcpStream::connect_timeout(
            &(std::net::Ipv4Addr::LOCALHOST, port).into(),
            Duration::from_millis(200),
        );
        assert!(refused.is_err(), "the port is free when off");
        let on = set(&w, true, port);
        assert_eq!(on["running"], json!(true), "{on}");
        assert_eq!(ping(port, &key), 200);
    }
    set(&w, false, port);
}

/// A client still connected to the old server must not stop the port being taken again.
#[test]
fn off_then_on_with_a_connection_still_open() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let (_, key, _) = create(&w, "Reader");

    let mut idle = connect(port);
    let mut half = connect(port);
    half.write_all(head(port, &key, 100).as_bytes()).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    set(&w, false, port);
    let on = set(&w, true, port);
    assert_eq!(on["running"], json!(true), "{on}");
    assert_eq!(on["error"], Value::Null);
    assert_eq!(ping(port, &key), 200);

    let _ = idle.write_all(b"x");
    let _ = half.write_all(&[b' '; 100]);
    let (status, _) = read_answer(&mut half);
    assert_ne!(status, 200, "the old server runs nothing more");
    set(&w, false, port);
}

#[test]
fn changing_the_port_moves_the_server_and_frees_the_old_port() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let (a, b) = (free_port(), free_port());
    set(&w, true, a);
    let (_, key, _) = create(&w, "Reader");

    let moved = set(&w, true, b);
    assert_eq!(moved["running"], json!(true));
    assert_eq!(moved["port"], json!(b));
    assert_eq!(ping(b, &key), 200);
    // Another program can take the old port now.
    drop(TcpListener::bind(("127.0.0.1", a)).unwrap());
    // And the app can go back.
    let back = set(&w, true, a);
    assert_eq!(back["running"], json!(true), "{back}");
    assert_eq!(ping(a, &key), 200);
    set(&w, false, a);
}

#[test]
fn turning_on_twice_keeps_the_same_server() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let (_, key, _) = create(&w, "Reader");
    let again = set(&w, true, port);
    assert_eq!(again["running"], json!(true));
    assert_eq!(again["error"], Value::Null);
    assert_eq!(ping(port, &key), 200);
    set(&w, false, port);
}

#[test]
fn ports_outside_the_range_are_refused_and_nothing_changes() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let before = invoke(&w, "agent_access_state", json!({})).unwrap();
    for port in [0, 80, 1023, 65536, 70000] {
        let e = invoke(
            &w,
            "agent_access_set",
            json!({ "enabled": true, "port": port }),
        )
        .unwrap_err();
        assert_eq!(e["code"], json!("invalid"), "{port}");
        assert_eq!(
            e["message"],
            json!("The port must be a number from 1024 to 65535.")
        );
    }
    assert_eq!(invoke(&w, "agent_access_state", json!({})).unwrap(), before);
    for port in [1024, 65535] {
        // The ends of the range are allowed. Off, so nothing tries to bind them.
        assert_eq!(set(&w, false, port)["port"], json!(port));
    }
}

#[test]
fn port_and_switch_outlive_the_app() {
    let dir = tempfile::tempdir().unwrap();
    {
        let (_app, w) = app_with(state_in(dir.path()));
        set(&w, false, 9001);
    }
    let (_app, w) = app_with(state_in(dir.path()));
    assert_eq!(
        invoke(&w, "agent_access_state", json!({})).unwrap()["port"],
        json!(9001)
    );
}

// ---- keys and connections ----

#[test]
fn the_state_has_the_shape_the_frontend_expects() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let made = invoke(
        &w,
        "agent_connection_create",
        json!({ "name": "  Reader  " }),
    )
    .unwrap();
    let mut keys: Vec<&String> = made.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["connection", "key", "snippet"]);
    let connection = &made["connection"];
    assert_eq!(connection["name"], json!("Reader"), "trimmed");
    let created = connection["created"].as_i64().unwrap();
    assert!(
        created > 1_700_000_000_000,
        "Unix milliseconds, got {created}"
    );

    let state = invoke(&w, "agent_access_state", json!({})).unwrap();
    assert_eq!(
        state,
        json!({ "enabled": false, "port": 8765, "running": false, "error": null, "requireKey": false, "keylessCommand": "claude mcp add --transport http gene-tree http://127.0.0.1:8765/mcp", "connections": [connection] })
    );
}

#[test]
fn a_connection_needs_a_name() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let e = invoke(&w, "agent_connection_create", json!({ "name": "   " })).unwrap_err();
    assert_eq!(e["code"], json!("invalid"));
    assert_eq!(e["message"], json!("Give the connection a name."));
    let state = invoke(&w, "agent_access_state", json!({})).unwrap();
    assert_eq!(state["connections"], json!([]));
}

#[test]
fn revoking_an_unknown_connection_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    for id in [Uuid::from_u128(5).to_string(), "not-an-id".to_string()] {
        let e = invoke(&w, "agent_connection_revoke", json!({ "id": id })).unwrap_err();
        assert_eq!(e["code"], json!("not_found"));
    }
}

#[test]
fn each_key_is_its_own_agent() {
    let dir = tempfile::tempdir().unwrap();
    let (app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let (a, key_a, _) = create(&w, "First");
    let (b, key_b, _) = create(&w, "Second");
    assert_ne!(key_a, key_b);

    let host = AppHost::new(app.handle().clone());
    let who = |key: &str| host.authenticate(&key_hash(key));
    assert_eq!(
        who(&key_a),
        Some(AgentCtx {
            id: a.parse().unwrap(),
            name: "First".into()
        })
    );
    assert_eq!(
        who(&key_b),
        Some(AgentCtx {
            id: b.parse().unwrap(),
            name: "Second".into()
        })
    );
    assert_eq!(who("gt_unknown"), None);

    // Revoking one leaves the other.
    invoke(&w, "agent_connection_revoke", json!({ "id": a })).unwrap();
    assert_eq!(ping(port, &key_a), 401);
    assert_eq!(ping(port, &key_b), 200);
    set(&w, false, port);
}

#[test]
fn revoking_while_off_means_it_fails_after_turning_on() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    let (id, key, _) = create(&w, "Reader");
    invoke(&w, "agent_connection_revoke", json!({ "id": id })).unwrap();
    set(&w, true, port);
    assert_eq!(ping(port, &key), 401);
    set(&w, false, port);
}

/// The request was let in before the revoke, and its body arrives after it.
#[test]
fn a_request_already_under_way_when_revoked_runs_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let (id, key, _) = create(&w, "Reader");

    let body = rpc("ping", json!({})).to_string();
    let mut s = connect(port);
    s.write_all(head(port, &key, body.len()).as_bytes())
        .unwrap();
    std::thread::sleep(Duration::from_millis(200));
    invoke(&w, "agent_connection_revoke", json!({ "id": id })).unwrap();
    s.write_all(body.as_bytes()).unwrap();
    assert_eq!(read_answer(&mut s).0, 401);
    set(&w, false, port);
}

#[test]
fn the_keychain_entry_is_named_after_the_connection() {
    let id = Uuid::from_u128(5);
    assert_eq!(KeychainHashes::user(&id), format!("mcp-{id}"));
}

// ---- reading, closing, exiting ----

#[test]
fn rules_are_the_ones_agents_get() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let rules = invoke(&w, "agent_rules", json!({})).unwrap();
    assert_eq!(rules, json!(gt_mcp::RULES));
}

#[test]
fn reading_works_through_the_app_and_a_closed_tree_answers_plainly() {
    let dir = tempfile::tempdir().unwrap();
    let project = empty_project(dir.path());
    let (app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let (_, key, _) = create(&w, "Reader");

    let call = rpc(
        "tools/call",
        json!({ "name": "tree_summary", "arguments": {} }),
    );
    let closed = post(port, &key, &call);
    assert_eq!(closed.0, 200);
    assert!(closed.1.contains("isError"), "{}", closed.1);

    open(&app.state::<AppState>(), &project);
    let opened = post(port, &key, &call);
    assert_eq!(opened.0, 200);
    let v: Value = serde_json::from_str(&opened.1).unwrap();
    assert!(v["result"]["isError"].is_null(), "{v}");
    assert!(
        v["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("counts")
    );

    // Closing the tree leaves the server up, with nothing to read.
    commands::close_project_impl(&app.state::<AppState>());
    let closed = post(port, &key, &call);
    assert_eq!(closed.0, 200);
    let v: Value = serde_json::from_str(&closed.1).unwrap();
    assert_eq!(v["result"]["isError"], json!(true), "{v}");
    set(&w, false, port);
}

#[test]
fn a_host_kept_alive_by_a_late_request_finds_no_session() {
    let dir = tempfile::tempdir().unwrap();
    let project = empty_project(dir.path());
    let (app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let (_, key, _) = create(&w, "Reader");
    let state = app.state::<AppState>();
    open(&state, &project);

    let call = rpc(
        "tools/call",
        json!({ "name": "tree_summary", "arguments": {} }),
    )
    .to_string();
    let mut late = connect(port);
    late.write_all(head(port, &key, call.len()).as_bytes())
        .unwrap();
    std::thread::sleep(Duration::from_millis(200));

    // As `RunEvent::Exit` does: the server first, then the tree.
    let host = AppHost::new(app.handle().clone());
    agent_access::shutdown(&state);
    commands::close_project_impl(&state);

    // The socket is still open here, or its thread is on its way out. Either way, nothing ran.
    let _ = late.write_all(call.as_bytes());
    let (status, _) = read_answer(&mut late);
    assert_ne!(status, 200);

    let mut ran = false;
    let got = host.with_session(&mut |_s| {
        ran = true;
        Ok(())
    });
    assert_eq!(got.unwrap_err().code, ErrorCode::NoProject);
    assert!(!ran);
    assert_eq!(host.authenticate(&key_hash(&key)), None);
    assert!(
        !AppState::lock(&state.settings)
            .agent_access
            .connections
            .is_empty(),
        "shutting down forgets nothing"
    );
    assert!(project.join("tree.db").is_file());
}

#[test]
fn shutdown_frees_the_port_and_twice_is_fine() {
    let dir = tempfile::tempdir().unwrap();
    let (app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let state = app.state::<AppState>();
    agent_access::shutdown(&state);
    agent_access::shutdown(&state);
    drop(TcpListener::bind(("127.0.0.1", port)).unwrap());
    // Shutting down is not turning off: the choice is kept for the next start.
    assert_eq!(
        invoke(&w, "agent_access_state", json!({})).unwrap()["enabled"],
        json!(true)
    );
}

// ---- the event ----

#[test]
fn changes_reach_the_page_as_an_event_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let (app, _w) = app_with(state_in(dir.path()));
    let got: Arc<Mutex<Vec<String>>> = Arc::default();
    let sink = Arc::clone(&got);
    app.listen(agent_access::CHANGED_EVENT, move |e| {
        sink.lock().unwrap().push(e.payload().to_string());
    });
    assert_eq!(agent_access::CHANGED_EVENT, "tree-changed-by-agent");

    let host = AppHost::new(app.handle().clone());
    let touched = [Uuid::from_u128(1), Uuid::from_u128(2)];
    let agent = AgentCtx {
        id: Uuid::from_u128(7),
        name: "Reader".into(),
    };
    host.changed(&touched, &agent);

    let deadline = Instant::now() + Duration::from_secs(5);
    while got.lock().unwrap().is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    let list = got.lock().unwrap();
    let payload: Value = serde_json::from_str(&list[0]).unwrap();
    assert_eq!(
        payload,
        json!({
            "touched": [touched[0].to_string(), touched[1].to_string()],
            "agent": "Reader",
        })
    );
}

// ---- when a store cannot be written: a folder where the temp file goes makes every save fail ----

const REVOKE_FAILED: &str = "Access stopped for now, but this could not be recorded; it may return after restart. Try again.";
const OFF_NOT_SAVED: &str = "Agent access is off now, but this could not be saved, so it will come back on at the next start.";

fn break_settings(root: &Path) {
    std::fs::create_dir_all(root.join("config").join("settings.json.tmp")).unwrap();
}

fn heal_settings(root: &Path) {
    std::fs::remove_dir(root.join("config").join("settings.json.tmp")).unwrap();
}

fn break_hashes(root: &Path) {
    std::fs::create_dir_all(root.join("config").join("agent-hashes.json.tmp")).unwrap();
}

fn heal_hashes(root: &Path) {
    std::fs::remove_dir(root.join("config").join("agent-hashes.json.tmp")).unwrap();
}

fn state_of(w: &WebviewWindow<MockRuntime>) -> Value {
    invoke(w, "agent_access_state", json!({})).unwrap()
}

fn revoke(w: &WebviewWindow<MockRuntime>, id: &str) -> Result<Value, Value> {
    invoke(w, "agent_connection_revoke", json!({ "id": id }))
}

fn saved(root: &Path) -> settings_file::AppSettings {
    settings_file::load_settings(&root.join("config"))
}

fn hash_is_kept(root: &Path, id: &str) -> bool {
    FileHashes::new(&root.join("config"))
        .get(&id.parse().unwrap())
        .is_some()
}

fn ids_listed(state: &Value) -> Vec<String> {
    state["connections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect()
}

fn refused_connecting(port: u16) -> bool {
    TcpStream::connect(("127.0.0.1", port)).is_err()
}

#[test]
fn turning_off_stops_the_server_even_if_it_cannot_be_saved() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let (_, key, _) = create(&w, "Reader");
    assert_eq!(ping(port, &key), 200);

    break_settings(dir.path());
    let off = set(&w, false, port);
    assert_eq!(off["enabled"], false);
    assert_eq!(off["running"], false);
    assert_eq!(off["error"], OFF_NOT_SAVED);
    assert!(refused_connecting(port));
    assert!(
        saved(dir.path()).agent_access.enabled,
        "the file is as it was"
    );
}

#[test]
fn off_that_could_not_be_saved_is_saved_by_a_later_action() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    break_settings(dir.path());
    set(&w, false, port);
    heal_settings(dir.path());

    let state = state_of(&w);
    assert_eq!(state["error"], Value::Null);
    assert!(!saved(dir.path()).agent_access.enabled);
}

#[test]
fn off_that_could_not_be_saved_is_saved_at_shutdown() {
    let dir = tempfile::tempdir().unwrap();
    let (app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    break_settings(dir.path());
    set(&w, false, port);
    heal_settings(dir.path());

    agent_access::shutdown(&app.state::<AppState>());
    assert!(!saved(dir.path()).agent_access.enabled);
}

#[test]
fn revoking_with_both_stores_failing_stops_the_key_and_can_be_tried_again() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let (id, key, _) = create(&w, "Reader");

    break_settings(dir.path());
    break_hashes(dir.path());
    let e = revoke(&w, &id).unwrap_err();
    assert_eq!(e["message"], REVOKE_FAILED);
    assert_eq!(ping(port, &key), 401, "stopped at once");

    // The row stays, so there is something to try again on, and the state says why.
    let state = state_of(&w);
    assert_eq!(ids_listed(&state), std::slice::from_ref(&id));
    assert_eq!(state["error"], REVOKE_FAILED);

    // Still failing: the same answer, not "no such connection".
    let again = revoke(&w, &id).unwrap_err();
    assert_eq!(again["message"], REVOKE_FAILED);

    heal_settings(dir.path());
    heal_hashes(dir.path());
    revoke(&w, &id).unwrap();
    let state = state_of(&w);
    assert!(ids_listed(&state).is_empty());
    assert_eq!(state["error"], Value::Null);
    assert!(saved(dir.path()).agent_access.connections.is_empty());
    assert!(saved(dir.path()).agent_access.revoked.is_empty());
    assert!(!hash_is_kept(dir.path(), &id));
    assert_eq!(ping(port, &key), 401);

    // Finished now, so a third time is a plain "not found".
    let e = revoke(&w, &id).unwrap_err();
    assert_eq!(e["code"], json!(ErrorCode::NotFound));
}

#[test]
fn a_key_stays_refused_when_the_server_is_moved_after_a_failed_revoke() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    set(&w, true, free_port());
    let (id, key, _) = create(&w, "Reader");
    break_settings(dir.path());
    break_hashes(dir.path());
    revoke(&w, &id).unwrap_err();

    heal_settings(dir.path());
    let port = free_port();
    set(&w, true, port);
    assert_eq!(
        ping(port, &key),
        401,
        "a restart of the server keeps it out"
    );
}

#[test]
fn revoking_with_only_settings_failing_keeps_the_key_out_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let key;
    {
        let (app, w) = app_with(state_in(dir.path()));
        set(&w, true, port);
        let (id, k, _) = create(&w, "Reader");
        key = k;
        break_settings(dir.path());
        let e = revoke(&w, &id).unwrap_err();
        assert_eq!(e["message"], REVOKE_FAILED);
        assert!(!hash_is_kept(dir.path(), &id), "this store did work");
        agent_access::shutdown(&app.state::<AppState>());
    }
    // Still broken at the restart: the file lists it, but its hash is gone.
    let (app, w) = app_with(state_in(dir.path()));
    agent_access::resume(app.handle());
    let state = state_of(&w);
    assert_eq!(ids_listed(&state).len(), 1);
    assert!(
        state["error"].as_str().unwrap().contains("Reader"),
        "{state}"
    );
    assert_eq!(ping(port, &key), 401);
    agent_access::shutdown(&app.state::<AppState>());
}

#[test]
fn revoking_with_only_the_keychain_failing_is_retried_at_the_next_start() {
    let dir = tempfile::tempdir().unwrap();
    let id;
    {
        let (app, w) = app_with(state_in(dir.path()));
        set(&w, true, free_port());
        id = create(&w, "Reader").0;
        break_hashes(dir.path());
        let e = revoke(&w, &id).unwrap_err();
        assert_eq!(e["message"], REVOKE_FAILED);
        assert!(saved(dir.path()).agent_access.connections.is_empty());
        assert_eq!(
            saved(dir.path()).agent_access.revoked,
            [id.parse::<Uuid>().unwrap()],
            "the id is on disk, so a later start can finish"
        );
        agent_access::shutdown(&app.state::<AppState>());
        assert!(hash_is_kept(dir.path(), &id), "still failing at shutdown");
    }
    heal_hashes(dir.path());
    let (app, w) = app_with(state_in(dir.path()));
    agent_access::resume(app.handle());
    assert!(!hash_is_kept(dir.path(), &id));
    assert!(saved(dir.path()).agent_access.revoked.is_empty());
    assert_eq!(state_of(&w)["error"], Value::Null);
    agent_access::shutdown(&app.state::<AppState>());
}

#[test]
fn a_failed_revoke_is_finished_by_later_actions() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let id = create(&w, "Reader").0;
    break_settings(dir.path());
    break_hashes(dir.path());
    revoke(&w, &id).unwrap_err();
    heal_settings(dir.path());
    heal_hashes(dir.path());

    // Nothing was asked of this connection: another action is enough.
    let other = create(&w, "Second").0;
    let state = state_of(&w);
    assert_eq!(ids_listed(&state), [other]);
    assert_eq!(state["error"], Value::Null);
    assert!(!hash_is_kept(dir.path(), &id));
}

#[test]
fn a_failed_revoke_is_finished_at_shutdown() {
    let dir = tempfile::tempdir().unwrap();
    let (app, w) = app_with(state_in(dir.path()));
    set(&w, true, free_port());
    let id = create(&w, "Reader").0;
    break_settings(dir.path());
    break_hashes(dir.path());
    revoke(&w, &id).unwrap_err();
    heal_settings(dir.path());
    heal_hashes(dir.path());

    agent_access::shutdown(&app.state::<AppState>());
    assert!(saved(dir.path()).agent_access.connections.is_empty());
    assert!(!hash_is_kept(dir.path(), &id));
}

#[test]
fn revoking_the_connection_an_error_names_clears_the_error() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let (id, other) = {
        let (app, w) = app_with(state_in(dir.path()));
        let ids = (create(&w, "Reader").0, create(&w, "Second").0);
        set(&w, true, port);
        agent_access::shutdown(&app.state::<AppState>());
        ids
    };
    let hashes = dir.path().join("config").join(HASH_FILE);
    let map: serde_json::Map<String, Value> =
        serde_json::from_str(&std::fs::read_to_string(&hashes).unwrap()).unwrap();
    let kept: serde_json::Map<String, Value> =
        map.into_iter().filter(|(k, _)| *k == other).collect();
    std::fs::write(&hashes, Value::Object(kept).to_string()).unwrap();

    let (app, w) = app_with(state_in(dir.path()));
    agent_access::resume(app.handle());
    assert!(
        state_of(&w)["error"].as_str().unwrap().contains("Reader"),
        "the error names it"
    );
    revoke(&w, &id).unwrap();
    assert_eq!(state_of(&w)["error"], Value::Null);
    assert_eq!(state_of(&w)["running"], true);
    agent_access::shutdown(&app.state::<AppState>());
}

#[test]
fn moving_to_a_taken_port_keeps_the_working_server() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let (_, key, _) = create(&w, "Reader");
    let holder = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let taken = holder.local_addr().unwrap().port();

    let state = set(&w, true, taken);
    assert_eq!(state["running"], true);
    assert_eq!(state["port"], port, "still where it works");
    assert!(
        state["error"]
            .as_str()
            .unwrap()
            .contains(&taken.to_string())
    );
    assert_eq!(ping(port, &key), 200);
    assert_eq!(saved(dir.path()).agent_access.port, port);
}

#[test]
fn moving_when_the_move_cannot_be_saved_keeps_the_working_server() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let (_, key, _) = create(&w, "Reader");
    let next = free_port();

    break_settings(dir.path());
    let e = invoke(
        &w,
        "agent_access_set",
        json!({ "enabled": true, "port": next }),
    )
    .unwrap_err();
    assert!(
        e["message"]
            .as_str()
            .unwrap()
            .starts_with("Could not save settings")
    );
    assert_eq!(ping(port, &key), 200);
    assert!(refused_connecting(next), "nothing left on the new port");
    assert_eq!(state_of(&w)["port"], port);
}

#[test]
fn connection_names_are_trimmed_short_and_plain_and_not_repeated() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let make = |name: String| invoke(&w, "agent_connection_create", json!({ "name": name }));
    let message = |e: Value| e["message"].as_str().unwrap().to_string();

    let made = make("  Reader  ".into()).unwrap();
    assert_eq!(made["connection"]["name"], "Reader");

    assert_eq!(
        message(make("x".repeat(101)).unwrap_err()),
        "Keep the connection name to 100 characters or fewer."
    );
    // Characters, not bytes: 100 Cyrillic letters are 200 bytes.
    make("я".repeat(100)).unwrap();

    for bad in ["Two\nlines", "Tab\tinside", "Bell\u{7}"] {
        assert_eq!(
            message(make(bad.into()).unwrap_err()),
            "The connection name cannot contain line breaks or other control characters."
        );
    }
    for repeat in ["Reader", " reader ", "READER"] {
        assert_eq!(
            message(make(repeat.into()).unwrap_err()),
            format!(
                "A connection named \"{}\" already exists. Choose another name.",
                repeat.trim()
            )
        );
    }
    assert_eq!(state_of(&w)["connections"].as_array().unwrap().len(), 2);
}

#[test]
fn a_stored_hash_with_a_sign_is_not_a_hash() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path()).unwrap();
    let id = Uuid::from_u128(5);
    let signed = "+a".repeat(32);
    std::fs::write(
        dir.path().join(HASH_FILE),
        json!({ id.to_string(): signed }).to_string(),
    )
    .unwrap();
    assert_eq!(FileHashes::new(dir.path()).get(&id), None);

    std::fs::write(
        dir.path().join(HASH_FILE),
        json!({ id.to_string(): "0a".repeat(32) }).to_string(),
    )
    .unwrap();
    assert_eq!(FileHashes::new(dir.path()).get(&id), Some([0x0a; 32]));
}

// ---- no key required (the default) ----

const KEYLESS_COMMAND: &str = "claude mcp add --transport http gene-tree http://127.0.0.1:8765/mcp";
const BOTH_NOT_SAVED: &str = "Agent access is off now, but this could not be saved, so it will come back on at the next start. A key is required now, but this could not be saved, so after the next start no key will be required.";
const REQUIRE_NOT_SAVED: &str = "A key is required now, but this could not be saved, so after the next start no key will be required.";

fn require(w: &WebviewWindow<MockRuntime>, required: bool) -> Result<Value, Value> {
    invoke(
        w,
        "agent_access_require_key",
        json!({ "required": required }),
    )
}

/// A request with no `Authorization` header at all; `extra` are more header lines.
fn post_bare_with(port: u16, extra: &str, body: &Value) -> (u16, String) {
    let text = body.to_string();
    let mut s = connect(port);
    let head = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\n\r\n",
        text.len()
    );
    s.write_all(head.as_bytes()).unwrap();
    s.write_all(text.as_bytes()).unwrap();
    read_answer(&mut s)
}

fn post_bare(port: u16, body: &Value) -> (u16, String) {
    post_bare_with(port, "", body)
}

fn ping_bare(port: u16) -> u16 {
    post_bare(port, &rpc("ping", json!({}))).0
}

fn tool_json(answer: &(u16, String)) -> Value {
    assert_eq!(answer.0, 200, "{}", answer.1);
    let v: Value = serde_json::from_str(&answer.1).unwrap();
    assert!(v["result"]["isError"].is_null(), "{v}");
    serde_json::from_str(v["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
}

fn tool_bare(port: u16, name: &str, arguments: Value) -> Value {
    tool_json(&post_bare(
        port,
        &rpc(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        ),
    ))
}

#[test]
fn a_new_state_asks_for_no_key_and_access_is_still_off() {
    let dir = tempfile::tempdir().unwrap();
    let (app, w) = app_with(state_in(dir.path()));
    agent_access::resume(app.handle());
    let state = state_of(&w);
    assert_eq!(state["enabled"], false);
    assert_eq!(state["running"], false);
    assert_eq!(state["requireKey"], false);
    assert_eq!(state["keylessCommand"], KEYLESS_COMMAND);
    assert!(refused_connecting(8765) || ping_bare(8765) != 200);
    assert!(
        !settings_file::AppSettings::default()
            .agent_access
            .require_key
    );
    assert!(!settings_file::AppSettings::default().agent_access.enabled);
}

#[test]
fn an_old_settings_file_reads_as_no_key_required_with_access_still_off() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    std::fs::create_dir_all(&config).unwrap();
    // As an earlier build wrote it: no requireKey anywhere.
    std::fs::write(
        config.join(SETTINGS_FILE),
        r#"{"agentAccess":{"enabled":false,"port":9123,"connections":[]},"backupsToKeep":5}"#,
    )
    .unwrap();
    let loaded = settings_file::load_settings(&config);
    assert!(!loaded.agent_access.require_key);
    assert!(!loaded.agent_access.enabled);
    assert_eq!(loaded.agent_access.port, 9123);
    assert_eq!(loaded.backups_to_keep, 5);

    let (app, w) = app_with(state_in(dir.path()));
    agent_access::resume(app.handle());
    let state = state_of(&w);
    assert_eq!(state["requireKey"], false);
    assert_eq!(state["running"], false, "access was off and stays off");
    assert!(refused_connecting(9123));
}

#[test]
fn an_old_settings_file_with_access_on_comes_back_keyless() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let config = dir.path().join("config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join(SETTINGS_FILE),
        format!(r#"{{"agentAccess":{{"enabled":true,"port":{port},"connections":[]}}}}"#),
    )
    .unwrap();
    let (app, _w) = app_with(state_in(dir.path()));
    agent_access::resume(app.handle());
    assert_eq!(ping_bare(port), 200);
    agent_access::shutdown(&app.state::<AppState>());
}

#[test]
fn a_settings_file_that_cannot_be_read_gives_defaults_with_access_off() {
    // Existing behaviour kept: an unreadable or malformed file is the defaults, so access is off.
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    std::fs::create_dir_all(&config).unwrap();
    for text in [
        "{ not json",
        r#"{"agentAccess":{"enabled":true,"requireKey":"yes"}}"#,
    ] {
        std::fs::write(config.join(SETTINGS_FILE), text).unwrap();
        let loaded = settings_file::load_settings(&config);
        assert!(!loaded.agent_access.enabled, "{text}");
        assert!(!loaded.agent_access.require_key, "{text}");
    }
}

#[test]
fn without_a_key_a_request_runs_as_the_builtin_agent_that_the_log_lists() {
    let dir = tempfile::tempdir().unwrap();
    let project = empty_project(dir.path());
    let state = state_in(dir.path());
    open(&state, &project);
    let (_app, w) = app_with(state);
    let port = free_port();
    set(&w, true, port);
    assert_eq!(
        invoke(&w, "change_log_agents", json!({})).unwrap(),
        json!([]),
        "nobody has written yet"
    );

    let batch = tool_bare(port, "begin_batch", json!({ "title": "Recording 7" }))["batch"].clone();
    tool_bare(
        port,
        "add_person",
        json!({ "batch": batch, "given": "Ivan", "surnames": ["Koval"] }),
    );
    let agents = invoke(&w, "change_log_agents", json!({})).unwrap();
    assert_eq!(
        agents,
        json!([{ "id": gt_mcp::KEYLESS_AGENT_ID, "name": gt_mcp::KEYLESS_AGENT_NAME }])
    );
    // It is never one of the connections, and it cannot be revoked.
    assert_eq!(state_of(&w)["connections"], json!([]));
    let e = revoke(&w, &gt_mcp::KEYLESS_AGENT_ID.to_string()).unwrap_err();
    assert_eq!(e["code"], "not_found");
    assert_eq!(ping_bare(port), 200);
    set(&w, false, port);
}

#[test]
fn without_a_key_a_valid_key_keeps_its_connection_name_and_a_bad_one_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let project = empty_project(dir.path());
    let state = state_in(dir.path());
    open(&state, &project);
    let (_app, w) = app_with(state);
    let port = free_port();
    set(&w, true, port);
    let (id, key, snippet) = create(&w, "Reader");
    assert!(
        snippet.contains(&key),
        "a new connection's command has its key"
    );

    let batch = tool_json(&post(
        port,
        &key,
        &rpc(
            "tools/call",
            json!({ "name": "begin_batch", "arguments": { "title": "Recording 8" } }),
        ),
    ))["batch"]
        .clone();
    tool_json(&post(
        port,
        &key,
        &rpc(
            "tools/call",
            json!({ "name": "add_person", "arguments": { "batch": batch, "given": "Olena", "surnames": ["Koval"] } }),
        ),
    ));
    let agents = invoke(&w, "change_log_agents", json!({})).unwrap();
    assert_eq!(agents, json!([{ "id": id, "name": "Reader" }]));

    assert_eq!(ping(port, "gt_not_a_key"), 401);
    revoke(&w, &id).unwrap();
    assert_eq!(
        ping(port, &key),
        401,
        "a revoked key is 401, not the built-in agent"
    );
    assert_eq!(
        ping_bare(port),
        200,
        "no key at all is still the built-in agent"
    );
    set(&w, false, port);
}

#[test]
fn requiring_a_key_refuses_the_next_request_and_not_requiring_lets_it_in_again() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let (_, key, _) = create(&w, "Reader");
    assert_eq!(ping_bare(port), 200);

    let on = require(&w, true).unwrap();
    assert_eq!(on["requireKey"], true);
    assert_eq!(on["keylessCommand"], Value::Null);
    assert_eq!(on["error"], Value::Null);
    assert_eq!(ping_bare(port), 401);
    assert_eq!(ping(port, &key), 200);
    assert!(saved(dir.path()).agent_access.require_key);

    let off = require(&w, false).unwrap();
    assert_eq!(off["requireKey"], false);
    assert_eq!(
        off["keylessCommand"],
        format!("claude mcp add --transport http gene-tree http://127.0.0.1:{port}/mcp")
    );
    assert_eq!(ping_bare(port), 200);
    assert!(!saved(dir.path()).agent_access.require_key);
    set(&w, false, port);
}

#[test]
fn the_requirement_survives_a_restart_and_a_stop() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    {
        let (app, w) = app_with(state_in(dir.path()));
        set(&w, true, port);
        require(&w, true).unwrap();
        agent_access::shutdown(&app.state::<AppState>());
    }
    let (app, w) = app_with(state_in(dir.path()));
    agent_access::resume(app.handle());
    assert_eq!(state_of(&w)["requireKey"], true);
    assert_eq!(ping_bare(port), 401);
    // Off and on again does not open it.
    set(&w, false, port);
    set(&w, true, port);
    assert_eq!(ping_bare(port), 401);
    agent_access::shutdown(&app.state::<AppState>());
}

#[test]
fn stopping_closes_the_door_for_a_request_with_no_key_too() {
    let dir = tempfile::tempdir().unwrap();
    let (app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let host = AppHost::new(app.handle().clone());
    assert!(host.authenticate_keyless().is_some());
    set(&w, false, port);
    assert!(
        host.authenticate_keyless().is_none(),
        "a late request finds no door"
    );
    assert!(refused_connecting(port));
}

#[test]
fn no_connection_can_take_the_builtin_agents_name_in_either_language() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    for name in [
        " agent WITHOUT a key ",
        "Agent without a key",
        "agent   without\ta  key",
        "Агент без ключа",
        "  агент   БЕЗ\tКЛЮЧА ",
    ] {
        let e = invoke(&w, "agent_connection_create", json!({ "name": name })).unwrap_err();
        assert_eq!(e["code"], "invalid", "{name:?}");
    }
    assert_eq!(state_of(&w)["connections"], json!([]));
    // Near misses are fine.
    invoke(
        &w,
        "agent_connection_create",
        json!({ "name": "Agent without a key 2" }),
    )
    .unwrap();
}

#[test]
fn a_connection_that_already_had_the_name_still_works_under_its_own_id() {
    // Made before the name was reserved: it is in the file and keeps working.
    let dir = tempfile::tempdir().unwrap();
    let project = empty_project(dir.path());
    let config = dir.path().join("config");
    let id = Uuid::from_u128(5);
    let key = "gt_old_connection_key";
    let mut saved = settings_file::load_settings(&config);
    saved
        .agent_access
        .connections
        .push(settings_file::ConnectionDto {
            id,
            name: "Агент без ключа".into(),
            created: 1,
        });
    settings_file::save_settings(&config, &saved).unwrap();
    FileHashes::new(&config).set(&id, &key_hash(key)).unwrap();
    let state = state_in(dir.path());
    open(&state, &project);
    let (_app, w) = app_with(state);
    let port = free_port();
    set(&w, true, port);
    let batch = tool_json(&post(
        port,
        key,
        &rpc(
            "tools/call",
            json!({ "name": "begin_batch", "arguments": { "title": "Recording 1" } }),
        ),
    ))["batch"]
        .clone();
    tool_json(&post(
        port,
        key,
        &rpc(
            "tools/call",
            json!({ "name": "add_person", "arguments": { "batch": batch, "given": "Ivan", "surnames": ["Koval"] } }),
        ),
    ));
    let agents = invoke(&w, "change_log_agents", json!({})).unwrap();
    assert_eq!(
        agents[0]["id"],
        json!(id),
        "its own id, not the built-in one: {agents}"
    );
    let batches = invoke(&w, "change_batches", json!({})).unwrap();
    assert_eq!(batches[0]["actorId"], json!(id));
    set(&w, false, port);
}

#[test]
fn a_batch_names_its_agent_by_id() {
    let dir = tempfile::tempdir().unwrap();
    let project = empty_project(dir.path());
    let state = state_in(dir.path());
    open(&state, &project);
    let (_app, w) = app_with(state);
    let port = free_port();
    set(&w, true, port);
    tool_bare(port, "begin_batch", json!({ "title": "Recording 1" }));
    let batches = invoke(&w, "change_batches", json!({})).unwrap();
    assert_eq!(batches[0]["actorId"], json!(gt_mcp::KEYLESS_AGENT_ID));
    set(&w, false, port);
}

#[test]
fn a_key_that_could_not_be_saved_is_not_hidden_by_access_being_off() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    // Access off from the start, and nothing about it unsaved: only the key is.
    break_settings(dir.path());
    let on = require(&w, true).unwrap();
    assert_eq!(on["enabled"], false);
    assert_eq!(on["error"], REQUIRE_NOT_SAVED);
    heal_settings(dir.path());
    assert_eq!(state_of(&w)["error"], Value::Null);
}

#[test]
fn both_failed_saves_are_said_together() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    break_settings(dir.path());
    assert_eq!(set(&w, false, port)["error"], OFF_NOT_SAVED);
    assert_eq!(require(&w, true).unwrap()["error"], BOTH_NOT_SAVED);
    heal_settings(dir.path());
    assert_eq!(state_of(&w)["error"], Value::Null);
    let file = saved(dir.path());
    assert!(!file.agent_access.enabled && file.agent_access.require_key);
}

#[test]
fn requiring_a_key_stops_a_request_already_past_its_head() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let body = rpc("ping", json!({})).to_string();
    let mut s = connect(port);
    let head = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes()).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    require(&w, true).unwrap();
    s.write_all(body.as_bytes()).unwrap();
    assert_eq!(read_answer(&mut s).0, 401);
    set(&w, false, port);
}

#[test]
fn requiring_a_key_that_cannot_be_saved_applies_now_says_so_and_can_be_tried_again() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    break_settings(dir.path());
    let on = require(&w, true).unwrap();
    assert_eq!(on["requireKey"], true);
    assert_eq!(on["error"], REQUIRE_NOT_SAVED);
    assert_eq!(ping_bare(port), 401, "applied in memory first");
    assert!(
        !saved(dir.path()).agent_access.require_key,
        "the file is as it was"
    );

    // Trying again while it still fails says the same.
    assert_eq!(require(&w, true).unwrap()["error"], REQUIRE_NOT_SAVED);
    heal_settings(dir.path());
    let again = require(&w, true).unwrap();
    assert_eq!(again["error"], Value::Null);
    assert!(saved(dir.path()).agent_access.require_key);
    assert_eq!(ping_bare(port), 401);
    set(&w, false, port);
}

#[test]
fn a_requirement_that_could_not_be_saved_is_saved_by_a_later_action() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    break_settings(dir.path());
    require(&w, true).unwrap();
    heal_settings(dir.path());
    assert_eq!(state_of(&w)["error"], Value::Null);
    assert!(saved(dir.path()).agent_access.require_key);
    set(&w, false, port);
}

#[test]
fn not_requiring_a_key_applies_only_once_it_is_saved() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    require(&w, true).unwrap();
    break_settings(dir.path());
    let e = require(&w, false).unwrap_err();
    assert_eq!(e["code"], "io");
    assert_eq!(state_of(&w)["requireKey"], true, "fails closed");
    assert_eq!(ping_bare(port), 401);
    heal_settings(dir.path());
    assert_eq!(require(&w, false).unwrap()["requireKey"], false);
    assert_eq!(ping_bare(port), 200);
    set(&w, false, port);
}

#[test]
fn without_a_key_a_web_page_and_a_wrong_host_are_still_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (_app, w) = app_with(state_in(dir.path()));
    let port = free_port();
    set(&w, true, port);
    let body = rpc("ping", json!({}));
    for origin in ["https://evil.example", "null", "http://localhost:3000"] {
        let line = format!("Origin: {origin}\r\n");
        assert_eq!(post_bare_with(port, &line, &body).0, 403, "{origin}");
    }
    let text = body.to_string();
    let mut s = connect(port);
    let head = format!(
        "POST /mcp HTTP/1.1\r\nHost: evil.example:{port}\r\nContent-Length: {}\r\n\r\n",
        text.len()
    );
    s.write_all(head.as_bytes()).unwrap();
    s.write_all(text.as_bytes()).unwrap();
    assert_eq!(read_answer(&mut s).0, 403);
    set(&w, false, port);
}

#[test]
fn without_a_key_the_agent_still_cannot_change_who_is_alive_or_claim_a_name() {
    let dir = tempfile::tempdir().unwrap();
    let project = empty_project(dir.path());
    let state = state_in(dir.path());
    open(&state, &project);
    let (_app, w) = app_with(state);
    let port = free_port();
    set(&w, true, port);
    let forged = json!({ "id": Uuid::from_u128(99), "name": "The owner", "kind": "user" });
    let batch = tool_bare(
        port,
        "begin_batch",
        json!({ "title": "Recording 9", "name": "The owner", "actor": forged, "agent": forged }),
    )["batch"]
        .clone();
    let person = tool_bare(
        port,
        "add_person",
        json!({ "batch": batch, "given": "Ivan", "surnames": ["Koval"], "actor": forged }),
    )["id"]
        .clone();
    let refused = post_bare(
        port,
        &rpc(
            "tools/call",
            json!({ "name": "add_event", "arguments": { "batch": batch, "person": person, "kind": "death", "date": "1950" } }),
        ),
    );
    assert!(refused.1.contains("alive or dead"), "{}", refused.1);
    let agents = invoke(&w, "change_log_agents", json!({})).unwrap();
    assert_eq!(
        agents,
        json!([{ "id": gt_mcp::KEYLESS_AGENT_ID, "name": gt_mcp::KEYLESS_AGENT_NAME }])
    );
    set(&w, false, port);
}
