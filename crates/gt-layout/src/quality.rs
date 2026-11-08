//! How readable a laid out chart is, in plain counts: what overlaps what. No names, so the numbers can be
//! reported for any tree, and the tests can hold the layout to them.

use std::collections::HashMap;

use crate::types::{EdgeKind, Layout};

/// Lines closer than this along the same axis read as one line.
const SAME_LINE: f32 = 1.0;
/// A line this far inside a box is on top of it, not touching its edge.
const INSIDE: f32 = 0.5;

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
    let mut starts: HashMap<(u32, u32), usize> = HashMap::new();
    let mut horizontal: Vec<Seg> = Vec::new();
    let mut vertical: Vec<Seg> = Vec::new();
    for (i, e) in l.edges.iter().enumerate() {
        // The children of one couple start at one point and share a bar: they are one owner.
        let owner = if matches!(e.kind, EdgeKind::Parent | EdgeKind::Sibling) && e.pts.len() >= 2 {
            let next = starts.len();
            *starts
                .entry((e.pts[0].to_bits(), e.pts[1].to_bits()))
                .or_insert(next)
        } else {
            usize::MAX - i
        };
        for p in e.pts.windows(4).step_by(2) {
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
    for h in &horizontal {
        for v in &vertical {
            if h.owner != v.owner && v.a > h.lo && v.a < h.hi && h.a > v.lo && h.a < v.hi {
                q.line_crossings += 1;
            }
        }
    }
    q
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
