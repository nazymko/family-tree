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
//! 3. Positions: a walk over the families from the top makes a tree of units. Each part is drawn on its own and
//!    packed against the others row by row (a tidy tree): children under their parents, siblings together by birth,
//!    the parents of someone who married in on the outer side of the couple.
//! 4. Lines: one connector per child from the couple to a bar; bars that would touch go in separate lanes, in the
//!    order that crosses the fewest lines. A line that passes a row goes through a column free of boxes.
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
    /// Height of the people's area under the heading (0 for an empty block).
    pub body_h: f32,
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
/// Distance between the lanes of a channel.
const LANE: f64 = 12.0;
/// The first lane, below the boxes of a row.
const BUS_TOP: f64 = 14.0;
/// Two bars closer than this end to end read as one line, so they take different lanes.
const BAR_GAP: f64 = 28.0;
const LANE_PASSES: usize = 8;
/// How many placements are tried per family block, and up to which size.
const MAX_TRIES: usize = 48;
const MAX_UNITS_TO_TRY: usize = 1500;
/// Lines to a child with two sets of parents arrive this far apart.
const SECOND_LINE: f64 = 14.0;
/// Horizontal gaps as parts of `Metrics::gap_x`: brothers and sisters closer than separate families.
const GAP_SIBLINGS: f64 = 0.6;
const GAP_GROUPS: f64 = 1.2;

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
}

/// What a placed part of the chart occupies, row by row, and the units placed directly in it.
struct Shape {
    spans: Vec<(f64, f64)>,
    items: Vec<(usize, f64)>,
}

impl Shape {
    fn empty(rows: usize) -> Self {
        Self {
            spans: vec![(f64::INFINITY, f64::NEG_INFINITY); rows],
            items: Vec::new(),
        }
    }

    fn is_used(span: (f64, f64)) -> bool {
        span.0 <= span.1
    }

    /// The shift nearest to `want` that keeps `other` wholly on one side of this shape, in every row both use.
    /// Without a wish, as close as it can go.
    fn clear_shift(
        &self,
        other: &Self,
        want: Option<f64>,
        right: bool,
        gap: impl Fn(usize) -> f64,
    ) -> f64 {
        let mut limit: Option<f64> = None;
        for (r, (&a, &b)) in self.spans.iter().zip(&other.spans).enumerate() {
            if Self::is_used(a) && Self::is_used(b) {
                let s = if right {
                    a.1 + gap(r) - b.0
                } else {
                    a.0 - gap(r) - b.1
                };
                limit = Some(limit.map_or(s, |l: f64| if right { l.max(s) } else { l.min(s) }));
            }
        }
        match (want, limit) {
            (Some(w), Some(l)) => {
                if right {
                    w.max(l)
                } else {
                    w.min(l)
                }
            }
            (Some(w), None) => w,
            (None, Some(l)) => l,
            (None, None) => 0.0,
        }
    }

    fn add(&mut self, other: Self, dx: f64) {
        for (a, b) in self.spans.iter_mut().zip(other.spans) {
            if Self::is_used(b) {
                *a = (a.0.min(b.0 + dx), a.1.max(b.1 + dx));
            }
        }
        self.items
            .extend(other.items.into_iter().map(|(u, x)| (u, x + dx)));
    }
}

/// A unit in a family's row of children, through `person`; `end` is the side of the row it was moved to.
struct Kid {
    unit: usize,
    person: usize,
    end: Option<bool>,
}

/// A family reached from a parent: its children hang below the unit.
struct Down {
    fam: usize,
    kids: Vec<Kid>,
    others: Vec<usize>,
}

/// A family reached from a child: the parents sit above and the brothers and sisters beside the unit.
struct Up {
    member: usize,
    fam: usize,
    right: bool,
    parents: Vec<usize>,
    sibs: Vec<Kid>,
}

#[derive(Default)]
struct Branches {
    downs: Vec<Down>,
    ups: Vec<Up>,
}

/// A member of the unit with parents the walk has not reached: they hang off the side of the unit.
fn rising(unit: &Unit, fam_seen: &[bool], sh: &Shared) -> Option<usize> {
    unit.members
        .iter()
        .copied()
        .find(|&m| sh.child_in[m].iter().any(|&f| !fam_seen[f]))
}

