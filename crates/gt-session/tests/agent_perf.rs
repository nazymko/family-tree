#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! What one agent write costs the open views, on a tree with a long agent change log. Measurement only:
//! `cargo test -p gt-session --test agent_perf -- --ignored --nocapture` (debug build unless `--release`).

mod common;

use common::*;
use gt_gedcom::synthetic::synthetic_tree;
use gt_session::*;
use gt_store::{Actor, Batch, Project};
use std::time::{Duration, Instant};
use uuid::Uuid;

fn agent() -> Actor {
    Actor::Agent {
        id: Uuid::from_u128(7),
        name: "Transcript reader".into(),
    }
}

fn note(id: Uuid, text: &str) -> EditCommand {
    EditCommand::SaveNote {
        target: OwnerRef {
            kind: OwnerKind::Person,
            id,
        },
        note: NoteInput {
            id: None,
            kind: NoteKindDto::General,
            text: text.into(),
        },
    }
}

fn birth(owner: Uuid, raw: &str) -> EditCommand {
    EditCommand::SaveEvent {
        owner: EventOwner {
            kind: EventOwnerKind::Person,
            id: owner,
        },
        event: EventInput {
            id: None,
            kind: "baptism".into(),
            date_raw: Some(raw.into()),
            place_text: None,
            place_id: None,
            description: None,
            age: None,
            cause: None,
            negative: false,
            private: false,
            preferred: true,
            participants: None,
        },
    }
}

fn med<T>(n: usize, mut f: impl FnMut() -> T) -> Duration {
    let mut v: Vec<Duration> = (0..n)
        .map(|_| {
            let t = Instant::now();
            let _ = f();
            t.elapsed()
        })
        .collect();
    v.sort();
    v[v.len() / 2]
}

#[allow(clippy::vec_init_then_push, clippy::explicit_counter_loop)]
fn run(people: usize, families: usize, writes: usize, notes: usize) {
    let tree = synthetic_tree(people, families);
    let ids: Vec<Uuid> = tree.persons.keys().copied().collect();
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("project");
    let mut p = Project::create(&proj).unwrap();
    p.save_tree(&tree, today()).unwrap();
    let batch = Uuid::from_u128(99);
    p.batch_create(&Batch {
        id: batch,
        title: "Recording".into(),
        source_file: None,
        actor_id: Uuid::from_u128(7),
        actor_name: "Transcript reader".into(),
        started: NOW_MS,
        ended: None,
        report: None,
    })
    .unwrap();
    drop(p);
    let mut s = Session::open(&proj, SessionClock::fixed(today(), NOW_MS)).unwrap();
    let t = Instant::now();
    for i in 0..writes {
        let who = ids[i % ids.len().min(people)];
        let cmd = if i < notes {
            note(who, "A line from the recording")
        } else {
            birth(who, &format!("{}", 1800 + i % 100))
        };
        s.commit_command_as(cmd, agent(), Some(batch)).unwrap();
    }
    let all = s.log(LogFilterDto::default(), 0, 200).unwrap();
    println!(
        "== {people} people, {writes} agent writes ({} log groups), built in {:?}",
        all.total,
        t.elapsed()
    );
    // The person with the most agent changes.
    let busiest = ids[0];
    let mut table: Vec<(&str, Duration)> = Vec::new();
    table.push((
        "unreviewed_people",
        med(7, || s.unreviewed_people().unwrap()),
    ));
    table.push((
        "person_agent_changes (busiest)",
        med(7, || s.person_agent_changes(busiest).unwrap()),
    ));
    table.push((
        "change_log first page (25)",
        med(7, || s.log(LogFilterDto::default(), 0, 25).unwrap()),
    ));
    table.push(("change_batches", med(7, || s.batches().unwrap())));
    // A write bumps the revision, so the whole chart is laid out again on the next open.
    let mut k = 0usize;
    let mut open_after_write = Vec::new();
    for _ in 0..3 {
        s.commit_command_as(note(ids[k % ids.len()], "x"), agent(), Some(batch))
            .unwrap();
        k += 1;
        let t = Instant::now();
        let _ = s.chart_open_whole().unwrap();
        open_after_write.push(t.elapsed());
    }
    open_after_write.sort();
    table.push(("chart_open_whole after a write", open_after_write[1]));
    table.push((
        "chart_open_whole cached",
        med(7, || s.chart_open_whole().unwrap()),
    ));
    let who = ids[0];
    table.push((
        "chart_open descendants (6 gen)",
        med(3, || {
            s.chart_open(&ChartRequest {
                kind: ChartKind::Descendants,
                root: who,
                generations: 6,
                aunts_uncles: None,
            })
            .unwrap()
        }),
    ));
    let sum: Duration = table
        .iter()
        .filter(|(n, _)| {
            matches!(
                *n,
                "unreviewed_people"
                    | "person_agent_changes (busiest)"
                    | "change_log first page (25)"
                    | "change_batches"
                    | "chart_open_whole after a write"
            )
        })
        .map(|(_, d)| *d)
        .sum();
    for (n, d) in &table {
        println!("{n:<36} {d:>12?}");
    }
    println!("{:<36} {sum:>12?}", "sum per agent event (one of each)");
}

#[test]
#[ignore = "measurement"]
fn small_tree_with_a_long_agent_log() {
    run(85, 30, 220, 46);
}

#[test]
#[ignore = "measurement"]
fn five_thousand_people() {
    run(5_000, 1_800, 3_000, 700);
}

/// The views that follow an agent's writes must not scan the whole log once per group: a long log of
/// groups, none undone, is read in a fraction of a second even in a debug build.
#[test]
fn reading_who_is_unreviewed_and_the_batches_does_not_scan_the_log_per_group() {
    let tree = synthetic_tree(85, 30);
    let ids: Vec<Uuid> = tree.persons.keys().copied().collect();
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("project");
    let mut p = Project::create(&proj).unwrap();
    p.save_tree(&tree, today()).unwrap();
    let batch = Uuid::from_u128(99);
    p.batch_create(&Batch {
        id: batch,
        title: "Recording".into(),
        source_file: None,
        actor_id: Uuid::from_u128(7),
        actor_name: "Transcript reader".into(),
        started: NOW_MS,
        ended: None,
        report: None,
    })
    .unwrap();
    drop(p);
    let mut s = Session::open(&proj, SessionClock::fixed(today(), NOW_MS)).unwrap();
    for i in 0..1500 {
        s.commit_command_as(note(ids[i % ids.len()], "A line"), agent(), Some(batch))
            .unwrap();
    }
    let t = Instant::now();
    let unreviewed = s.unreviewed_people().unwrap();
    let a = t.elapsed();
    let t = Instant::now();
    let batches = s.batches().unwrap();
    let b = t.elapsed();
    assert_eq!(unreviewed.len(), ids.len());
    assert_eq!((batches[0].groups, batches[0].standing), (1500, 1500));
    assert_eq!(batches[0].unreviewed as usize, ids.len());
    assert!(
        a < Duration::from_millis(400),
        "unreviewed_people took {a:?}"
    );
    assert!(b < Duration::from_millis(400), "batches took {b:?}");
}
