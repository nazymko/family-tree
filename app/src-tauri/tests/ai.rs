//! The AI reader with a fake network and a fake key store. No real request is ever made.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::type_complexity)]
use gt_app_lib::ai::{self, AiHttp, HttpReply, KeyStore};
use serde_json::Value;
use std::sync::Mutex;
use std::time::Duration;

const FAKE_KEY: &str = "test-key-not-a-real-secret-123";

#[derive(Default)]
struct FakeHttp {
    reply: Mutex<Option<Result<HttpReply, String>>>,
    seen: Mutex<Vec<(String, Vec<(String, String)>, String)>>,
}
impl FakeHttp {
    fn answering(status: u16, body: &str) -> Self {
        let f = Self::default();
        *f.reply.lock().unwrap() = Some(Ok(HttpReply {
            status,
            body: body.into(),
        }));
        f
    }
    fn failing(why: &str) -> Self {
        let f = Self::default();
        *f.reply.lock().unwrap() = Some(Err(why.into()));
        f
    }
    fn calls(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
}
impl AiHttp for FakeHttp {
    fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
        _t: Duration,
    ) -> Result<HttpReply, String> {
        self.seen
            .lock()
            .unwrap()
            .push((url.into(), headers.to_vec(), body.into()));
        self.reply
            .lock()
            .unwrap()
            .take()
            .unwrap_or(Err("no reply".into()))
    }
}

