//! Whole-tree chart: every person of a project in one picture.
//!
//! Generations are rows (parents above children, partners side by side), each connected
//! component is laid out on its own and the components are packed on shelves, largest first,
//! each under a heading. People with no family link at all go into a final block.
//!
//! The pipeline per component:
//! 1. Generation numbers by a breadth-first walk over the person / family graph. Contradicting
//!    constraints (data errors, marriages across generations) lose against the constraint met
//!    first, so the walk visits every person once and always ends.
//! 2. Units: people in the same row who are partners form one box group.
//! 3. Order: sibling groups stay together, siblings by birth; barycenter sweeps order the groups.
//! 4. Positions: alternating down and up sweeps, each row solved exactly by isotonic regression
//!    (least displacement from where parents and children pull, without overlap).
//! 5. Lines: one connector per child from the couple to a bar, bars in lanes so they do not merge.
//!
//! Everything is a function of the input order, so the same data gives the same picture.

use std::collections::{HashMap, HashSet};

use crate::types::{Edge, EdgeKind, Key, Layout, Metrics, Node, Rect, new_node};

/// One family: up to two partners and the children, as dense person keys.
#[derive(Debug, Clone, Default)]
pub struct WholeFamily {
    pub partners: [Option<Key>; 2],
    pub children: Vec<Key>,
}

/// Everything the whole-tree layout reads. Keys are `0..count`.
#[derive(Debug, Clone, Default)]
pub struct WholeInput {
    pub count: usize,
    pub families: Vec<WholeFamily>,
    /// Sort key of each person's birth (children are ordered by it).
    pub birth: Vec<Option<i32>>,
    /// Surname of each person, for the block headings.
    pub surname: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Family,
    Unconnected,
}

/// A heading above one connected component (or above the unconnected people).
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub kind: BlockKind,
    /// Most frequent surname of a family block; empty when nobody has one.
    pub surname: String,
    pub people: u32,
    /// The heading strip; the people sit below it.
    pub rect: Rect,
}

/// What the whole-tree chart needs beyond a plain [`Layout`].
#[derive(Debug, Clone)]
pub struct WholeExtras {
    pub blocks: Vec<Block>,
    parents: Vec<Vec<u32>>,
    children: Vec<Vec<u32>>,
    node_of: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct WholeLayout {
    pub layout: Layout,
    pub extras: WholeExtras,
}

impl WholeExtras {
    /// The node of a person (every person has exactly one).
    pub fn node_of(&self, key: Key) -> Option<u32> {
        self.node_of
            .get(key as usize)
            .copied()
            .filter(|&n| n != NONE32)
    }

    /// The node, its ancestors and its descendants, ascending.
    pub fn lineage(&self, node: u32) -> Vec<u32> {
        let mut seen: HashSet<u32> = HashSet::from([node]);
        for adj in [&self.parents, &self.children] {
            let mut stack = vec![node];
            while let Some(v) = stack.pop() {
                for &w in adj.get(v as usize).map_or(&[][..], Vec::as_slice) {
                    if seen.insert(w) {
                        stack.push(w);
                    }
                }
            }
        }
        let mut out: Vec<u32> = seen.into_iter().collect();
        out.sort_unstable();
        out
    }
}

const NONE32: u32 = u32::MAX;
const UNSET: i32 = i32::MIN;
const NONE: usize = usize::MAX;
const HEADING_H: f64 = 28.0;
const HEADING_GAP: f64 = 12.0;
const BLOCK_GAP: f64 = 96.0;
const MIN_BLOCK_W: f64 = 240.0;
const LANE: f64 = 8.0;
const SWEEPS_ORDER: usize = 6;
const SWEEPS_PLACE: usize = 4;

struct Fam {
    partners: Vec<usize>,
    kids: Vec<usize>,
}

fn sanitise(input: &WholeInput) -> Vec<Fam> {
    let n = input.count;
    let birth = |c: usize| input.birth.get(c).copied().flatten().unwrap_or(i32::MAX);
    let mut out = Vec::new();
    for f in &input.families {
        let mut partners: Vec<usize> = Vec::new();
        for p in f.partners.iter().flatten() {
            let p = *p as usize;
            if p < n && !partners.contains(&p) {
                partners.push(p);
            }
        }
        let mut seen: HashSet<usize> = HashSet::new();
        let mut kids: Vec<usize> = Vec::new();
        for &c in &f.children {
            let c = c as usize;
            if c < n && !partners.contains(&c) && seen.insert(c) {
                kids.push(c);
            }
        }
        // Birth order; undated children keep their listed order after the dated ones.
        kids.sort_by_key(|&c| birth(c));
        if partners.len() + kids.len() >= 2 {
            out.push(Fam { partners, kids });
        }
    }
    out
}

struct Dsu(Vec<usize>);

impl Dsu {
    fn find(&mut self, mut x: usize) -> usize {
        while self.0[x] != x {
            self.0[x] = self.0[self.0[x]];
            x = self.0[x];
        }
        x
    }
    /// The root of a set is its smallest member.
    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.0[a.max(b)] = a.min(b);
        }
    }
}

