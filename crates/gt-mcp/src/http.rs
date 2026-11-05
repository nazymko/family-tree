//! The listener: loopback only, a small fixed pool of workers, `POST /mcp` and nothing else.
//!
//! Every check runs before the body is read, cheapest and least telling first: Host, Origin, path,
//! key, method, announced size. `tiny_http` itself buffers bodies of up to 1 KiB before a request
//! reaches a worker, on that connection's own thread.

use crate::Host;
use crate::protocol::{self, HttpMeta, Reply};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{self, Read};
use std::net::SocketAddr;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tiny_http::{Header, Method, Request, Response, Server};

const WORKERS: usize = 4;
const POLL: Duration = Duration::from_millis(100);
/// How long stopping waits for a worker that is still reading a stalled body.
const STOP_WAIT: Duration = Duration::from_secs(2);
const MAX_BODY: u64 = 8 * 1024 * 1024;
/// `tiny_http` reads bodies up to this size by itself; a longer one waits on the socket.
const LIBRARY_BUFFERED: usize = 1024;
/// Dropping a request allocates all it announced and still unread, so a wild `Content-Length`
/// would abort the whole app. Above this size the refused request is leaked instead.
const SAFE_TO_DROP: usize = 256 * 1024 * 1024;

/// The running server. `stop()` or dropping it shuts the listener down.
pub struct ServerHandle {
    server: Arc<Server>,
    stopping: Arc<AtomicBool>,
    running: Arc<AtomicUsize>,
    workers: Vec<JoinHandle<()>>,
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

