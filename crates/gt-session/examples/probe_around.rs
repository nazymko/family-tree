//! Measures the chart built around people, with all relatives and every generation, on a project folder.
//! Prints only numbers and kinds: no names, no ids.
//!
//! cargo run -p gt-session --example probe_around -- <project dir> <person id> [<person id> ...]
//!
//! `Session::open` may write (lock, migrations): run it on a COPY of the folder.

use gt_session::*;
use uuid::Uuid;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: probe_around <project dir> <person id> [<person id> ...]");
        std::process::exit(2);
    }
    let ids: Vec<Uuid> = args[1..]
        .iter()
        .map(|a| Uuid::parse_str(a).expect("person id is a uuid"))
        .collect();
    let mut s = Session::open(std::path::Path::new(&args[0]), SessionClock::system())
        .expect("open the project");
    let meta = s
        .chart_open(&ChartRequest {
            kind: ChartKind::Around,
            root: ids[0],
            generations: 255,
            aunts_uncles: None,
            kin: Some(AroundKin::All),
            roots: Some(ids.clone()),
        })
        .expect("open the chart");
    println!(
        "nodes {} bounds {:.0} x {:.0} truncated {}",
        meta.node_count, meta.bounds.w, meta.bounds.h, meta.truncated
    );
    let (q, kinds) = s.chart_quality(meta.chart_id).expect("measure");
    println!(
        "edges parent {} partner {} dup {} sibling {}",
        kinds[0], kinds[1], kinds[2], kinds[3]
    );
    println!("{q:?}");
    let all = Rect {
        x: -1e7,
        y: -1e7,
        w: 2e7,
        h: 2e7,
    };
    let w = s.chart_window(meta.chart_id, &all, 1.0).expect("window");
    let node = |id: u32| w.nodes.iter().find(|n| n.id == id).expect("node");
    // The generation axis: the one on which most connected cards are apart.
    let apart = |axis: usize| {
        w.edges
            .iter()
            .filter(|e| matches!(e.kind, EdgeKindDto::Parent))
            .filter(|e| {
                let (a, b) = (node(e.from), node(e.to));
                if axis == 0 {
                    a.x + a.w <= b.x || b.x + b.w <= a.x
                } else {
                    a.y + a.h <= b.y || b.y + b.h <= a.y
                }
            })
            .count()
    };
    let axis = usize::from(apart(1) >= apart(0));
    println!("generation axis {}", if axis == 1 { "y" } else { "x" });
    let mut offending = 0;
    for e in w
        .edges
        .iter()
        .filter(|e| !matches!(e.kind, EdgeKindDto::Partner | EdgeKindDto::Dup))
    {
        let (a, b) = (node(e.from), node(e.to));
        let (lo, hi) = if axis == 1 {
            (a.y.min(b.y), (a.y + a.h).max(b.y + b.h))
        } else {
            (a.x.min(b.x), (a.x + a.w).max(b.x + b.w))
        };
        let along: Vec<f32> = e.pts.chunks_exact(2).map(|p| p[axis]).collect();
        let (pmin, pmax) = along
            .iter()
            .fold((f32::MAX, f32::MIN), |(l, h), &v| (l.min(v), h.max(v)));
        let outside = pmin < lo - 0.5 || pmax > hi + 0.5;
        // A sibling bar sits just above its row on purpose.
        if outside && matches!(e.kind, EdgeKindDto::Parent) {
            offending += 1;
            println!(
                "offending {:?}: parent gen {} child gen {} span {:.0}..{:.0} line {:.0}..{:.0}",
                e.kind, a.generation, b.generation, lo, hi, pmin, pmax
            );
        }
    }
    println!("offending parent connectors {offending}");
    s.chart_close(meta.chart_id);
}