/// Shared read-only view of the families.
struct Shared<'a> {
    fams: &'a [Fam],
    partner_in: &'a [Vec<usize>],
    child_in: &'a [Vec<usize>],
    m: &'a Metrics,
}

#[derive(Clone)]
struct Scratch {
    level: Vec<i32>,
    fam_level: Vec<i32>,
    local: Vec<usize>,
    unit_of: Vec<usize>,
    pos_in_unit: Vec<usize>,
    px: Vec<f64>,
    row: Vec<usize>,
}

struct CompOut {
    /// Person, left, top, row.
    people: Vec<(usize, f64, f64, usize)>,
    /// From person, to person, kind, polyline.
    edges: Vec<(usize, usize, EdgeKind, Vec<f64>)>,
    w: f64,
    h: f64,
}

struct Unit {
    members: Vec<usize>,
    row: usize,
    w: f64,
    x: f64,
    group: usize,
}

struct Group {
    row: usize,
    fam: Option<usize>,
    units: Vec<usize>,
    anchors: Vec<usize>,
}

/// Least-squares positions `x_i` with `x_{i+1} >= x_i + w_i + gap_i`, closest to `desired`
/// (pool-adjacent-violators on the offset-corrected targets).
fn solve_row(widths: &[f64], gaps: &[f64], desired: &[f64]) -> Vec<f64> {
    let n = widths.len();
    let mut offset = Vec::with_capacity(n);
    let mut acc = 0.0;
    for (i, w) in widths.iter().enumerate() {
        offset.push(acc);
        acc += w + gaps.get(i).copied().unwrap_or(0.0);
    }
    // Blocks of (sum of targets, count).
    let mut blocks: Vec<(f64, usize)> = Vec::with_capacity(n);
    for i in 0..n {
        blocks.push((desired[i] - offset[i], 1));
        while blocks.len() >= 2 {
            let (s2, c2) = blocks[blocks.len() - 1];
            let (s1, c1) = blocks[blocks.len() - 2];
            if s1 / c1 as f64 > s2 / c2 as f64 {
                blocks.truncate(blocks.len() - 2);
                blocks.push((s1 + s2, c1 + c2));
            } else {
                break;
            }
        }
    }
    let mut out = Vec::with_capacity(n);
    for (s, c) in blocks {
        let mean = s / c as f64;
        for _ in 0..c {
            let i = out.len();
            out.push(mean + offset[i]);
        }
    }
    out
}

/// Stable sort of the items that have a key; items without one keep their slot.
fn sort_keyed(items: &mut [usize], key: impl Fn(usize) -> Option<f64>) {
    let slots: Vec<usize> = (0..items.len())
        .filter(|&i| key(items[i]).is_some())
        .collect();
    let mut keyed: Vec<(f64, usize)> = slots
        .iter()
        .map(|&i| (key(items[i]).unwrap_or(0.0), items[i]))
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (&slot, (_, item)) in slots.iter().zip(keyed) {
        items[slot] = item;
    }
}

struct Comp {
    units: Vec<Unit>,
    groups: Vec<Group>,
    rows: Vec<Vec<usize>>,
    kids_units: Vec<Vec<usize>>,
    gap_in: f64,
    gap_group: f64,
}

impl Comp {
    fn centre(&self, u: usize) -> f64 {
        self.units[u].x + self.units[u].w / 2.0
    }

    fn group_width(&self, g: usize) -> f64 {
        let us = &self.groups[g].units;
        us.iter().map(|&u| self.units[u].w).sum::<f64>()
            + self.gap_in * us.len().saturating_sub(1) as f64
    }

    fn lay_group(&mut self, g: usize, left: f64) {
        let mut x = left;
        for i in 0..self.groups[g].units.len() {
            let u = self.groups[g].units[i];
            self.units[u].x = x;
            x += self.units[u].w + self.gap_in;
        }
    }