/// Children with parents-in-law go to the end of the row, the side their partner's family hangs from, because a
/// line to them from outside would otherwise cross every line to the brothers and sisters on that side. The others
/// keep their birth order. With `only` every such child goes to that side. `forced` holds the sides chosen to bring
/// the two ends of a loop together, which win over the rest.
fn arrange(
    (fam, kids): (usize, Vec<(usize, usize)>),
    units: &mut [Unit],
    fam_seen: &[bool],
    only: Option<bool>,
    forced: &Forced,
    sh: &Shared,
    sc: &mut Scratch,
) -> Vec<Kid> {
    let n = kids.len();
    let rises: Vec<Option<usize>> = kids
        .iter()
        .map(|&(u, _)| rising(&units[u], fam_seen, sh))
        .collect();
    let pinned = |unit: usize| forced.get(&(fam, unit)).filter(|_| only.is_none());
    let count = kids
        .iter()
        .zip(&rises)
        .filter(|((u, _), r)| r.is_some() && pinned(*u).is_none())
        .count();
    let (mut left, mut middle, mut right) = (Vec::new(), Vec::new(), Vec::new());
    let mut nth = 0;
    for (i, &(unit, person)) in kids.iter().enumerate() {
        let pin = pinned(unit).copied();
        if rises[i].is_none() && pin.is_none() {
            middle.push(Kid {
                unit,
                person,
                end: None,
            });
            continue;
        }
        let to_right = pin.map_or_else(
            || {
                let free = only.unwrap_or(if count == 1 { i * 2 >= n - 1 } else { nth > 0 });
                nth += 1;
                free
            },
            |(side, _)| side,
        );
        // Whoever links the unit to the other family, if anybody but the child does.
        let who = rises[i]
            .or_else(|| {
                let m = &units[unit].members;
                m.iter()
                    .copied()
                    .find(|&q| q != person && !sh.child_in[q].is_empty())
            })
            .unwrap_or(person);
        // The partner with the parents goes on the outside.
        let (pos_who, pos_kid) = (sc.pos_in_unit[who], sc.pos_in_unit[person]);
        if who != person && (pos_who > pos_kid) != to_right {
            units[unit].members.reverse();
            for (k, &m) in units[unit].members.iter().enumerate() {
                sc.pos_in_unit[m] = k;
            }
        }
        let kid = Kid {
            unit,
            person,
            end: Some(to_right),
        };
        if to_right {
            right.push(kid);
        } else {
            left.push(kid);
        }
    }
    // Stable: the far end of a loop stays outside the near end, the others keep their order.
    right.sort_by_key(|k: &Kid| pinned(k.unit).map_or(0, |&(_, rank)| rank));
    left.append(&mut middle);
    left.append(&mut right);
    left
}

/// The unit to start from: on the top row, with the most people below it.
fn pick_root(units: &[Unit], sh: &Shared, sc: &Scratch) -> usize {
    let mut by_row: Vec<usize> = (0..units.len()).collect();
    by_row.sort_by_key(|&u| std::cmp::Reverse(units[u].row));
    let mut below = vec![0.0f64; units.len()];
    for &u in &by_row {
        let mut seen: Vec<usize> = Vec::new();
        for &p in &units[u].members {
            for &f in &sh.partner_in[p] {
                for &c in &sh.fams[f].kids {
                    let cu = sc.unit_of[c];
                    if units[cu].row > units[u].row && !seen.contains(&cu) {
                        seen.push(cu);
                        below[u] += 1.0 + below[cu];
                    }
                }
            }
        }
    }
    let top = units.iter().map(|u| u.row).min().unwrap_or(0);
    let mut root = 0;
    let mut best = f64::NEG_INFINITY;
    for (u, unit) in units.iter().enumerate() {
        if unit.row == top && below[u] > best {
            best = below[u];
            root = u;
        }
    }
    root
}

