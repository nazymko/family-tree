#![allow(clippy::unwrap_used)]
use gt_core::DayOrder;
use gt_core::date::GedcomVersion;
use gt_gedcom::synthetic::synthetic_tree;
use gt_gedcom::{ExportOptions, ImportOptions, export, import};
use std::time::Instant;

/// Spike gate (spec 4.3): importing 50,000 persons / 20,000 families takes under 10 s.
/// Run with `cargo test -p gt-gedcom --release --test perf -- --ignored`.
#[test]
#[ignore]
fn import_50k_under_ten_seconds() {
    let tree = synthetic_tree(50_000, 20_000);
    assert_eq!(tree.persons.len(), 50_000);
    assert_eq!(tree.families.len(), 20_000);
    let t0 = Instant::now();
    let bytes = export(
        &tree,
        &ExportOptions {
            version: GedcomVersion::V551,
        },
    );
    let export_time = t0.elapsed();
    let t1 = Instant::now();
    let out = import(
        &bytes,
        &ImportOptions {
            day_order: DayOrder::Dmy,
            media_roots: vec![],
            ..Default::default()
        },
    );
    let import_time = t1.elapsed();
    eprintln!(
        "export {export_time:?}, import {import_time:?}, {} bytes",
        bytes.len()
    );
    assert_eq!(out.tree.persons.len(), 50_000);
    assert_eq!(out.tree.families.len(), 20_000);
    assert!(
        out.report.repairs.is_empty(),
        "{:?}",
        out.report.repairs.first()
    );
    assert!(
        import_time.as_secs_f64() < 10.0,
        "import took {import_time:?}"
    );
}
