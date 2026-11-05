//! The `gtmedia:` scheme: `gtmedia://localhost/<uuid>/<thumb|preview|original>` (Windows and
//! Android serve it as `http://gtmedia.localhost/...`). Only the media id and the size word
//! are read from the URL; neither is ever used as a file name. The session finds the file.

use crate::state::AppState;
use gt_session::{ApiError, ErrorCode, MediaSize};
use tauri::http::{Response, StatusCode, header};
use tauri::{Manager, Runtime};
use uuid::Uuid;

pub const SCHEME: &str = "gtmedia";
/// `gtmedia://localhost/trace/picture`: the picture being traced.
pub const TRACE_PATH: &str = "/trace/picture";
pub const CACHE_CONTROL: &str = "private, max-age=31536000, immutable";

/// `Some` only for exactly `/<hyphenated uuid>/<thumb|preview|original>`.
pub fn parse_path(path: &str) -> Option<(Uuid, MediaSize)> {
    let rest = path.strip_prefix('/')?;
    let (id, size) = rest.split_once('/')?;
    // 8-4-4-4-12 hex digits only: no braces, urn prefix or other forms `Uuid` would accept.
    let shaped = id.len() == 36
        && id.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        });
    if !shaped {
        return None;
    }
    let size = match size {
        "thumb" => MediaSize::Thumb,
        "preview" => MediaSize::Preview,
        "original" => MediaSize::Original,
        _ => return None,
    };
    Some((Uuid::parse_str(id).ok()?, size))
}

fn text(status: StatusCode, msg: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(msg.as_bytes().to_vec())
        .unwrap_or_default()
}

fn status_for(e: &ApiError) -> StatusCode {
    match e.code {
        ErrorCode::NotFound | ErrorCode::NoProject => StatusCode::NOT_FOUND,
        ErrorCode::Invalid => StatusCode::UNSUPPORTED_MEDIA_TYPE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// What a `Range: bytes=...` header asks for, first range only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteRange {
    /// `bytes=a-` or `bytes=a-b`.
    From(u64, Option<u64>),
    /// `bytes=-n`: the last n bytes.
    Last(u64),
}

/// `None` when the header is absent or not a single well-formed byte range, so the whole file is
/// sent (a player that asked oddly still gets sound).
pub fn parse_range(header: &str) -> Option<ByteRange> {
    let spec = header.trim().strip_prefix("bytes=")?;
    let first = spec.split(',').next()?.trim();
    let (a, b) = first.split_once('-')?;
    match (a.trim(), b.trim()) {
        ("", n) => Some(ByteRange::Last(n.parse().ok()?)),
        (a, "") => Some(ByteRange::From(a.parse().ok()?, None)),
        (a, b) => {
            let (a, b) = (a.parse().ok()?, b.parse().ok()?);
            (a <= b).then_some(ByteRange::From(a, Some(b)))
        }
    }
}

fn partial(state: &AppState, id: Uuid, range: ByteRange) -> Response<Vec<u8>> {
    let slice = state.with_session(|s| {
        let (start, end) = match range {
            ByteRange::From(a, b) => (a, b),
            ByteRange::Last(n) => {
                // Learn the size from one byte, then ask for the tail.
                let total = s.media_range(id, 0, Some(0))?.map_or(0, |one| one.total);
                (total.saturating_sub(n), None)
            }
        };
        s.media_range(id, start, end).map(|r| (r, start))
    });
    match slice {
        Ok((Some(piece), _)) => {
            let last = piece.start + piece.bytes.len() as u64 - 1;
            Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, piece.mime)
                .header(header::ACCEPT_RANGES, "bytes")
                .header(
                    header::CONTENT_RANGE,
                    format!("bytes {}-{last}/{}", piece.start, piece.total),
                )
                .header(header::CACHE_CONTROL, CACHE_CONTROL)
                .body(piece.bytes)
                .unwrap_or_default()
        }
        Ok((None, _)) => Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(b"Range not satisfiable.".to_vec())
            .unwrap_or_default(),
        Err(e) => text(status_for(&e), &e.message),
    }
}

/// Builds the response for a request path. Pure apart from reading the session.
pub fn respond(state: &AppState, path: &str) -> Response<Vec<u8>> {
    respond_range(state, path, None)
}

/// As `respond`, honouring a `Range` header on an original file, so a player can seek in a long
/// recording without the whole file being read for each request.
pub fn respond_range(state: &AppState, path: &str, range: Option<&str>) -> Response<Vec<u8>> {
    if path == TRACE_PATH {
        // The picture being traced; never cached, because a new one replaces it.
        return match state.with_session(|s| s.trace_image_bytes()) {
            Ok((bytes, mime)) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header(header::CACHE_CONTROL, "no-store")
                .body(bytes)
                .unwrap_or_default(),
            Err(e) => text(status_for(&e), &e.message),
        };
    }
    let Some((id, size)) = parse_path(path) else {
        return text(StatusCode::BAD_REQUEST, "Malformed media address.");
    };
    if size == MediaSize::Original
        && let Some(range) = range.and_then(parse_range)
    {
        return partial(state, id, range);
    }
    match state.with_session(|s| s.media_bytes(id, size)) {
        Ok((bytes, mime)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CACHE_CONTROL, CACHE_CONTROL)
            .body(bytes)
            .unwrap_or_default(),
        Err(e) => text(status_for(&e), &e.message),
    }
}

/// Registers the scheme on a builder. The work runs on the blocking pool, so a thumbnail that
/// has to be decoded never stalls the UI thread.
pub fn register<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.register_asynchronous_uri_scheme_protocol(SCHEME, |ctx, request, responder| {
        let app = ctx.app_handle().clone();
        let path = request.uri().path().to_string();
        let range = request
            .headers()
            .get(header::RANGE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        tauri::async_runtime::spawn_blocking(move || {
            let resp = match app.try_state::<AppState>() {
                Some(state) => respond_range(&state, &path, range.as_deref()),
                None => text(StatusCode::NOT_FOUND, "Not ready."),
            };
            responder.respond(resp);
        });
    })
}
