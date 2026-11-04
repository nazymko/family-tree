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

/// Builds the response for a request path. Pure apart from reading the session.
pub fn respond(state: &AppState, path: &str) -> Response<Vec<u8>> {
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
    match state.with_session(|s| s.media_bytes(id, size)) {
        Ok((bytes, mime)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
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
        tauri::async_runtime::spawn_blocking(move || {
            let resp = match app.try_state::<AppState>() {
                Some(state) => respond(&state, &path),
                None => text(StatusCode::NOT_FOUND, "Not ready."),
            };
            responder.respond(resp);
        });
    })
}
