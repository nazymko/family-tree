#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::print_stderr)]
//! How long agent writes and the first page of the log take on a tree of 5,000 people, over real
//! HTTP, and how long each write holds the session lock. Slow, so run by hand:
//!
//!     cargo test -p gt-mcp --test timing -- --ignored --nocapture
//!
//! (add `--release` for the release profile). The numbers go to stderr.

#[path = "common/client.rs"]
mod client;
mod common;

use client::HttpClient;
use common::{KEY, ctx, draft, hash};
use gt_mcp::{AgentCtx, Host, serve};
use gt_session::{
    ApiError, EditCommand, ErrorCode, NoteInput, NoteKindDto, OwnerKind, OwnerRef, Session,
    SessionClock,
};
use gt_store::Project;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

const PEOPLE: usize = 5_000;

/// A host that records how long each `with_session` call kept the session (not the wait for it).
struct Timed {
    session: Mutex<Option<Session>>,
    keys: HashMap<[u8; 32], AgentCtx>,
    holds: Mutex<Vec<Duration>>,
    _dir: tempfile::TempDir,
}

impl Host for Timed {
    fn with_session<T>(
        &self,
        f: &mut dyn FnMut(&mut Session) -> Result<T, ApiError>,
    ) -> Result<T, ApiError> {
        let mut guard = self.session.lock().unwrap();
        let start = Instant::now();
        let out = match guard.as_mut() {
            Some(s) => f(s),
            None => Err(ApiError::new(ErrorCode::NoProject, "nothing open")),
        };
        self.holds.lock().unwrap().push(start.elapsed());
        out
    }
    fn authenticate(&self, key_sha256: &[u8; 32]) -> Option<AgentCtx> {
        self.keys.get(key_sha256).cloned()
    }
    fn changed(&self, _: &[Uuid], _: &AgentCtx) {}
}

/// A tree of `PEOPLE` people, made the way the app makes them, one command each.
fn big() -> (Arc<Timed>, Vec<Uuid>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("project");
    drop(Project::create(&path).unwrap());
    let mut session = Session::open(&path, SessionClock::system()).unwrap();
    let start = Instant::now();
    let ids: Vec<Uuid> = (0..PEOPLE)
        .map(|i| {
            let d = draft(
                &format!("Person{i}"),
                "Synthetic",
                None,
                Some(&format!("{}", 1800 + i % 150)),
            );
            session
                .apply(EditCommand::CreatePerson { draft: d })
                .unwrap()
                .focus
                .unwrap()
        })
        .collect();
    eprintln!("seeded {PEOPLE} people in {:?}", start.elapsed());
    let host = Timed {
        session: Mutex::new(Some(session)),
        keys: HashMap::from([(hash(KEY), ctx())]),
        holds: Mutex::new(Vec::new()),
        _dir: dir,
    };
    (Arc::new(host), ids)
}

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    sorted[((sorted.len() as f64 * p) as usize).min(sorted.len() - 1)]
}

fn report(what: &str, mut v: Vec<Duration>) {
    v.sort();
    let total: Duration = v.iter().sum();
    eprintln!(
        "{what}: n={} total={:?} mean={:?} p50={:?} p95={:?} max={:?}",
        v.len(),
        total,
        total / v.len() as u32,
        percentile(&v, 0.5),
        percentile(&v, 0.95),
        v.last().unwrap()
    );
}

#[test]
#[ignore = "slow: 5,000 people and 300 writes over HTTP"]
fn three_hundred_agent_writes_on_five_thousand_people() {
    let (host, ids) = big();
    let server = serve(Arc::clone(&host), 0).unwrap();
    let c = HttpClient::new(server.port());
    let batch = c.call("begin_batch", json!({"title": "Timing"}))["batch"].clone();
    host.holds.lock().unwrap().clear();
    let mut writes = Vec::new();
    let start = Instant::now();
    for i in 0..300 {
        let anchor = ids[(i * 16) % PEOPLE];
        let (tool, args) = match i % 3 {
            0 => (
                "add_person",
                json!({"batch": batch, "given": format!("Child{i}"), "surnames": ["Synthetic"],
                       "source_file": "timing", "number": i.to_string(),
                       "relation": {"to": anchor, "as": "child"}}),
            ),
            1 => (
                "add_note",
                json!({"batch": batch, "target": {"kind": "person", "id": anchor}, "text": format!("Note {i}")}),
            ),
            _ => (
                "add_event",
                json!({"batch": batch, "person": anchor, "kind": "residence", "place": format!("Place {i}")}),
            ),
        };
        let t = Instant::now();
        c.call(tool, args);
        writes.push(t.elapsed());
    }
    let total = start.elapsed();
    eprintln!("300 writes took {total:?} in all");
    report("per write over HTTP", writes);
    report(
        "session lock held per call",
        host.holds.lock().unwrap().clone(),
    );
    server.stop();
}

#[test]
#[ignore = "slow: 5,000 people and 5,000 agent changes"]
fn the_first_page_of_the_log_with_five_thousand_changes() {
    let (host, ids) = big();
    let agent = ctx();
    {
        let mut guard = host.session.lock().unwrap();
        let s = guard.as_mut().unwrap();
        let batch = s.begin_batch(&agent, "Timing", None).unwrap();
        let start = Instant::now();
        for (i, id) in ids.iter().enumerate() {
            s.apply_as_agent(
                EditCommand::SaveNote {
                    target: OwnerRef {
                        kind: OwnerKind::Person,
                        id: *id,
                    },
                    note: NoteInput {
                        id: None,
                        kind: NoteKindDto::General,
                        text: format!("Note {i}"),
                    },
                },
                &agent,
                batch,
            )
            .unwrap();
        }
        eprintln!("{PEOPLE} agent notes in {:?}", start.elapsed());
        // Take back a tenth of the groups, so that some of them carry a revert.
        let filter = gt_store::LogFilter {
            batch: Some(batch),
            ..Default::default()
        };
        let (groups, total) = s.project().log_page(&filter, 0, 500).unwrap();
        eprintln!("log has {total} agent groups");
        for g in groups {
            s.undo_group(g.group).unwrap();
        }
    }
    let server = serve(Arc::clone(&host), 0).unwrap();
    let c = HttpClient::new(server.port());
    for (what, args) in [
        ("first page, no filter", json!({"limit": 50})),
        (
            "first page, unreviewed only",
            json!({"limit": 50, "filters": {"unreviewedOnly": true}}),
        ),
        (
            "first page, one agent",
            json!({"limit": 50, "filters": {"actorKind": "agent"}}),
        ),
    ] {
        let mut times = Vec::new();
        for _ in 0..5 {
            let t = Instant::now();
            let page = c.call("get_log", args.clone());
            times.push(t.elapsed());
            assert!(page["total"].as_u64().unwrap() >= 500);
        }
        report(what, times);
    }
    server.stop();
}
