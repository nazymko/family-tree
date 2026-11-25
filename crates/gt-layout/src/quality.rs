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
    /// Total length of the horizontal pieces of parent-to-child connectors (buses and detours).
    pub horizontal_connectors: f32,
    /// The longest single horizontal piece of a parent-to-child connector.
    pub longest_horizontal_connector: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy)]
struct Seg {
    a: f32,
    lo: f32,
    hi: f32,
    owner: usize,
    /// The index of the edge this segment is part of.
    edge: usize,
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

/// Measures a chart. Close to linear for a laid out chart (lines and cards are looked up by band and position); the
/// fold of the compact print layout measures every chart it tries with it.
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
    for e in &l.edges {
        let family = matches!(e.kind, EdgeKind::Parent | EdgeKind::Sibling) && e.pts.len() >= 2;
        for p in e.pts.windows(4).step_by(2) {
            let (dx, dy) = ((p[2] - p[0]).abs(), (p[3] - p[1]).abs());
            q.line_length += dx + dy;
            q.longest_segment = q.longest_segment.max(dx.max(dy));
            if family && dy < 0.01 {
                q.horizontal_connectors += dx;
                q.longest_horizontal_connector = q.longest_horizontal_connector.max(dx);
            }
        }
    }
    let (mut horizontal, mut vertical) = segments(l);
    box_overlaps(l, |_, _| q.box_overlaps += 1);
    through_boxes(l, &horizontal, &vertical, |_, _| q.lines_through_boxes += 1);

    q.line_overlaps = overlaps(&mut horizontal) + overlaps(&mut vertical);
    q.bus_clashes = bus_clashes(&horizontal);
    q.aligned_verticals = aligned(&vertical);
    crossings(&horizontal, &vertical, |_, _| q.line_crossings += 1);
    q
}