/// Walks the families from the root. Each unit and family is reached once, so the walk is a tree, and links that
/// close a loop (cousins who marry, a second set of parents) stay plain lines. Returns the units in walk order.
fn walk(
    units: &mut [Unit],
    root: usize,
    forced: &Forced,
    sh: &Shared,
    sc: &mut Scratch,
    branches: &mut [Branches],
) -> Walked {
    let mut fam_seen = vec![false; sh.fams.len()];
    let mut parent: Vec<Option<(usize, usize, bool)>> = vec![None; units.len()];
    let mut loops: Vec<(usize, usize)> = Vec::new();
    let mut ends: Vec<(usize, usize, bool)> = Vec::new();
    let mut seen = vec![false; units.len()];
    let mut entry: Vec<Option<usize>> = vec![None; units.len()];
    let mut prefers_right = vec![true; units.len()];
    seen[root] = true;
    let mut order = vec![root];
    let mut head = 0;
    while head < order.len() {
        let u = order[head];
        head += 1;
        let members = units[u].members.clone();
        for &p in &members {
            for &f in &sh.partner_in[p] {
                if std::mem::replace(&mut fam_seen[f], true) {
                    continue;
                }
                let mut others = Vec::new();
                for &q in &sh.fams[f].partners {
                    let qu = sc.unit_of[q];
                    if !std::mem::replace(&mut seen[qu], true) {
                        parent[qu] = Some((u, f, false));
                        others.push(qu);
                        order.push(qu);
                    } else if qu != u {
                        loops.push((u, qu));
                    }
                }
                let mut kids = Vec::new();
                for &c in &sh.fams[f].kids {
                    let cu = sc.unit_of[c];
                    if !std::mem::replace(&mut seen[cu], true) {
                        parent[cu] = Some((u, f, true));
                        entry[cu] = Some(c);
                        kids.push((cu, c));
                        order.push(cu);
                    } else if cu != u && parent[cu] != Some((u, f, true)) {
                        loops.push((u, cu));
                    }
                }
                let kids = arrange((f, kids), units, &fam_seen, None, forced, sh, sc);
                for k in &kids {
                    prefers_right[k.unit] = k.end.unwrap_or(true);
                    if let Some(side) = k.end {
                        ends.push((f, k.unit, side));
                    }
                }
                branches[u].downs.push(Down {
                    fam: f,
                    kids,
                    others,
                });
            }
        }
        for &p in &members {
            for &f in &sh.child_in[p] {
                if std::mem::replace(&mut fam_seen[f], true) {
                    continue;
                }
                let right = match entry[u].filter(|&e| e != p) {
                    Some(e) => sc.pos_in_unit[p] > sc.pos_in_unit[e],
                    None => {
                        let (pos, last) = (sc.pos_in_unit[p] * 2, members.len() - 1);
                        if pos == last {
                            prefers_right[u]
                        } else {
                            pos > last
                        }
                    }
                };
                let mut parents = Vec::new();
                for &q in &sh.fams[f].partners {
                    let qu = sc.unit_of[q];
                    if !std::mem::replace(&mut seen[qu], true) {
                        parent[qu] = Some((u, f, false));
                        prefers_right[qu] = right;
                        parents.push(qu);
                        order.push(qu);
                    } else if qu != u {
                        loops.push((u, qu));
                    }
                }
                let mut sibs = Vec::new();
                for &c in &sh.fams[f].kids {
                    let cu = sc.unit_of[c];
                    if !std::mem::replace(&mut seen[cu], true) {
                        parent[cu] = Some((u, f, false));
                        entry[cu] = Some(c);
                        prefers_right[cu] = right;
                        sibs.push((cu, c));
                        order.push(cu);
                    } else if cu != u && parent[cu] != Some((u, f, false)) {
                        loops.push((u, cu));
                    }
                }
                let sibs = arrange((f, sibs), units, &fam_seen, Some(right), forced, sh, sc);
                branches[u].ups.push(Up {
                    member: p,
                    fam: f,
                    right,
                    parents,
                    sibs,
                });
            }
        }
    }
    Walked {
        order,
        parent,
        loops,
        ends,
    }
}

/// Sides of the row of children, by (family, unit): which side, and the order among those moved to the right.
type Forced = HashMap<(usize, usize), (bool, u8)>;

struct Walked {
    order: Vec<usize>,
    /// The unit, family and "as a child below it" each unit was reached through.
    parent: Vec<Option<(usize, usize, bool)>>,
    /// Pairs of units joined by a family the walk had already passed.
    loops: Vec<(usize, usize)>,
    /// The children moved to an end of their row: family, unit, side.
    ends: Vec<(usize, usize, bool)>,
}