    fn shut_down(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        self.server.unblock();
        // A worker stuck on a stalled client must not hold up the app: it ends with that client.
        let deadline = Instant::now() + STOP_WAIT;
        while self.running.load(Ordering::SeqCst) > 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        if self.running.load(Ordering::SeqCst) == 0 {
            for worker in self.workers.drain(..) {
                let _ = worker.join();
            }
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.shut_down();
    }
}

/// Decrements the count of running workers however the worker ends.
struct Running(Arc<AtomicUsize>);

impl Drop for Running {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Starts listening on `127.0.0.1:<port>`. Port 0 picks a free port; see [`ServerHandle::port`].
pub fn serve<H: Host>(host: Arc<H>, port: u16) -> Result<ServerHandle, io::Error> {
    let server = Server::http(("127.0.0.1", port)).map_err(io::Error::other)?;
    let addr = server
        .server_addr()
        .to_ip()
        .ok_or_else(|| io::Error::other("the listener has no address"))?;
    let server = Arc::new(server);
    let stopping = Arc::new(AtomicBool::new(false));
    let running = Arc::new(AtomicUsize::new(WORKERS));
    let mut workers = Vec::with_capacity(WORKERS);
    for _ in 0..WORKERS {
        let server = Arc::clone(&server);
        let host = Arc::clone(&host);
        let stopping = Arc::clone(&stopping);
        let guard = Running(Arc::clone(&running));
        workers.push(thread::spawn(move || {
            let _guard = guard;
            worker(&server, &*host, addr.port(), &stopping);
        }));
    }
    Ok(ServerHandle {
        server,
        stopping,
        running,
        workers,
        addr,
    })
}

fn worker<H: Host>(server: &Server, host: &H, port: u16, stopping: &AtomicBool) {
    while !stopping.load(Ordering::SeqCst) {
        match server.recv_timeout(POLL) {
            Ok(Some(request)) => handle(host, port, request),
            Ok(None) => {}
            // The listener failed for good; nothing more will come.
            Err(_) => return,
        }
    }
}

fn handle<H: Host>(host: &H, port: u16, mut request: Request) {
    if !allowed_host(&request, port) || !allowed_origin(&request, port) {
        return refuse(request, 403, None);
    }
    if path_of(request.url()) != "/mcp" {
        return refuse(request, 404, None);
    }
    let Some(ctx) = bearer_hash(&request).and_then(|hash| host.authenticate(&hash)) else {
        return refuse(request, 401, Some(("WWW-Authenticate", "Bearer")));
    };
    if *request.method() != Method::Post {
        return refuse(request, 405, Some(("Allow", "POST")));
    }
    if request.body_length().is_some_and(|n| n as u64 > MAX_BODY) {
        return refuse(request, 413, None);
    }
    let meta = HttpMeta {
        protocol_version: header_value(&request, "MCP-Protocol-Version"),
        method: header_value(&request, "Mcp-Method"),
        name: header_value(&request, "Mcp-Name"),
    };
    let mut body = Vec::new();
    let read = request
        .as_reader()
        .take(MAX_BODY + 1)
        .read_to_end(&mut body);
    if read.is_err() {
        return drop(request.respond(Response::empty(400)));
    }
    if body.len() as u64 > MAX_BODY {
        // Only a chunked body gets here, and that kind has no drain on drop.
        return drop(request.respond(Response::empty(413)));
    }
    let reply = match serde_json::from_slice::<Value>(&body) {
        Err(_) => protocol::parse_error(),
        Ok(value) => {
            let answered = catch_unwind(AssertUnwindSafe(|| {
                protocol::dispatch(host, &ctx, &value, Some(&meta))
            }));
            answered.unwrap_or(Reply {
                status: 500,
                body: None,
            })
        }
    };
    let _ = request.respond(json_response(&reply));
}

fn json_response(reply: &Reply) -> Response<io::Cursor<Vec<u8>>> {
    let bytes = reply
        .body
        .as_ref()
        .map(Value::to_string)
        .unwrap_or_default();
    let mut response = Response::from_data(bytes.into_bytes())
        .with_status_code(reply.status)
        .with_chunked_threshold(usize::MAX);
    if reply.body.is_some()
        && let Ok(header) = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
    {
        response.add_header(header);
    }
    response
}

/// Answers a request whose body nobody will read, without letting the client pin a worker.
///
/// Answering a request drops it, and the drop reads the rest of an announced body from the
/// socket: that blocks for as long as the client keeps it open, and allocates all of it at once.
fn refuse(request: Request, status: u16, header: Option<(&str, &str)>) {
    let mut response = Response::empty(status);
    if let Some((name, value)) = header
        && let Ok(header) = Header::from_bytes(name.as_bytes(), value.as_bytes())
    {
        response.add_header(header);
    }
    match request.body_length() {
        Some(n) if n > SAFE_TO_DROP => {
            // The one way to answer without the drop. It costs this connection's socket and
            // thread until the app exits; only a hand-made request announces this much.
            std::mem::forget(request.upgrade("none", response));
        }
        Some(n) if n > LIBRARY_BUFFERED => {
            // The drain ends with the client. One thread per connection, as `tiny_http` has.
            let _ = thread::Builder::new().spawn(move || {
                let _ = request.respond(response);
            });
        }
        _ => {
            let _ = request.respond(response);
        }
    }
}

fn path_of(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}

/// All values of a header, whose name is matched without regard to case.
fn header_values<'a>(request: &'a Request, name: &'static str) -> impl Iterator<Item = &'a str> {
    request
        .headers()
        .iter()
        .filter(move |h| h.field.equiv(name))
        .map(|h| h.value.as_str())
}

fn header_value(request: &Request, name: &'static str) -> Option<String> {
    header_values(request, name).next().map(str::to_string)
}

/// One Host, naming this server on loopback. It stops a web page reaching us by DNS rebinding.
fn allowed_host(request: &Request, port: u16) -> bool {
    let mut values = header_values(request, "Host");
    let (Some(value), None) = (values.next(), values.next()) else {
        return false;
    };
    value.eq_ignore_ascii_case(&format!("127.0.0.1:{port}"))
        || value.eq_ignore_ascii_case(&format!("localhost:{port}"))
}

/// No Origin (not a browser), or this server's own.
fn allowed_origin(request: &Request, port: u16) -> bool {
    header_values(request, "Origin").all(|value| {
        value.eq_ignore_ascii_case(&format!("http://127.0.0.1:{port}"))
            || value.eq_ignore_ascii_case(&format!("http://localhost:{port}"))
    })
}

/// The SHA-256 of the one bearer key in the request.
fn bearer_hash(request: &Request) -> Option<[u8; 32]> {
    let mut values = header_values(request, "Authorization");
    let (Some(value), None) = (values.next(), values.next()) else {
        return None;
    };
    let (scheme, key) = value.split_once(' ')?;
    let key = key.trim();
    if !scheme.eq_ignore_ascii_case("bearer") || key.is_empty() {
        return None;
    }
    Some(Sha256::digest(key.as_bytes()).into())
}
