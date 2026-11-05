//! The listener: loopback only, one short-lived thread per connection under a hard cap, one
//! request per connection, `POST /mcp` and nothing else.
//!
//! Every check runs on the parsed head, before any body byte is waited for: Host, Origin, path,
//! key, method, then how the body is framed. A request has one deadline for its head and body
//! together, so a client sending a byte at a time cannot outlast it.
//!
//! Written on `std::net` because `tiny_http` cannot keep these promises: dropping a request there
//! allocates whatever `Content-Length` announced, so one request with no key could abort the app.

use crate::Host;
use crate::protocol::{self, HttpMeta, Reply, ToolGate};
use serde_json::Value;
use std::io::{self, ErrorKind, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_HEAD: usize = 16 * 1024;
const MAX_HEADERS: usize = 64;
const MAX_BODY: u64 = 8 * 1024 * 1024;
/// A head arrives at once from any real client, so a silent socket gives up its slot soon.
const HEAD_TIMEOUT: Duration = Duration::from_secs(5);
/// How often the accept loop looks for a new connection or the order to stop.
const ACCEPT_POLL: Duration = Duration::from_millis(20);
/// After answering, how long and how much of what the client still sends is thrown away. Closing
/// with unread data resets the connection, and a reset can destroy the answer on its way.
const LINGER: Duration = Duration::from_millis(250);
const LINGER_BYTES: usize = 1024 * 1024;

/// How much the server takes on.
#[derive(Debug, Clone, Copy)]
pub struct ServerLimits {
    /// Connections served at once. Over this, a connection gets a 503 and is closed.
    pub max_connections: usize,
    /// The deadline for receiving a whole request, and for sending a whole answer.
    pub timeout: Duration,
}

impl Default for ServerLimits {
    fn default() -> Self {
        Self {
            max_connections: 32,
            timeout: Duration::from_secs(30),
        }
    }
}

/// The running server. `stop()` or dropping it ends the accept loop at once, and no request starts
/// a tool after that. It does not wait for open connections. A connection thread can outlive this
/// handle, keeping its `Arc<H>`, for at most: what remains of the read deadline, then the wait for
/// the host's session and the tool call, then the write deadline, then 250 ms of linger. Only the
/// wait and the tool call have no limit here, so see [`crate::Host`] for what that asks of a host.
pub struct ServerHandle {
    stopping: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
    addr: SocketAddr,
}

impl ServerHandle {
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn stop(self) {}
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        if let Some(accept) = self.accept.take() {
            let _ = accept.join();
        }
    }
}

/// Starts listening on `127.0.0.1:<port>`. Port 0 picks a free port; see [`ServerHandle::port`].
pub fn serve<H: Host>(host: Arc<H>, port: u16) -> Result<ServerHandle, io::Error> {
    serve_with(host, port, ServerLimits::default())
}

/// [`serve`] with limits of the caller's choosing.
pub fn serve_with<H: Host>(
    host: Arc<H>,
    port: u16,
    limits: ServerLimits,
) -> Result<ServerHandle, io::Error> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
    // Polled, so that stopping never depends on another connection arriving.
    listener.set_nonblocking(true)?;
    let addr = listener.local_addr()?;
    let stopping = Arc::new(AtomicBool::new(false));
    let stop = Arc::clone(&stopping);
    let gate = Arc::new(ToolGate::default());
    let accept = thread::Builder::new()
        .name("gt-mcp-accept".into())
        .spawn(move || accept_loop(&listener, &host, addr.port(), limits, &stop, &gate))?;
    Ok(ServerHandle {
        stopping,
        accept: Some(accept),
        addr,
    })
}