/// Bends the branches around each loop towards each other, so the line that closes it is short: on the way up from
/// both ends of the link every child moves to the side of its family's row that faces the other end, and the two
/// branches that meet end up next to each other.
fn bend_loops(walked: &Walked, xs: &[f64]) -> Forced {
    let mut depth = vec![0usize; xs.len()];
    for &u in &walked.order {
        if let Some((p, _, _)) = walked.parent[u] {
            depth[u] = depth[p] + 1;
        }
    }
    let mut forced = Forced::new();
    for &(a, b) in &walked.loops {
        let (a, b) = if xs[a] <= xs[b] { (a, b) } else { (b, a) };
        let (mut p, mut q) = (a, b);
        let (mut tops_a, mut tops_b): (Option<usize>, Option<usize>) = (None, None);
        while p != q {
            let up_a = depth[p] >= depth[q];
            let v = if up_a { p } else { q };
            let Some((up, fam, kid)) = walked.parent[v] else {
                break;
            };
            if kid {
                forced.entry((fam, v)).or_insert((up_a, 0));
            }
            if up_a {
                tops_a = Some(v);
                p = up;
            } else {
                tops_b = Some(v);
                q = up;
            }
        }
        if let (Some(ta), Some(tb)) = (tops_a, tops_b)
            && let (Some((_, fa, true)), Some((_, fb, true))) =
                (walked.parent[ta], walked.parent[tb])
            && fa == fb
        {
            forced.insert((fa, ta), (true, 0));
            forced.insert((fa, tb), (true, 1));
        }
    }
    forced
}

/// How far children are from their parents, added up: the shorter, the easier to follow.
fn spread(xs: &[f64], units: &[Unit], sh: &Shared, sc: &Scratch) -> f64 {
    let x_of = |p: usize| {
        let u = sc.unit_of[p];
        xs[u] + sc.pos_in_unit[p] as f64 * f64::from(sh.m.node_w + sh.m.couple_gap)
    };
    let mut total = 0.0;
    for unit in units {
        for &p in &unit.members {
            for &f in &sh.partner_in[p] {
                total += sh.fams[f]
                    .kids
                    .iter()
                    .map(|&k| (x_of(k) - x_of(p)).abs())
                    .sum::<f64>();
            }
        }
    }
    total
}

/// Places every unit of a component: returns the left edge of each. Children sit under their parents, a family's
/// descendants stay together, and in-laws hang off the outer side. Parts are packed row by row, as close as their
/// outlines allow, so no two boxes can ever overlap whatever the data says.
fn place_units(units: &mut [Unit], rows_n: usize, sh: &Shared, sc: &mut Scratch) -> Vec<f64> {
    let start: Vec<Vec<usize>> = units.iter().map(|u| u.members.clone()).collect();
    let restore = |units: &mut [Unit], sc: &mut Scratch| {
        for (u, m) in units.iter_mut().zip(&start) {
            u.members.clone_from(m);
            for (k, &p) in m.iter().enumerate() {
                sc.pos_in_unit[p] = k;
            }
        }
    };
    // The plain walk, then the loops bent together, then each child at the end of a row tried on the other end;
    // whatever makes the lines shorter stays. Every try is a full placement, so big families get few.
    let mut forced = Forced::new();
    let (first, walked) = place_once(units, rows_n, &forced, sh, sc);
    let mut score = spread(&first, units, sh, sc);
    let mut ends = walked.ends.clone();
    if !walked.loops.is_empty() {
        let bent = bend_loops(&walked, &first);
        restore(units, sc);
        let (xs, w) = place_once(units, rows_n, &bent, sh, sc);
        let s = spread(&xs, units, sh, sc);
        if s < score {
            (score, forced, ends) = (s, bent, w.ends);
        }
    }
    let tries = if units.len() > MAX_UNITS_TO_TRY {
        0
    } else {
        MAX_TRIES
    };
    for &(fam, unit, side) in ends.iter().take(tries) {
        let mut trial = forced.clone();
        trial.insert((fam, unit), (!side, 0));
        restore(units, sc);
        let (xs, _) = place_once(units, rows_n, &trial, sh, sc);
        let s = spread(&xs, units, sh, sc);
        if s < score {
            (score, forced) = (s, trial);
        }
    }
    restore(units, sc);
    place_once(units, rows_n, &forced, sh, sc).0
}

