//! "Read names with AI": sends the traced picture to Anthropic's Messages API and turns the
//! answer into suggestions.
//!
//! The network call sits behind [`AiHttp`] and the key store behind [`KeyStore`], so tests use
//! fakes and never touch the network or the OS keychain. The API key lives ONLY in the OS keychain.
//! It is passed around as a plain `&str`, put in one request header, and never logged, stored in a
//! struct that derives `Debug`, or echoed in an error message.

use base64::Engine as _;
use gt_session::{AiReadResult, ApiError, ErrorCode, parse_ai_reply};
use serde_json::{Value, json};
use std::time::Duration;

pub const API_URL: &str = "https://api.anthropic.com/v1/messages";
pub const API_VERSION: &str = "2026-10-21";
pub const BETA_FALLBACKS: &str = "server-side-fallback-2026-07-01";
pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
/// A dense picture with the reader thinking hard can take minutes.
pub const TIMEOUT: Duration = Duration::from_secs(300);
/// The longest side of the picture that is sent, in pixels.
pub const MAX_SIDE: u32 = 2000;
const MAX_TOKENS: u32 = 16_000;
pub(crate) const KEYCHAIN_SERVICE: &str = "gene-tree";
const KEYCHAIN_USER: &str = "anthropic-api-key";

pub const PROMPT: &str = "This is a picture of a family tree. Find every person who is named on it. \
Answer with strict JSON only, no other text, in this shape:\n\
{\"people\":[{\"id\":\"p1\",\"given\":\"\",\"surname\":\"\",\"patronymic\":\"\",\"sex\":\"male|female|unknown\",\
\"birth\":\"\",\"death\":\"\",\"box\":{\"x\":0.0,\"y\":0.0,\"w\":0.0,\"h\":0.0},\"confidence\":0.0}],\
\"relations\":[{\"kind\":\"parent|partner\",\"from\":\"p1\",\"to\":\"p2\"}]}\n\
Rules: id is a short unique text such as p1, p2. box is the place of the person's name on the picture, \
as fractions of the picture width and height from 0 to 1 (x and y are the top left corner). \
Leave out sex, birth and death when the picture does not show them. Use a year or a date as written. \
confidence is from 0 to 1. For kind parent, from is the parent and to is the child. For kind partner, \
from and to are the two partners. Only use ids that appear in people. Do not invent people or links \
that the picture does not show.";

/// One finished HTTP call.
pub struct HttpReply {
    pub status: u16,
    pub body: String,
}

/// The one network call. Implemented with `reqwest` for the app and with fakes in tests.
pub trait AiHttp: Send + Sync {
    fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
        timeout: Duration,
    ) -> Result<HttpReply, String>;
}

/// Where the API key is kept.
pub trait KeyStore: Send + Sync {
    fn get(&self) -> Option<String>;
    fn set(&self, key: &str) -> Result<(), String>;
    fn remove(&self) -> Result<(), String>;
}

/// Set by the "cancel" command to stop the read that is in flight.
pub static CANCEL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `reqwest` (async client, rustls). The request runs as a task on the app's runtime; the calling
/// thread (a blocking-pool thread) waits for it and watches [`CANCEL`], aborting the task, which
/// drops the request and closes the connection.
pub struct ReqwestHttp;

impl AiHttp for ReqwestHttp {
    fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
        timeout: Duration,
    ) -> Result<HttpReply, String> {
        use std::sync::atomic::Ordering;
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| e.without_url().to_string())?;
        let mut req = client.post(url).body(body.to_owned());
        for (k, v) in headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let task = tauri::async_runtime::spawn(async move {
            let resp = req.send().await.map_err(|e| {
                if e.is_timeout() {
                    "timeout".to_string()
                } else {
                    e.without_url().to_string()
                }
            })?;
            let status = resp.status().as_u16();
            let body = resp.text().await.map_err(|e| e.without_url().to_string())?;
            Ok::<HttpReply, String>(HttpReply { status, body })
        });
        loop {
            if CANCEL.load(Ordering::SeqCst) {
                task.abort();
                return Err("cancelled".into());
            }
            if task.inner().is_finished() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        tauri::async_runtime::block_on(task).map_err(|_| "task failed".to_string())?
    }
}

/// The OS keychain (Windows Credential Manager, macOS Keychain, Secret Service).
pub struct Keychain;

impl Keychain {
    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER).map_err(|e| e.to_string())
    }
}

