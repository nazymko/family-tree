#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The real listener on an ephemeral port, spoken to over a plain socket so every header is under
//! the test's control. The security checks must answer before any body is read: those tests send
//! the headers only and expect a reply anyway.

mod common;

use common::*;
use gt_mcp::{AgentCtx, Host, RULES, ServerHandle, ServerLimits, serve, serve_with};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// A body length announced and never sent: a reply proves no byte of it was waited for.
const UNREAD: usize = 100;

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

struct Fixture<H = TestHost> {
    host: Arc<H>,
    server: ServerHandle,
}

fn start() -> Fixture {
    let host = Arc::new(empty_host());
    let server = serve(Arc::clone(&host), 0).unwrap();
    Fixture { host, server }
}

fn start_with(max_connections: usize, timeout: Duration) -> Fixture {
    let host = Arc::new(empty_host());
    let limits = ServerLimits {
        max_connections,
        timeout,
    };
    let server = serve_with(Arc::clone(&host), 0, limits).unwrap();
    Fixture { host, server }
}

impl<H> Fixture<H> {
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
        self.head_with_length(lines, &length.to_string())
    }

    /// A POST head with this exact `Content-Length` text.
    fn head_with_length(&self, lines: &[String], length: &str) -> String {
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

    fn key_line(&self) -> String {
        format!("Authorization: Bearer {KEY}")
    }

    /// A ping that may meet a full server: the status if one came back at all.
    fn try_ping(&self) -> Option<u16> {
        let body = request("ping", json!({})).to_string();
        let head = self.head_only(&[self.host_line(), self.key_line()], body.len());
        let mut s = self.connect();
        s.write_all(head.as_bytes()).ok()?;
        s.write_all(body.as_bytes()).ok()?;
        let mut all = Vec::new();
        let _ = s.read_to_end(&mut all);
        let text = String::from_utf8_lossy(&all);
        text.split_whitespace().nth(1)?.parse().ok()
    }

    /// Waits for a slot to come free: pings until one is answered.
    fn assert_recovers(&self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.try_ping() != Some(200) {
            assert!(Instant::now() < deadline, "no slot came free");
            std::thread::sleep(Duration::from_millis(20));
        }
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
fn transfer_encoding_is_refused_unread() {
    let f = start();
    for (coding, status) in [
        ("chunked", 411),
        ("CHUNKED", 411),
        ("gzip, chunked", 501),
        ("gzip", 501),
        ("identity", 501),
    ] {
        let head = format!(
            "POST /mcp HTTP/1.1\r\n{}\r\n{}\r\nTransfer-Encoding: {coding}\r\n\r\n",
            f.host_line(),
            f.key_line()
        );
        assert_eq!(f.raw(&head, b"").status, status, "{coding}");
    }
    // A stranger learns nothing about it.
    let head = format!(
        "POST /mcp HTTP/1.1\r\n{}\r\nTransfer-Encoding: chunked\r\n\r\n",
        f.host_line()
    );
    assert_eq!(f.raw(&head, b"").status, 401);
}

#[test]
fn missing_content_length_411() {
    let f = start();
    let head = format!(
        "POST /mcp HTTP/1.1\r\n{}\r\n{}\r\n\r\n",
        f.host_line(),
        f.key_line()
    );
    assert_eq!(f.raw(&head, b"").status, 411);
}

#[test]
fn bad_content_length_400() {
    let f = start();
    for length in [
        "abc",
        "",
        "+5",
        "-1",
        "5, 5",
        "0x10",
        "1e3",
        "18446744073709551616",
        "99999999999999999999999999999999",
    ] {
        let head = f.head_with_length(&[f.host_line(), f.key_line()], length);
        assert_eq!(f.raw(&head, b"").status, 400, "{length:?}");
    }
    for (first, second) in [("5", "5"), ("5", "6")] {
        let head = f.head_with_length(
            &[
                f.host_line(),
                f.key_line(),
                format!("Content-Length: {first}"),
            ],
            second,
        );
        assert_eq!(f.raw(&head, b"").status, 400, "{first} and {second}");
    }
}

#[test]
fn oversized_head_431() {
    let f = start();
    let pad = format!("X-Pad: {}", "a".repeat(20 * 1024));
    let head = f.head_only(&[f.host_line(), f.key_line(), pad], UNREAD);
    assert_eq!(f.raw(&head, b"").status, 431);
    // Too many headers, each of them small.
    let many: Vec<String> = (0..200).map(|i| format!("X-{i}: 1")).collect();
    let head = f.head_only(&many, UNREAD);
    assert_eq!(f.raw(&head, b"").status, 431);
    // A large head within the limit is fine.
    let pad = format!("X-Pad: {}", "a".repeat(12 * 1024));
    let body = request("ping", json!({})).to_string();
    let head = f.head_only(&[f.host_line(), f.key_line(), pad], body.len());
    assert_eq!(f.raw(&head, body.as_bytes()).status, 200);
}

#[test]
fn malformed_head_400() {
    let f = start();
    for head in [
        "garbage\r\n\r\n",
        "POST /mcp HTTP/9.9\r\n\r\n",
        "\u{1}\u{2}\r\n\r\n",
    ] {
        assert_eq!(f.raw(head, b"").status, 400, "{head:?}");
    }
}

#[test]
fn nothing_of_the_body_is_waited_for_before_the_key() {
    // The body never comes and the socket timeout is far off: only an answer from the head can
    // return in time.
    let f = start_with(8, Duration::from_secs(60));
    for length in [1, UNREAD, 1024, 1025, 1024 * 1024] {
        let head = f.head_only(
            &[f.host_line(), "Authorization: Bearer wrong".to_string()],
            length,
        );
        let started = Instant::now();
        assert_eq!(f.raw(&head, b"").status, 401, "{length}");
        assert!(started.elapsed() < Duration::from_secs(5), "{length}");
    }
}

#[test]
fn expect_continue_is_answered_after_the_checks() {
    let f = start();
    let body = request("ping", json!({})).to_string();
    let lines = [
        f.host_line(),
        f.key_line(),
        "Expect: 100-continue".to_string(),
    ];
    let mut s = f.connect();
    s.write_all(f.head_only(&lines, body.len()).as_bytes())
        .unwrap();
    let mut seen = Vec::new();
    let mut buf = [0u8; 256];
    while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = s.read(&mut buf).unwrap();
        assert_ne!(n, 0);
        seen.extend_from_slice(&buf[..n]);
    }
    assert!(seen.starts_with(b"HTTP/1.1 100"), "{seen:?}");
    s.write_all(body.as_bytes()).unwrap();
    assert_eq!(read_reply(&mut s).status, 200);
    // Without a key, no invitation to send anything.
    let lines = [f.host_line(), "Expect: 100-continue".to_string()];
    let r = f.raw(&f.head_only(&lines, UNREAD), b"");
    assert_eq!(r.status, 401);
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
    assert_eq!(init["result"]["instructions"], gt_mcp::INSTRUCTIONS);
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
    assert_eq!(r.json()["result"]["instructions"], gt_mcp::INSTRUCTIONS);

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
fn one_request_per_connection() {
    let f = start();
    let body = request("ping", json!({})).to_string();
    let head = format!(
        "POST /mcp HTTP/1.1\r\n{}\r\n{}\r\nContent-Length: {}\r\n\r\n",
        f.host_line(),
        f.key_line(),
        body.len()
    );
    let mut s = f.connect();
    s.write_all(head.as_bytes()).unwrap();
    s.write_all(body.as_bytes()).unwrap();
    let r = read_reply(&mut s);
    assert_eq!(r.status, 200);
    assert!(
        r.head.to_lowercase().contains("connection: close"),
        "{}",
        r.head
    );
    // The server closes, whatever the client asked for.
    let mut rest = Vec::new();
    s.read_to_end(&mut rest).unwrap();
    assert!(rest.is_empty());
}

/// A client that sends its whole body before reading still gets the refusal, not a reset.
#[test]
fn refusal_survives_a_body_nobody_read() {
    let f = start();
    let body = vec![b'x'; 100_000];
    let head = f.head_with_length(&[f.host_line()], &body.len().to_string());
    for _ in 0..20 {
        let mut s = f.connect();
        s.write_all(head.as_bytes()).unwrap();
        s.write_all(&body).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(read_reply(&mut s).status, 401);
    }
}

#[test]
fn every_answer_says_close() {
    let f = start();
    let head = f.head_only(&[f.host_line()], UNREAD);
    let r = f.raw(&head, b"");
    assert!(
        r.head.to_lowercase().contains("connection: close"),
        "{}",
        r.head
    );
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
    let key = f.key_line();
    let u64_max = u64::MAX.to_string();
    let beyond = "99999999999999999999999999999999";
    for (length, lines, status) in [
        ("1000000000000000", vec![f.host_line()], 401),
        (u64_max.as_str(), vec![f.host_line()], 401),
        (beyond, vec![f.host_line()], 401),
        ("1000000000000000", vec![f.host_line(), key.clone()], 413),
        (u64_max.as_str(), vec![f.host_line(), key.clone()], 413),
        (beyond, vec![f.host_line(), key.clone()], 400),
        (
            u64_max.as_str(),
            vec!["Host: evil.example".to_string()],
            403,
        ),
    ] {
        let head = f.head_with_length(&lines, length);
        assert_eq!(f.raw(&head, b"").status, status, "{length}");
        // Still alive and answering.
        assert_eq!(f.post(&[], &initialize_body()).status, 200);
    }
}

#[test]
fn wild_requests_leave_nothing_behind() {
    // More requests than there are slots: a leak of even one per request would fill the server.
    let f = start_with(4, Duration::from_secs(30));
    let head = f.head_with_length(&[f.host_line()], &u64::MAX.to_string());
    for _ in 0..40 {
        f.assert_recovers();
        let mut s = f.connect();
        let _ = s.write_all(head.as_bytes());
        let mut all = Vec::new();
        let _ = s.read_to_end(&mut all);
    }
    f.assert_recovers();
}

#[test]
fn slow_client_times_out_and_frees_its_slot() {
    let timeout = Duration::from_millis(300);
    // Nothing sent, half a head, and half a body. Only one slot, so each must give it back.
    let f = start_with(1, timeout);
    let body_head = f.head_only(&[f.host_line(), f.key_line()], 100);
    for sent in ["", "POST /mcp HTTP/1.1\r\nHo", body_head.as_str()] {
        let mut s = f.connect();
        s.write_all(sent.as_bytes()).unwrap();
        let started = Instant::now();
        assert_eq!(read_reply(&mut s).status, 408, "{sent:?}");
        assert!(started.elapsed() < Duration::from_secs(5), "{sent:?}");
        f.assert_recovers();
    }
}

#[test]
fn dripping_client_cannot_outlast_the_timeout() {
    // Each byte would reset a per-read timeout; the limit is on the whole request.
    let f = start_with(1, Duration::from_millis(500));
    let mut s = f.connect();
    let mut writer = s.try_clone().unwrap();
    let drip = std::thread::spawn(move || {
        for _ in 0..200 {
            if writer.write_all(b"X").is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
    let started = Instant::now();
    let mut all = Vec::new();
    let _ = s.read_to_end(&mut all);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    f.assert_recovers();
    drop(s);
    drip.join().unwrap();
}

#[test]
fn connection_cap_503_and_recovery() {
    let f = start_with(2, Duration::from_secs(60));
    let held = [f.connect(), f.connect()];
    let mut over = f.connect();
    let r = read_reply(&mut over);
    assert_eq!(r.status, 503);
    assert!(
        r.head.to_lowercase().contains("connection: close"),
        "{}",
        r.head
    );
    // Still full while they stay.
    let mut again = f.connect();
    assert_eq!(read_reply(&mut again).status, 503);
    drop(held);
    f.assert_recovers();
}

#[test]
fn stop_with_a_connection_left_open_returns_promptly() {
    let f = start_with(8, Duration::from_secs(60));
    let mut half = f.connect();
    half.write_all(b"POST /mcp HTTP/1.1\r\nHo").unwrap();
    let _idle = f.connect();
    std::thread::sleep(Duration::from_millis(100));
    let port = f.port();
    let started = Instant::now();
    f.server.stop();
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
    assert_closes(port);
}

#[test]
fn drop_with_a_connection_left_open_returns_promptly() {
    let f = start_with(8, Duration::from_secs(60));
    let _idle = f.connect();
    std::thread::sleep(Duration::from_millis(100));
    let started = Instant::now();
    drop(f.server);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn no_request_runs_after_stop() {
    let f = start_with(8, Duration::from_secs(60));
    let mut open = f.connect();
    std::thread::sleep(Duration::from_millis(100));
    let body = request("ping", json!({})).to_string();
    let head = f.head_only(&[f.host_line(), f.key_line()], body.len());
    f.server.stop();
    let _ = open.write_all(head.as_bytes());
    let _ = open.write_all(body.as_bytes());
    let mut all = Vec::new();
    let _ = open.read_to_end(&mut all);
    let text = String::from_utf8_lossy(&all);
    assert!(!text.contains(" 200 "), "{text}");
}

#[test]
fn client_cannot_claim_other_name() {
    let f = start();
    let forged = json!({"id": uuid::Uuid::from_u128(99), "name": "The owner", "kind": "user"});
    let meta = json!({
        "io.modelcontextprotocol/clientInfo": {"name": "The owner", "version": "1"},
        "name": "The owner",
        "actor": forged,
        "agent": forged,
    });
    let init = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "The owner", "version": "1"},
        },
    });
    assert_eq!(f.post(&[], &init).status, 200);
    let call = |name: &str, mut arguments: Value| -> Value {
        arguments["name"] = json!("The owner");
        arguments["actor"] = forged.clone();
        arguments["agent"] = forged.clone();
        arguments["agent_name"] = json!("The owner");
        arguments["actor_id"] = forged["id"].clone();
        arguments["_meta"] = meta.clone();
        let body = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments, "_meta": meta},
        });
        let reply = f.post(&[], &body).json();
        assert_ne!(reply["result"]["isError"], json!(true), "{reply}");
        serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    };
    let batch = call("begin_batch", json!({"title": "Recording 2"}))["batch"].clone();
    call(
        "add_person",
        json!({"batch": batch, "given": "Ivan", "surnames": ["Koval"]}),
    );

    let log = call("get_log", json!({"filters": {"batch": batch}}));
    let row = &log["items"][0];
    assert_eq!(row["actor"]["name"], "Transcript reader", "{row}");
    assert_eq!(row["actor"]["id"], json!(ctx().id), "{row}");
    assert!(!log.to_string().contains("The owner"), "{log}");
}

