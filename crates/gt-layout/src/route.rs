//! Routing of the parent-to-child connectors. One helper for every chart that draws families (the relatives chart and
//! every "built around" mode), so the rules live in one place:
//!
//! * a family (one set of parents and their children) has its own connector: drop(s) from the parents, one horizontal
//!   bus, drops to each child. Children of different parent sets never share a bus;
//! * buses of different families in the gap between two rows that are close along x get different lanes (different y),
//!   ordered so that as few lines as possible cross; the row gap grows when more lanes are needed;
//! * vertical lines of different families keep a minimum distance, so no line reads as the continuation of another
//!   (the drop of a parent may slide a little along the card edge, or along the partner connector, to get clear);
//! * a couple's children hang from the middle of the partner connector between the two cards; a single parent's
//!   children hang from the card.

use std::collections::HashSet;

use crate::types::{Edge, EdgeKind, Metrics, Node};

/// One family to draw.
pub(crate) struct Family {
    /// The parents drawn on the chart (none: brothers and sisters joined by a bar above their row).
    pub parents: Vec<u32>,
    pub children: Vec<u32>,
    /// The bus may not run above this y (the row of cousins that the lines pass below).
    pub zone_top: Option<f32>,
    pub kind: EdgeKind,
}

impl Family {
    pub fn new(parents: Vec<u32>, children: Vec<u32>, zone_top: Option<f32>) -> Self {
        Family {
            parents,
            children,
            zone_top,
            kind: EdgeKind::Parent,
        }
    }

    /// Brothers and sisters that hang from no parent drawn on the chart.
    pub fn barred(children: Vec<u32>) -> Self {
        Family {
            parents: Vec::new(),
            children,
            zone_top: None,
            kind: EdgeKind::Sibling,
        }
    }
}

/// Half the room a drop has to slide along the partner connector.
const COUPLE_SLIDE: f32 = 6.0;
/// A drop keeps this far from the corners of its card.
const CARD_MARGIN: f32 = 16.0;
/// Candidate positions of a sliding drop are this far apart.
const SLIDE_STEP: f32 = 4.0;
/// The most lanes tried in every order to find the one with the fewest crossings.
const MAX_PERMUTED: usize = 6;

struct Src {
    node: u32,
    x: f32,
    /// Where the line starts (the card's bottom edge, the partner connector's height, or the top of the root card).
    y: f32,
    /// Top of the corridor the line runs in: the card row, for a line between two partner cards.
    top: f32,
    lo: f32,
    hi: f32,
}

struct Fam {
    kind: EdgeKind,
    parents: Vec<u32>,
    kids: Vec<u32>,
    srcs: Vec<Src>,
    /// Lines that go up from a card to the bus (the root card of a bar) are children, not trunks.
    orphan: bool,
    couple: bool,
    zone_top: f32,
    zone_bot: f32,
    lo: f32,
    hi: f32,
    lane: usize,
    lanes: usize,
    bus_y: f32,
}

impl Fam {
    fn bus(&self) -> bool {
        self.hi - self.lo > 0.5
    }
    /// x of the lines that come from above the bus.
    fn up_xs(&self) -> Vec<f32> {
        if self.orphan {
            Vec::new()
        } else {
            self.srcs.iter().map(|s| s.x).collect()
        }
    }
    /// x of the lines that leave the bus downward.
    fn down_xs(&self, nodes: &[Node]) -> Vec<f32> {
        let mut v: Vec<f32> = self
            .kids
            .iter()
            .map(|&k| nodes[k as usize].rect.cx())
            .collect();
        if self.orphan {
            v.extend(self.srcs.iter().map(|s| s.x));
        }
        v
    }
}

/// A vertical line, for the distance check.
struct Vert {
    fam: usize,
    x: f32,
    y0: f32,
    y1: f32,
    /// The card this line lands on.
    lands: Option<u32>,
    /// The card this line leaves.
    leaves: Option<u32>,
    /// The `(family, source)` that can slide, when this line is a trunk.
    slide: Option<(usize, usize)>,
}

fn key(y: f32) -> i64 {
    (y * 4.0).round() as i64
}

