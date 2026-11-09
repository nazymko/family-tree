//! How readable a laid out chart is, in plain counts: what overlaps what. No names, so the numbers can be
//! reported for any tree, and the tests can hold the layout to them.

use std::collections::HashMap;

use crate::types::{EdgeKind, Layout};

/// Lines closer than this along the same axis read as one line.
const SAME_LINE: f32 = 1.0;
/// A line this far inside a box is on top of it, not touching its edge.
const INSIDE: f32 = 0.5;
/// Buses of different families on one lane (closer than this in y) must be at least this far apart along x.
const BUS_LANE: f32 = 6.0;
const BUS_GAP: f32 = 40.0;
/// Vertical lines of different families closer than this in x, with nothing but the width of a gap between them in
/// y, read as one line.
const ALIGNED: f32 = 20.0;

/// What overlaps what in one chart. Lines of one couple's children share a bar on purpose: those don't count.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Quality {
    pub boxes: usize,
    pub lines: usize,
    /// Pairs of boxes on top of each other.
    pub box_overlaps: usize,
    /// Line segments that pass over a box that is not one of the two ends of the line.
    pub lines_through_boxes: usize,
    /// Pairs of segments of different families running on top of each other.
    pub line_overlaps: usize,
    /// Pairs of segments of different families that cross.
    pub line_crossings: usize,
    /// Pairs of horizontal bars of different families on one lane that overlap or end closer than a gap apart: they
    /// read as one bar.
    pub bus_clashes: usize,
    /// Pairs of vertical lines of different families that run within half a gap of each other in the same gap
    /// between rows, or across a partner connector from a line in the next gap: they read as one line.
    pub aligned_verticals: usize,
    /// Parent-to-child connectors with a point outside the span of the two rows they join (measured along the
    /// axis the generations run on): a bus along the edge of the chart instead of between its rows.
    pub buses_outside_rows: usize,
    /// Line points outside the box that holds every card.
    pub lines_outside_cards: usize,
    /// Total length of the lines.
    pub line_length: f32,
    /// The longest straight piece of a line.
    pub longest_segment: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy)]
struct Seg {
    a: f32,
    lo: f32,
    hi: f32,
    owner: usize,
    from: u32,
    to: u32,
    /// Part of a parent-to-child connector.
    family: bool,
    /// The card this (vertical) segment lands on / leaves, when it is the last / first of its line.
    lands: Option<u32>,
    leaves: Option<u32>,
    /// A vertical line that starts between two partner cards also counts from the top of their row.
    reach_top: Option<f32>,
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// Measures a chart. Quadratic in the worst case; meant for tests and measurements, not for drawing.
pub fn quality(l: &Layout) -> Quality {
    let mut q = Quality {
        boxes: l.nodes.len(),
        lines: l.edges.len(),
        ..Quality::default()
    };
    for n in &l.nodes {
        q.width = q.width.max(n.rect.x + n.rect.w);
        q.height = q.height.max(n.rect.y + n.rect.h);
    }
    // The edges of one family start at one point (a couple's) or end at one point (a child's), or both.
    let mut dsu: Vec<usize> = (0..l.edges.len()).collect();
    let mut seen: HashMap<(u32, u32), usize> = HashMap::new();
    for (i, e) in l.edges.iter().enumerate() {
        if !matches!(e.kind, EdgeKind::Parent | EdgeKind::Sibling) || e.pts.len() < 4 {
            continue;
        }
        let n = e.pts.len();
        for (x, y) in [(e.pts[0], e.pts[1]), (e.pts[n - 2], e.pts[n - 1])] {
            match seen.entry((x.to_bits(), y.to_bits())) {
                std::collections::hash_map::Entry::Occupied(o) => {
                    let (a, b) = (find(&mut dsu, i), find(&mut dsu, *o.get()));
                    dsu[a] = b;
                }
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(i);
                }
            }
        }
    }
    q.buses_outside_rows = buses_outside_rows(l);
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for n in &l.nodes {
        x0 = x0.min(n.rect.x);
        y0 = y0.min(n.rect.y);
        x1 = x1.max(n.rect.x + n.rect.w);
        y1 = y1.max(n.rect.y + n.rect.h);
    }
    q.lines_outside_cards = l
        .edges
        .iter()
        .filter(|e| e.kind != EdgeKind::Dup)
        .flat_map(|e| e.pts.chunks_exact(2))
        .filter(|p| p[0] < x0 - 0.5 || p[0] > x1 + 0.5 || p[1] < y0 - 0.5 || p[1] > y1 + 0.5)
        .count();
    let mut horizontal: Vec<Seg> = Vec::new();
    let mut vertical: Vec<Seg> = Vec::new();
    for (i, e) in l.edges.iter().enumerate() {
        let family = matches!(e.kind, EdgeKind::Parent | EdgeKind::Sibling) && e.pts.len() >= 2;
        let owner = if family {
            find(&mut dsu, i)
        } else {
            usize::MAX - i
        };
        let segs = (e.pts.len() / 2).saturating_sub(1);
        let reach_top = (family && e.pts.len() >= 4)
            .then(|| {
                let from = &l.nodes[e.from as usize].rect;
                (e.pts[1] < from.y + from.h - 1.0 && e.pts[1] > from.y + 1.0).then_some(from.y)
            })
            .flatten();
        for (k, p) in e.pts.windows(4).step_by(2).enumerate() {
            let (dx, dy) = ((p[2] - p[0]).abs(), (p[3] - p[1]).abs());
            q.line_length += dx + dy;
            q.longest_segment = q.longest_segment.max(dx.max(dy));
            let seg = |a: f32, s: f32, t: f32| Seg {
                a,
                lo: s.min(t),
                hi: s.max(t),
                owner,
                from: e.from,
                to: e.to,
                family,
                lands: (k + 1 == segs).then_some(e.to),
                leaves: (k == 0).then_some(e.from),
                reach_top: if k == 0 { reach_top } else { None },
            };
            if dy < 0.01 && dx >= 0.01 {
                horizontal.push(seg(p[1], p[0], p[2]));
            } else if dx < 0.01 && dy >= 0.01 {
                vertical.push(seg(p[0], p[1], p[3]));
            }
        }
    }

