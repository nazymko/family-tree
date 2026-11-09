//! A big married-couple chart (several ancestor generations with their brothers and sisters, aunts and uncles with
//! families, cousins, siblings, children, grandchildren, one-parent families, sibling groups of unknown parents):
//! every parent-to-child connector must run between the rows it joins, never along the edge of the chart.

mod common;

use common::MapSource;
use gt_layout::{Direction, Key, Layout, ManyKin, Metrics, around_many_kin, quality};

struct Gen {
    s: MapSource,
    next: Key,
}

impl Gen {
    fn id(&mut self) -> Key {
        self.next += 1;
        self.next
    }

    /// `p` gets a partner (unless `lone`) and `kids` children.
    fn family(&mut self, p: Key, lone: bool, kids: usize) -> (Option<Key>, Vec<Key>) {
        let partner = (!lone).then(|| self.id());
        let ks: Vec<Key> = (0..kids).map(|_| self.id()).collect();
        self.s.family(Some(p), partner, &ks);
        (partner, ks)
    }

    /// Descendants of `p`: families `depth` generations down, some with one known parent.
    fn down(&mut self, p: Key, depth: u8, seed: u32) {
        if depth == 0 {
            return;
        }
        let (_, ks) = self.family(p, seed % 3 == 0, 1 + (seed as usize % 2));
        for (i, k) in ks.into_iter().enumerate() {
            self.down(k, depth - 1, seed + 1 + i as u32);
        }
    }

    /// The ancestors of `p`, `depth` generations up. Every couple has brothers and sisters of the child, with
    /// families.
    fn up(&mut self, p: Key, depth: u8, seed: u32) {
        if depth == 0 {
            return;
        }
        let (f, m) = (self.id(), self.id());
        let sibs: Vec<Key> = (0..(seed % 3)).map(|_| self.id()).collect();
        let mut kids = vec![p];
        kids.extend(&sibs);
        // One parent unknown in some families.
        let m = (seed % 4 != 1).then_some(m);
        self.s.family(Some(f), m, &kids);
        for (i, &sb) in sibs.iter().enumerate() {
            self.down(sb, 2, seed + i as u32);
        }
        self.up(f, depth - 1, seed + 1);
        if let Some(m) = m {
            self.up(m, depth - 1, seed + 2);
        }
    }
}

/// The chosen couple (1, 2) with ancestors and relatives.
fn world(depth: u8, orphan_groups: bool) -> (MapSource, Vec<Key>) {
    let mut g = Gen {
        s: MapSource::default(),
        next: 2,
    };
    g.up(1, depth, 2);
    g.up(2, depth, 5);
    let kids = [g.next + 1, g.next + 2, g.next + 3];
    g.s.family(Some(1), Some(2), &kids);
    g.next += 3;
    for (i, k) in kids.into_iter().enumerate() {
        g.down(k, 2, i as u32);
    }
    if orphan_groups {
        // Brothers and sisters whose parents are unknown.
        let (a, b, c) = (g.id(), g.id(), g.id());
        g.s.family(None, None, &[a, b, c]);
        g.down(a, 2, 1);
        g.down(b, 2, 4);
    }
    (g.s, vec![1, 2])
}

fn check(s: &MapSource, chosen: &[Key], gens: u8, to: Direction, kin: ManyKin, what: &str) {
    let base = Direction::Top;
    let m = Metrics::default().for_direction(base, to);
    let mut l: Layout = around_many_kin(s, chosen, gens, kin, &m, 100_000);
    l.orient(base, to);
    assert!(!l.nodes.is_empty(), "{what}");
    let q = quality(&l);
    if std::env::var("DUMP").is_ok() && (q.buses_outside_rows, q.lines_outside_cards) != (0, 0) {
        for e in &l.edges {
            let (a, b) = (l.nodes[e.from as usize].rect, l.nodes[e.to as usize].rect);
            eprintln!("{:?} from y{} to y{} pts {:?}", e.kind, a.y, b.y, e.pts);
        }
    }
    assert_eq!(
        (q.buses_outside_rows, q.lines_outside_cards),
        (0, 0),
        "{what}: {q:?}"
    );
}

#[test]
fn no_family_bus_runs_along_the_chart_edge() {
    for depth in [2u8, 3, 4] {
        for orphans in [false, true] {
            let (s, chosen) = world(depth, orphans);
            for order in [chosen.clone(), chosen.iter().rev().copied().collect()] {
                for gens in [2u8, 3, 4, 255] {
                    for kin in [ManyKin::Line, ManyKin::Siblings, ManyKin::All] {
                        for to in [
                            Direction::Top,
                            Direction::Bottom,
                            Direction::Left,
                            Direction::Right,
                        ] {
                            let what = format!(
                                "depth {depth} orphans {orphans} order {order:?} gens {gens} {kin:?} {to:?}"
                            );
                            check(&s, &order, gens, to, kin, &what);
                        }
                    }
                }
            }
        }
    }
}

/// `DUMP_JSON=<file> cargo test -p gt-layout --test top_bus dump -- --ignored`: the layout of the big synthetic chart as
/// JSON (nodes as x, y, w, h; edges as kind and points), for drawing.
#[test]
#[ignore]
fn dump() {
    let Ok(path) = std::env::var("DUMP_JSON") else {
        return;
    };
    let (s, chosen) = world(4, true);
    let l = around_many_kin(&s, &chosen, 255, ManyKin::All, &Metrics::default(), 100_000);
    let nodes: Vec<String> = l
        .nodes
        .iter()
        .map(|n| format!("[{},{},{},{}]", n.rect.x, n.rect.y, n.rect.w, n.rect.h))
        .collect();
    let edges: Vec<String> = l
        .edges
        .iter()
        .map(|e| format!("[\"{:?}\",{:?}]", e.kind, e.pts))
        .collect();
    std::fs::write(
        path,
        format!(
            "{{\"nodes\":[{}],\"edges\":[{}]}}",
            nodes.join(","),
            edges.join(",")
        ),
    )
    .unwrap();
}