    fn pack_row(&mut self, r: usize) {
        let mut x = 0.0;
        for i in 0..self.rows[r].len() {
            let g = self.rows[r][i];
            self.lay_group(g, x);
            x += self.group_width(g) + self.gap_group;
        }
    }

    /// Down sweep key: where the parents of the group are.
    fn parent_key(&self, g: usize) -> Option<f64> {
        let a = &self.groups[g].anchors;
        if a.is_empty() {
            return None;
        }
        Some(a.iter().map(|&u| self.centre(u)).sum::<f64>() / a.len() as f64)
    }

    /// Up sweep: where the children of the unit are.
    fn kids_key(&self, u: usize) -> Option<f64> {
        let k = &self.kids_units[u];
        if k.is_empty() {
            return None;
        }
        Some(k.iter().map(|&c| self.centre(c)).sum::<f64>() / k.len() as f64)
    }

    fn group_kids_key(&self, g: usize) -> Option<f64> {
        let ks: Vec<f64> = self.groups[g]
            .units
            .iter()
            .filter_map(|&u| self.kids_key(u))
            .collect();
        if ks.is_empty() {
            return None;
        }
        Some(ks.iter().sum::<f64>() / ks.len() as f64)
    }

    fn down(&mut self, r: usize, order: bool) {
        let mut row = std::mem::take(&mut self.rows[r]);
        if order {
            sort_keyed(&mut row, |g| self.parent_key(g));
        }
        let widths: Vec<f64> = row.iter().map(|&g| self.group_width(g)).collect();
        let desired: Vec<f64> = row
            .iter()
            .zip(&widths)
            .map(|(&g, w)| {
                self.parent_key(g)
                    .map_or_else(|| self.units[self.groups[g].units[0]].x, |k| k - w / 2.0)
            })
            .collect();
        let gaps = vec![self.gap_group; row.len().saturating_sub(1)];
        let lefts = solve_row(&widths, &gaps, &desired);
        for (&g, &l) in row.iter().zip(&lefts) {
            self.lay_group(g, l);
        }
        self.rows[r] = row;
    }

    fn up(&mut self, r: usize, order: bool) {
        let mut row = std::mem::take(&mut self.rows[r]);
        if order {
            sort_keyed(&mut row, |g| self.group_kids_key(g));
        }
        let flat: Vec<usize> = row
            .iter()
            .flat_map(|&g| self.groups[g].units.iter().copied())
            .collect();
        let widths: Vec<f64> = flat.iter().map(|&u| self.units[u].w).collect();
        let desired: Vec<f64> = flat
            .iter()
            .map(|&u| {
                self.kids_key(u)
                    .map_or(self.units[u].x, |k| k - self.units[u].w / 2.0)
            })
            .collect();
        let gaps: Vec<f64> = flat
            .windows(2)
            .map(|p| {
                if self.units[p[0]].group == self.units[p[1]].group {
                    self.gap_in
                } else {
                    self.gap_group
                }
            })
            .collect();
        let lefts = solve_row(&widths, &gaps, &desired);
        for (&u, &l) in flat.iter().zip(&lefts) {
            self.units[u].x = l;
        }
        self.rows[r] = row;
    }
}

fn generations(members: &[usize], sh: &Shared, sc: &mut Scratch) {
    let mut queue: Vec<(bool, usize)> = Vec::new();
    for &s in members {
        if sc.level[s] != UNSET {
            continue;
        }
        sc.level[s] = 0;
        queue.clear();
        queue.push((false, s));
        let mut head = 0;
        while head < queue.len() {
            let (is_fam, id) = queue[head];
            head += 1;
            if is_fam {
                let level = sc.fam_level[id];
                let f = &sh.fams[id];
                for &p in &f.partners {
                    if sc.level[p] == UNSET {
                        sc.level[p] = level;
                        queue.push((false, p));
                    }
                }
                for &c in &f.kids {
                    if sc.level[c] == UNSET {
                        sc.level[c] = level + 1;
                        queue.push((false, c));
                    }
                }
            } else {
                let g = sc.level[id];
                for &f in &sh.partner_in[id] {
                    if sc.fam_level[f] == UNSET {
                        sc.fam_level[f] = g;
                        queue.push((true, f));
                    }
                }
                for &f in &sh.child_in[id] {
                    if sc.fam_level[f] == UNSET {
                        sc.fam_level[f] = g - 1;
                        queue.push((true, f));
                    }
                }
            }
        }
    }
    let low = members.iter().map(|&p| sc.level[p]).min().unwrap_or(0);
    for &p in members {
        sc.level[p] -= low;
    }
}

