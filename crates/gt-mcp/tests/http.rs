#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The real listener on an ephemeral port, spoken to over a plain socket so every header is under
//! the test's control. The security checks must answer before any body is read: those tests send
//! the headers only and expect a reply anyway.

mod common;

use common::*;
use gt_mcp::{RULES, ServerHandle, serve};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A body length above what the HTTP library buffers by itself, so it reaches the checks unread.
const UNREAD: usize = 2048;

struct Reply {
    status: u16,
    head: String,
    body: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

struct Fixture {
    host: Arc<TestHost>,
    server: ServerHandle,
}

fn start() -> Fixture {
    let host = Arc::new(empty_host());
    let server = serve(Arc::clone(&host), 0).unwrap();
    Fixture { host, server }
}

impl Fixture {
    fn port(&self) -> u16 {
        self.server.port()
    }

    fn good_host(&self) -> String {
        format!("127.0.0.1:{}", self.port())
    }

    fn connect(&self) -> TcpStream {
        let s = TcpStream::connect(("127.0.0.1", self.port())).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        s
    }

    /// Sends the head as given and reads the reply to the end.
    fn raw(&self, head: &str, body: &[u8]) -> Reply {
        let mut s = self.connect();
        s.write_all(head.as_bytes()).unwrap();
        s.write_all(body).unwrap();
        read_reply(&mut s)
    }

    /// A POST to `/mcp` with the good host and key and the extra header lines given.
    fn post(&self, extra: &[(&str, &str)], body: &Value) -> Reply {
        let text = body.to_string();
        let mut head = format!(
            "POST /mcp HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {KEY}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nConnection: close\r\nContent-Length: {}\r\n",
            self.good_host(),
            text.len()
        );
        for (name, value) in extra {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str("\r\n");
        self.raw(&head, text.as_bytes())
    }

    fn post_modern(&self, method: &str, name: Option<&str>, params: Value) -> Reply {
        let mut extra = vec![("MCP-Protocol-Version", MODERN), ("Mcp-Method", method)];
        if let Some(n) = name {
            extra.push(("Mcp-Name", n));
        }
        self.post(&extra, &modern_request(method, params))
    }

    /// A POST head announcing `length` bytes, of which none are sent.
    fn head_only(&self, lines: &[String], length: usize) -> String {
        let mut head = "POST /mcp HTTP/1.1\r\n".to_string();
        for line in lines {
            head.push_str(line);
            head.push_str("\r\n");
        }
        head.push_str(&format!(
            "Content-Length: {length}\r\nConnection: close\r\n\r\n"
        ));
        head
    }

    fn host_line(&self) -> String {
        format!("Host: {}", self.good_host())
    }
}

fn read_reply(s: &mut TcpStream) -> Reply {
    let mut all = Vec::new();
    let mut buf = [0u8; 8192];
    // Reads until the announced body is complete, so an open connection cannot stall the test.
    loop {
        let n = match s.read(&mut buf) {
            Ok(n) => n,
            Err(e) if !all.is_empty() && e.kind() == std::io::ErrorKind::ConnectionReset => 0,
            Err(e) => panic!("read failed: {e}"),
        };
        if n == 0 {
            break;
        }
        all.extend_from_slice(&buf[..n]);
        if let Some(end) = find_end(&all) {
            all.truncate(end);
            break;
        }
    }
    let split = all.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&all[..split]).to_string();
    let status = head.split_whitespace().nth(1).unwrap().parse().unwrap();
    Reply {
        status,
        head,
        body: all[split + 4..].to_vec(),
    }
}

/// Where a reply ends, once its head and all of its `Content-Length` body have arrived.
fn find_end(all: &[u8]) -> Option<usize> {
    let split = all.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(&all[..split]).to_lowercase();
    // A request announcing a wild length is answered with no end marked: the status is all there is.
    if head.contains("connection: upgrade") {
        return Some(split + 4);
    }
    let length: usize = head
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .and_then(|v| v.trim().parse().ok())?;
    (all.len() >= split + 4 + length).then_some(split + 4 + length)
}

fn initialize_body() -> Value {
    request(
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "1"},
        }),
    )
}

// ---- security checks, all answered before the body ----

#[test]
fn no_key_401() {
    let f = start();
    let head = f.head_only(&[f.host_line()], UNREAD);
    let r = f.raw(&head, b"");
    assert_eq!(r.status, 401);
    assert!(
        r.head.to_lowercase().contains("www-authenticate: bearer"),
        "{}",
        r.head
    );
}