    let mut nodes: Vec<_> = l.nodes.iter().collect();
    nodes.sort_by(|a, b| a.rect.x.total_cmp(&b.rect.x));
    for (i, a) in nodes.iter().enumerate() {
        for b in &nodes[i + 1..] {
            if b.rect.x >= a.rect.x + a.rect.w {
                break;
            }
            if a.rect.y < b.rect.y + b.rect.h && b.rect.y < a.rect.y + a.rect.h {
                q.box_overlaps += 1;
            }
        }
    }
    for n in &l.nodes {
        let r = n.rect;
        let (x0, x1, y0, y1) = (
            r.x + INSIDE,
            r.x + r.w - INSIDE,
            r.y + INSIDE,
            r.y + r.h - INSIDE,
        );
        let clear = |s: &Seg| n.id == s.from || n.id == s.to;
        q.lines_through_boxes += horizontal
            .iter()
            .filter(|s| !clear(s) && s.a > y0 && s.a < y1 && s.hi > x0 && s.lo < x1)
            .count();
        q.lines_through_boxes += vertical
            .iter()
            .filter(|s| !clear(s) && s.a > x0 && s.a < x1 && s.hi > y0 && s.lo < y1)
            .count();
    }

    q.line_overlaps = overlaps(&mut horizontal) + overlaps(&mut vertical);
    q.bus_clashes = bus_clashes(&horizontal);
    q.aligned_verticals = aligned(&vertical);
    for h in &horizontal {
        for v in &vertical {
            if h.owner != v.owner && v.a > h.lo && v.a < h.hi && h.a > v.lo && h.a < v.hi {
                q.line_crossings += 1;
            }
        }
    }
    q
}

/// Parent connectors that leave the span of the two cards they join along the generation axis. The axis is the one
/// on which most such pairs of cards are disjoint (the rows, or for a sideways chart the columns). A bus outside the
/// span runs along the chart's edge instead of between the rows.
fn buses_outside_rows(l: &Layout) -> usize {
    let edges: Vec<_> = l
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Parent && e.pts.len() >= 4)
        .collect();
    let disjoint = |e: &&crate::types::Edge, axis: usize| {
        let (a, b) = (&l.nodes[e.from as usize].rect, &l.nodes[e.to as usize].rect);
        let (a0, a1, b0, b1) = if axis == 0 {
            (a.x, a.x + a.w, b.x, b.x + b.w)
        } else {
            (a.y, a.y + a.h, b.y, b.y + b.h)
        };
        a1 <= b0 || b1 <= a0
    };
    let down = edges.iter().filter(|e| disjoint(e, 1)).count();
    let across = edges.iter().filter(|e| disjoint(e, 0)).count();
    let axis = usize::from(down >= across);
    edges
        .iter()
        .filter(|e| {
            let (a, b) = (&l.nodes[e.from as usize].rect, &l.nodes[e.to as usize].rect);
            let (lo, hi) = if axis == 1 {
                (a.y.min(b.y), (a.y + a.h).max(b.y + b.h))
            } else {
                (a.x.min(b.x), (a.x + a.w).max(b.x + b.w))
            };
            e.pts
                .chunks_exact(2)
                .any(|p| p[axis] < lo - 0.5 || p[axis] > hi + 0.5)
        })
        .count()
}

/// Pairs of segments of different owners on the same line that share some length.
fn overlaps(segs: &mut [Seg]) -> usize {
    segs.sort_by(|p, q| p.a.total_cmp(&q.a).then(p.lo.total_cmp(&q.lo)));
    let mut count = 0;
    for (i, s) in segs.iter().enumerate() {
        for t in &segs[i + 1..] {
            if t.a - s.a >= SAME_LINE {
                break;
            }
            if s.owner != t.owner && t.lo < s.hi - SAME_LINE && s.lo < t.hi - SAME_LINE {
                count += 1;
            }
        }
    }
    count
}