/// Partner units: connected sets of same-row partners, in a walk order that keeps partners next
/// to each other where it can.
fn make_units(members: &[usize], sh: &Shared, sc: &mut Scratch) -> Vec<Unit> {
    for (i, &p) in members.iter().enumerate() {
        sc.local[p] = i;
    }
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); members.len()];
    for &p in members {
        for &f in &sh.partner_in[p] {
            for &q in &sh.fams[f].partners {
                if q != p && sc.level[q] == sc.level[p] && !adj[sc.local[p]].contains(&q) {
                    adj[sc.local[p]].push(q);
                }
            }
        }
    }
    let mut units: Vec<Unit> = Vec::new();
    let mut seen = vec![false; members.len()];
    for &s in members {
        if seen[sc.local[s]] {
            continue;
        }
        // The whole connected set first, to pick the end to start from.
        let mut set = vec![s];
        seen[sc.local[s]] = true;
        let mut i = 0;
        while i < set.len() {
            let p = set[i];
            i += 1;
            for &q in &adj[sc.local[p]] {
                if !seen[sc.local[q]] {
                    seen[sc.local[q]] = true;
                    set.push(q);
                }
            }
        }
        let start = set
            .iter()
            .copied()
            .min_by_key(|&p| (adj[sc.local[p]].len(), p))
            .unwrap_or(s);
        let mut order: Vec<usize> = Vec::with_capacity(set.len());
        let mut done: HashSet<usize> = HashSet::new();
        let mut stack = vec![start];
        while let Some(p) = stack.pop() {
            if !done.insert(p) {
                continue;
            }
            order.push(p);
            stack.extend(adj[sc.local[p]].iter().rev().copied());
        }
        let k = order.len() as f64;
        let w = k * f64::from(sh.m.node_w) + (k - 1.0) * f64::from(sh.m.couple_gap);
        let u = units.len();
        for (pos, &p) in order.iter().enumerate() {
            sc.unit_of[p] = u;
            sc.pos_in_unit[p] = pos;
        }
        units.push(Unit {
            row: sc.level[order[0]] as usize,
            members: order,
            w,
            x: 0.0,
            group: NONE,
        });
    }
    units
}