/// Lays out the connectors of `families`. May move rows down to make room for lanes: `nodes` and `other` (edges
/// already built from the old positions) move together. Returns the family edges plus any partner connector that had to
/// be added between two parents standing side by side.
pub(crate) fn route(
    nodes: &mut [Node],
    other: &mut Vec<Edge>,
    families: Vec<Family>,
    m: &Metrics,
) -> Vec<Edge> {
    let mut have_partner: HashSet<(u32, u32)> = other
        .iter()
        .filter(|e| e.kind == EdgeKind::Partner)
        .map(|e| (e.from.min(e.to), e.from.max(e.to)))
        .collect();
    let sep_x = m.gap_x / 2.0;
    let bus_sep = m.node_w + 2.0 * m.gap_x;

    // 1. Sources and zones.
    let mut fams: Vec<Fam> = Vec::new();
    for f in families {
        let mut kids = f.children.clone();
        kids.sort_by(|&a, &b| {
            nodes[a as usize]
                .rect
                .x
                .total_cmp(&nodes[b as usize].rect.x)
        });
        kids.dedup();
        if kids.is_empty() || (f.parents.is_empty() && kids.len() < 2) {
            continue;
        }
        let zone_bot = kids
            .iter()
            .map(|&k| nodes[k as usize].rect.y)
            .fold(f32::INFINITY, f32::min);
        let mut srcs = Vec::new();
        let mut couple = false;
        let mut orphan = false;
        let mut zone_top = zone_bot - m.gap_y * 4.0;
        if f.parents.is_empty() {
            orphan = true;
            let root = kids.remove(0);
            let r = nodes[root as usize].rect;
            srcs.push(Src {
                node: root,
                x: r.cx(),
                y: r.y,
                top: r.y,
                lo: r.cx(),
                hi: r.cx(),
            });
        } else {
            zone_top = f
                .parents
                .iter()
                .map(|&p| nodes[p as usize].rect.bottom())
                .fold(f32::NEG_INFINITY, f32::max);
            if let Some(z) = f.zone_top {
                zone_top = zone_top.max(z);
            }
            let adjacent = if let [a, b] = f.parents[..] {
                let (ra, rb) = (nodes[a as usize].rect, nodes[b as usize].rect);
                let (l, r) = if ra.x <= rb.x { (ra, rb) } else { (rb, ra) };
                let gap = r.x - l.right();
                ((l.y - r.y).abs() < 0.5 && (-0.01..=m.couple_gap + 1.0).contains(&gap))
                    .then_some((a, b, l, r))
            } else {
                None
            };
            if let Some((a, b, l, r)) = adjacent {
                couple = true;
                let x = (l.right() + r.x) / 2.0;
                let slide = ((r.x - l.right()) / 2.0 - 4.0).clamp(0.0, COUPLE_SLIDE);
                srcs.push(Src {
                    node: f.parents[0],
                    x,
                    y: l.cy(),
                    top: l.y,
                    lo: x - slide,
                    hi: x + slide,
                });
                if have_partner.insert((a.min(b), a.max(b))) {
                    let (from, to) = if nodes[a as usize].rect.x <= nodes[b as usize].rect.x {
                        (a, b)
                    } else {
                        (b, a)
                    };
                    let (ra, rb) = (nodes[from as usize].rect, nodes[to as usize].rect);
                    other.push(Edge {
                        from,
                        to,
                        kind: EdgeKind::Partner,
                        pts: vec![ra.right(), ra.cy(), rb.x, rb.cy()],
                    });
                }
            } else {
                for &p in &f.parents {
                    let r = nodes[p as usize].rect;
                    let room = (r.w / 2.0 - CARD_MARGIN).max(0.0);
                    srcs.push(Src {
                        node: p,
                        x: r.cx(),
                        y: r.bottom(),
                        top: r.bottom(),
                        lo: r.cx() - room,
                        hi: r.cx() + room,
                    });
                }
            }
        }
        fams.push(Fam {
            kind: f.kind,
            parents: f.parents,
            kids,
            srcs,
            orphan,
            couple,
            zone_top,
            zone_bot,
            lo: 0.0,
            hi: 0.0,
            lane: 0,
            lanes: 1,
            bus_y: 0.0,
        });
    }

    slide_trunks(&mut fams, nodes, sep_x);

    // 2. Spans.
    for f in &mut fams {
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for s in &f.srcs {
            lo = lo.min(s.x);
            hi = hi.max(s.x);
        }
        for &k in &f.kids {
            let c = nodes[k as usize].rect.cx();
            lo = lo.min(c);
            hi = hi.max(c);
        }
        f.lo = lo;
        f.hi = hi;
    }

    // 3. Lanes, zone by zone (a zone is the gap above one row of children).
    let mut order: Vec<usize> = (0..fams.len()).filter(|&i| fams[i].bus()).collect();
    order.sort_by(|&a, &b| {
        key(fams[a].zone_bot)
            .cmp(&key(fams[b].zone_bot))
            .then(fams[a].lo.total_cmp(&fams[b].lo))
    });
    let mut zones: Vec<(i64, f32, f32, usize)> = Vec::new(); // key, bottom, top, most lanes
    let mut at = 0;
    while at < order.len() {
        let zk = key(fams[order[at]].zone_bot);
        let mut end = at;
        while end < order.len() && key(fams[order[end]].zone_bot) == zk {
            end += 1;
        }
        let group = &order[at..end];
        let most = assign_lanes(&mut fams, group, nodes, bus_sep);
        let top = group
            .iter()
            .map(|&i| fams[i].zone_top)
            .fold(f32::NEG_INFINITY, f32::max);
        zones.push((zk, fams[group[0]].zone_bot, top, most));
        at = end;
    }

    // 4. Make room: a zone that needs more height pushes every row below it down.
    let margin = m.gap_y;
    let mut grow: Vec<(f32, f32)> = Vec::new(); // (bottom, extra)
    for &(_, bot, top, most) in &zones {
        let need = 2.0 * margin + (most.saturating_sub(1)) as f32 * m.gap_y;
        let extra = (need - (bot - top)).max(0.0).ceil();
        if extra > 0.0 {
            grow.push((bot, extra));
        }
    }
    grow.sort_by(|a, b| a.0.total_cmp(&b.0));
    let shift = |y: f32| -> f32 {
        grow.iter()
            .filter(|g| g.0 <= y + 0.01)
            .map(|g| g.1)
            .sum::<f32>()
    };
    if !grow.is_empty() {
        for n in nodes.iter_mut() {
            n.rect.y += shift(n.rect.y);
        }
        for e in other.iter_mut() {
            for p in e.pts.chunks_exact_mut(2) {
                p[1] += shift(p[1]);
            }
        }
        for f in &mut fams {
            f.zone_top += shift(f.zone_top);
            f.zone_bot += shift(f.zone_bot);
            for s in &mut f.srcs {
                s.y += shift(s.y);
                s.top += shift(s.top);
            }
        }
        for z in &mut zones {
            let (t, b) = (z.2, z.1);
            z.2 = t + shift(t);
            z.1 = b + shift(b);
        }
    }
    // Bus heights.
    for &(zk, bot, top, most) in &zones {
        let (lo_y, hi_y) = (top + margin, (bot - margin).max(top + margin));
        let step = if most > 1 {
            ((hi_y - lo_y) / (most - 1) as f32).clamp(m.gap_y, m.gap_y * 2.0)
        } else {
            0.0
        };
        let mid = (top + bot) / 2.0;
        for f in fams.iter_mut().filter(|f| f.bus() && key(f.zone_bot) == zk) {
            f.bus_y = mid + (f.lane as f32 - (f.lanes as f32 - 1.0) / 2.0) * step;
        }
    }
    for f in fams.iter_mut().filter(|f| !f.bus()) {
        f.bus_y = (f.zone_top + f.zone_bot) / 2.0;
    }

    // 5. Edges.
    let mut edges = Vec::new();
    for f in &fams {
        for s in &f.srcs {
            let sx = s.x;
            let from = if f.couple { f.parents[0] } else { s.node };
            let ks: &[u32] = &f.kids;
            for &k in ks {
                let t = nodes[k as usize].rect;
                let tx = t.cx();
                let pts = if (sx - tx).abs() < 0.01 {
                    vec![sx, s.y, sx, t.y]
                } else {
                    vec![sx, s.y, sx, f.bus_y, tx, f.bus_y, tx, t.y]
                };
                edges.push(Edge {
                    from,
                    to: k,
                    kind: f.kind,
                    pts,
                });
            }
        }
    }
    edges
}