/// Pairs of bars of different families on one lane that overlap or end less than [`BUS_GAP`] apart.
fn bus_clashes(horizontal: &[Seg]) -> usize {
    let mut bars: Vec<&Seg> = horizontal.iter().filter(|s| s.family).collect();
    bars.sort_by(|p, q| p.a.total_cmp(&q.a));
    let mut count = 0;
    for (i, s) in bars.iter().enumerate() {
        for t in &bars[i + 1..] {
            if t.a - s.a >= BUS_LANE {
                break;
            }
            if s.owner != t.owner && t.lo < s.hi + BUS_GAP && s.lo < t.hi + BUS_GAP {
                count += 1;
            }
        }
    }
    count
}

/// Pairs of vertical lines of different families that line up: closer than [`ALIGNED`] along x and touching along y.
/// A line that lands on a card and the line that leaves the same card are one line through it: not a pair.
fn aligned(vertical: &[Seg]) -> usize {
    let mut v: Vec<&Seg> = vertical.iter().filter(|s| s.family).collect();
    v.sort_by(|p, q| p.a.total_cmp(&q.a));
    let mut count = 0;
    for (i, s) in v.iter().enumerate() {
        for t in &v[i + 1..] {
            if t.a - s.a >= ALIGNED {
                break;
            }
            if s.owner == t.owner {
                continue;
            }
            let top = |u: &Seg| u.reach_top.map_or(u.lo, |r| r.min(u.lo));
            if top(t) > s.hi + SAME_LINE || top(s) > t.hi + SAME_LINE {
                continue;
            }
            let chain = |p: &Seg, q: &Seg| p.lands.is_some() && p.lands == q.leaves;
            if chain(s, t) || chain(t, s) {
                continue;
            }
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Edge, Rect, new_node};

    fn card(id: u32, x: f32, y: f32) -> crate::types::Node {
        new_node(
            id,
            id,
            Rect {
                x,
                y,
                w: 100.0,
                h: 50.0,
            },
            0,
        )
    }

    fn parent(from: u32, to: u32, pts: Vec<f32>) -> Edge {
        Edge {
            from,
            to,
            kind: EdgeKind::Parent,
            pts,
        }
    }

    /// Two families (0 -> 2, 3 and 1 -> 4, 5) in one gap.
    fn two_families(bus_a: (f32, f32, f32), bus_b: (f32, f32, f32), xa: f32, xb: f32) -> Layout {
        let nodes = vec![
            card(0, 0.0, 0.0),
            card(1, 400.0, 0.0),
            card(2, 0.0, 150.0),
            card(3, 100.0, 150.0),
            card(4, 400.0, 150.0),
            card(5, 500.0, 150.0),
        ];
        let line = |from, to, x: f32, bus: (f32, f32, f32), tx: f32| {
            parent(from, to, vec![x, 50.0, x, bus.0, tx, bus.0, tx, 150.0])
        };
        let edges = vec![
            line(0, 2, xa, bus_a, 50.0),
            line(0, 3, xa, bus_a, 150.0),
            line(1, 4, xb, bus_b, 450.0),
            line(1, 5, xb, bus_b, 550.0),
        ];
        Layout::finish(nodes, edges, 0, false)
    }

    #[test]
    fn buses_of_two_families_end_to_end_on_one_lane_are_flagged() {
        // 50..150 and 150..250 on one lane: touching.
        let l = two_families((100.0, 0.0, 0.0), (100.0, 0.0, 0.0), 50.0, 250.0);
        // Rebuild the second family's tail to start at x = 170 (20 apart from the first's end).
        let mut l2 = l.clone();
        for e in l2.edges.iter_mut().skip(2) {
            e.pts[4] -= 280.0;
        }
        let q = quality(&Layout::finish(
            l2.nodes.clone(),
            l2.edges.clone(),
            0,
            false,
        ));
        assert!(q.bus_clashes > 0, "{q:?}");
        // On different lanes, or far apart, they are clean.
        let apart = two_families((100.0, 0.0, 0.0), (80.0, 0.0, 0.0), 50.0, 450.0);
        assert_eq!(quality(&apart).bus_clashes, 0);
    }

    #[test]
    fn vertical_lines_of_two_families_lined_up_are_flagged() {
        let near = two_families((100.0, 0.0, 0.0), (100.0, 0.0, 0.0), 50.0, 60.0);
        assert!(quality(&near).aligned_verticals > 0);
        let far = two_families((100.0, 0.0, 0.0), (100.0, 0.0, 0.0), 50.0, 450.0);
        assert_eq!(quality(&far).aligned_verticals, 0);
    }
}
