#![allow(dead_code, clippy::unwrap_used, clippy::indexing_slicing)]
//! A host around a synthetic tree, seeded through `Session::apply`, and helpers to call tools.

use gt_mcp::{AgentCtx, Host, handle_message};
use gt_session::*;
use gt_store::Project;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;
use tempfile::TempDir;
use uuid::Uuid;

pub const MODERN: &str = "2026-07-28";
pub const KEY: &str = "gt_test_key_not_a_secret";

pub struct TestHost {
    pub session: Mutex<Option<Session>>,
    pub keys: Mutex<HashMap<[u8; 32], AgentCtx>>,
    pub _dir: Option<TempDir>,
}

pub fn ctx() -> AgentCtx {
    AgentCtx {
        id: Uuid::from_u128(7),
        name: "Transcript reader".into(),
    }
}

pub fn hash(key: &str) -> [u8; 32] {
    Sha256::digest(key.as_bytes()).into()
}

impl Host for TestHost {
    fn with_session<T>(
        &self,
        f: &mut dyn FnMut(&mut Session) -> Result<T, ApiError>,
    ) -> Result<T, ApiError> {
        let mut guard = self.session.lock().unwrap();
        match guard.as_mut() {
            Some(s) => f(s),
            None => Err(ApiError::new(ErrorCode::NoProject, "nothing open")),
        }
    }

    fn authenticate(&self, key_sha256: &[u8; 32]) -> Option<AgentCtx> {
        self.keys.lock().unwrap().get(key_sha256).cloned()
    }

    fn changed(&self, _touched: &[Uuid]) {}
}

/// A host with the test key and no tree open.
pub fn no_tree_host() -> TestHost {
    TestHost {
        session: Mutex::new(None),
        keys: Mutex::new(HashMap::from([(hash(KEY), ctx())])),
        _dir: None,
    }
}

/// A host with the test key and an empty tree open.
pub fn empty_host() -> TestHost {
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("project");
    drop(Project::create(&proj).unwrap());
    let session = Session::open(&proj, SessionClock::system()).unwrap();
    TestHost {
        session: Mutex::new(Some(session)),
        keys: Mutex::new(HashMap::from([(hash(KEY), ctx())])),
        _dir: Some(dir),
    }
}

pub fn with<T>(host: &TestHost, f: impl FnOnce(&mut Session) -> T) -> T {
    f(host.session.lock().unwrap().as_mut().unwrap())
}

pub fn draft(
    given: &str,
    surname: &str,
    patronymic: Option<&str>,
    born: Option<&str>,
) -> PersonDraft {
    PersonDraft {
        given: Some(given.into()),
        surnames: vec![surname.into()],
        patronymic: patronymic.map(Into::into),
        sex: SexKind::Unknown,
        birth_date_raw: born.map(Into::into),
        birth_place_text: None,
    }
}

pub fn add_person(s: &mut Session, d: PersonDraft) -> Uuid {
    s.apply(EditCommand::CreatePerson { draft: d })
        .unwrap()
        .focus
        .unwrap()
}

pub fn add_relative(s: &mut Session, anchor: Uuid, relation: Relation, d: PersonDraft) -> Uuid {
    s.apply(EditCommand::QuickAdd {
        anchor,
        relation,
        draft: d,
        family: None,
        new_parent_set: None,
        parent_rel: None,
    })
    .unwrap()
    .focus
    .unwrap()
}

pub fn add_note(s: &mut Session, person: Uuid, text: &str) {
    s.apply(EditCommand::SaveNote {
        target: OwnerRef {
            kind: OwnerKind::Person,
            id: person,
        },
        note: NoteInput {
            id: None,
            kind: NoteKindDto::General,
            text: text.into(),
        },
    })
    .unwrap();
}

pub fn add_name_variant(s: &mut Session, person: Uuid, given: &str, surname: &str) {
    s.apply(EditCommand::SaveName {
        person,
        name: NameInput {
            id: None,
            kind: NameKindDto::Aka,
            kind_text: None,
            preferred: false,
            private: false,
            prefix: None,
            given: Some(given.into()),
            call: None,
            surname_prefix: None,
            surnames: vec![surname.into()],
            patronymic: None,
            suffix: None,
            nickname: None,
            lang: None,
            date_raw: None,
        },
    })
    .unwrap();
}

/// A legacy request: no `_meta`, so the transport-free handler serves it in the older era.
pub fn request(method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})
}

/// A `2026-07-28` request with the required per-request metadata.
pub fn modern_request(method: &str, mut params: Value) -> Value {
    params["_meta"] = json!({
        "io.modelcontextprotocol/protocolVersion": MODERN,
        "io.modelcontextprotocol/clientInfo": {"name": "test", "version": "1"},
        "io.modelcontextprotocol/clientCapabilities": {},
    });
    request(method, params)
}

/// Calls a tool and returns its whole `result`.
pub fn call_raw(host: &TestHost, tool: &str, args: Value) -> Value {
    let reply = handle_message(
        host,
        &ctx(),
        &request("tools/call", json!({"name": tool, "arguments": args})),
    )
    .unwrap();
    assert!(reply.get("error").is_none(), "protocol error: {reply}");
    reply["result"].clone()
}

/// Calls a tool that must succeed and parses its first text block as JSON.
pub fn call(host: &TestHost, tool: &str, args: Value) -> Value {
    let result = call_raw(host, tool, args);
    assert_ne!(result["isError"], json!(true), "tool error: {result}");
    let text = result["content"][0]["text"].as_str().unwrap();
    serde_json::from_str(text).unwrap()
}

/// Calls a tool that must fail and returns its message.
pub fn call_err(host: &TestHost, tool: &str, args: Value) -> String {
    let result = call_raw(host, tool, args);
    assert_eq!(
        result["isError"],
        json!(true),
        "expected an error: {result}"
    );
    result["content"][0]["text"].as_str().unwrap().to_string()
}