/// A taken connection slot, given back however the connection's thread ends.
struct Slot(Arc<AtomicUsize>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn accept_loop<H: Host>(
    listener: &TcpListener,
    host: &Arc<H>,
    port: u16,
    limits: ServerLimits,
    stopping: &Arc<AtomicBool>,
    gate: &Arc<ToolGate>,
) {
    let live = Arc::new(AtomicUsize::new(0));
    while !stopping.load(Ordering::SeqCst) {
        let Ok((stream, _)) = listener.accept() else {
            // Usually nothing waiting. Anything else, like being out of handles, may pass too.
            thread::sleep(ACCEPT_POLL);
            continue;
        };
        if live.load(Ordering::SeqCst) >= limits.max_connections {
            refuse_full(stream);
            continue;
        }
        live.fetch_add(1, Ordering::SeqCst);
        let slot = Slot(Arc::clone(&live));
        let host = Arc::clone(host);
        let stopping = Arc::clone(stopping);
        let gate = Arc::clone(gate);
        // If the thread cannot start, the closure is dropped: the slot returns, the socket closes.
        let _ = thread::Builder::new()
            .name("gt-mcp-connection".into())
            .spawn(move || {
                let _slot = slot;
                connection(&*host, stream, port, limits.timeout, &stopping, &gate);
            });
    }
}

/// Answers from the accept thread, which must never block: all of it happens without waiting.
fn refuse_full(mut stream: TcpStream) {
    if stream.set_nonblocking(true).is_err() {
        return;
    }
    let _ = stream.write_all(&encode(&Answer::status(503)));
    let _ = stream.shutdown(Shutdown::Write);
    let mut sink = [0u8; 4096];
    let mut left = LINGER_BYTES;
    while left > 0 {
        match stream.read(&mut sink) {
            Ok(n) if n > 0 => left = left.saturating_sub(n),
            _ => break,
        }
    }
}

/// What goes back. An empty `body` means an empty answer.
struct Answer {
    status: u16,
    body: Vec<u8>,
    json: bool,
    header: Option<(&'static str, &'static str)>,
}

impl Answer {
    fn status(status: u16) -> Self {
        Self {
            status,
            body: Vec::new(),
            json: false,
            header: None,
        }
    }

    fn with_header(status: u16, name: &'static str, value: &'static str) -> Self {
        Self {
            header: Some((name, value)),
            ..Self::status(status)
        }
    }

    fn from_reply(reply: &Reply) -> Self {
        let body = reply.body.as_ref().map(Value::to_string);
        Self {
            json: body.is_some(),
            body: body.unwrap_or_default().into_bytes(),
            ..Self::status(reply.status)
        }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        411 => "Length Required",
        413 => "Content Too Large",
        431 => "Request Header Fields Too Large",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    }
}

fn encode(answer: &Answer) -> Vec<u8> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        answer.status,
        reason(answer.status),
        answer.body.len()
    );
    if answer.json {
        head.push_str("Content-Type: application/json\r\n");
    }
    if let Some((name, value)) = answer.header {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let mut bytes = head.into_bytes();
    bytes.extend_from_slice(&answer.body);
    bytes
}

fn connection<H: Host>(
    host: &H,
    mut stream: TcpStream,
    port: u16,
    timeout: Duration,
    stopping: &AtomicBool,
    gate: &ToolGate,
) {
    // On some systems an accepted socket inherits the listener's non-blocking mode.
    if stream.set_nonblocking(false).is_err() {
        return;
    }
    let _ = stream.set_nodelay(true);
    let deadline = Instant::now() + timeout;
    let Some(answer) = serve_one(host, &mut stream, port, deadline, timeout, stopping, gate) else {
        return;
    };
    if write_by(&mut stream, &encode(&answer), Instant::now() + timeout).is_ok() {
        linger(&mut stream);
    }
}