fn layout_component(members: &[usize], sh: &Shared, sc: &mut Scratch) -> CompOut {
    let m = sh.m;
    let node_w = f64::from(m.node_w);
    let node_h = f64::from(m.node_h);
    let step_p = node_w + f64::from(m.couple_gap);

    for &p in members {
        for &f in &sh.partner_in[p] {
            sc.fam_level[f] = UNSET;
        }
        for &f in &sh.child_in[p] {
            sc.fam_level[f] = UNSET;
        }
        sc.level[p] = UNSET;
    }
    generations(members, sh, sc);
    let units = make_units(members, sh, sc);
    let rows_n = members
        .iter()
        .map(|&p| sc.level[p] as usize)
        .max()
        .unwrap_or(0)
        + 1;

    // Sibling groups: a unit sits with the siblings of its first member that has parents.
    let primary = |p: usize| sh.child_in[p].first().copied();
    let mut groups: Vec<Group> = Vec::new();
    let mut by_fam: HashMap<(usize, usize), usize> = HashMap::new();
    let mut kpos = vec![0usize; units.len()];
    let mut units = units;
    for u in 0..units.len() {
        let pick = units[u]
            .members
            .iter()
            .copied()
            .find_map(|p| primary(p).map(|f| (p, f)));
        let row = units[u].row;
        let g = match pick {
            Some((p, f)) => {
                kpos[u] = sh.fams[f].kids.iter().position(|&c| c == p).unwrap_or(0);
                *by_fam.entry((f, row)).or_insert_with(|| {
                    groups.push(Group {
                        row,
                        fam: Some(f),
                        units: Vec::new(),
                        anchors: Vec::new(),
                    });
                    groups.len() - 1
                })
            }
            None => {
                groups.push(Group {
                    row,
                    fam: None,
                    units: Vec::new(),
                    anchors: Vec::new(),
                });
                groups.len() - 1
            }
        };
        groups[g].units.push(u);
        units[u].group = g;
    }
    for g in &mut groups {
        g.units.sort_by_key(|&u| (kpos[u], u));
    }
    for (g, grp) in groups.iter_mut().enumerate() {
        let Some(f) = grp.fam else { continue };
        let mut anchors: Vec<usize> = Vec::new();
        for &q in &sh.fams[f].partners {
            let u = sc.unit_of[q];
            if units[u].group != g && !anchors.contains(&u) {
                anchors.push(u);
            }
        }
        grp.anchors = anchors;
    }
    // Units holding the primary children of each unit's families.
    let mut kids_units: Vec<Vec<usize>> = vec![Vec::new(); units.len()];
    for (u, ku) in kids_units.iter_mut().enumerate() {
        for &p in &units[u].members {
            for &f in &sh.partner_in[p] {
                for &c in &sh.fams[f].kids {
                    let cu = sc.unit_of[c];
                    if primary(c) == Some(f) && cu != u && !ku.contains(&cu) {
                        ku.push(cu);
                    }
                }
            }
        }
    }

    // First-visit order of a depth-first walk over parent and child links.
    let mut disc = vec![NONE; units.len()];
    let mut counter = 0;
    for s in 0..units.len() {
        let mut stack = vec![s];
        while let Some(u) = stack.pop() {
            if disc[u] != NONE {
                continue;
            }
            disc[u] = counter;
            counter += 1;
            let mut next: Vec<usize> = groups[units[u].group].anchors.clone();
            next.extend(kids_units[u].iter().copied());
            stack.extend(next.into_iter().rev());
        }
    }
    let mut rows: Vec<Vec<usize>> = vec![Vec::new(); rows_n];
    for (g, grp) in groups.iter().enumerate() {
        rows[grp.row].push(g);
    }
    let rank =
        |g: usize, groups: &[Group]| groups[g].units.iter().map(|&u| disc[u]).min().unwrap_or(0);
    for row in &mut rows {
        row.sort_by_key(|&g| (rank(g, &groups), g));
    }

    let mut c = Comp {
        units,
        groups,
        rows,
        kids_units,
        gap_in: f64::from(m.gap_x),
        gap_group: f64::from(m.gap_x) * 2.0,
    };
    for r in 0..rows_n {
        c.pack_row(r);
    }
    for s in 0..SWEEPS_ORDER + SWEEPS_PLACE {
        let order = s < SWEEPS_ORDER;
        for r in 1..rows_n {
            c.down(r, order);
        }
        for r in (0..rows_n.saturating_sub(1)).rev() {
            c.up(r, order);
        }
    }

    // Left edge to zero, person boxes.
    let min_x = c.units.iter().map(|u| u.x).fold(f64::INFINITY, f64::min);
    for u in &c.units {
        for (k, &p) in u.members.iter().enumerate() {
            sc.px[p] = u.x - min_x + k as f64 * step_p;
            sc.row[p] = u.row;
        }
    }
    let cx = |sc: &Scratch, p: usize| sc.px[p] + node_w / 2.0;

    // Where each child line starts (anchor), and the lane of its bar.
    struct Link {
        fam: usize,
        anchor_x: f64,
        anchor_row: usize,
        /// Start at the vertical middle of a partner line instead of below the boxes.
        mid: bool,
        lo: f64,
        hi: f64,
    }
    let mut links: Vec<Link> = Vec::new();
    for &p in members {
        for &f in &sh.partner_in[p] {
            if sh.fams[f].partners[0] != p || sh.fams[f].kids.is_empty() {
                continue;
            }
            let fam = &sh.fams[f];
            let (ax, arow, mid) = match fam.partners.get(1) {
                None => (cx(sc, p), sc.row[p], false),
                Some(&q) => {
                    let adjacent = sc.unit_of[p] == sc.unit_of[q]
                        && sc.pos_in_unit[p].abs_diff(sc.pos_in_unit[q]) == 1
                        && sc.row[p] == sc.row[q];
                    if adjacent {
                        let (a, b) = if sc.px[p] < sc.px[q] { (p, q) } else { (q, p) };
                        ((sc.px[a] + node_w + sc.px[b]) / 2.0, sc.row[p], true)
                    } else {
                        (
                            (cx(sc, p) + cx(sc, q)) / 2.0,
                            sc.row[p].max(sc.row[q]),
                            false,
                        )
                    }
                }
            };
            let mut lo = ax;
            let mut hi = ax;
            for &k in &fam.kids {
                lo = lo.min(cx(sc, k));
                hi = hi.max(cx(sc, k));
            }
            links.push(Link {
                fam: f,
                anchor_x: ax,
                anchor_row: arow,
                mid,
                lo,
                hi,
            });
        }
    }
    // Lanes per channel (the gap below a row), greedy over left edges.
    let mut order: Vec<usize> = (0..links.len()).collect();
    order.sort_by(|&a, &b| {
        (links[a].anchor_row, links[a].lo, links[a].hi, links[a].fam)
            .partial_cmp(&(links[b].anchor_row, links[b].lo, links[b].hi, links[b].fam))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut lane_of = vec![0usize; links.len()];
    let mut lanes: Vec<Vec<f64>> = vec![Vec::new(); rows_n];
    for &i in &order {
        let l = &links[i];
        let lane_rights = &mut lanes[l.anchor_row];
        match lane_rights.iter().position(|&right| right + 6.0 < l.lo) {
            Some(k) => {
                lane_rights[k] = l.hi;
                lane_of[i] = k;
            }
            None => {
                lane_rights.push(l.hi);
                lane_of[i] = lane_rights.len() - 1;
            }
        }
    }
    let channel = |r: usize| (f64::from(m.gap_y) * 4.0).max(28.0 + lanes[r].len() as f64 * LANE);
    let mut row_y = vec![0.0f64; rows_n];
    for r in 1..rows_n {
        row_y[r] = row_y[r - 1] + node_h + channel(r - 1);
    }

    let mut out = CompOut {
        people: Vec::with_capacity(members.len()),
        edges: Vec::new(),
        w: 0.0,
        h: 0.0,
    };
    for &p in members {
        let (x, y) = (sc.px[p], row_y[sc.row[p]]);
        out.people.push((p, x, y, sc.row[p]));
        out.w = out.w.max(x + node_w);
        out.h = out.h.max(y + node_h);
    }

    // Partner lines, once per pair.
    let mut pairs: HashSet<(usize, usize)> = HashSet::new();
    for fam in sh.fams.iter() {
        let [Some(&p), Some(&q)] = [fam.partners.first(), fam.partners.get(1)] else {
            continue;
        };
        if members.binary_search(&p).is_err() || !pairs.insert((p.min(q), p.max(q))) {
            continue;
        }
        // Left one first on a shared row, otherwise the upper one.
        let left_first = if sc.row[p] == sc.row[q] {
            sc.px[p] < sc.px[q]
        } else {
            sc.row[p] < sc.row[q]
        };
        let (a, b) = if left_first { (p, q) } else { (q, p) };
        let (ya, yb) = (row_y[sc.row[a]], row_y[sc.row[b]]);
        let adjacent = sc.unit_of[a] == sc.unit_of[b]
            && sc.pos_in_unit[a].abs_diff(sc.pos_in_unit[b]) == 1
            && sc.row[a] == sc.row[b];
        let pts = if adjacent {
            let (l, r) = if sc.px[a] < sc.px[b] { (a, b) } else { (b, a) };
            vec![
                sc.px[l] + node_w,
                ya + node_h / 2.0,
                sc.px[r],
                ya + node_h / 2.0,
            ]
        } else {
            let under = (ya + node_h).max(yb + node_h) + 8.0;
            vec![
                cx(sc, a),
                ya + node_h,
                cx(sc, a),
                under,
                cx(sc, b),
                under,
                cx(sc, b),
                yb + node_h,
            ]
        };
        out.edges.push((a, b, EdgeKind::Partner, pts));
    }
    // Child lines.
    for (i, l) in links.iter().enumerate() {
        let fam = &sh.fams[l.fam];
        let ay = row_y[l.anchor_row]
            + if l.mid {
                node_h / 2.0
            } else {
                node_h + if fam.partners.len() > 1 { 8.0 } else { 0.0 }
            };
        let bus = row_y[l.anchor_row] + node_h + 12.0 + lane_of[i] as f64 * LANE;
        for &k in &fam.kids {
            let kx = cx(sc, k);
            let below = sc.row[k] > l.anchor_row;
            let end = if below {
                row_y[sc.row[k]]
            } else {
                row_y[sc.row[k]] + node_h
            };
            let mut pts = vec![l.anchor_x, ay, l.anchor_x, bus, kx, bus, kx, end];
            dedupe_points(&mut pts);
            out.edges.push((fam.partners[0], k, EdgeKind::Parent, pts));
        }
    }
    out
}

fn dedupe_points(pts: &mut Vec<f64>) {
    let mut out: Vec<f64> = Vec::with_capacity(pts.len());
    for p in pts.chunks_exact(2) {
        let n = out.len();
        if n >= 2 && out[n - 2] == p[0] && out[n - 1] == p[1] {
            continue;
        }
        out.extend_from_slice(p);
    }
    *pts = out;
}

struct Placed {
    kind: BlockKind,
    surname: String,
    people: usize,
    w: f64,
    comp: Option<CompOut>,
}

/// Lay out every person once. Deterministic for a given input.
pub fn whole(input: &WholeInput, m: &Metrics) -> WholeLayout {
    let n = input.count;
    let fams = sanitise(input);
    let mut partner_in: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut child_in: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut dsu = Dsu((0..n).collect());
    for (i, f) in fams.iter().enumerate() {
        let all: Vec<usize> = f.partners.iter().chain(&f.kids).copied().collect();
        for &p in &f.partners {
            partner_in[p].push(i);
        }
        for &c in &f.kids {
            child_in[c].push(i);
        }
        for &p in &all[1..] {
            dsu.union(all[0], p);
        }
    }
    let sh = Shared {
        fams: &fams,
        partner_in: &partner_in,
        child_in: &child_in,
        m,
    };

    let mut comp_of_root = vec![NONE; n];
    let mut comps: Vec<Vec<usize>> = Vec::new();
    let mut loose: Vec<usize> = Vec::new();
    for p in 0..n {
        if partner_in[p].is_empty() && child_in[p].is_empty() {
            loose.push(p);
            continue;
        }
        let root = dsu.find(p);
        if comp_of_root[root] == NONE {
            comp_of_root[root] = comps.len();
            comps.push(Vec::new());
        }
        comps[comp_of_root[root]].push(p);
    }

    let mut sc = Scratch {
        level: vec![UNSET; n],
        fam_level: vec![UNSET; fams.len()],
        local: vec![0; n],
        unit_of: vec![NONE; n],
        pos_in_unit: vec![0; n],
        px: vec![0.0; n],
        row: vec![0; n],
    };
    let mut placed: Vec<Placed> = Vec::new();
    for members in &comps {
        let out = layout_component(members, &sh, &mut sc);
        let mut count: HashMap<&str, usize> = HashMap::new();
        for &p in members {
            let s = input.surname.get(p).map_or("", String::as_str);
            if !s.is_empty() {
                *count.entry(s).or_default() += 1;
            }
        }
        let surname = count
            .into_iter()
            .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))
            .map_or_else(String::new, |(s, _)| s.to_string());
        placed.push(Placed {
            kind: BlockKind::Family,
            surname,
            people: members.len(),
            w: out.w.max(MIN_BLOCK_W),
            comp: Some(out),
        });
    }
    // Largest first; ties keep the order of their first person.
    placed.sort_by_key(|p| std::cmp::Reverse(p.people));

    // Shelf width from the total area, never narrower than the widest block.
    let pitch_x = f64::from(m.node_w + m.gap_x);
    let pitch_y = f64::from(m.node_h + m.gap_y);
    let comp_area: f64 = placed
        .iter()
        .map(|p| {
            let h = p.comp.as_ref().map_or(0.0, |c| c.h) + HEADING_H + HEADING_GAP;
            (p.w + BLOCK_GAP) * (h + BLOCK_GAP)
        })
        .sum();
    let area = comp_area + loose.len() as f64 * pitch_x * pitch_y;
    let widest = placed.iter().map(|p| p.w).fold(MIN_BLOCK_W, f64::max);
    let shelf_w = widest.max(area.sqrt() * 1.6);

    let mut blocks: Vec<Block> = Vec::new();
    let mut nodes: Vec<Node> = Vec::with_capacity(n);
    let mut node_of = vec![NONE32; n];
    let mut raw_edges: Vec<(usize, usize, EdgeKind, Vec<f32>)> = Vec::new();
    let rect0 = Rect {
        x: 0.0,
        y: 0.0,
        w: m.node_w,
        h: m.node_h,
    };
    let (mut x, mut y, mut shelf_h) = (0.0f64, 0.0f64, 0.0f64);
    for p in &placed {
        let comp = p.comp.as_ref();
        let h = comp.map_or(0.0, |c| c.h) + HEADING_H + HEADING_GAP;
        if x > 0.0 && x + p.w > shelf_w {
            y += shelf_h + BLOCK_GAP;
            x = 0.0;
            shelf_h = 0.0;
        }
        blocks.push(Block {
            kind: p.kind,
            surname: p.surname.clone(),
            people: p.people as u32,
            rect: Rect {
                x: x as f32,
                y: y as f32,
                w: p.w as f32,
                h: HEADING_H as f32,
            },
        });
        if let Some(c) = comp {
            let oy = y + HEADING_H + HEADING_GAP;
            let mut people: Vec<&(usize, f64, f64, usize)> = c.people.iter().collect();
            people.sort_by(|a, b| {
                a.2.total_cmp(&b.2)
                    .then(a.1.total_cmp(&b.1))
                    .then(a.0.cmp(&b.0))
            });
            for &&(person, px, py, row) in &people {
                let id = nodes.len() as u32;
                let node = new_node(
                    id,
                    person as Key,
                    Rect {
                        x: (x + px) as f32,
                        y: (oy + py) as f32,
                        ..rect0
                    },
                    row as i16,
                );
                node_of[person] = id;
                nodes.push(node);
            }
            for (from, to, kind, pts) in &c.edges {
                let pts: Vec<f32> = pts
                    .chunks_exact(2)
                    .flat_map(|q| [(x + q[0]) as f32, (oy + q[1]) as f32])
                    .collect();
                raw_edges.push((*from, *to, *kind, pts));
            }
        }
        x += p.w + BLOCK_GAP;
        shelf_h = shelf_h.max(h);
    }
    if !loose.is_empty() {
        if x > 0.0 {
            y += shelf_h + BLOCK_GAP;
        }
        let cols =
            (((shelf_w + f64::from(m.gap_x)) / pitch_x).floor() as usize).clamp(1, loose.len());
        let oy = y + HEADING_H + HEADING_GAP;
        blocks.push(Block {
            kind: BlockKind::Unconnected,
            surname: String::new(),
            people: loose.len() as u32,
            rect: Rect {
                x: 0.0,
                y: y as f32,
                w: (cols as f64 * pitch_x - f64::from(m.gap_x)).max(MIN_BLOCK_W) as f32,
                h: HEADING_H as f32,
            },
        });
        for (i, &person) in loose.iter().enumerate() {
            let id = nodes.len() as u32;
            let node = new_node(
                id,
                person as Key,
                Rect {
                    x: ((i % cols) as f64 * pitch_x) as f32,
                    y: (oy + (i / cols) as f64 * pitch_y) as f32,
                    ..rect0
                },
                0,
            );
            node_of[person] = id;
            nodes.push(node);
        }
    }

    // Edges in node ids, navigation links, lineage adjacency.
    let id_of = |p: usize| node_of[p];
    let edges: Vec<Edge> = raw_edges
        .into_iter()
        .map(|(from, to, kind, pts)| Edge {
            from: id_of(from),
            to: id_of(to),
            kind,
            pts,
        })
        .collect();
    let mut parents: Vec<Vec<u32>> = vec![Vec::new(); nodes.len()];
    let mut children: Vec<Vec<u32>> = vec![Vec::new(); nodes.len()];
    for f in &fams {
        for &c in &f.kids {
            for &p in &f.partners {
                let (pn, cn) = (id_of(p), id_of(c));
                if !parents[cn as usize].contains(&pn) {
                    parents[cn as usize].push(pn);
                    children[pn as usize].push(cn);
                }
            }
        }
    }
    for (i, f) in fams.iter().enumerate() {
        let Some(&first) = f.partners.first() else {
            continue;
        };
        let kids: Vec<u32> = f
            .kids
            .iter()
            .filter(|&&c| child_in[c].first() == Some(&i))
            .map(|&c| id_of(c))
            .collect();
        for &p in &f.partners {
            let nav = &mut nodes[id_of(p) as usize].nav;
            if nav.child.is_none() {
                nav.child = kids.first().copied();
            }
        }
        for (j, &k) in kids.iter().enumerate() {
            let nav = &mut nodes[k as usize].nav;
            nav.parent = Some(id_of(first));
            nav.prev = j.checked_sub(1).map(|q| kids[q]);
            nav.next = kids.get(j + 1).copied();
        }
    }

    let layout = Layout::finish(nodes, edges, 0, false);
    WholeLayout {
        layout,
        extras: WholeExtras {
            blocks,
            parents,
            children,
            node_of,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_solver_keeps_order_and_gaps() {
        let x = solve_row(&[10.0, 10.0, 10.0], &[5.0, 5.0], &[0.0, 0.0, 0.0]);
        assert_eq!(x, vec![-10.0, 5.0, 20.0]);
        let x = solve_row(&[10.0, 10.0], &[5.0], &[100.0, 0.0]);
        assert!(x[1] >= x[0] + 15.0 - 1e-9);
    }

    #[test]
    fn keyless_items_keep_their_slot() {
        let mut v = vec![0usize, 1, 2, 3];
        let keys = [Some(3.0), None, Some(1.0), Some(2.0)];
        sort_keyed(&mut v, |i| keys[i]);
        assert_eq!(v, vec![2, 1, 3, 0]);
    }
}