#[test]
fn wrong_key_and_wrong_scheme_401() {
    let f = start();
    for auth in [
        "Authorization: Bearer not-the-key".to_string(),
        format!("Authorization: Basic {KEY}"),
        format!("Authorization: {KEY}"),
        "Authorization: Bearer ".to_string(),
    ] {
        let head = f.head_only(&[f.host_line(), auth.clone()], UNREAD);
        assert_eq!(f.raw(&head, b"").status, 401, "{auth}");
    }
}

#[test]
fn two_keys_401() {
    let f = start();
    let head = f.head_only(
        &[
            f.host_line(),
            format!("Authorization: Bearer {KEY}"),
            "Authorization: Bearer other".to_string(),
        ],
        UNREAD,
    );
    assert_eq!(f.raw(&head, b"").status, 401);
}

#[test]
fn revoked_key_401() {
    let f = start();
    assert_eq!(f.post(&[], &initialize_body()).status, 200);
    f.host.keys.lock().unwrap().clear();
    assert_eq!(f.post(&[], &initialize_body()).status, 401);
}

#[test]
fn foreign_origin_403() {
    let f = start();
    let auth = format!("Authorization: Bearer {KEY}");
    for origin in [
        "https://evil.example".to_string(),
        "null".to_string(),
        format!("http://evil.example:{}", f.port()),
        format!("https://127.0.0.1:{}", f.port()),
        "http://127.0.0.1:1".to_string(),
        "http://127.0.0.1".to_string(),
    ] {
        let head = f.head_only(
            &[f.host_line(), auth.clone(), format!("Origin: {origin}")],
            UNREAD,
        );
        assert_eq!(f.raw(&head, b"").status, 403, "{origin}");
    }
    // The app's own loopback origins pass.
    for origin in [
        format!("http://127.0.0.1:{}", f.port()),
        format!("http://localhost:{}", f.port()),
    ] {
        let r = f.post(&[("Origin", origin.as_str())], &initialize_body());
        assert_eq!(r.status, 200, "{origin}");
    }
}

#[test]
fn foreign_host_403() {
    let f = start();
    let auth = format!("Authorization: Bearer {KEY}");
    for host in [
        format!("evil.example:{}", f.port()),
        "evil.example".to_string(),
        "127.0.0.1".to_string(),
        "127.0.0.1:1".to_string(),
        format!("127.0.0.1.evil.example:{}", f.port()),
    ] {
        let head = f.head_only(&[format!("Host: {host}"), auth.clone()], UNREAD);
        assert_eq!(f.raw(&head, b"").status, 403, "{host}");
    }
    let localhost = format!(
        "POST /mcp HTTP/1.1\r\nHost: localhost:{}\r\nAuthorization: Bearer {KEY}\r\nConnection: close\r\nContent-Length: 2\r\n\r\n{{}}",
        f.port()
    );
    assert_ne!(f.raw(&localhost, b"").status, 403);
}

#[test]
fn checks_run_in_order() {
    let f = start();
    // A foreign host loses to nothing, even with a bad origin and no key.
    let head = f.head_only(
        &[
            "Host: evil.example".to_string(),
            "Origin: https://evil.example".to_string(),
        ],
        UNREAD,
    );
    assert_eq!(f.raw(&head, b"").status, 403);
    // A foreign origin is refused before the key is looked at.
    let head = f.head_only(
        &[f.host_line(), "Origin: https://evil.example".to_string()],
        UNREAD,
    );
    assert_eq!(f.raw(&head, b"").status, 403);
}

#[test]
fn binds_loopback_only() {
    let f = start();
    assert_eq!(
        f.server.local_addr().ip(),
        "127.0.0.1".parse::<IpAddr>().unwrap()
    );
    // The machine's own network address, when it has one, must not reach the server.
    let probe = UdpSocket::bind("0.0.0.0:0").unwrap();
    if probe.connect("192.0.2.1:9").is_ok()
        && let Ok(addr) = probe.local_addr()
        && !addr.ip().is_loopback()
        && !addr.ip().is_unspecified()
    {
        let target = SocketAddr::new(addr.ip(), f.port());
        assert!(
            TcpStream::connect_timeout(&target, Duration::from_secs(2)).is_err(),
            "reachable on {target}"
        );
    }
}