/// The answer to the one request on this connection. `None`: the client left, so nobody to tell.
fn serve_one<H: Host>(
    host: &H,
    stream: &mut TcpStream,
    port: u16,
    deadline: Instant,
    timeout: Duration,
    stopping: &AtomicBool,
    gate: &ToolGate,
) -> Option<Answer> {
    let mut buffer = vec![0u8; MAX_HEAD];
    let head_deadline = deadline.min(Instant::now() + HEAD_TIMEOUT);
    let (head, head_len, filled) = match read_head(stream, &mut buffer, head_deadline) {
        Ok(read) => read,
        Err(answer) => return answer,
    };
    if !allowed_host(&head, port) || !allowed_origin(&head, port) {
        return Some(Answer::status(403));
    }
    if head.path() != "/mcp" {
        return Some(Answer::status(404));
    }
    let hash = bearer_hash(&head);
    let Some(hash) = hash.filter(|hash| host.authenticate(hash).is_some()) else {
        return Some(refuse_key());
    };
    if head.method != "POST" {
        return Some(Answer::with_header(405, "Allow", "POST"));
    }
    let length = match body_length(&head) {
        Ok(length) => length,
        Err(status) => return Some(Answer::status(status)),
    };
    let mut versions = head.values("MCP-Protocol-Version");
    if let Some(first) = versions.next()
        && versions.any(|other| other != first)
    {
        return Some(Answer::from_reply(&protocol::conflicting_version()));
    }
    let expects_continue = head
        .value("Expect")
        .is_some_and(|v| v.eq_ignore_ascii_case("100-continue"));
    if expects_continue {
        let invite = b"HTTP/1.1 100 Continue\r\n\r\n";
        write_by(stream, invite, Instant::now() + timeout).ok()?;
    }
    let early = buffer.get(head_len..filled).unwrap_or_default();
    let body = match read_body(stream, early, length, deadline) {
        Ok(body) => body,
        Err(answer) => return answer,
    };
    // Stopping does not wait for this thread, so a request must not start a tool after it.
    if stopping.load(Ordering::SeqCst) {
        return Some(Answer::status(503));
    }
    let Ok(value) = serde_json::from_slice::<Value>(&body) else {
        return Some(Answer::from_reply(&protocol::parse_error()));
    };
    let meta = HttpMeta {
        protocol_version: head.value("MCP-Protocol-Version").map(str::to_string),
        method: head.value("Mcp-Method").map(str::to_string),
        name: head.value("Mcp-Name").map(str::to_string),
    };
    // The body can take its time, and the key can be revoked meanwhile.
    let Some(ctx) = host.authenticate(&hash) else {
        return Some(refuse_key());
    };
    let reply = catch_unwind(AssertUnwindSafe(|| {
        protocol::dispatch(host, &ctx, &value, Some(&meta), Some(gate))
    }));
    Some(reply.map_or(Answer::status(500), |r| Answer::from_reply(&r)))
}

fn refuse_key() -> Answer {
    Answer::with_header(401, "WWW-Authenticate", "Bearer")
}

struct Head {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
}

impl Head {
    fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or_default()
    }

    /// All values of a header, whose name is matched without regard to case.
    fn values<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> {
        self.headers
            .iter()
            .filter(move |(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    fn value<'a>(&'a self, name: &'a str) -> Option<&'a str> {
        self.values(name).next()
    }
}

/// Reads until the head is complete. Returns it, its length, and how much of `buffer` was filled,
/// which may include the start of the body.
fn read_head(
    stream: &mut TcpStream,
    buffer: &mut [u8],
    deadline: Instant,
) -> Result<(Head, usize, usize), Option<Answer>> {
    let mut filled = 0;
    loop {
        match parse_head(buffer.get(..filled).unwrap_or_default()) {
            Ok(Some((head, len))) => return Ok((head, len, filled)),
            Ok(None) => {}
            Err(status) => return Err(Some(Answer::status(status))),
        }
        let Some(room) = buffer.get_mut(filled..).filter(|room| !room.is_empty()) else {
            return Err(Some(Answer::status(431)));
        };
        filled += read_by(stream, room, deadline)?;
    }
}

/// The head and its length, `None` while it is incomplete, or the status to refuse it with.
fn parse_head(bytes: &[u8]) -> Result<Option<(Head, usize)>, u16> {
    let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
    let mut request = httparse::Request::new(&mut headers);
    match request.parse(bytes) {
        Ok(httparse::Status::Complete(len)) => {
            let head = Head {
                method: request.method.unwrap_or_default().to_string(),
                target: request.path.unwrap_or_default().to_string(),
                headers: request
                    .headers
                    .iter()
                    .map(|h| {
                        // Text that is not UTF-8 can match nothing we compare against.
                        let value = std::str::from_utf8(h.value).unwrap_or("\u{FFFD}");
                        (h.name.to_string(), value.trim_ascii().to_string())
                    })
                    .collect(),
            };
            Ok(Some((head, len)))
        }
        Ok(httparse::Status::Partial) => Ok(None),
        Err(httparse::Error::TooManyHeaders) => Err(431),
        Err(_) => Err(400),
    }
}

/// How long the body is, or the status to refuse it with. Decided from the head alone.
fn body_length(head: &Head) -> Result<u64, u16> {
    let mut codings = head.values("Transfer-Encoding");
    if let Some(first) = codings.next() {
        // Chunked is a coding clients can do without, so ask for a length. Anything else is a
        // coding this server does not implement.
        let only_chunked = first.eq_ignore_ascii_case("chunked") && codings.next().is_none();
        return Err(if only_chunked { 411 } else { 501 });
    }
    let mut lengths = head.values("Content-Length");
    let Some(text) = lengths.next() else {
        return Err(411);
    };
    if lengths.next().is_some() || text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(400);
    }
    let length: u64 = text.parse().map_err(|_| 400_u16)?;
    if length > MAX_BODY {
        return Err(413);
    }
    Ok(length)
}

fn read_body(
    stream: &mut TcpStream,
    early: &[u8],
    length: u64,
    deadline: Instant,
) -> Result<Vec<u8>, Option<Answer>> {
    let length = usize::try_from(length).map_err(|_| Some(Answer::status(413)))?;
    let mut body = vec![0u8; length];
    let mut filled = early.len().min(length);
    if let (Some(start), Some(from)) = (body.get_mut(..filled), early.get(..filled)) {
        start.copy_from_slice(from);
    }
    while let Some(room) = body.get_mut(filled..).filter(|room| !room.is_empty()) {
        filled += read_by(stream, room, deadline)?;
    }
    Ok(body)
}

/// One read within the deadline. A silent end of the connection or a broken one has nobody to
/// answer; running out of time gets a 408.
fn read_by(
    stream: &mut TcpStream,
    room: &mut [u8],
    deadline: Instant,
) -> Result<usize, Option<Answer>> {
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || stream.set_read_timeout(Some(left)).is_err() {
            return Err(Some(Answer::status(408)));
        }
        match stream.read(room) {
            Ok(0) => return Err(None),
            Ok(n) => return Ok(n),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) if is_timeout(&e) => return Err(Some(Answer::status(408))),
            Err(_) => return Err(None),
        }
    }
}