/// Slides the drops of parents (along the card edge, or along the partner connector) so that no vertical line of
/// another family runs within `sep` of them in the same gap or in a neighbouring one.
fn slide_trunks(fams: &mut [Fam], nodes: &[Node], sep: f32) {
    let reach = 1.0;
    let mut verts: Vec<Vert> = Vec::new();
    for (i, f) in fams.iter().enumerate() {
        if !f.orphan {
            for (si, s) in f.srcs.iter().enumerate() {
                verts.push(Vert {
                    fam: i,
                    x: s.x,
                    y0: s.top,
                    y1: f.zone_bot,
                    lands: None,
                    leaves: Some(s.node),
                    slide: (s.hi > s.lo).then_some((i, si)),
                });
            }
        } else {
            let s = &f.srcs[0];
            verts.push(Vert {
                fam: i,
                x: s.x,
                y0: f.zone_top,
                y1: s.y,
                lands: Some(s.node),
                leaves: None,
                slide: None,
            });
        }
        for &k in &f.kids {
            let r = nodes[k as usize].rect;
            verts.push(Vert {
                fam: i,
                x: r.cx(),
                y0: f.zone_top,
                y1: r.y,
                lands: Some(k),
                leaves: None,
                slide: None,
            });
        }
    }
    let clash = |verts: &[Vert], v: usize, x: f32, kids: &[u32], node: u32| -> Option<f32> {
        let a = &verts[v];
        let mut worst: Option<f32> = None;
        for (w, b) in verts.iter().enumerate() {
            if w == v || b.fam == a.fam {
                continue;
            }
            let d = (b.x - x).abs();
            if d >= sep {
                continue;
            }
            let ydist = (a.y0.max(b.y0) - a.y1.min(b.y1)).max(0.0);
            if ydist > reach {
                continue;
            }
            // A line that lands on this card and the line that leaves it are one line through the card.
            if b.lands == Some(node) || b.leaves.is_some_and(|c| kids.contains(&c)) {
                continue;
            }
            worst = Some(worst.map_or(d, |w| w.min(d)));
        }
        worst
    };
    for _ in 0..3 {
        let mut moved = false;
        for v in 0..verts.len() {
            let Some((fi, si)) = verts[v].slide else {
                continue;
            };
            let (x0, lo, hi, node) = {
                let s = &fams[fi].srcs[si];
                (s.x, s.lo, s.hi, s.node)
            };
            let kids = fams[fi].kids.clone();
            let base = (lo + hi) / 2.0;
            if clash(&verts, v, x0, &kids, node).is_none() {
                continue;
            }
            // The nearest free position to the centre, then the one farthest from the clash.
            let mut best: Option<(f32, f32)> = None; // (x, closest clash; infinity = free)
            let steps = ((hi - lo) / 2.0 / SLIDE_STEP).floor() as i32;
            for n in 0..=steps {
                for sign in [1.0, -1.0] {
                    if n == 0 && sign < 0.0 {
                        continue;
                    }
                    let x = base + sign * n as f32 * SLIDE_STEP;
                    if x < lo - 0.01 || x > hi + 0.01 {
                        continue;
                    }
                    let d = clash(&verts, v, x, &kids, node).unwrap_or(f32::INFINITY);
                    if best.is_none_or(|b| d > b.1) {
                        best = Some((x, d));
                    }
                }
                if best.is_some_and(|b| b.1.is_infinite()) {
                    break;
                }
            }
            if let Some((x, _)) = best
                && (x - x0).abs() > 0.01
            {
                fams[fi].srcs[si].x = x;
                verts[v].x = x;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
}

/// Gives every bus of one zone a lane: buses that are near each other along x get different lanes, in the order that
/// crosses the fewest lines. Returns the most lanes any group of buses in the zone needs.
fn assign_lanes(fams: &mut [Fam], group: &[usize], nodes: &[Node], bus_sep: f32) -> usize {
    // `group` is sorted by left end. Components of the "too close" relation.
    let n = group.len();
    let mut comp: Vec<usize> = (0..n).collect();
    fn find(c: &mut [usize], mut i: usize) -> usize {
        while c[i] != i {
            c[i] = c[c[i]];
            i = c[i];
        }
        i
    }
    let mut lane = vec![0usize; n];
    let mut near: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut active: Vec<usize> = Vec::new();
    for j in 0..n {
        let b = &fams[group[j]];
        active.retain(|&i| fams[group[i]].hi + bus_sep > b.lo);
        let mut used = HashSet::new();
        for &i in &active {
            near[j].push(i);
            near[i].push(j);
            used.insert(lane[i]);
            let (ri, rj) = (find(&mut comp, i), find(&mut comp, j));
            comp[ri] = rj;
        }
        lane[j] = (0..).find(|l| !used.contains(l)).unwrap_or(0);
        active.push(j);
    }
    let mut most = 1;
    let mut roots: Vec<usize> = Vec::new();
    for j in 0..n {
        let r = find(&mut comp, j);
        if !roots.contains(&r) {
            roots.push(r);
        }
    }
    for &r in &roots {
        let members: Vec<usize> = (0..n).filter(|&j| find(&mut comp, j) == r).collect();
        let lanes = members.iter().map(|&j| lane[j]).max().unwrap_or(0) + 1;
        most = most.max(lanes);
        if lanes > 1 && lanes <= MAX_PERMUTED {
            let perm = best_order(fams, group, nodes, &members, &lane, &near, lanes);
            for &j in &members {
                lane[j] = perm[lane[j]];
            }
        }
        for &j in &members {
            fams[group[j]].lane = lane[j];
            fams[group[j]].lanes = lanes;
        }
    }
    most
}

/// The assignment of colours to lanes (top to bottom) that crosses the fewest lines.
fn best_order(
    fams: &[Fam],
    group: &[usize],
    nodes: &[Node],
    members: &[usize],
    lane: &[usize],
    near: &[Vec<usize>],
    lanes: usize,
) -> Vec<usize> {
    // cost[a][b]: lines crossed when family a lies above family b.
    let strictly =
        |xs: &[f32], lo: f32, hi: f32| xs.iter().filter(|&&x| x > lo + 0.5 && x < hi - 0.5).count();
    let ups: Vec<Vec<f32>> = members.iter().map(|&j| fams[group[j]].up_xs()).collect();
    let downs: Vec<Vec<f32>> = members
        .iter()
        .map(|&j| fams[group[j]].down_xs(nodes))
        .collect();
    let pos = |j: usize| members.iter().position(|&m| m == j).unwrap_or(0);
    let mut pairs: Vec<(usize, usize, usize, usize)> = Vec::new(); // (ia, ib, cost a above b, cost b above a)
    for &j in members {
        for &i in &near[j] {
            if i < j {
                let (a, b) = (pos(i), pos(j));
                let (fa, fb) = (&fams[group[i]], &fams[group[j]]);
                let ab = strictly(&downs[a], fb.lo, fb.hi) + strictly(&ups[b], fa.lo, fa.hi);
                let ba = strictly(&downs[b], fa.lo, fa.hi) + strictly(&ups[a], fb.lo, fb.hi);
                pairs.push((i, j, ab, ba));
            }
        }
    }
    let mut perm: Vec<usize> = (0..lanes).collect();
    let mut best = perm.clone();
    let mut best_cost = usize::MAX;
    let cost = |perm: &[usize]| -> usize {
        pairs
            .iter()
            .map(|&(i, j, ab, ba)| {
                if perm[lane[i]] < perm[lane[j]] {
                    ab
                } else {
                    ba
                }
            })
            .sum()
    };
    // Heap's algorithm over the lane order; the identity comes first, so ties keep the plain colouring.
    let mut c = vec![0usize; lanes];
    best_cost = best_cost.min(cost(&perm));
    let mut i = 0;
    while i < lanes {
        if c[i] < i {
            if i % 2 == 0 {
                perm.swap(0, i);
            } else {
                perm.swap(c[i], i);
            }
            let k = cost(&perm);
            if k < best_cost {
                best_cost = k;
                best = perm.clone();
            }
            c[i] += 1;
            i = 0;
        } else {
            c[i] = 0;
            i += 1;
        }
    }
    best
}