/// The straight pieces of every line, horizontal and vertical, each with the family it belongs to.
fn segments(l: &Layout) -> (Vec<Seg>, Vec<Seg>) {
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
            let seg = |a: f32, s: f32, t: f32| Seg {
                a,
                lo: s.min(t),
                hi: s.max(t),
                owner,
                edge: i,
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

    (horizontal, vertical)
}

/// Pairs of cards on top of each other: `hit(a, b)` with their indices.
fn box_overlaps(l: &Layout, mut hit: impl FnMut(usize, usize)) {
    let mut nodes: Vec<usize> = (0..l.nodes.len()).collect();
    nodes.sort_by(|&a, &b| l.nodes[a].rect.x.total_cmp(&l.nodes[b].rect.x));
    for (i, &ia) in nodes.iter().enumerate() {
        let a = &l.nodes[ia].rect;
        for &ib in &nodes[i + 1..] {
            let b = &l.nodes[ib].rect;
            if b.x >= a.x + a.w {
                break;
            }
            if a.y < b.y + b.h && b.y < a.y + a.h {
                hit(ia, ib);
            }
        }
    }
}

/// How much trouble each card is in, as (cards on top of it or lines through it, plus cards under its own lines;
/// crossings of its own lines). Its own lines are the lines that start or end at it. The fold of the compact print
/// layout compares a folded run's cards with the same people in the unfolded chart, to tell which grids made a
/// chart worse. Also the crossings of every line.
pub(crate) fn trouble(l: &Layout) -> (Vec<(usize, usize)>, Vec<usize>) {
    let (horizontal, vertical) = segments(l);
    let mut node = vec![(0usize, 0usize); l.nodes.len()];
    let mut edge = vec![(0usize, 0usize); l.edges.len()];
    box_overlaps(l, |a, b| {
        node[a].0 += 1;
        node[b].0 += 1;
    });
    through_boxes(l, &horizontal, &vertical, |n, s| {
        node[n].0 += 1;
        edge[s.edge].0 += 1;
    });
    crossings(&horizontal, &vertical, |h, v| {
        edge[h.edge].1 += 1;
        edge[v.edge].1 += 1;
    });
    for (e, &c) in l.edges.iter().zip(&edge) {
        for end in [e.from, e.to] {
            if let Some(n) = node.get_mut(end as usize) {
                n.0 += c.0;
                n.1 += c.1;
            }
        }
    }
    (node, edge.iter().map(|e| e.1).collect())
}

/// The height of one band of [`Bands`].
const BAND: f32 = 64.0;

/// Items indexed by horizontal bands of the chart: an item is in every band its `lo..hi` (along y) reaches, sorted
/// along x within a band. A line at one y meets only the items of that y's band.
struct Bands<T> {
    y0: f32,
    bands: Vec<Vec<(f32, T)>>,
}

impl<T: Copy> Bands<T> {
    fn new(items: impl Iterator<Item = (f32, f32, f32, T)> + Clone) -> Self {
        let y0 = items.clone().map(|i| i.1).fold(f32::INFINITY, f32::min);
        let y1 = items.clone().map(|i| i.2).fold(f32::NEG_INFINITY, f32::max);
        let n = if y0.is_finite() && y1.is_finite() {
            (((y1 - y0) / BAND) as usize + 1).min(1 << 20)
        } else {
            0
        };
        let mut b = Bands {
            y0,
            bands: (0..n).map(|_| Vec::new()).collect(),
        };
        for (x, lo, hi, t) in items {
            let (Some(i), Some(j)) = (b.band(lo), b.band(hi)) else {
                continue;
            };
            for band in &mut b.bands[i..=j] {
                band.push((x, t));
            }
        }
        for band in &mut b.bands {
            band.sort_by(|p, q| p.0.total_cmp(&q.0));
        }
        b
    }

    fn band(&self, y: f32) -> Option<usize> {
        if self.bands.is_empty() || !y.is_finite() {
            return None;
        }
        Some((((y - self.y0) / BAND).max(0.0) as usize).min(self.bands.len() - 1))
    }

    /// The items of the band of `y` whose x is in `lo..=hi`.
    fn at(&self, y: f32, lo: f32, hi: f32) -> &[(f32, T)] {
        let Some(i) = self.band(y) else { return &[] };
        let band = &self.bands[i];
        let a = band.partition_point(|p| p.0 < lo);
        let b = band.partition_point(|p| p.0 <= hi);
        &band[a..b.max(a)]
    }
}

/// Line segments that pass over a card that is not one of their two ends ([`Quality::lines_through_boxes`]).
fn through_boxes(
    l: &Layout,
    horizontal: &[Seg],
    vertical: &[Seg],
    mut hit: impl FnMut(usize, &Seg),
) {
    let inside = |n: &crate::types::Node| {
        let r = n.rect;
        (
            r.x + INSIDE,
            r.x + r.w - INSIDE,
            r.y + INSIDE,
            r.y + r.h - INSIDE,
        )
    };
    let clear = |n: &crate::types::Node, s: &Seg| n.id == s.from || n.id == s.to;
    let wide = l.nodes.iter().map(|n| n.rect.w).fold(0.0, f32::max) + 1.0;
    // Cards by the left of their inside, in bands along y.
    let cards = Bands::new(l.nodes.iter().enumerate().map(|(i, n)| {
        let (x0, _, y0, y1) = inside(n);
        (x0, y0, y1, i)
    }));
    for s in horizontal {
        for &(_, i) in cards.at(s.a, s.lo - wide, s.hi) {
            let n = &l.nodes[i];
            let (x0, x1, y0, y1) = inside(n);
            if !clear(n, s) && s.a > y0 && s.a < y1 && s.hi > x0 && s.lo < x1 {
                hit(i, s);
            }
        }
    }
    // A vertical segment meets the cards of every band it reaches: check each card once.
    let mut by_x: Vec<usize> = (0..l.nodes.len()).collect();
    by_x.sort_by(|&a, &b| l.nodes[a].rect.x.total_cmp(&l.nodes[b].rect.x));
    let xs: Vec<f32> = by_x.iter().map(|&i| inside(&l.nodes[i]).0).collect();
    for s in vertical {
        let a = xs.partition_point(|&x| x < s.a - wide);
        let b = xs.partition_point(|&x| x <= s.a);
        for &i in &by_x[a..b.max(a)] {
            let n = &l.nodes[i];
            let (x0, x1, y0, y1) = inside(n);
            if !clear(n, s) && s.a > x0 && s.a < x1 && s.hi > y0 && s.lo < y1 {
                hit(i, s);
            }
        }
    }
}

/// Pairs of a horizontal and a vertical segment of different families that cross ([`Quality::line_crossings`]).
fn crossings(horizontal: &[Seg], vertical: &[Seg], mut hit: impl FnMut(&Seg, &Seg)) {
    let bands = Bands::new(vertical.iter().map(|v| (v.a, v.lo, v.hi, v)));
    for h in horizontal {
        for &(_, v) in bands.at(h.a, h.lo, h.hi) {
            if h.owner != v.owner && v.a > h.lo && v.a < h.hi && h.a > v.lo && h.a < v.hi {
                hit(h, v);
            }
        }
    }
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

    /// The banded look-ups count exactly what comparing every line with every card and every line would.
    #[test]
    fn banded_counts_match_every_pair() {
        let mut seed = 7u64;
        let mut rnd = |n: u32| {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 33) % u64::from(n)) as f32
        };
        let nodes: Vec<_> = (0..300).map(|i| card(i, rnd(4000), rnd(3000))).collect();
        let edges: Vec<Edge> = (0..400)
            .map(|_| {
                let (from, to) = (rnd(300) as u32, rnd(300) as u32);
                let (x0, y0, x1, y1, y2) = (rnd(4000), rnd(3000), rnd(4000), rnd(3000), rnd(3000));
                parent(from, to, vec![x0, y0, x0, y1, x1, y1, x1, y2])
            })
            .collect();
        let l = Layout::finish(nodes, edges, 0, false);
        let (h, v) = segments(&l);
        let mut through = 0;
        for n in &l.nodes {
            let r = n.rect;
            let (x0, x1, y0, y1) = (
                r.x + INSIDE,
                r.x + r.w - INSIDE,
                r.y + INSIDE,
                r.y + r.h - INSIDE,
            );
            let clear = |s: &Seg| n.id == s.from || n.id == s.to;
            through += h
                .iter()
                .filter(|s| !clear(s) && s.a > y0 && s.a < y1 && s.hi > x0 && s.lo < x1)
                .count();
            through += v
                .iter()
                .filter(|s| !clear(s) && s.a > x0 && s.a < x1 && s.hi > y0 && s.lo < y1)
                .count();
        }
        let mut crossing = 0;
        for a in &h {
            for b in &v {
                if a.owner != b.owner && b.a > a.lo && b.a < a.hi && a.a > b.lo && a.a < b.hi {
                    crossing += 1;
                }
            }
        }
        let q = quality(&l);
        assert!(through > 0 && crossing > 0);
        assert_eq!(q.lines_through_boxes, through);
        assert_eq!(q.line_crossings, crossing);
    }
}