#[test]
fn busy_port_is_an_error() {
    let f = start();
    let host = Arc::new(empty_host());
    assert!(serve(host, f.port()).is_err());
}

// ---- a key can stop working while its request is on the way ----

#[test]
fn a_key_revoked_after_the_head_gets_no_data() {
    let f = start();
    let body = request(
        "tools/call",
        json!({"name": "tree_summary", "arguments": {}}),
    )
    .to_string();
    let head = f.head_only(&[f.host_line(), f.key_line()], body.len());
    let mut s = f.connect();
    s.write_all(head.as_bytes()).unwrap();
    // Long enough for the head to pass its checks.
    std::thread::sleep(Duration::from_millis(200));
    f.host.keys.lock().unwrap().clear();
    s.write_all(body.as_bytes()).unwrap();
    let r = read_reply(&mut s);
    assert_eq!(r.status, 401);
    assert!(
        r.head.to_lowercase().contains("www-authenticate: bearer"),
        "{}",
        r.head
    );
}

// ---- a silent socket cannot sit on a slot ----

#[test]
fn a_silent_socket_loses_its_slot_after_about_five_seconds() {
    let f = start_with(1, Duration::from_secs(60));
    let started = Instant::now();
    let mut silent = f.connect();
    let r = read_reply(&mut silent);
    let waited = started.elapsed();
    assert_eq!(r.status, 408);
    assert!(waited >= Duration::from_secs(4), "{waited:?}");
    assert!(waited < Duration::from_secs(8), "{waited:?}");
    f.assert_recovers();
}