impl KeyStore for Keychain {
    fn get(&self) -> Option<String> {
        Self::entry()
            .ok()?
            .get_password()
            .ok()
            .filter(|k| !k.is_empty())
    }
    fn set(&self, key: &str) -> Result<(), String> {
        Self::entry()?.set_password(key).map_err(|e| e.to_string())
    }
    fn remove(&self) -> Result<(), String> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

fn err(code: ErrorCode, msg: &str) -> ApiError {
    ApiError::new(code, msg)
}

/// Shrinks the picture so its longest side is at most [`MAX_SIDE`] and encodes it as JPEG.
pub fn downscale_to_jpeg(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
    // The session's renderer applies the EXIF orientation first, so the reader sees the picture
    // upright, as the screen shows it, and the boxes it returns match what the user sees.
    gt_session::thumbs::render_px(bytes, MAX_SIDE).map_err(|_| {
        err(
            ErrorCode::Invalid,
            "The picture could not be opened. Choose a JPEG, PNG, WebP, GIF or TIFF picture.",
        )
    })
}

/// The JSON the reader must answer with (structured output). Optional fields may be left out.
pub fn answer_schema() -> Value {
    let text = json!({ "type": "string" });
    let num = json!({ "type": "number" });
    json!({
        "type": "object",
        "properties": {
            "people": { "type": "array", "items": {
                "type": "object",
                "properties": {
                    "id": text, "given": text, "surname": text, "patronymic": text,
                    "sex": { "type": "string", "enum": ["male", "female", "unknown"] },
                    "birth": text, "death": text,
                    "box": { "type": "object",
                        "properties": { "x": num, "y": num, "w": num, "h": num },
                        "required": ["x", "y", "w", "h"], "additionalProperties": false },
                    "confidence": num
                },
                "required": ["id", "given", "surname", "patronymic", "box", "confidence"],
                "additionalProperties": false
            } },
            "relations": { "type": "array", "items": {
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "enum": ["parent", "partner"] },
                    "from": text, "to": text
                },
                "required": ["kind", "from", "to"], "additionalProperties": false
            } }
        },
        "required": ["people", "relations"],
        "additionalProperties": false
    })
}

/// The request body. Structured output (`output_config.format`) with an explicit effort; no
/// sampling parameters, no prefill, no forced tool use. `fallbacks: "default"` lets Anthropic
/// retry on another model if a safety check declines the picture.
pub fn build_request(model: &str, jpeg: &[u8]) -> Value {
    let data = base64::engine::general_purpose::STANDARD.encode(jpeg);
    json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "output_config": {
            "effort": "high",
            "format": { "type": "json_schema", "schema": answer_schema() }
        },
        "fallbacks": "default",
        "messages": [{
            "role": "user",
            "content": [
                { "type": "image", "source": { "type": "base64", "media_type": "image/jpeg", "data": data } },
                { "type": "text", "text": PROMPT }
            ]
        }]
    })
}

/// The request headers. The key appears only here.
pub fn build_headers(key: &str) -> Vec<(String, String)> {
    vec![
        ("x-api-key".into(), key.into()),
        ("anthropic-version".into(), API_VERSION.into()),
        ("anthropic-beta".into(), BETA_FALLBACKS.into()),
        ("content-type".into(), "application/json".into()),
    ]
}

/// Turns the HTTP reply into the model's text, or into an error that says what to do.
/// The second value is true when the answer was cut off at the output limit.
pub fn reply_text(reply: &HttpReply) -> Result<(String, bool), ApiError> {
    match reply.status {
        200..=299 => {}
        401 | 403 => {
            return Err(err(
                ErrorCode::Invalid,
                "Anthropic did not accept the key. Remove the key and enter it again.",
            ));
        }
        429 => {
            return Err(err(
                ErrorCode::Conflict,
                "Anthropic says there have been too many requests. Wait a minute and try again.",
            ));
        }
        400 => {
            return Err(err(
                ErrorCode::Invalid,
                "Anthropic could not use this picture. It may be too large or in a form it cannot read. Try a smaller or clearer picture.",
            ));
        }
        413 => {
            return Err(err(
                ErrorCode::Invalid,
                "The picture is too large to send. Try a smaller picture.",
            ));
        }
        s if s >= 500 => {
            return Err(err(
                ErrorCode::Io,
                "Anthropic's service is not working right now. Try again in a few minutes.",
            ));
        }
        s => {
            return Err(err(
                ErrorCode::Io,
                &format!("Anthropic answered with an unexpected code ({s}). Try again later."),
            ));
        }
    }
    let v: Value = serde_json::from_str(&reply.body).map_err(|_| {
        err(
            ErrorCode::Io,
            "The answer from Anthropic could not be read. Try again.",
        )
    })?;
    if v.get("stop_reason").and_then(Value::as_str) == Some("refusal") {
        return Err(err(
            ErrorCode::Invalid,
            "The AI reader declined to read this picture. Trace it by hand instead.",
        ));
    }
    let stop = v.get("stop_reason").and_then(Value::as_str);
    if stop == Some("model_context_window_exceeded") {
        return Err(err(
            ErrorCode::Invalid,
            "The picture is too detailed to read in one go. Try a smaller part of it.",
        ));
    }
    let cut = stop == Some("max_tokens");
    // After a server-side fallback the answer is whatever follows the LAST `fallback` block; text before
    // it belongs to the attempt that was declined.
    let text: String = v
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            let from = blocks
                .iter()
                .rposition(|b| b.get("type").and_then(Value::as_str) == Some("fallback"))
                .map_or(0, |i| i + 1);
            blocks[from..]
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    Ok((text, cut))
}

