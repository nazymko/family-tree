mod common;

use std::time::Instant;

use common::MapSource;
use gt_layout::{Metrics, Rect, descendants};

fn ten_k() -> MapSource {
    let n = 10_000u32;
    let tree: Vec<(u32, Vec<u32>)> = (0..n)
        .map(|i| (i, (3 * i + 1..=3 * i + 3).filter(|&c| c < n).collect()))
        .collect();
    MapSource::plain_tree(&tree)
}

#[test]
#[ignore = "timing test: run with --release -- --ignored"]
fn layout_10k_under_100ms_release() {
    let s = ten_k();
    let t = Instant::now();
    let l = descendants(&s, 0, 255, &Metrics::default(), 1_000_000);
    let el = t.elapsed();
    assert_eq!(l.nodes.len(), 10_000);
    assert!(el.as_millis() < 100, "took {el:?}");
}

#[test]
#[ignore = "timing test: run with --release -- --ignored"]
fn query_viewport_under_1ms() {
    let l = descendants(&ten_k(), 0, 255, &Metrics::default(), 1_000_000);
    let b = l.bounds;
    let t = Instant::now();
    let mut hits = 0usize;
    let mut seed = 12345u64;
    for _ in 0..1000 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let fx = (seed >> 33) as f32 / (1u64 << 31) as f32;
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let fy = (seed >> 33) as f32 / (1u64 << 31) as f32;
        let r = Rect {
            x: b.x + fx * (b.w - 1500.0).max(1.0),
            y: b.y + fy * (b.h - 900.0).max(1.0),
            w: 1500.0,
            h: 900.0,
        };
        let (n, e) = l.query(&r);
        hits += n.len() + e.len();
    }
    let avg = t.elapsed().as_secs_f64() * 1000.0 / 1000.0;
    assert!(hits > 0);
    assert!(avg < 1.0, "average {avg} ms");
}