// ---- only a few tool calls at once ----

/// Counts how many sessions are in use at once, each held for `hold`.
struct SlowHost {
    inner: TestHost,
    hold: Duration,
    running: AtomicUsize,
    peak: AtomicUsize,
}

impl Host for SlowHost {
    fn with_session<T>(
        &self,
        f: &mut dyn FnMut(&mut gt_session::Session) -> Result<T, gt_session::ApiError>,
    ) -> Result<T, gt_session::ApiError> {
        let now = self.running.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(now, Ordering::SeqCst);
        std::thread::sleep(self.hold);
        let out = self.inner.with_session(f);
        self.running.fetch_sub(1, Ordering::SeqCst);
        out
    }

    fn authenticate(&self, key_sha256: &[u8; 32]) -> Option<AgentCtx> {
        self.inner.authenticate(key_sha256)
    }

    fn authenticate_keyless(&self) -> Option<AgentCtx> {
        self.inner.authenticate_keyless()
    }

    fn changed(&self, touched: &[uuid::Uuid], agent: &AgentCtx) {
        self.inner.changed(touched, agent);
    }
}

fn start_slow(hold: Duration) -> Fixture<SlowHost> {
    let host = Arc::new(SlowHost {
        inner: empty_host(),
        hold,
        running: AtomicUsize::new(0),
        peak: AtomicUsize::new(0),
    });
    let server = serve(Arc::clone(&host), 0).unwrap();
    Fixture { host, server }
}