/// Is a key stored?
pub fn key_status(keys: &dyn KeyStore) -> bool {
    keys.get().is_some()
}

/// A key must be one run of printable characters; this stops a pasted sentence from being sent as a header.
pub fn check_key(key: &str) -> Result<&str, ApiError> {
    let key = key.trim();
    if key.len() < 8 || key.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(err(
            ErrorCode::Invalid,
            "That does not look like an API key. Copy the whole key from your Anthropic account and try again.",
        ));
    }
    Ok(key)
}

pub fn set_key(keys: &dyn KeyStore, key: &str) -> Result<(), ApiError> {
    let key = check_key(key)?;
    keys.set(key).map_err(|_| {
        err(
            ErrorCode::Io,
            "The key could not be saved in the system keychain. Check that the keychain is available.",
        )
    })
}

pub fn remove_key(keys: &dyn KeyStore) -> Result<(), ApiError> {
    keys.remove().map_err(|_| {
        err(
            ErrorCode::Io,
            "The key could not be removed from the system keychain.",
        )
    })
}

/// The whole read. `consent` must be true on every call (the dialog asks each time); `new_key` is
/// a key typed into the dialog, saved only after Anthropic accepts it.
pub fn read_picture(
    http: &dyn AiHttp,
    keys: &dyn KeyStore,
    consent: bool,
    new_key: Option<&str>,
    picture: &[u8],
    model: &str,
) -> Result<AiReadResult, ApiError> {
    if !consent {
        return Err(err(
            ErrorCode::Invalid,
            "Tick \"I understand\" before sending the picture.",
        ));
    }
    let typed = new_key.map(str::trim).filter(|k| !k.is_empty());
    if let Some(k) = typed {
        check_key(k)?;
    }
    let stored = keys.get();
    let key = match (typed, stored.as_deref()) {
        (Some(k), _) | (None, Some(k)) => k,
        (None, None) => {
            return Err(err(
                ErrorCode::Invalid,
                "Enter your Anthropic API key first.",
            ));
        }
    };
    let jpeg = downscale_to_jpeg(picture)?;
    let body = build_request(model, &jpeg).to_string();
    let reply = http
        .post_json(API_URL, &build_headers(key), &body, TIMEOUT)
        .map_err(|why| {
            if why == "cancelled" {
                err(ErrorCode::Conflict, "Reading was cancelled.")
            } else if why == "timeout" {
                err(
                    ErrorCode::Io,
                    "Anthropic took too long to answer. Try again, or trace the picture by hand.",
                )
            } else {
                err(
                    ErrorCode::Io,
                    "Could not reach Anthropic. Check the internet connection and try again.",
                )
            }
        })?;
    let (text, cut) = reply_text(&reply)?;
    if let Some(k) = typed {
        // Anthropic accepted it; keep it for next time. A failure to save is not fatal.
        let _ = keys.set(k);
    }
    let mut result = parse_ai_reply(&text);
    result.truncated |= cut;
    Ok(result)
}

/// [`read_picture`] for async callers (the Tauri command). The blocking HTTP client must never run on
/// an async worker thread, so the whole read goes to the blocking pool.
pub async fn read_picture_async<H, K>(
    http: std::sync::Arc<H>,
    keys: std::sync::Arc<K>,
    consent: bool,
    new_key: Option<String>,
    picture: Vec<u8>,
    model: String,
) -> Result<AiReadResult, ApiError>
where
    H: AiHttp + 'static,
    K: KeyStore + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        read_picture(
            &*http,
            &*keys,
            consent,
            new_key.as_deref(),
            &picture,
            &model,
        )
    })
    .await
    .map_err(|_| {
        err(
            ErrorCode::Internal,
            "The reader stopped unexpectedly. Try again.",
        )
    })?
}