#[test]
fn get_405() {
    let f = start();
    for method in ["GET", "DELETE", "PUT", "OPTIONS"] {
        let head = format!(
            "{method} /mcp HTTP/1.1\r\n{}\r\nAuthorization: Bearer {KEY}\r\nConnection: close\r\n\r\n",
            f.host_line()
        );
        let r = f.raw(&head, b"");
        assert_eq!(r.status, 405, "{method}");
        assert!(r.head.to_lowercase().contains("allow: post"), "{}", r.head);
    }
    // Without a key a method on /mcp is still a 401: nothing is said to strangers.
    let head = format!(
        "GET /mcp HTTP/1.1\r\n{}\r\nConnection: close\r\n\r\n",
        f.host_line()
    );
    assert_eq!(f.raw(&head, b"").status, 401);
}

#[test]
fn other_paths_404() {
    let f = start();
    for path in [
        "/",
        "/mcp/",
        "/mcpx",
        "/other",
        "/.well-known/oauth-protected-resource",
    ] {
        let head = format!(
            "POST {path} HTTP/1.1\r\n{}\r\nAuthorization: Bearer {KEY}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
            f.host_line()
        );
        assert_eq!(f.raw(&head, b"").status, 404, "{path}");
    }
    // The endpoint with a query string is still the endpoint.
    let head = format!(
        "GET /mcp?x=1 HTTP/1.1\r\n{}\r\nAuthorization: Bearer {KEY}\r\nConnection: close\r\n\r\n",
        f.host_line()
    );
    assert_eq!(f.raw(&head, b"").status, 405);
}

#[test]
fn oversized_body_413() {
    let f = start();
    let head = f.head_only(
        &[f.host_line(), format!("Authorization: Bearer {KEY}")],
        8 * 1024 * 1024 + 1,
    );
    assert_eq!(
        f.raw(&head, b"").status,
        413,
        "refused from the announced length alone"
    );
}

#[test]
fn oversized_chunked_body_413() {
    let f = start();
    let head = format!(
        "POST /mcp HTTP/1.1\r\n{}\r\nAuthorization: Bearer {KEY}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
        f.host_line()
    );
    let mut s = f.connect();
    s.write_all(head.as_bytes()).unwrap();
    // One byte past the limit is enough: no length is announced, so the count is what decides.
    let over = 8 * 1024 * 1024 + 1;
    s.write_all(format!("{:x}\r\n", over * 2).as_bytes())
        .unwrap();
    s.write_all(&vec![b' '; over]).unwrap();
    assert_eq!(read_reply(&mut s).status, 413);
}

#[test]
fn body_at_the_limit_is_read() {
    let f = start();
    let mut body = initialize_body().to_string().into_bytes();
    body.resize(8 * 1024 * 1024, b' ');
    let head = format!(
        "POST /mcp HTTP/1.1\r\n{}\r\nAuthorization: Bearer {KEY}\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
        f.host_line(),
        body.len()
    );
    assert_eq!(f.raw(&head, &body).status, 200);
}

// ---- JSON-RPC over HTTP ----

#[test]
fn bad_json_400() {
    let f = start();
    let head = format!(
        "POST /mcp HTTP/1.1\r\n{}\r\nAuthorization: Bearer {KEY}\r\nConnection: close\r\nContent-Length: 5\r\n\r\n",
        f.host_line()
    );
    let r = f.raw(&head, b"{nope");
    assert_eq!(r.status, 400);
    assert_eq!(r.json()["error"]["code"], -32700);
}