fn call_at_once(f: &Fixture<SlowHost>, calls: usize) -> Vec<Value> {
    let port = f.port();
    let threads: Vec<_> = (0..calls)
        .map(|_| {
            std::thread::spawn(move || {
                let body =
                    request("tools/call", json!({"name": "tree_summary", "arguments": {}}))
                        .to_string();
                let head = format!(
                    "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {KEY}\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
                s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
                s.write_all(head.as_bytes()).unwrap();
                read_reply(&mut s).json()
            })
        })
        .collect();
    threads.into_iter().map(|t| t.join().unwrap()).collect()
}

#[test]
fn at_most_two_tool_calls_run_at_once_and_the_rest_wait_their_turn() {
    let f = start_slow(Duration::from_millis(200));
    let replies = call_at_once(&f, 6);
    for reply in &replies {
        assert_ne!(reply["result"]["isError"], json!(true), "{reply}");
    }
    assert_eq!(f.host.peak.load(Ordering::SeqCst), 2);
}

#[test]
fn a_tool_call_that_waits_too_long_gets_a_clear_busy_error() {
    let f = start_slow(Duration::from_secs(4));
    let port_check = {
        // Other requests do not wait for the tools.
        let f2 = &f;
        std::thread::scope(|scope| {
            let calls = scope.spawn(|| call_at_once(f2, 5));
            std::thread::sleep(Duration::from_millis(500));
            let started = Instant::now();
            let ping = f2.post(&[], &request("ping", json!({})));
            assert_eq!(ping.status, 200);
            let quick = started.elapsed();
            (calls.join().unwrap(), quick)
        })
    };
    let (replies, quick) = port_check;
    assert!(quick < Duration::from_secs(1), "{quick:?}");
    let busy: Vec<&Value> = replies
        .iter()
        .filter(|r| r["result"]["isError"] == json!(true))
        .collect();
    assert_eq!(busy.len(), 3, "{replies:?}");
    for reply in busy {
        let text = reply["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("busy"), "{text}");
    }
    assert_eq!(f.host.peak.load(Ordering::SeqCst), 2);
}

// ---- headers ----

#[test]
fn key_is_trimmed_of_ascii_space_only() {
    let f = start();
    let body = request("ping", json!({})).to_string();
    let good = f.head_only(
        &[f.host_line(), format!("Authorization: Bearer   {KEY}  ")],
        body.len(),
    );
    assert_eq!(f.raw(&good, body.as_bytes()).status, 200);
    // A no-break space is not ASCII: it stays in the key, so the key is wrong.
    let bad = f.head_only(
        &[f.host_line(), format!("Authorization: Bearer {KEY}\u{a0}")],
        body.len(),
    );
    assert_eq!(f.raw(&bad, body.as_bytes()).status, 401);
    let host = format!("Host: {}\u{a0}", f.good_host());
    let bad = f.head_only(&[host, f.key_line()], body.len());
    assert_eq!(f.raw(&bad, body.as_bytes()).status, 403);
}

#[test]
fn differing_protocol_version_headers_get_400() {
    let f = start();
    let ping = request("ping", json!({}));
    let r = f.post(
        &[
            ("MCP-Protocol-Version", "2025-06-18"),
            ("mcp-protocol-version", "2025-11-25"),
        ],
        &ping,
    );
    assert_eq!(r.status, 400);
    let same = f.post(
        &[
            ("MCP-Protocol-Version", "2025-06-18"),
            ("MCP-Protocol-Version", "2025-06-18"),
        ],
        &ping,
    );
    assert_eq!(same.status, 200);
}

#[test]
fn initialize_with_an_invalid_version_header_gets_400() {
    let f = start();
    let r = f.post(
        &[("MCP-Protocol-Version", "not-a-version")],
        &initialize_body(),
    );
    assert_eq!(r.status, 400);
    assert_eq!(r.json()["id"], 1);
    let fine = f.post(
        &[("MCP-Protocol-Version", "2025-06-18")],
        &initialize_body(),
    );
    assert_eq!(fine.status, 200);
    let none = f.post(&[], &initialize_body());
    assert_eq!(none.status, 200);
}

/// The key was good when the head came in, and revoked before the body did.
#[test]
fn a_key_revoked_while_the_body_is_on_its_way_gets_401() {
    let f = start();
    let body = request("ping", json!({})).to_string();
    let head = f.head_only(&[f.host_line(), f.key_line()], body.len());
    let mut open = f.connect();
    open.write_all(head.as_bytes()).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    f.host.keys.lock().unwrap().clear();
    open.write_all(body.as_bytes()).unwrap();
    assert_eq!(read_reply(&mut open).status, 401);
}

/// A client still connected to a stopped server must not hold the port against the next one.
#[test]
fn the_port_can_be_taken_again_at_once_with_clients_still_connected() {
    let f = start_with(8, Duration::from_secs(60));
    let port = f.port();
    let idle = f.connect();
    let mut half = f.connect();
    let head = f.head_only(&[f.host_line(), f.key_line()], UNREAD);
    half.write_all(head.as_bytes()).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    f.server.stop();
    let next = serve(Arc::clone(&f.host), port).unwrap();
    assert_eq!(next.port(), port);
    drop((idle, half));
}

// ---- no key required: the owner chose to let any program on this computer in ----

fn start_keyless() -> Fixture {
    let f = start();
    *f.host.keyless.lock().unwrap() = Some(gt_mcp::keyless_agent());
    f
}

impl<H> Fixture<H> {
    /// A POST to `/mcp` with the good host, no `Authorization` header, and the extra lines given.
    fn post_bare(&self, extra: &[(&str, &str)], body: &Value) -> Reply {
        let text = body.to_string();
        let mut head = format!(
            "POST /mcp HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n",
            self.good_host(),
            text.len()
        );
        for (name, value) in extra {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str("\r\n");
        self.raw(&head, text.as_bytes())
    }
}

fn call_body(name: &str, arguments: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {"name": name, "arguments": arguments},
    })
}

/// A tool call as the keyless agent (`keyed` false) or as the keyed one; the tool's own JSON.
fn tool_call(f: &Fixture, keyed: bool, name: &str, arguments: Value) -> Value {
    let body = call_body(name, arguments);
    let reply = if keyed {
        f.post(&[], &body)
    } else {
        f.post_bare(&[], &body)
    };
    assert_eq!(reply.status, 200);
    let reply = reply.json();
    assert_ne!(reply["result"]["isError"], json!(true), "{reply}");
    serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
}

fn tool_refusal(f: &Fixture, name: &str, arguments: Value) -> String {
    let reply = f.post_bare(&[], &call_body(name, arguments));
    assert_eq!(reply.status, 200);
    let reply = reply.json();
    assert_eq!(reply["result"]["isError"], json!(true), "{reply}");
    reply["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn keyless_default_host_still_needs_a_key() {
    // A host that does not opt in refuses a request with no key.
    let f = start();
    assert_eq!(f.post_bare(&[], &initialize_body()).status, 401);
}

#[test]
fn keyless_no_header_runs_as_the_builtin_agent() {
    let f = start_keyless();
    assert_eq!(f.post_bare(&[], &initialize_body()).status, 200);
    let batch =
        tool_call(&f, false, "begin_batch", json!({"title": "Recording 3"}))["batch"].clone();
    tool_call(
        &f,
        false,
        "add_person",
        json!({"batch": batch, "given": "Ivan", "surnames": ["Koval"]}),
    );
    let log = tool_call(&f, false, "get_log", json!({"filters": {"batch": batch}}));
    let row = &log["items"][0];
    assert_eq!(row["actor"]["id"], json!(gt_mcp::KEYLESS_AGENT_ID), "{row}");
    assert_eq!(row["actor"]["name"], gt_mcp::KEYLESS_AGENT_NAME, "{row}");
    assert_eq!(row["actor"]["kind"], "agent", "{row}");
    // The change is told to the app under that identity, so it is badged and reviewable.
    let changes = f.host.changes.lock().unwrap();
    assert!(!changes.is_empty());
    assert!(
        changes
            .iter()
            .all(|(_, a)| a.id == gt_mcp::KEYLESS_AGENT_ID)
    );
}

#[test]
fn keyless_agent_id_is_fixed() {
    assert_eq!(
        gt_mcp::KEYLESS_AGENT_ID.to_string(),
        "01927a00-0000-7000-8000-6e6f6b657900"
    );
}

#[test]
fn keyless_a_valid_key_runs_as_its_connection() {
    let f = start_keyless();
    let batch =
        tool_call(&f, true, "begin_batch", json!({"title": "Recording 4"}))["batch"].clone();
    tool_call(
        &f,
        true,
        "add_person",
        json!({"batch": batch, "given": "Olena", "surnames": ["Koval"]}),
    );
    let log = tool_call(&f, true, "get_log", json!({"filters": {"batch": batch}}));
    let row = &log["items"][0];
    assert_eq!(row["actor"]["name"], "Transcript reader", "{row}");
    assert_eq!(row["actor"]["id"], json!(ctx().id), "{row}");
}

#[test]
fn keyless_a_bad_key_is_401_and_never_the_builtin_agent() {
    let f = start_keyless();
    for auth in [
        "Authorization: Bearer not-the-key".to_string(),
        format!("Authorization: Basic {KEY}"),
        format!("Authorization: {KEY}"),
        "Authorization: Bearer ".to_string(),
        "Authorization:".to_string(),
    ] {
        let head = f.head_only(&[f.host_line(), auth.clone()], UNREAD);
        let r = f.raw(&head, b"");
        assert_eq!(r.status, 401, "{auth}");
        assert!(
            r.head.to_lowercase().contains("www-authenticate: bearer"),
            "{}",
            r.head
        );
    }
    let two = f.head_only(
        &[
            f.host_line(),
            format!("Authorization: Bearer {KEY}"),
            "Authorization: Bearer other".to_string(),
        ],
        UNREAD,
    );
    assert_eq!(f.raw(&two, b"").status, 401);
}

#[test]
fn keyless_a_revoked_key_is_401() {
    let f = start_keyless();
    assert_eq!(f.post(&[], &initialize_body()).status, 200);
    f.host.keys.lock().unwrap().clear();
    assert_eq!(f.post(&[], &initialize_body()).status, 401);
    // A client with no key at all is let in; one that still sends its old key is told.
    assert_eq!(f.post_bare(&[], &initialize_body()).status, 200);
}

#[test]
fn requiring_a_key_takes_effect_for_the_next_request_and_back() {
    let f = start_keyless();
    assert_eq!(f.post_bare(&[], &initialize_body()).status, 200);
    *f.host.keyless.lock().unwrap() = None;
    assert_eq!(f.post_bare(&[], &initialize_body()).status, 401);
    assert_eq!(f.post(&[], &initialize_body()).status, 200);
    *f.host.keyless.lock().unwrap() = Some(gt_mcp::keyless_agent());
    assert_eq!(f.post_bare(&[], &initialize_body()).status, 200);
}

#[test]
fn requiring_a_key_stops_a_request_already_past_its_head() {
    let f = start_keyless();
    let body = request(
        "tools/call",
        json!({"name": "tree_summary", "arguments": {}}),
    )
    .to_string();
    let head = f.head_only(&[f.host_line()], body.len());
    let mut s = f.connect();
    s.write_all(head.as_bytes()).unwrap();
    // Long enough for the head to pass its checks as the keyless agent.
    std::thread::sleep(Duration::from_millis(200));
    *f.host.keyless.lock().unwrap() = None;
    s.write_all(body.as_bytes()).unwrap();
    let r = read_reply(&mut s);
    assert_eq!(r.status, 401);
    assert!(!String::from_utf8_lossy(&r.body).contains("result"));
}

#[test]
fn keyless_a_web_page_is_refused() {
    let f = start_keyless();
    for origin in [
        "https://evil.example".to_string(),
        "null".to_string(),
        format!("http://evil.example:{}", f.port()),
        "http://127.0.0.1:1".to_string(),
        "http://localhost:3000".to_string(),
    ] {
        let head = f.head_only(&[f.host_line(), format!("Origin: {origin}")], UNREAD);
        assert_eq!(f.raw(&head, b"").status, 403, "{origin}");
        let with_body = f.post_bare(&[("Origin", origin.as_str())], &initialize_body());
        assert_eq!(with_body.status, 403, "{origin}");
    }
    assert!(f.host.changes.lock().unwrap().is_empty());
}

#[test]
fn keyless_a_foreign_host_is_refused() {
    let f = start_keyless();
    for host in [
        format!("evil.example:{}", f.port()),
        "evil.example".to_string(),
        "127.0.0.1".to_string(),
        "127.0.0.1:1".to_string(),
        format!("127.0.0.1.evil.example:{}", f.port()),
    ] {
        let head = f.head_only(&[format!("Host: {host}")], UNREAD);
        assert_eq!(f.raw(&head, b"").status, 403, "{host}");
    }
    let none = f.head_only(&[], UNREAD);
    assert_eq!(f.raw(&none, b"").status, 403);
    let two = f.head_only(&[f.host_line(), f.host_line()], UNREAD);
    assert_eq!(f.raw(&two, b"").status, 403);
}

#[test]
fn keyless_loopback_only_one_path_and_method() {
    let f = start_keyless();
    assert!(f.server.local_addr().ip().is_loopback());
    let get = format!(
        "GET /mcp HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        f.good_host()
    );
    assert_eq!(f.raw(&get, b"").status, 405);
    let other = format!(
        "POST /other HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Length: 2\r\n\r\n{{}}",
        f.good_host()
    );
    assert_eq!(f.raw(&other, b"").status, 404);
}

#[test]
fn keyless_body_limits() {
    let f = start_keyless();
    let big = f.head_only(&[f.host_line()], 8 * 1024 * 1024 + 1);
    assert_eq!(f.raw(&big, b"").status, 413);
    let none = format!(
        "POST /mcp HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        f.good_host()
    );
    assert_eq!(f.raw(&none, b"").status, 411);
    let chunked = f.head_only(
        &[f.host_line(), "Transfer-Encoding: chunked".into()],
        UNREAD,
    );
    assert_eq!(f.raw(&chunked, b"").status, 411);
    let bad = f.head_with_length(&[f.host_line()], "12abc");
    assert_eq!(f.raw(&bad, b"").status, 400);
    f.assert_recovers();
}

#[test]
fn keyless_at_most_two_tool_calls_run_at_once() {
    let f = start_slow(Duration::from_millis(200));
    *f.host.inner.keyless.lock().unwrap() = Some(gt_mcp::keyless_agent());
    let port = f.port();
    let threads: Vec<_> = (0..6)
        .map(|_| {
            std::thread::spawn(move || {
                let body =
                    request("tools/call", json!({"name": "tree_summary", "arguments": {}}))
                        .to_string();
                let head = format!(
                    "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
                s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
                s.write_all(head.as_bytes()).unwrap();
                read_reply(&mut s).json()
            })
        })
        .collect();
    for t in threads {
        let reply = t.join().unwrap();
        assert_ne!(reply["result"]["isError"], json!(true), "{reply}");
    }
    assert_eq!(f.host.peak.load(Ordering::SeqCst), 2);
}

#[test]
fn keyless_client_cannot_claim_other_name() {
    let f = start_keyless();
    let forged = json!({"id": uuid::Uuid::from_u128(99), "name": "The owner", "kind": "user"});
    let meta = json!({
        "io.modelcontextprotocol/clientInfo": {"name": "The owner", "version": "1"},
        "name": "The owner",
        "actor": forged,
        "agent": forged,
    });
    let init = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "The owner", "version": "1"},
        },
    });
    assert_eq!(f.post_bare(&[], &init).status, 200);
    let call = |name: &str, mut arguments: Value| -> Value {
        arguments["name"] = json!("The owner");
        arguments["actor"] = forged.clone();
        arguments["agent"] = forged.clone();
        arguments["agent_name"] = json!("The owner");
        arguments["actor_id"] = forged["id"].clone();
        arguments["_meta"] = meta.clone();
        let body = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments, "_meta": meta},
        });
        let reply = f.post_bare(&[], &body).json();
        assert_ne!(reply["result"]["isError"], json!(true), "{reply}");
        serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    };
    let batch = call("begin_batch", json!({"title": "Recording 5"}))["batch"].clone();
    call(
        "add_person",
        json!({"batch": batch, "given": "Ivan", "surnames": ["Koval"]}),
    );
    let log = call("get_log", json!({"filters": {"batch": batch}}));
    let row = &log["items"][0];
    assert_eq!(row["actor"]["name"], gt_mcp::KEYLESS_AGENT_NAME, "{row}");
    assert_eq!(row["actor"]["id"], json!(gt_mcp::KEYLESS_AGENT_ID), "{row}");
    assert!(!log.to_string().contains("The owner"), "{log}");
}