/// Writes everything within the deadline, however slowly the client takes it.
fn write_by(stream: &mut TcpStream, mut bytes: &[u8], deadline: Instant) -> io::Result<()> {
    while !bytes.is_empty() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(ErrorKind::TimedOut.into());
        }
        stream.set_write_timeout(Some(left))?;
        match stream.write(bytes) {
            Ok(0) => return Err(ErrorKind::WriteZero.into()),
            Ok(n) => bytes = bytes.get(n..).unwrap_or_default(),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// A timed-out read is `TimedOut` on Windows and `WouldBlock` elsewhere.
fn is_timeout(e: &io::Error) -> bool {
    matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock)
}

/// Ends the connection so that the answer arrives, whatever the client was still sending.
fn linger(stream: &mut TcpStream) {
    if stream.shutdown(Shutdown::Write).is_err() {
        return;
    }
    let deadline = Instant::now() + LINGER;
    let mut sink = [0u8; 4096];
    let mut left = LINGER_BYTES;
    while left > 0 {
        let wait = deadline.saturating_duration_since(Instant::now());
        if wait.is_zero() || stream.set_read_timeout(Some(wait)).is_err() {
            return;
        }
        match stream.read(&mut sink) {
            Ok(n) if n > 0 => left = left.saturating_sub(n),
            _ => return,
        }
    }
}

/// One Host, naming this server on loopback. It stops a web page reaching us by DNS rebinding.
fn allowed_host(head: &Head, port: u16) -> bool {
    let mut values = head.values("Host");
    let (Some(value), None) = (values.next(), values.next()) else {
        return false;
    };
    value.eq_ignore_ascii_case(&format!("127.0.0.1:{port}"))
        || value.eq_ignore_ascii_case(&format!("localhost:{port}"))
}

/// No Origin (not a browser), or this server's own.
fn allowed_origin(head: &Head, port: u16) -> bool {
    head.values("Origin").all(|value| {
        value.eq_ignore_ascii_case(&format!("http://127.0.0.1:{port}"))
            || value.eq_ignore_ascii_case(&format!("http://localhost:{port}"))
    })
}

/// The SHA-256 of the one bearer key in the request.
fn bearer_hash(head: &Head) -> Option<[u8; 32]> {
    let mut values = head.values("Authorization");
    let (Some(value), None) = (values.next(), values.next()) else {
        return None;
    };
    let (scheme, key) = value.split_once(' ')?;
    let key = key.trim_ascii();
    if !scheme.eq_ignore_ascii_case("bearer") || key.is_empty() {
        return None;
    }
    Some(crate::key_hash(key))
}
