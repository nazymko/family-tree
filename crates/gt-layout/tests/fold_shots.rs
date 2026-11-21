#![allow(clippy::unwrap_used)]
//! Writes the layouts behind the print-layout screenshots (a synthetic tree, keys only) as JSON for the picture test:
//! `SHOTS_DIR=<folder> cargo test -p gt-layout --test fold_shots -- --ignored`, then the gated "print layout
//! screenshots" test of `app/src/lib/chart/ChartPicture.test.ts` draws them.

mod common;

use std::fmt::Write as _;

use common::MapSource;
use gt_layout::{AroundBase, BranchDepth, EdgeKind, Key, Layout, Metrics, OpenSpec, around_open};

/// Person 1 with 4 generations of ancestors (Ahnentafel keys 2..31); every ancestor below the top row and person 1
/// have 12 to 20 brothers and sisters without partners or children, all opened (the fixture of `around_fold.rs`).
fn big_chart() -> (MapSource, Vec<OpenSpec>) {
    let mut s = MapSource::default();
    let mut open = Vec::new();
    for n in 1..16u32 {
        let count = 12 + (n * 7) % 9;
        let mut kids: Vec<Key> = (0..count).map(|i| 1000 + n * 100 + i).collect();
        kids.insert(kids.len() / 2, n);
        s.family(Some(2 * n), Some(2 * n + 1), &kids);
        open.push(OpenSpec {
            key: n,
            siblings: Some(BranchDepth::All),
            children: None,
        });
    }
    (s, open)
}

fn json(l: &Layout) -> String {
    let mut out = format!("{{\"root\":{},\"nodes\":[", l.root);
    for (i, n) in l.nodes.iter().enumerate() {
        let sep = if i == 0 { "" } else { "," };
        let dup = n.dup_of.map_or("null".to_string(), |d| d.to_string());
        let _ = write!(
            out,
            "{sep}{{\"id\":{},\"key\":{},\"x\":{},\"y\":{},\"w\":{},\"h\":{},\"gen\":{},\"dupOf\":{dup}}}",
            n.id, n.key, n.rect.x, n.rect.y, n.rect.w, n.rect.h, n.r#gen
        );
    }
    out.push_str("],\"edges\":[");
    for (i, e) in l.edges.iter().enumerate() {
        let sep = if i == 0 { "" } else { "," };
        let kind = match e.kind {
            EdgeKind::Parent => "parent",
            EdgeKind::Partner => "partner",
            EdgeKind::Sibling => "sibling",
            EdgeKind::Dup => "dup",
        };
        let pts: Vec<String> = e.pts.iter().map(f32::to_string).collect();
        let _ = write!(
            out,
            "{sep}{{\"from\":{},\"to\":{},\"kind\":\"{kind}\",\"pts\":[{}]}}",
            e.from,
            e.to,
            pts.join(",")
        );
    }
    out.push_str("]}");
    out
}

#[test]
#[ignore = "writes screenshot data: SHOTS_DIR=<folder> ... -- --ignored"]
fn write_fold_layouts() {
    let Ok(dir) = std::env::var("SHOTS_DIR") else {
        return;
    };
    let (s, open) = big_chart();
    let screen = Metrics::default();
    let print = Metrics::compact();
    let lay = |fold, m: &Metrics| around_open(&s, 1, 4, AroundBase::Line, &open, fold, m, 100_000);
    let today = lay(false, &screen);
    let compact = lay(true, &screen);
    assert_ne!(today.bounds, compact.bounds, "the fixture folds");
    for (name, l) in [
        ("fold-off", today),
        ("fold-on", compact),
        ("fold-print", lay(true, &print)),
    ] {
        std::fs::write(format!("{dir}/{name}.json"), json(&l)).unwrap();
    }
}
