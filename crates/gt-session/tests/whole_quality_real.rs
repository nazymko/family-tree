#![allow(clippy::unwrap_used)]
//! Measures the whole-tree chart of a real file, read through a temporary copy. Prints counts only, never a name.
//!
//! `GT_REAL_TREE=<file> cargo test -p gt-session --test whole_quality_real -- --ignored --nocapture`
mod common;

use common::*;
use gt_layout::{Metrics, quality, whole};

#[test]
#[ignore = "needs a real tree file in GT_REAL_TREE"]
fn measure_a_real_tree() {
    let path = std::env::var("GT_REAL_TREE").unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let copy = scratch.path().join(format!("copy{}", extension(&path)));
    std::fs::copy(&path, &copy).unwrap();
    let format = gt_import::detect(&copy).unwrap();
    let converted = gt_import::convert(format, &copy, &gt_gedcom::ImportOptions::default())
        .unwrap_or_else(|e| panic!("cannot read the file: {}", e.message));
    let (_dir, s) = open_tree(&converted.outcome.tree);
    let input = s.whole_input();
    let w = whole(&input, &Metrics::default());
    println!("families: {}", input.families.len());
    let two: usize = input
        .families
        .iter()
        .filter(|f| f.partners.iter().flatten().count() == 2)
        .count();
    println!("couples: {two}");
    println!("blocks: {}", w.extras.blocks.len());
    let mut sizes: Vec<u32> = w.extras.blocks.iter().map(|b| b.people).collect();
    sizes.truncate(8);
    println!("largest blocks: {sizes:?}");
    println!(
        "widest block: {:.0}, tallest: {:.0}",
        w.extras.blocks.iter().map(|b| b.rect.w).fold(0.0, f32::max),
        w.extras.blocks.iter().map(|b| b.body_h).fold(0.0, f32::max)
    );
    println!("{:#?}", quality(&w.layout));
    if let Ok(out) = std::env::var("GT_SHAPE_SVG") {
        std::fs::write(out, shape_svg(&w.layout)).unwrap();
    }
}

/// The chart as blank boxes and lines: its shape, with nothing that identifies anybody.
fn shape_svg(l: &gt_layout::Layout) -> String {
    use std::fmt::Write;
    let q = quality(l);
    let mut s = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='-20 -20 {} {}' width='{}' height='{}' style='background:#fff'>",
        q.width + 40.0,
        q.height + 40.0,
        q.width + 40.0,
        q.height + 40.0
    );
    for n in &l.nodes {
        let r = n.rect;
        write!(
            s,
            "<rect x='{}' y='{}' width='{}' height='{}' fill='#eef' stroke='#669'/>",
            r.x, r.y, r.w, r.h
        )
        .unwrap();
    }
    for e in &l.edges {
        let pts: Vec<String> = e.pts.iter().map(f32::to_string).collect();
        let colour = if e.kind == gt_layout::EdgeKind::Partner {
            "#c33"
        } else {
            "#333"
        };
        write!(
            s,
            "<polyline fill='none' stroke='{colour}' points='{}'/>",
            pts.join(" ")
        )
        .unwrap();
    }
    s.push_str("</svg>");
    s
}

fn extension(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .map_or_else(String::new, |e| format!(".{}", e.to_string_lossy()))
}