#[test]
fn older_client_full_handshake() {
    let f = start();
    // 1. initialize, no version header yet.
    let r = f.post(&[], &initialize_body());
    assert_eq!(r.status, 200);
    assert!(
        r.head
            .to_lowercase()
            .contains("content-type: application/json"),
        "{}",
        r.head
    );
    assert!(
        !r.head.to_lowercase().contains("mcp-session-id"),
        "no sessions"
    );
    let init = r.json();
    assert_eq!(init["id"], 1);
    let version = init["result"]["protocolVersion"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(version, "2025-06-18");
    assert_eq!(init["result"]["instructions"], RULES);
    // 2. notifications/initialized: 202 and nothing else.
    let header = [("MCP-Protocol-Version", version.as_str())];
    let r = f.post(
        &header,
        &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    );
    assert_eq!(r.status, 202);
    assert!(r.body.is_empty());
    // 3. ping.
    let r = f.post(&header, &request("ping", json!({})));
    assert_eq!((r.status, r.json()["result"].clone()), (200, json!({})));
    // 4. tools/list.
    let r = f.post(&header, &request("tools/list", json!({})));
    assert_eq!(r.status, 200);
    assert!(
        r.json()["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "search_people")
    );
    // 5. tools/call, reading the seeded tree.
    with(&f.host, |s| {
        add_person(s, draft("Іван", "Коваль", None, None))
    });
    let r = f.post(
        &header,
        &request(
            "tools/call",
            json!({"name": "search_people", "arguments": {"query": "Коваль"}}),
        ),
    );
    assert_eq!(r.status, 200);
    let result = r.json()["result"].clone();
    assert_ne!(result["isError"], json!(true), "{result}");
    let page: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(page["items"][0]["name"], "Іван Коваль");
}

#[test]
fn older_client_without_version_header_counts_as_oldest() {
    let f = start();
    let r = f.post(&[], &request("tools/list", json!({})));
    assert_eq!(r.status, 200);
    assert!(r.json()["result"].get("resultType").is_none());
}

#[test]
fn unsupported_header_version_400() {
    let f = start();
    let r = f.post(
        &[("MCP-Protocol-Version", "1900-01-01")],
        &request("tools/list", json!({})),
    );
    assert_eq!(r.status, 400);
    assert_eq!(r.json()["error"]["code"], -32022);
}

#[test]
fn notification_with_unsupported_version_400() {
    let f = start();
    let r = f.post(
        &[("MCP-Protocol-Version", "1900-01-01")],
        &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    );
    assert_eq!(r.status, 400);
    assert!(r.json().get("id").is_none());
}

#[test]
fn modern_client_flow() {
    let f = start();
    let r = f.post_modern("server/discover", None, json!({}));
    assert_eq!(r.status, 200);
    assert_eq!(r.json()["result"]["instructions"], RULES);

    let r = f.post_modern("tools/list", None, json!({}));
    assert_eq!(r.status, 200);
    assert_eq!(r.json()["result"]["cacheScope"], "private");

    let r = f.post_modern(
        "tools/call",
        Some("get_rules"),
        json!({"name": "get_rules", "arguments": {}}),
    );
    assert_eq!(r.status, 200);
    let result = r.json()["result"].clone();
    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["content"][0]["text"], RULES);
}

#[test]
fn mirrored_headers_must_match_the_body() {
    let f = start();
    let call = modern_request("tools/call", json!({"name": "get_rules", "arguments": {}}));
    let check = |extra: &[(&str, &str)]| {
        let r = f.post(extra, &call);
        assert_eq!(r.status, 400, "{extra:?}");
        assert_eq!(r.json()["error"]["code"], -32020, "{extra:?}");
        assert_eq!(r.json()["id"], 1);
    };
    // Each required header missing.
    check(&[("Mcp-Method", "tools/call"), ("Mcp-Name", "get_rules")]);
    check(&[("MCP-Protocol-Version", MODERN), ("Mcp-Name", "get_rules")]);
    check(&[
        ("MCP-Protocol-Version", MODERN),
        ("Mcp-Method", "tools/call"),
    ]);
    // Each one disagreeing with the body.
    check(&[
        ("MCP-Protocol-Version", "2025-06-18"),
        ("Mcp-Method", "tools/call"),
        ("Mcp-Name", "get_rules"),
    ]);
    check(&[
        ("MCP-Protocol-Version", MODERN),
        ("Mcp-Method", "tools/list"),
        ("Mcp-Name", "get_rules"),
    ]);
    check(&[
        ("MCP-Protocol-Version", MODERN),
        ("Mcp-Method", "tools/call"),
        ("Mcp-Name", "tree_summary"),
    ]);
    // A broken encoding of the name.
    check(&[
        ("MCP-Protocol-Version", MODERN),
        ("Mcp-Method", "tools/call"),
        ("Mcp-Name", "=?base64?***?="),
    ]);
}

#[test]
fn mirrored_name_may_be_base64() {
    let f = start();
    // "get_rules" in the spec's sentinel form.
    let r = f.post_modern(
        "tools/call",
        Some("=?base64?Z2V0X3J1bGVz?="),
        json!({"name": "get_rules", "arguments": {}}),
    );
    assert_eq!(r.status, 200);
}

#[test]
fn header_names_are_case_insensitive() {
    let f = start();
    let r = f.post(
        &[
            ("mcp-protocol-version", MODERN),
            ("MCP-METHOD", "tools/list"),
        ],
        &modern_request("tools/list", json!({})),
    );
    assert_eq!(r.status, 200);
}

#[test]
fn modern_error_statuses() {
    let f = start();
    // Unknown method: 404 with a JSON-RPC body.
    let r = f.post_modern("no/such", None, json!({}));
    assert_eq!(r.status, 404);
    assert_eq!(r.json()["error"]["code"], -32601);
    // Missing required metadata: 400.
    let mut body = modern_request("tools/list", json!({}));
    body["params"]["_meta"]
        .as_object_mut()
        .unwrap()
        .remove("io.modelcontextprotocol/clientCapabilities");
    let r = f.post(
        &[
            ("MCP-Protocol-Version", MODERN),
            ("Mcp-Method", "tools/list"),
        ],
        &body,
    );
    assert_eq!(r.status, 400);
    assert_eq!(r.json()["error"]["code"], -32602);
    // Header says modern, body has no metadata at all.
    let r = f.post(
        &[
            ("MCP-Protocol-Version", MODERN),
            ("Mcp-Method", "tools/list"),
        ],
        &request("tools/list", json!({})),
    );
    assert_eq!(r.status, 400);
    assert_eq!(r.json()["error"]["code"], -32602);
    // An older client's unknown method stays a plain JSON-RPC error.
    let r = f.post(&[], &request("no/such", json!({})));
    assert_eq!(r.status, 200);
    assert_eq!(r.json()["error"]["code"], -32601);
}

#[test]
fn tool_errors_are_results_over_http() {
    let f = start();
    let r = f.post(
        &[],
        &request(
            "tools/call",
            json!({"name": "get_person", "arguments": {"id": "nope"}}),
        ),
    );
    assert_eq!(r.status, 200);
    assert_eq!(r.json()["result"]["isError"], true);
}

#[test]
fn session_header_is_ignored() {
    let f = start();
    let r = f.post(&[("Mcp-Session-Id", "whatever")], &initialize_body());
    assert_eq!(r.status, 200);
    assert!(!r.head.to_lowercase().contains("mcp-session-id"));
}

#[test]
fn keep_alive_connection_serves_several_requests() {
    let f = start();
    let mut s = f.connect();
    for _ in 0..3 {
        let body = request("ping", json!({})).to_string();
        let head = format!(
            "POST /mcp HTTP/1.1\r\n{}\r\nAuthorization: Bearer {KEY}\r\nContent-Length: {}\r\n\r\n",
            f.host_line(),
            body.len()
        );
        s.write_all(head.as_bytes()).unwrap();
        s.write_all(body.as_bytes()).unwrap();
        assert_eq!(read_reply(&mut s).status, 200);
    }
}

/// The accept thread ends just after the server is dropped, so give it a moment.
fn assert_closes(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while TcpStream::connect(("127.0.0.1", port)).is_ok() {
        assert!(
            Instant::now() < deadline,
            "port {port} still takes connections"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn stop_closes_the_listener() {
    let f = start();
    let port = f.port();
    f.server.stop();
    assert_closes(port);
}

#[test]
fn drop_closes_the_listener() {
    let host = Arc::new(empty_host());
    let server = serve(Arc::clone(&host), 0).unwrap();
    let port = server.port();
    drop(server);
    assert_closes(port);
}

/// Requests refused unread whose senders then sit on the open connection, with or without a key.
fn silent_clients(f: &Fixture, count: usize) -> Vec<TcpStream> {
    (0..count)
        .map(|_| {
            let mut s = f.connect();
            let head = f.head_only(&[f.host_line()], UNREAD);
            s.write_all(head.as_bytes()).unwrap();
            assert_eq!(read_reply(&mut s).status, 401);
            s
        })
        .collect()
}

#[test]
fn silent_clients_do_not_pin_the_workers() {
    let f = start();
    let _held = silent_clients(&f, 16);
    assert_eq!(f.post(&[], &initialize_body()).status, 200);
}

#[test]
fn silent_clients_do_not_hold_up_stopping() {
    let f = start();
    let _held = silent_clients(&f, 16);
    let started = Instant::now();
    let port = f.port();
    f.server.stop();
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    assert_closes(port);
}

#[test]
fn wild_content_length_does_not_bring_the_app_down() {
    let f = start();
    let auth = format!("Authorization: Bearer {KEY}");
    for (length, lines, status) in [
        (1_000_000_000_000_000_usize, vec![f.host_line()], 401),
        (
            1_000_000_000_000_000,
            vec![f.host_line(), auth.clone()],
            413,
        ),
        (usize::MAX, vec![f.host_line(), auth.clone()], 413),
        (usize::MAX, vec!["Host: evil.example".to_string()], 403),
    ] {
        let head = f.head_only(&lines, length);
        assert_eq!(f.raw(&head, b"").status, status, "{length}");
    }
    // Still alive and answering.
    assert_eq!(f.post(&[], &initialize_body()).status, 200);
}

#[test]
fn busy_port_is_an_error() {
    let f = start();
    let host = Arc::new(empty_host());
    assert!(serve(host, f.port()).is_err());
}