fn place_once(
    units: &mut [Unit],
    rows_n: usize,
    forced: &Forced,
    sh: &Shared,
    sc: &mut Scratch,
) -> (Vec<f64>, Walked) {
    let m = sh.m;
    let node_w = f64::from(m.node_w);
    let step = node_w + f64::from(m.couple_gap);
    let gap_sibling = f64::from(m.gap_x) * GAP_SIBLINGS;
    let gap_group = f64::from(m.gap_x) * GAP_GROUPS;
    let root = pick_root(units, sh, sc);
    let mut branches: Vec<Branches> = (0..units.len()).map(|_| Branches::default()).collect();
    let walked = walk(units, root, forced, sh, sc, &mut branches);
    let order = &walked.order;

    let units: &[Unit] = units;
    let (pos, unit_of) = (&sc.pos_in_unit, &sc.unit_of);
    let centre = |p: usize| pos[p] as f64 * step + node_w / 2.0;
    let mut shapes: Vec<Option<Shape>> = (0..units.len()).map(|_| None).collect();
    let mut placed: Vec<Vec<(usize, f64)>> = vec![Vec::new(); units.len()];

    for &u in order.iter().rev() {
        let mut c = Shape::empty(rows_n);
        c.spans[units[u].row] = (0.0, units[u].w);
        let groups = |_: usize| gap_group;
        let mut downs = std::mem::take(&mut branches[u].downs);
        let anchor = |f: usize| {
            let inside: Vec<f64> = sh.fams[f]
                .partners
                .iter()
                .filter(|&&q| unit_of[q] == u)
                .map(|&q| centre(q))
                .collect();
            inside.iter().sum::<f64>() / inside.len().max(1) as f64
        };
        downs.sort_by(|a, b| anchor(a.fam).total_cmp(&anchor(b.fam)));
        for d in downs {
            let at = anchor(d.fam);
            let kid_row = |r: usize, k: &Kid| {
                if r == units[k.unit].row {
                    gap_sibling
                } else {
                    gap_group
                }
            };
            let mut mid = Shape::empty(rows_n);
            let mut ends: Vec<f64> = Vec::new();
            let (mut lefts, mut rights) = (Vec::new(), Vec::new());
            let mut balance = 0.0;
            for k in d.kids {
                let width = units[k.unit].w + gap_sibling;
                match k.end {
                    Some(true) => {
                        balance -= width / 2.0;
                        rights.push(k);
                    }
                    Some(false) => {
                        balance += width / 2.0;
                        lefts.push(k);
                    }
                    None => {
                        if let Some(t) = shapes[k.unit].take() {
                            let dx = mid.clear_shift(&t, None, true, |r| kid_row(r, &k));
                            mid.add(t, dx);
                            ends.push(dx + centre(k.person));
                        }
                    }
                }
            }
            let has_middle = !ends.is_empty();
            if has_middle {
                let middle_of = (ends[0] + ends[ends.len() - 1]) / 2.0;
                let dx = c.clear_shift(&mid, Some(at - middle_of + balance), true, groups);
                c.add(mid, dx);
            }
            let mut wish_centre = !has_middle;
            for k in rights {
                if let Some(t) = shapes[k.unit].take() {
                    let want = wish_centre.then(|| at - centre(k.person));
                    wish_centre = false;
                    let dx = c.clear_shift(&t, want, true, |r| kid_row(r, &k));
                    c.add(t, dx);
                }
            }
            for k in lefts.iter().rev() {
                if let Some(t) = shapes[k.unit].take() {
                    let want = wish_centre.then(|| at - centre(k.person));
                    wish_centre = false;
                    let dx = c.clear_shift(&t, want, false, |r| kid_row(r, k));
                    c.add(t, dx);
                }
            }
            for o in d.others {
                if let Some(t) = shapes[o].take() {
                    let dx = c.clear_shift(&t, None, true, groups);
                    c.add(t, dx);
                }
            }
        }

        // Parents and their other children, each block on its own side.
        let row = units[u].row;
        let near = |r: usize| if r == row { gap_sibling } else { gap_group };
        let mut blocks: Vec<(Shape, f64, bool)> = Vec::new();
        for up in std::mem::take(&mut branches[u].ups) {
            let member_cx = centre(up.member);
            let mut block = Shape::empty(rows_n);
            let mut sib_ends: Vec<f64> = Vec::new();
            for k in &up.sibs {
                if let Some(t) = shapes[k.unit].take() {
                    let dx = block.clear_shift(&t, None, true, near);
                    block.add(t, dx);
                    sib_ends.push(dx + centre(k.person));
                }
            }
            // Where the member sits in the block's frame, next to the nearest brother or sister.
            let span = block.spans[row];
            let member_at = if !Shape::is_used(span) {
                0.0
            } else if up.right {
                span.0 - gap_sibling - (units[u].w - member_cx)
            } else {
                span.1 + gap_sibling + member_cx
            };
            let lo = sib_ends.iter().copied().fold(member_at, f64::min);
            let hi = sib_ends.iter().copied().fold(member_at, f64::max);
            let over = f64::midpoint(lo, hi);
            let mut parents = Shape::empty(rows_n);
            let mut xs: Vec<f64> = Vec::new();
            for &q in &up.parents {
                if let Some(t) = shapes[q].take() {
                    let dx = parents.clear_shift(&t, None, true, groups);
                    parents.add(t, dx);
                    for &p in &sh.fams[up.fam].partners {
                        if unit_of[p] == q {
                            xs.push(dx + centre(p));
                        }
                    }
                }
            }
            if !xs.is_empty() {
                let mean = xs.iter().sum::<f64>() / xs.len() as f64;
                let dx = block.clear_shift(&parents, Some(over - mean), up.right, groups);
                block.add(parents, dx);
            }
            blocks.push((block, member_cx - member_at, up.right));
        }
        // Parents of both partners: share the room above the couple evenly.
        let first_left = blocks.iter().position(|b| !b.2);
        let first_right = blocks.iter().position(|b| b.2);
        if let (Some(l), Some(r)) = (first_left, first_right) {
            let (lw, rw) = (blocks[l].1, blocks[r].1);
            let mut excess = 0.0f64;
            for (i, (&a, &b)) in blocks[l].0.spans.iter().zip(&blocks[r].0.spans).enumerate() {
                if Shape::is_used(a) && Shape::is_used(b) {
                    excess = excess.max(a.1 + lw + groups(i) - (b.0 + rw));
                }
            }
            blocks[l].1 -= excess / 2.0;
            blocks[r].1 += excess / 2.0;
        }
        for (block, want, right) in blocks {
            let dx = c.clear_shift(&block, Some(want), right, near);
            c.add(block, dx);
        }

        placed[u] = std::mem::take(&mut c.items);
        c.items.push((u, 0.0));
        shapes[u] = Some(c);
    }

    let mut xs = vec![0.0f64; units.len()];
    for &u in order {
        for &(v, dx) in &placed[u] {
            xs[v] = xs[u] + dx;
        }
    }
    (xs, walked)
}