#[derive(Default)]
struct FakeKeys(Mutex<Option<String>>);
impl KeyStore for FakeKeys {
    fn get(&self) -> Option<String> {
        self.0.lock().unwrap().clone()
    }
    fn set(&self, key: &str) -> Result<(), String> {
        *self.0.lock().unwrap() = Some(key.into());
        Ok(())
    }
    fn remove(&self) -> Result<(), String> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

fn picture(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(w, h, image::Rgb([200, 180, 160]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn ok_body(text: &str) -> String {
    serde_json::json!({"stop_reason":"end_turn","content":[{"type":"text","text":text}]})
        .to_string()
}

const ANSWER: &str = r#"{"people":[{"id":"p1","given":"Ivan","surname":"Koval","box":{"x":0.1,"y":0.1,"w":0.1,"h":0.1}}],"relations":[]}"#;

#[test]
fn request_has_headers_model_and_an_image_block() {
    let jpeg = ai::downscale_to_jpeg(&picture(300, 200)).unwrap();
    let body = ai::build_request(ai::DEFAULT_MODEL, &jpeg);
    assert_eq!(body["model"], "claude-opus-5-5");
    let content = &body["messages"][0]["content"];
    assert_eq!(content[0]["type"], "image");
    assert_eq!(content[0]["source"]["type"], "base64");
    assert_eq!(content[0]["source"]["media_type"], "image/jpeg");
    assert!(content[0]["source"]["data"].as_str().unwrap().len() > 100);
    assert_eq!(content[1]["type"], "text");
    // Nothing the newest models reject.
    for k in ["temperature", "top_p", "top_k", "tool_choice"] {
        assert!(body.get(k).is_none(), "{k}");
    }
    let h = ai::build_headers(FAKE_KEY);
    assert!(h.contains(&("x-api-key".into(), FAKE_KEY.into())));
    assert!(h.contains(&("anthropic-version".into(), "2023-06-01".into())));
    assert!(h.contains(&("content-type".into(), "application/json".into())));
}

#[test]
fn big_pictures_are_shrunk_to_2000_pixels() {
    let jpeg = ai::downscale_to_jpeg(&picture(4000, 1000)).unwrap();
    let img = image::load_from_memory(&jpeg).unwrap();
    assert_eq!((img.width(), img.height()), (2000, 500));
    let small =
        image::load_from_memory(&ai::downscale_to_jpeg(&picture(300, 200)).unwrap()).unwrap();
    assert_eq!((small.width(), small.height()), (300, 200));
    assert!(ai::downscale_to_jpeg(b"not a picture").is_err());
}

#[test]
fn a_read_sends_one_request_and_parses_the_answer() {
    let http = FakeHttp::answering(200, &ok_body(ANSWER));
    let keys = FakeKeys::default();
    let r = ai::read_picture(
        &http,
        &keys,
        true,
        Some(FAKE_KEY),
        &picture(400, 300),
        ai::DEFAULT_MODEL,
    )
    .unwrap();
    assert_eq!(r.people.len(), 1);
    assert_eq!(http.calls(), 1);
    let seen = http.seen.lock().unwrap();
    assert_eq!(seen[0].0, ai::API_URL);
    assert!(seen[0].1.contains(&("x-api-key".into(), FAKE_KEY.into())));
    // The key is in the header only, never in the body.
    assert!(!seen[0].2.contains(FAKE_KEY));
    // A key that was accepted is kept.
    assert_eq!(keys.get().as_deref(), Some(FAKE_KEY));
}

#[test]
fn nothing_is_sent_without_consent_or_without_a_key() {
    let http = FakeHttp::answering(200, &ok_body(ANSWER));
    let keys = FakeKeys::default();
    assert!(ai::read_picture(&http, &keys, false, Some(FAKE_KEY), &picture(10, 10), "m").is_err());
    assert!(ai::read_picture(&http, &keys, true, None, &picture(10, 10), "m").is_err());
    assert_eq!(http.calls(), 0);
    assert_eq!(keys.get(), None);
}

#[test]
fn errors_say_what_to_do_and_never_contain_the_key() {
    let cases = [
        (
            401,
            r#"{"error":{"message":"invalid x-api-key"}}"#,
            "Remove the key",
        ),
        (429, "{}", "Wait a minute"),
        (529, "{}", "not working right now"),
        (
            400,
            r#"{"error":{"message":"image too large"}}"#,
            "image too large",
        ),
    ];
    for (status, body, expect) in cases {
        let http = FakeHttp::answering(status, body);
        let keys = FakeKeys::default();
        let e = ai::read_picture(&http, &keys, true, Some(FAKE_KEY), &picture(10, 10), "m")
            .unwrap_err();
        assert!(e.message.contains(expect), "{status}: {}", e.message);
        assert!(!e.message.contains(FAKE_KEY));
        // A rejected key is not kept.
        assert_eq!(keys.get(), None, "{status}");
    }
    let keys = FakeKeys::default();
    let e = ai::read_picture(
        &FakeHttp::failing("timeout"),
        &keys,
        true,
        Some(FAKE_KEY),
        &picture(10, 10),
        "m",
    )
    .unwrap_err();
    assert!(e.message.contains("too long"));
    let e = ai::read_picture(
        &FakeHttp::failing("dns"),
        &keys,
        true,
        Some(FAKE_KEY),
        &picture(10, 10),
        "m",
    )
    .unwrap_err();
    assert!(e.message.contains("internet connection"));
}

#[test]
fn a_refusal_and_junk_replies_are_handled() {
    let refusal = serde_json::json!({"stop_reason":"refusal","content":[]}).to_string();
    let keys = FakeKeys::default();
    let e = ai::read_picture(
        &FakeHttp::answering(200, &refusal),
        &keys,
        true,
        Some(FAKE_KEY),
        &picture(10, 10),
        "m",
    )
    .unwrap_err();
    assert!(e.message.contains("declined"));
    let e = ai::read_picture(
        &FakeHttp::answering(200, "<html>"),
        &keys,
        true,
        Some(FAKE_KEY),
        &picture(10, 10),
        "m",
    )
    .unwrap_err();
    assert!(e.message.contains("could not be read"));
    // A truncated answer still gives the complete entries.
    let cut = ok_body(
        r#"{"people":[{"id":"p1","given":"Ann","box":{"x":0.1,"y":0.1,"w":0.1,"h":0.1}},{"id":"p2","giv"#,
    );
    let r = ai::read_picture(
        &FakeHttp::answering(200, &cut),
        &keys,
        true,
        Some(FAKE_KEY),
        &picture(10, 10),
        "m",
    )
    .unwrap();
    assert_eq!(r.people.len(), 1);
    assert!(r.truncated);
}

#[test]
fn key_store_functions_and_validation() {
    let keys = FakeKeys::default();
    assert!(!ai::key_status(&keys));
    assert!(ai::set_key(&keys, "short").is_err());
    assert!(ai::set_key(&keys, "has a space inside").is_err());
    ai::set_key(&keys, &format!("  {FAKE_KEY}\n")).unwrap();
    assert!(ai::key_status(&keys));
    ai::remove_key(&keys).unwrap();
    assert!(!ai::key_status(&keys));
    let _: Value = serde_json::json!(null);
}
