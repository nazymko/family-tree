#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Performance budgets (spec 7.4) on a synthetic 50k-person project, plus a hand-made 8-generation
//! pedigree so the chart budget measures a real ancestry. Run with
//! `cargo test -p gt-session --release --test budget -- --ignored --nocapture`.

use gt_core::living::{LivingRule, compute_living};
use gt_gedcom::synthetic::synthetic_tree;
use gt_gedcom::{ImportOptions, import};
use gt_session::*;
use gt_store::Project;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use uuid::Uuid;

const TODAY: i32 = 2_461_343; // 2026-11-03
const NOW_MS: i64 = 1_793_664_000_000;
const DEEP_GENERATIONS: u32 = 8;

/// A full binary ancestry: person `n` has parents `2n` and `2n+1`.
fn deep_pedigree() -> gt_core::tree::Tree {
    let mut ged = String::from("0 HEAD\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n");
    let last = (1u32 << DEEP_GENERATIONS) - 1;
    for n in 1..=last {
        let sex = if n % 2 == 0 { "M" } else { "F" };
        let depth = 31 - n.leading_zeros();
        writeln!(
            ged,
            "0 @I{n}@ INDI\n1 NAME Deep{n} /Pedigree/\n1 SEX {sex}\n1 BIRT\n2 DATE {}",
            1900 - 25 * depth
        )
        .unwrap();
    }
    for n in 1..(1u32 << (DEEP_GENERATIONS - 1)) {
        writeln!(
            ged,
            "0 @F{n}@ FAM\n1 HUSB @I{}@\n1 WIFE @I{}@\n1 CHIL @I{n}@",
            2 * n,
            2 * n + 1
        )
        .unwrap();
    }
    ged.push_str("0 TRLR\n");
    import(ged.as_bytes(), &ImportOptions::default()).tree
}

fn build(dir: &Path) {
    let mut tree = synthetic_tree(50_000, 15_000);
    let deep = deep_pedigree();
    tree.persons.extend(deep.persons);
    tree.families.extend(deep.families);
    tree.events.extend(deep.events);
    let mut p = Project::create(dir).unwrap();
    p.save_tree(&tree, TODAY).unwrap();
}

fn open(dir: &Path) -> (Session, Duration) {
    let t = Instant::now();
    let s = Session::open(dir, SessionClock::fixed(TODAY, NOW_MS)).unwrap();
    (s, t.elapsed())
}

/// One 50k project shared by the budget tests (building it takes seconds).
fn fixture() -> PathBuf {
    static DIR: OnceLock<(tempfile::TempDir, PathBuf)> = OnceLock::new();
    DIR.get_or_init(|| {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("big");
        build(&dir);
        (tmp, dir)
    })
    .1
    .clone()
}

fn deep_root(s: &Session) -> Uuid {
    let q = PersonQuery {
        text: "Deep1 ".into(),
        ..PersonQuery::default()
    };
    s.list_persons(&q, 0, 200)
        .items
        .iter()
        .find(|r| r.person.name.starts_with("Deep1 "))
        .expect("deep root")
        .person
        .id
}

/// Budgets are timings and a memory reading of one process: the tests take turns, so a parallel
/// `cargo test` neither adds CPU contention nor stacks several 50k sessions in memory.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Median of `n` timed runs, so one scheduler hiccup does not fail a budget.
fn median(n: usize, mut f: impl FnMut() -> Duration) -> Duration {
    let mut v: Vec<Duration> = (0..n).map(|_| f()).collect();
    v.sort();
    v[n / 2]
}

#[test]
#[ignore = "release-only budget check"]
fn open_under_2s() {
    let _turn = serial();
    let (s, opened) = open(&fixture());
    eprintln!("BUDGET open: {opened:?} (< 2s)");
    assert!(s.info().person_count >= 50_000);
    assert!(opened < Duration::from_secs(2), "open took {opened:?}");
}