#[test]
fn keyless_gate_refusals_and_living_status_hold() {
    let f = start_keyless();
    let id = with(&f.host, |s| {
        add_person(s, draft("Ivan", "Koval", None, None))
    });
    let batch =
        tool_call(&f, false, "begin_batch", json!({"title": "Recording 6"}))["batch"].clone();
    let living =
        |f: &Fixture| tool_call(f, false, "get_person", json!({"id": id}))["living"].clone();
    let was = living(&f);
    for (kind, date) in [("death", "1950"), ("burial", "1950"), ("birth", "1801")] {
        let message = tool_refusal(
            &f,
            "add_event",
            json!({"batch": batch, "person": id, "kind": kind, "date": date}),
        );
        assert!(message.contains("alive or dead"), "{kind}: {message}");
    }
    assert_eq!(living(&f), was);
    // The owner's own note is not the agent's to change, however the call is worded.
    with(&f.host, |s| add_note(s, id, "The owner wrote this"));
    let owners_note = with(&f.host, |s| s.tree().notes.keys().next().copied().unwrap());
    let message = tool_refusal(
        &f,
        "update_own_note",
        json!({"batch": batch, "note": owners_note, "text": "Changed"}),
    );
    assert!(!message.is_empty());
    with(&f.host, |s| {
        let note = &s.tree().notes[&owners_note];
        assert!(format!("{note:?}").contains("The owner wrote this"));
    });
}