/// A column free of boxes in the rows between `r0` and `r1`, nearest to the two ends of a line.
fn free_column(
    row_px: &[Vec<f64>],
    (r0, r1): (usize, usize),
    (xa, xb): (f64, f64),
    node_w: f64,
    side: f64,
) -> f64 {
    let between = &row_px[r0 + 1..r1];
    let mut cands = vec![xa, xb];
    for &x in between.iter().flatten() {
        cands.push(x - side);
        cands.push(x + node_w + side);
    }
    let free = |x: f64| {
        between
            .iter()
            .flatten()
            .all(|&px| x < px - 2.0 || x > px + node_w + 2.0)
    };
    cands
        .into_iter()
        .filter(|&x| free(x))
        .min_by(|p, q| {
            ((p - xa).abs() + (p - xb).abs())
                .total_cmp(&((q - xa).abs() + (q - xb).abs()))
                .then(p.total_cmp(q))
        })
        .unwrap_or(xa)
}

/// Where a couple's child lines start, and the extent of their bar.
struct Link {
    fam: usize,
    anchor_x: f64,
    anchor_row: usize,
    /// Start at the vertical middle of a partner line instead of below the boxes.
    mid: bool,
    kid_xs: Vec<f64>,
    lo: f64,
    hi: f64,
}