#[test]
#[ignore = "release-only budget check"]
fn list_page_under_30ms() {
    let _turn = serial();
    let (s, _) = open(&fixture());
    let q = PersonQuery::default();
    let _ = s.list_persons(&q, 0, 200); // builds the order cache
    let took = median(9, || {
        let t = Instant::now();
        let page = s.list_persons(&q, 20_000, 200);
        let d = t.elapsed();
        assert_eq!(page.items.len(), 200);
        d
    });
    eprintln!("BUDGET list page: {took:?} (< 30ms)");
    assert!(took < Duration::from_millis(30), "list page took {took:?}");
}

#[test]
#[ignore = "release-only budget check"]
fn search_under_100ms() {
    let _turn = serial();
    let (s, _) = open(&fixture());
    let took = median(9, || {
        let t = Instant::now();
        let hits = s.search("Anna", 50).unwrap();
        let d = t.elapsed();
        assert!(!hits.is_empty());
        d
    });
    eprintln!("BUDGET search: {took:?} (< 100ms)");
    assert!(took < Duration::from_millis(100), "search took {took:?}");
}

#[test]
#[ignore = "release-only budget check"]
fn first_sort_build_under_300ms() {
    let _turn = serial();
    let (s, _) = open(&fixture());
    let t = Instant::now();
    let first = s.list_persons(&PersonQuery::default(), 0, 200);
    let took = t.elapsed();
    eprintln!("BUDGET first sort build: {took:?} (< 300ms)");
    assert_eq!(first.items.len(), 200);
    assert!(
        took < Duration::from_millis(300),
        "first sort took {took:?}"
    );
}

#[test]
#[ignore = "release-only budget check"]
fn place_suggest_under_5ms() {
    let _turn = serial();
    let (mut s, _) = open(&fixture());
    let t = Instant::now();
    s.rebuild();
    let rebuilt = t.elapsed();
    let t = Instant::now();
    let idx = gt_session::lookup::PlaceIndex::build(s.tree());
    let built = t.elapsed();
    drop(idx);
    let prefixes = ["a", "ber", "ham", "s", "new", "", "zzz", "st"];
    let t = Instant::now();
    let mut n = 0;
    for _ in 0..25 {
        for p in prefixes {
            n += s.suggest_places(p, 20).len();
        }
    }
    let avg = t.elapsed() / (25 * prefixes.len() as u32);
    eprintln!(
        "place index build {built:?}, full rebuild {rebuilt:?}, avg suggest {avg:?} ({n} results)"
    );
    assert!(built < Duration::from_millis(100), "index build {built:?}");
    assert!(avg < Duration::from_millis(5), "suggest avg {avg:?}");
}

#[test]
#[ignore = "release-only budget check"]
fn apply_under_50ms() {
    let _turn = serial();
    let (mut s, _) = open(&fixture());
    let anchor = deep_root(&s);
    let (mut applies, mut undos, mut redos) = (Vec::new(), Vec::new(), Vec::new());
    for i in 0..15 {
        let t = Instant::now();
        s.apply(EditCommand::QuickAdd {
            anchor,
            relation: Relation::Child,
            draft: PersonDraft {
                given: Some(format!("Added{i}")),
                surnames: vec!["Budget".into()],
                patronymic: None,
                sex: SexKind::Unknown,
                birth_date_raw: Some("abt 1850".into()),
                birth_place_text: Some("Berlin, Germany".into()),
            },
            family: None,
            new_parent_set: None,
            parent_rel: None,
        })
        .unwrap();
        applies.push(t.elapsed());
        let t = Instant::now();
        s.undo().unwrap();
        undos.push(t.elapsed());
        let t = Instant::now();
        s.redo().unwrap();
        redos.push(t.elapsed());
    }
    s.verify_derived_state().unwrap();
    let mut all: Vec<Duration> = [applies.clone(), undos.clone(), redos.clone()].concat();
    all.sort();
    let med = |v: &mut Vec<Duration>| {
        v.sort();
        v[v.len() / 2]
    };
    let (ma, mu, mr) = (med(&mut applies), med(&mut undos), med(&mut redos));
    let worst = *all.last().unwrap();
    let p90 = all[all.len() * 9 / 10];
    eprintln!(
        "BUDGET apply: median apply {ma:?}, undo {mu:?}, redo {mr:?}, p90 of all {p90:?} (< 100ms), worst {worst:?} (median < 50ms)"
    );
    assert!(ma < Duration::from_millis(50), "apply median {ma:?}");
    assert!(mu < Duration::from_millis(50), "undo median {mu:?}");
    assert!(mr < Duration::from_millis(50), "redo median {mr:?}");
    // Secondary bound for the occasional SQLite commit stall.
    assert!(p90 < Duration::from_millis(100), "p90 {p90:?}");
}