/// The bars of one channel, top to bottom: each one goes in the first lane below every earlier bar it would touch.
/// Neighbours swap when that makes fewer lines cross. Returns the number of lanes.
fn assign_lanes(links: &[Link], ids: &mut [usize], lane_of: &mut [usize]) -> usize {
    ids.sort_by(|&a, &b| {
        let key = |l: &Link| (l.lo, l.hi, l.fam);
        key(&links[a])
            .partial_cmp(&key(&links[b]))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let touches = |a: &Link, b: &Link| a.lo < b.hi + BAR_GAP && b.lo < a.hi + BAR_GAP;
    // How many lines cross with `a`'s bar above `b`'s.
    let cost = |a: &Link, b: &Link| {
        let inside = |x: f64, l: &Link| x > l.lo && x < l.hi;
        a.kid_xs.iter().filter(|&&x| inside(x, b)).count() + usize::from(inside(b.anchor_x, a))
    };
    for _ in 0..LANE_PASSES {
        let mut changed = false;
        for i in 1..ids.len() {
            let (a, b) = (&links[ids[i - 1]], &links[ids[i]]);
            if touches(a, b) && cost(b, a) < cost(a, b) {
                ids.swap(i - 1, i);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut lanes = 0;
    for i in 0..ids.len() {
        let a = &links[ids[i]];
        let lane = ids[..i]
            .iter()
            .filter(|&&b| touches(a, &links[b]))
            .map(|&b| lane_of[b] + 1)
            .max()
            .unwrap_or(0);
        lane_of[ids[i]] = lane;
        lanes = lanes.max(lane + 1);
    }
    lanes
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
    relax(members, sh, sc);
    let low = members.iter().map(|&p| sc.level[p]).min().unwrap_or(0);
    for &p in members {
        sc.level[p] -= low;
    }
}

/// Passes of the relaxation. Data without a cycle settles in a handful; a cycle stops here.
const RELAX_PASSES: usize = 48;

/// Makes parents lie above their children wherever the data allows it. First partners are pulled onto
/// one row as well; when that cannot be satisfied together with the parent rule (a person who is
/// partner of their own ancestor, say), the partner rule is dropped and only parents-above-children is
/// kept. Levels only ever move down, so every pass is bounded and the result is deterministic.
fn relax(members: &[usize], sh: &Shared, sc: &mut Scratch) {
    let mut fams: Vec<usize> = members
        .iter()
        .flat_map(|&p| sh.partner_in[p].iter().chain(&sh.child_in[p]).copied())
        .collect();
    fams.sort_unstable();
    fams.dedup();
    let start: Vec<i32> = members.iter().map(|&p| sc.level[p]).collect();
    if !settle(&fams, sh, sc, true) {
        for (&p, &l) in members.iter().zip(&start) {
            sc.level[p] = l;
        }
        settle(&fams, sh, sc, false);
    }
}

fn settle(fams: &[usize], sh: &Shared, sc: &mut Scratch, equalise: bool) -> bool {
    for _ in 0..RELAX_PASSES {
        let mut changed = false;
        for &f in fams {
            let fam = &sh.fams[f];
            let Some(top) = fam.partners.iter().map(|&p| sc.level[p]).max() else {
                continue;
            };
            if equalise {
                for &p in &fam.partners {
                    if sc.level[p] < top {
                        sc.level[p] = top;
                        changed = true;
                    }
                }
            }
            for &c in &fam.kids {
                if sc.level[c] <= top {
                    sc.level[c] = top + 1;
                    changed = true;
                }
            }
        }
        if !changed {
            return true;
        }
    }
    false
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
    let mut units = make_units(members, sh, sc);
    let rows_n = members
        .iter()
        .map(|&p| sc.level[p] as usize)
        .max()
        .unwrap_or(0)
        + 1;
    let xs = place_units(&mut units, rows_n, sh, sc);

    // Left edge to zero, person boxes.
    let min_x = xs.iter().copied().fold(f64::INFINITY, f64::min);
    for (u, &x) in units.iter().zip(&xs) {
        for (k, &p) in u.members.iter().enumerate() {
            sc.px[p] = x - min_x + k as f64 * step_p;
            sc.row[p] = u.row;
        }
    }
    let cx = |sc: &Scratch, p: usize| sc.px[p] + node_w / 2.0;

    // Where each child line ends: a child with two sets of parents gets two lines side by side.
    let kid_x = |sc: &Scratch, k: usize, f: usize| {
        let sets = &sh.child_in[k];
        let i = sets.iter().position(|&s| s == f).unwrap_or(0) as f64;
        cx(sc, k) + (i - (sets.len() - 1) as f64 / 2.0) * SECOND_LINE
    };
    // Where each child line starts (anchor), and the lane of its bar.
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
            let kid_xs: Vec<f64> = fam.kids.iter().map(|&k| kid_x(sc, k, f)).collect();
            let lo = kid_xs.iter().copied().fold(ax, f64::min);
            let hi = kid_xs.iter().copied().fold(ax, f64::max);
            links.push(Link {
                fam: f,
                anchor_x: ax,
                anchor_row: arow,
                mid,
                kid_xs,
                lo,
                hi,
            });
        }
    }
    // Lanes per channel (the gap below a row).
    let mut lane_of = vec![0usize; links.len()];
    let mut lanes = vec![0usize; rows_n];
    let mut by_channel: Vec<Vec<usize>> = vec![Vec::new(); rows_n];
    for (i, l) in links.iter().enumerate() {
        by_channel[l.anchor_row].push(i);
    }
    for (r, ids) in by_channel.iter_mut().enumerate() {
        lanes[r] = assign_lanes(&links, ids, &mut lane_of);
    }
    let channel =
        |r: usize| (f64::from(m.gap_y) * 4.0).max(BUS_TOP + 14.0 + lanes[r] as f64 * LANE);
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

    let mut row_px: Vec<Vec<f64>> = vec![Vec::new(); rows_n];
    for &p in members {
        row_px[sc.row[p]].push(sc.px[p]);
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
        } else if sc.row[a] != sc.row[b] {
            // Down into the channel under the upper partner, across to a column that no box in the rows
            // between occupies, down to the channel above the lower partner's row, across, and in at the
            // top of the lower partner's box: never through a box.
            let (ra, rb) = if sc.row[a] < sc.row[b] {
                (a, b)
            } else {
                (b, a)
            };
            let (r0, r1) = (sc.row[ra], sc.row[rb]);
            let (yr, yl) = (row_y[r0], row_y[r1]);
            let side = f64::from(m.gap_x) * 0.3;
            let (xa, xb) = (cx(sc, ra), cx(sc, rb) + 14.0);
            let xv = free_column(&row_px, (r0, r1), (xa, xb), node_w, side);
            let (ca, cb) = (yr + node_h + 4.0, yl - 6.0);
            vec![xa, yr + node_h, xa, ca, xv, ca, xv, cb, xb, cb, xb, yl]
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
        let (a, b) = if sc.row[a] > sc.row[b] {
            (b, a)
        } else {
            (a, b)
        };
        out.edges.push((a, b, EdgeKind::Partner, pts));
    }
    // Brothers and sisters of a family with no partner on record: nothing above them to hang a line from,
    // so each neighbouring pair is joined by a bracket over their tops. Parent lines never go here.
    let mut loose: Vec<usize> = members
        .iter()
        .flat_map(|&k| sh.child_in[k].iter().copied())
        .filter(|&f| sh.fams[f].partners.is_empty())
        .collect();
    loose.sort_unstable();
    loose.dedup();
    for f in loose {
        for pair in sh.fams[f].kids.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let top = row_y[sc.row[a]].min(row_y[sc.row[b]]);
            let bus = top - BUS_TOP.min(f64::from(m.gap_y) * 2.0);
            let (xa, xb) = (cx(sc, a), cx(sc, b));
            out.edges.push((
                a,
                b,
                EdgeKind::Parent,
                vec![xa, row_y[sc.row[a]], xa, bus, xb, bus, xb, row_y[sc.row[b]]],
            ));
        }
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
        let bus = row_y[l.anchor_row] + node_h + BUS_TOP + lane_of[i] as f64 * LANE;
        for (&k, &kx) in fam.kids.iter().zip(&l.kid_xs) {
            let row = sc.row[k];
            let end = if row > l.anchor_row {
                row_y[row]
            } else {
                row_y[row] + node_h
            };
            let mut pts = vec![l.anchor_x, ay, l.anchor_x, bus];
            if row > l.anchor_row + 1 {
                // Past a row of boxes: through a free column, then along the channel above the child.
                let side = f64::from(m.gap_x) * 0.3;
                let xv = free_column(&row_px, (l.anchor_row, row), (l.anchor_x, kx), node_w, side);
                let near = row_y[row] - 6.0;
                pts.extend([xv, bus, xv, near, kx, near]);
            } else {
                pts.extend([kx, bus]);
            }
            pts.extend([kx, end]);
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
    whole_directed(input, m, false)
}

/// Like [`whole`]; with `reversed` every family is drawn upside down (the oldest generation at the
/// bottom of its block). Headings, block positions, node ids and navigation links stay as they are.
pub fn whole_directed(input: &WholeInput, m: &Metrics, reversed: bool) -> WholeLayout {
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
            body_h: comp.map_or(0.0, |c| c.h) as f32,
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
                        y: (oy
                            + if reversed {
                                c.h - py - f64::from(m.node_h)
                            } else {
                                py
                            }) as f32,
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
                    .flat_map(|q| {
                        let qy = if reversed { c.h - q[1] } else { q[1] };
                        [(x + q[0]) as f32, (oy + qy) as f32]
                    })
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
        let wide = (((shelf_w + f64::from(m.gap_x)) / pitch_x).floor() as usize).max(1);
        // Roughly square, so one person is not a pan across the whole shelf.
        let cols = ((loose.len() as f64).sqrt().ceil() as usize).clamp(1, wide.max(1));
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
            body_h: (loose.len().div_ceil(cols) as f64 * pitch_y - f64::from(m.gap_y)) as f32,
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