#[test]
#[ignore = "release-only budget check"]
fn chart_window_under_8ms() {
    let _turn = serial();
    let (mut s, _) = open(&fixture());
    let root = deep_root(&s);
    let meta = s
        .chart_open(&ChartRequest {
            kind: ChartKind::Pedigree,
            root,
            generations: DEEP_GENERATIONS,
            aunts_uncles: None,
        })
        .unwrap();
    let rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 1920.0,
        h: 1080.0,
    };
    let took = median(25, || {
        let t = Instant::now();
        let w = s.chart_window(meta.chart_id, &rect, 1.0).unwrap();
        let d = t.elapsed();
        assert!(!w.nodes.is_empty());
        d
    });
    eprintln!(
        "BUDGET chart window: {took:?} (< 8ms), {} nodes in chart",
        meta.node_count
    );
    assert!(took < Duration::from_millis(8), "chart window {took:?}");
}

#[test]
#[ignore = "release-only budget check"]
fn chart_open_pedigree_8_gens_under_50ms() {
    let _turn = serial();
    let (mut s, _) = open(&fixture());
    let root = deep_root(&s);
    let req = ChartRequest {
        kind: ChartKind::Pedigree,
        root,
        generations: DEEP_GENERATIONS,
        aunts_uncles: None,
    };
    let mut nodes = 0;
    let took = median(9, || {
        let t = Instant::now();
        let m = s.chart_open(&req).unwrap();
        let d = t.elapsed();
        nodes = m.node_count;
        d
    });
    eprintln!("BUDGET chart open: {took:?} (< 50ms), {nodes} nodes");
    assert!(nodes >= 200, "pedigree has only {nodes} nodes");
    assert!(took < Duration::from_millis(50), "chart open {took:?}");
}

/// The workspace forbids `unsafe`, so `GetProcessMemoryInfo` is out; `tasklist` reports the same
/// working set ("Mem Usage", in KB).
#[cfg(windows)]
fn resident_bytes() -> Option<u64> {
    let out = std::process::Command::new("tasklist")
        .args([
            "/FI",
            &format!("PID eq {}", std::process::id()),
            "/FO",
            "CSV",
            "/NH",
        ])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let mem = text.trim().rsplit("\",\"").next()?;
    let kb: u64 = mem
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()?;
    Some(kb * 1024)
}

#[cfg(target_os = "linux")]
fn resident_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kb: u64 = status
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))?
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse()
        .ok()?;
    Some(kb * 1024)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn resident_bytes() -> Option<u64> {
    None
}

#[test]
#[ignore = "release-only budget check"]
fn resident_memory_under_500mb() {
    let _turn = serial();
    let (s, _) = open(&fixture());
    let _ = s.list_persons(&PersonQuery::default(), 0, 200);
    #[cfg(any(windows, target_os = "linux"))]
    let bytes = resident_bytes().expect("could not read the process memory on a supported OS");
    #[cfg(not(any(windows, target_os = "linux")))]
    let Some(bytes) = resident_bytes() else {
        eprintln!("BUDGET memory: not measured on this OS");
        return;
    };
    let mb = bytes / (1024 * 1024);
    eprintln!("BUDGET memory: {mb} MB resident (< 500 MB), process includes the fixture builder");
    assert!(mb < 500, "resident {mb} MB");
}

#[test]
#[ignore = "release-only budget check"]
fn living_recompute_under_30ms() {
    let _turn = serial();
    let (s, _) = open(&fixture());
    let rule = LivingRule::default();
    let took = median(9, || {
        let t = Instant::now();
        let m = compute_living(s.tree(), &rule, TODAY);
        let d = t.elapsed();
        assert!(m.len() >= 50_000);
        d
    });
    eprintln!("BUDGET living recompute: {took:?} (< 30ms)");
    assert!(
        took < Duration::from_millis(30),
        "living recompute {took:?}"
    );
}
