//! Relatives chart: an hourglass centred on one person.
//!
//! Rows are generations (negative above the centred person). The structure is a chain of
//! *blocks*, one per ancestor, each holding a row *group*: the person and, for the centred
//! person and for the parents, their siblings (aunts and uncles when the option is on), each
//! with partners and a small tree of children below. A block's ancestors form the blocks above
//! it, side by side, and the group hangs under them. Every block owns a horizontal interval
//! that no other block of the same row shares, so boxes cannot overlap by construction.
//!
//! Work lists replace recursion; the only nesting is bounded by the generation limit.

use std::collections::{HashMap, VecDeque};

use crate::route::{Family, route};
use crate::types::{
    ChartSource, Edge, EdgeKind, Key, Layout, Metrics, Node, PartnerOf, Rect, Rel, dup_edge,
    link_children, new_node,
};

/// What to show around the centred person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelativesOptions {
    /// Generations shown above and below the centred person.
    pub generations: u8,
    /// Show the parents' brothers and sisters.
    pub aunts: bool,
}

/// Where a row group hangs: which of the block's parents (slot 0 and slot 1) it descends from.
type Hang = [bool; 2];

/// Which small tree of children hangs below a unit.
#[derive(Clone, Copy)]
enum Tree {
    /// The centred person: descendants down to the generation limit, with partners.
    Focus(u8),
    /// A sibling: children only, one generation.
    Niece,
    /// A parent's sibling: children only, one generation (the cousins).
    Aunt,
    /// A brother or sister of the person or of an ancestor (all-relatives mode): every descendant. The
    /// number is the generations from the person up to the common ancestor.
    Kin(u8),
}

/// How a blood relative `up` generations above the common ancestor and `down` below it is named.
fn kin_rel(up: u8, down: u8) -> Rel {
    match (up, down) {
        (1, 2) => Rel::NieceNephew,
        _ => Rel::Kin { up, down },
    }
}

/// The partner of a relative described by [`kin_rel`].
fn kin_partner(up: u8, down: u8) -> Rel {
    Rel::Partner(match (up, down) {
        (1, 1) => PartnerOf::Sibling,
        (2, 1) => PartnerOf::AuntUncle,
        _ => PartnerOf::Kin { up, down },
    })
}

/// A brother or sister of the ancestor `gens` generations up (0 = the centred person).
fn sibling_rel(gens: u8, half: bool) -> Rel {
    match gens {
        0 => Rel::Sibling { half },
        1 => Rel::AuntUncle { half },
        g => Rel::Kin {
            up: g.saturating_add(1),
            down: 1,
        },
    }
}

impl Tree {
    fn limit(self) -> u8 {
        match self {
            Tree::Focus(n) => n,
            Tree::Niece => 1,
            Tree::Aunt => 1,
            Tree::Kin(_) => 250,
        }
    }
}

struct Unit {
    /// Node id of the person; partners are the next `partners` node ids.
    person: u32,
    partners: u32,
    row: i32,
    /// A parent's sibling's child: drawn on a row of its own.
    rel_cousin: bool,
    /// Partner node whose union produced this unit (None: the parent's own bottom edge).
    attach: Option<u32>,
    children: Vec<usize>,
    /// Group roots only: the parents this person descends from.
    hang: Hang,
    /// The extent of the unit and of everything below it, one `(left, right)` pair per row below,
    /// measured from the unit's centre. Only rows that are present count: a neighbour has to clear
    /// each row of this contour, not a box around the whole subtree.
    ctr: Contour,
    /// Child centres, from this unit's centre.
    coff: Vec<f32>,
    /// Which way the children hang: -1 to the left, 1 to the right, 0 centred. A sibling's children
    /// hang away from the centred person, so they do not hold the siblings far apart.
    lean: i8,
    /// Absolute centre of the unit (person plus partners).
    cx: f32,
}

type Contour = Vec<(f32, f32)>;

/// Place contours left to right, each as close as `gap` to everything already placed, row by row
/// (Reingold and Tilford). Returns the centre of each item and the merged contour.
fn pack(items: &[&Contour], gap: f32) -> (Vec<f32>, Contour) {
    let mut places = Vec::with_capacity(items.len());
    let mut acc: Contour = Vec::new();
    for c in items {
        let shift = if acc.is_empty() {
            0.0
        } else {
            acc.iter()
                .zip(c.iter())
                .map(|(a, b)| a.1 + gap - b.0)
                .fold(f32::NEG_INFINITY, f32::max)
        };
        places.push(shift);
        for (d, &(lo, hi)) in c.iter().enumerate() {
            if let Some(a) = acc.get_mut(d) {
                a.0 = a.0.min(lo + shift);
                a.1 = a.1.max(hi + shift);
            } else {
                acc.push((lo + shift, hi + shift));
            }
        }
    }
    (places, acc)
}

struct Block {
    row: i32,
    /// The unit of the block's own person.
    anchor: usize,
    /// Group roots in display order.
    group: Vec<usize>,
    /// Blocks of the parents by slot.
    slots: [Option<usize>; 2],
    ups: Vec<usize>,
    up_off: Vec<f32>,
    /// Group unit centres, from the group's left edge.
    gplace: Vec<f32>,
    w: f32,
    /// Centre of the anchor person's box, from the block's left edge.
    acx: f32,
    goff: f32,
    left: f32,
    /// Parent slot this block fills (None: the centred person's own block).
    side: Option<usize>,
    /// All-relatives mode: absolute centre of the anchor's box.
    ax: f32,
}

type Pending = Vec<(Option<u32>, Vec<Key>)>;

struct Sibling {
    key: Key,
    hang: Hang,
    half: bool,
}

/// The siblings of `x` (including `x`) in birth order, with the parents each descends from.
/// A sibling is full when the union that produced it pairs the two parents of `x`.
fn siblings(src: &dyn ChartSource, x: Key) -> Vec<Sibling> {
    let ps = src.parents(x);
    let mut out: Vec<Sibling> = Vec::new();
    for slot in 0..2 {
        let Some(p) = ps[slot] else { continue };
        let other = ps[1 - slot];
        for (partner, kids) in src.unions(p) {
            // With one parent unknown nothing says the partner is a different person: do not claim "half".
            let full = other.is_none() || partner == other;
            for k in kids {
                if ps.contains(&Some(k)) {
                    continue;
                }
                match out.iter_mut().find(|s| s.key == k) {
                    Some(s) => {
                        s.hang[slot] = true;
                        s.half &= !full;
                    }
                    None => {
                        let mut hang = [false; 2];
                        hang[slot] = true;
                        out.push(Sibling {
                            key: k,
                            hang,
                            half: !full,
                        });
                    }
                }
            }
        }
    }
    // The other children of every family `x` is a child of, partners or not: a family without partners is not
    // reached from any parent, and a family beside the preferred one belongs to other parents.
    for (fps, kids) in src.child_families(x) {
        let half = fps.iter().any(Option::is_some);
        for k in kids {
            if k == x || ps.contains(&Some(k)) || out.iter().any(|s| s.key == k) {
                continue;
            }
            out.push(Sibling {
                key: k,
                hang: [false, false],
                half,
            });
        }
    }
    if !out.iter().any(|s| s.key == x) {
        out.push(Sibling {
            key: x,
            hang: [true, true],
            half: false,
        });
    }
    // Stable: undated people keep their listed order after the dated ones.
    out.sort_by_key(|s| src.birth_sort(s.key).unwrap_or(i32::MAX));
    out
}

impl Sibling {
    /// Hangs from no parent drawn on the chart: joined to its brothers and sisters by a bar instead.
    fn barred(&self) -> bool {
        self.hang == [false, false]
    }
}

/// The least gap between two partners' cards on these charts: wide enough for the partner connector, with the
/// children's line hanging from its middle, to be seen.
const PARTNER_GAP: f32 = 24.0;

fn partner_room(m: &Metrics) -> Metrics {
    Metrics {
        couple_gap: m.couple_gap.max(PARTNER_GAP),
        ..*m
    }
}

struct Builder<'a> {
    src: &'a dyn ChartSource,
    max_nodes: usize,
    rect: Rect,
    nodes: Vec<Node>,
    visited: HashMap<Key, u32>,
    units: Vec<Unit>,
    pending: Vec<Pending>,
    blocks: Vec<Block>,
    truncated: bool,
    /// Brothers, sisters and their families are drawn (the relatives chart); off for the chart of one line.
    siblings: bool,
    /// All relatives: every ancestor on the line gets its brothers and sisters and all their descendants.
    all: bool,
}

impl Builder<'_> {
    fn add_node(&mut self, key: Key, row: i32, rel: Rel) -> u32 {
        let id = self.nodes.len() as u32;
        let mut n = new_node(id, key, self.rect, row as i16);
        n.rel = Some(rel);
        match self.visited.get(&key) {
            Some(&first) => n.dup_of = Some(first),
            None => {
                self.visited.insert(key, id);
            }
        }
        self.nodes.push(n);
        id
    }

    fn make_unit(
        &mut self,
        key: Key,
        row: i32,
        rel: Rel,
        partner_rel: Option<Rel>,
        attach: Option<u32>,
    ) -> Option<usize> {
        if self.nodes.len() >= self.max_nodes {
            self.truncated = true;
            return None;
        }
        let person = self.add_node(key, row, rel);
        let mut partners = 0;
        let mut pend: Pending = Vec::new();
        if self.nodes[person as usize].dup_of.is_none() {
            for (partner, kids) in self.src.unions(key) {
                let mut at = None;
                if let (Some(pk), Some(pr)) = (partner, partner_rel) {
                    if self.nodes.len() >= self.max_nodes {
                        self.truncated = true;
                    } else {
                        at = Some(self.add_node(pk, row, pr));
                        partners += 1;
                    }
                }
                pend.push((at, kids));
            }
        }
        self.pending.push(pend);
        self.units.push(Unit {
            person,
            partners,
            row,
            rel_cousin: rel == Rel::Cousin,
            attach,
            children: Vec::new(),
            hang: [true, true],
            ctr: Vec::new(),
            coff: Vec::new(),
            lean: 0,
            cx: 0.0,
        });
        Some(self.units.len() - 1)
    }

    /// Create the children of `root` that `tree` allows, breadth first.
    fn expand(&mut self, root: usize, tree: Tree) {
        let limit = usize::from(tree.limit());
        let mut queue = VecDeque::from([(root, 0usize)]);
        while let Some((u, depth)) = queue.pop_front() {
            let unions = std::mem::take(&mut self.pending[u]);
            let person = self.units[u].person;
            if unions.iter().all(|(_, c)| c.is_empty()) {
                continue;
            }
            if depth >= limit {
                self.nodes[person as usize].more_down = true;
                continue;
            }
            let row = self.units[u].row + 1;
            let step = (depth + 1) as u8;
            let (rel, partner_rel) = match tree {
                Tree::Focus(_) => (
                    Rel::Descendant(step),
                    Some(Rel::Partner(PartnerOf::Descendant(step))),
                ),
                Tree::Niece => (Rel::NieceNephew, None),
                Tree::Aunt => (Rel::Cousin, None),
                Tree::Kin(up) => {
                    let down = step.saturating_add(1);
                    (kin_rel(up, down), Some(kin_partner(up, down)))
                }
            };
            let mut kid_nodes = Vec::new();
            'unions: for (attach, kids) in unions {
                for key in kids {
                    let Some(cu) = self.make_unit(key, row, rel, partner_rel, attach) else {
                        self.nodes[person as usize].more_down = true;
                        break 'unions;
                    };
                    self.units[u].children.push(cu);
                    kid_nodes.push(self.units[cu].person);
                    queue.push_back((cu, depth + 1));
                }
            }
            link_children(&mut self.nodes, person, &kid_nodes);
        }
    }

    /// `side` is the parent slot (0 father side, 1 mother side) the block fills, if it is a parent's block.
    fn make_block(
        &mut self,
        key: Key,
        row: i32,
        o: &RelativesOptions,
        side: Option<usize>,
    ) -> Option<usize> {
        if self.nodes.len() >= self.max_nodes {
            self.truncated = true;
            return None;
        }
        let dup = self.visited.contains_key(&key);
        let focus = row == 0;
        // All-relatives mode makes the line first and the families afterwards (`fill_group`), so a relative
        // who is also an ancestor never takes the ancestor's place.
        let with_sibs = !dup && !self.all && self.siblings && (focus || (row == -1 && o.aunts));
        let sibs = if with_sibs {
            let mut v = siblings(self.src, key);
            // The parent stands next to the other parent: last on the left side, first on the right.
            if let Some(i) = v.iter().position(|s| s.key == key) {
                let me = v.remove(i);
                match side {
                    Some(1) => v.insert(0, me),
                    Some(_) => v.push(me),
                    None => v.insert(i, me),
                }
            }
            v
        } else {
            vec![Sibling {
                key,
                hang: [true, true],
                half: false,
            }]
        };
        let (x_rel, x_partner) = if focus {
            (Rel::Focus, Some(Rel::Partner(PartnerOf::Focus)))
        } else {
            (Rel::Ancestor((-row) as u8), None)
        };
        let anchor = self.make_unit(key, row, x_rel, x_partner, None)?;
        if focus {
            self.expand(anchor, Tree::Focus(o.generations));
        }
        let mut group = Vec::new();
        for s in sibs {
            let u = if s.key == key {
                anchor
            } else {
                let (rel, pr, tree) = if focus {
                    (
                        Rel::Sibling { half: s.half },
                        Rel::Partner(PartnerOf::Sibling),
                        Tree::Niece,
                    )
                } else {
                    (
                        Rel::AuntUncle { half: s.half },
                        Rel::Partner(PartnerOf::AuntUncle),
                        Tree::Aunt,
                    )
                };
                let Some(u) = self.make_unit(s.key, row, rel, Some(pr), None) else {
                    continue;
                };
                self.expand(u, tree);
                u
            };
            self.units[u].hang = s.hang;
            group.push(u);
        }
        if let Some(ai) = group.iter().position(|&u| u == anchor) {
            for (k, &u) in group.iter().enumerate() {
                self.units[u].lean = match k.cmp(&ai) {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                };
            }
        }
        self.blocks.push(Block {
            row,
            anchor,
            group,
            slots: [None, None],
            ups: Vec::new(),
            up_off: Vec::new(),
            gplace: Vec::new(),
            w: 0.0,
            acx: 0.0,
            goff: 0.0,
            left: 0.0,
            side,
            ax: 0.0,
        });
        Some(self.blocks.len() - 1)
    }

    /// All-relatives mode: the brothers and sisters of block `bi`'s anchor, each with partners and all
    /// descendants. They stand on the outer side of the anchor: the father's slot on the left, the mother's on
    /// the right; the centred person's own brothers and sisters are placed in birth order around them.
    fn fill_group(&mut self, bi: usize) {
        let (anchor, row, side) = {
            let b = &self.blocks[bi];
            (b.anchor, b.row, b.side)
        };
        let person = self.units[anchor].person;
        if self.nodes[person as usize].dup_of.is_some() {
            return;
        }
        let key = self.nodes[person as usize].key;
        let gens = u8::try_from(-row).unwrap_or(u8::MAX);
        let mut sibs = siblings(self.src, key);
        if let Some(i) = sibs.iter().position(|s| s.key == key) {
            let me = sibs.remove(i);
            // Brothers and sisters stand on the outer side of the ancestor's couple, away from the partner: the left
            // partner's to the left, the right partner's to the right. Without a partner, on the outer side of the line.
            let slot = self.blocks.iter().find_map(|c| {
                (0..2)
                    .find(|&s| c.slots[s] == Some(bi))
                    .filter(|_| c.slots[0].is_some() && c.slots[1].is_some())
            });
            let left = slot.map_or(side == Some(0), |s| s == 0);
            match side {
                None => sibs.insert(i, me),
                Some(_) if left => sibs.push(me),
                Some(_) => sibs.insert(0, me),
            }
        }
        let mut group = Vec::new();
        for s in sibs {
            let u = if s.key == key {
                anchor
            } else {
                let up = gens.saturating_add(1);
                let rel = sibling_rel(gens, s.half);
                let Some(u) = self.make_unit(s.key, row, rel, Some(kin_partner(up, 1)), None)
                else {
                    continue;
                };
                self.expand(u, Tree::Kin(up));
                u
            };
            self.units[u].hang = s.hang;
            group.push(u);
        }
        if let Some(ai) = group.iter().position(|&u| u == anchor) {
            for (k, &u) in group.iter().enumerate() {
                self.units[u].lean = match k.cmp(&ai) {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                };
            }
        }
        self.blocks[bi].group = group;
    }
}

/// Lay out the relatives of `root`: ancestors up and descendants down `o.generations` rows,
/// the siblings and their children, and (when `o.aunts`) the parents' siblings.
pub fn relatives(
    src: &dyn ChartSource,
    root: Key,
    o: &RelativesOptions,
    m: &Metrics,
    max_nodes: usize,
) -> Layout {
    lay_out(src, root, o, true, false, false, m, max_nodes)
}

/// The chart "built around" one person: only their own line. The person stands in the middle, their
/// ancestors in rows above (the father's and the mother's lines, spreading to the sides) and their
/// descendants in rows below. The person's partners stand beside them and so do the partners of each
/// descendant; an ancestor's only partner is the other parent on the line. No brothers or sisters, aunts,
/// uncles, cousins or in-laws' families. `generations` is the number of rows above and below (255 = all).
pub fn around(
    src: &dyn ChartSource,
    root: Key,
    generations: u8,
    m: &Metrics,
    max_nodes: usize,
) -> Layout {
    let o = RelativesOptions {
        generations,
        aunts: false,
    };
    lay_out(src, root, &o, false, false, false, m, max_nodes)
}

/// [`around`] with the person's brothers and sisters beside them, each with their partners and descendants.
/// Aunts, uncles and cousins stay out.
pub fn around_with_siblings(
    src: &dyn ChartSource,
    root: Key,
    generations: u8,
    m: &Metrics,
    max_nodes: usize,
) -> Layout {
    let o = RelativesOptions {
        generations,
        aunts: false,
    };
    lay_out(src, root, &o, true, true, false, m, max_nodes)
}

/// [`around`] with all relatives: the brothers and sisters of the person and of every ancestor on the line
/// (aunts, uncles, great-aunts and so on) and ALL their descendants with partners, as far down as there are
/// any. Everyone stands in their own generation row: a cousin in the person's row, a cousin's child one
/// below, a grandparent's brother in the grandparents' row. The person stays in the middle, the line a compact
/// spine (each couple of parents side by side); the families hang on the outer sides and widen the chart
/// instead of overlapping.
pub fn around_all_kin(
    src: &dyn ChartSource,
    root: Key,
    generations: u8,
    m: &Metrics,
    max_nodes: usize,
) -> Layout {
    let o = RelativesOptions {
        generations,
        aunts: false,
    };
    lay_out(src, root, &o, true, true, true, m, max_nodes)
}

/// An ancestor block whose brothers and sisters stand inside the line: block, anchor, left side, units, offsets.
type InnerGroup = (usize, usize, bool, Vec<usize>, Vec<f32>);

/// All-relatives mode: absolute places for the line and the families. The line is a compact spine: the centred
/// person in the middle, each couple of parents side by side (one card width plus the partner gap apart) over the
/// middle of their child; where the couples of one row would meet, the father's side moves left and the
/// mother's side right, the least that keeps them apart. Then every ancestor's brothers and sisters, with
/// partners and all descendants, are hung outward on their side (the father's line on the left, the mother's on
/// the right), each clearing, row by row, everything nearer the middle: the person's own family, the line, and the
/// families placed before. Long bars out to a far family are fine; parents pulled apart are not.
fn solve_all(units: &mut [Unit], blocks: &mut [Block], m: &Metrics) {
    let gap = m.gap_x;
    let nw = m.node_w;
    let pstep = nw + m.couple_gap;
    let uw = |u: &Unit| nw * (1.0 + u.partners as f32) + m.couple_gap * u.partners as f32;
    let lo_row = units.iter().map(|u| u.row).min().unwrap_or(0);
    let hi_row = units
        .iter()
        .map(|u| u.row + u.ctr.len() as i32 - 1)
        .max()
        .unwrap_or(0);
    let n = (hi_row - lo_row + 1).max(1) as usize;
    // Per row: the leftmost and rightmost edge placed so far.
    let mut lf = vec![f32::INFINITY; n];
    let mut rf = vec![f32::NEG_INFINITY; n];
    let idx = |row: i32| (row - lo_row) as usize;

    // The centred person's own group, in birth order, with every descendant.
    {
        let group = blocks[0].group.clone();
        let (places, acc) = {
            let items: Vec<&Contour> = group.iter().map(|&u| &units[u].ctr).collect();
            pack(&items, gap)
        };
        let k = group
            .iter()
            .position(|&u| u == blocks[0].anchor)
            .unwrap_or(0);
        let pv = -(uw(&units[group[k]]) - nw) / 2.0;
        let refx = places[k] + pv;
        for (j, &u) in group.iter().enumerate() {
            units[u].cx = places[j] - refx;
        }
        blocks[0].ax = 0.0;
        for (d, &(l, h)) in acc.iter().enumerate() {
            let i = idx(blocks[0].row + d as i32);
            lf[i] = lf[i].min(l - refx);
            rf[i] = rf[i].max(h - refx);
        }
    }

    // The spine, a generation at a time.
    let depth = blocks.iter().map(|b| -b.row).max().unwrap_or(0);
    for k in 0..depth {
        let (mut left, right): (Vec<usize>, Vec<usize>) = (0..blocks.len())
            .filter(|&i| blocks[i].row == -k)
            .partition(|&i| blocks[i].side == Some(0));
        // The father's side nearest the middle first, so each couple lands outside the one before.
        left.reverse();
        for cb in left.into_iter().chain(right) {
            let parents: Vec<usize> = blocks[cb].slots.iter().flatten().copied().collect();
            if parents.is_empty() {
                continue;
            }
            let c = blocks[cb].ax;
            let i = idx(blocks[cb].row - 1);
            let mut xs: Vec<f32> = if parents.len() == 2 {
                vec![c - pstep / 2.0, c + pstep / 2.0]
            } else {
                vec![c]
            };
            match blocks[cb].side {
                Some(0) => {
                    let limit = (lf[i] - gap).min(-gap / 2.0);
                    let hi = xs
                        .iter()
                        .map(|x| x + nw / 2.0)
                        .fold(f32::NEG_INFINITY, f32::max);
                    let shift = (hi - limit).max(0.0);
                    xs.iter_mut().for_each(|x| *x -= shift);
                }
                Some(_) => {
                    let limit = (rf[i] + gap).max(gap / 2.0);
                    let lo = xs
                        .iter()
                        .map(|x| x - nw / 2.0)
                        .fold(f32::INFINITY, f32::min);
                    let shift = (limit - lo).max(0.0);
                    xs.iter_mut().for_each(|x| *x += shift);
                }
                None => {}
            }
            // Brothers and sisters on the inner side of their ancestor stand right beside it, so the couple makes
            // room for them (and for everything below them) before it is placed.
            let mut inner: Vec<InnerGroup> = Vec::new();
            for (pi, &ub) in parents.iter().enumerate() {
                let Some((left, true)) = sibling_side(units, blocks, ub) else {
                    continue;
                };
                let group = blocks[ub].group.clone();
                let ak = group
                    .iter()
                    .position(|&u| u == blocks[ub].anchor)
                    .unwrap_or(0);
                let items: Vec<&Contour> = group.iter().map(|&u| &units[u].ctr).collect();
                let (places, _) = pack(&items, gap);
                let offs: Vec<f32> = places.iter().map(|p| p - places[ak]).collect();
                let mut shift = 0.0f32;
                for (j, &u) in group.iter().enumerate() {
                    if j == ak {
                        continue;
                    }
                    let acx = xs[pi] + (uw(&units[blocks[ub].anchor]) - nw) / 2.0;
                    let row0 = units[u].row;
                    for (d, &(l, h)) in units[u].ctr.iter().enumerate() {
                        let r = idx(row0 + d as i32);
                        if left {
                            shift = shift.max((rf[r] + gap) - (acx + offs[j] + l));
                        } else {
                            shift = shift.min((lf[r] - gap) - (acx + offs[j] + h));
                        }
                    }
                }
                if shift.is_finite() && shift != 0.0 {
                    xs.iter_mut().for_each(|x| *x += shift);
                }
                inner.push((pi, ak, left, group, offs));
            }
            for (&ub, &x) in parents.iter().zip(&xs) {
                blocks[ub].ax = x;
                let a = blocks[ub].anchor;
                units[a].cx = x + (uw(&units[a]) - nw) / 2.0;
                lf[i] = lf[i].min(x - nw / 2.0);
                rf[i] = rf[i].max(x + nw / 2.0);
            }
            for (pi, ak, _, group, offs) in inner {
                let base = units[blocks[parents[pi]].anchor].cx;
                for (j, &u) in group.iter().enumerate() {
                    if j == ak {
                        continue;
                    }
                    units[u].cx = base + offs[j];
                    let row0 = units[u].row;
                    for (d, &(l, h)) in units[u].ctr.iter().enumerate() {
                        let r = idx(row0 + d as i32);
                        lf[r] = lf[r].min(units[u].cx + l);
                        rf[r] = rf[r].max(units[u].cx + h);
                    }
                }
            }
        }
    }

    // The families, nearest generation first, outward.
    for side in 0..2usize {
        let mut order: Vec<usize> = (1..blocks.len())
            .filter(|&i| blocks[i].side == Some(side))
            .collect();
        order.sort_by_key(|&i| {
            (
                -blocks[i].row,
                if side == 0 { -(i as i64) } else { i as i64 },
            )
        });
        for bi in order {
            if matches!(sibling_side(units, blocks, bi), Some((_, true))) {
                continue;
            }
            let group = blocks[bi].group.clone();
            let Some(k) = group.iter().position(|&u| u == blocks[bi].anchor) else {
                continue;
            };
            let others: Vec<usize> = if side == 0 {
                group[..k].iter().rev().copied().collect()
            } else {
                group[k + 1..].to_vec()
            };
            for u in others {
                let row0 = units[u].row;
                let mut cx = if side == 0 {
                    f32::INFINITY
                } else {
                    f32::NEG_INFINITY
                };
                for (d, &(l, h)) in units[u].ctr.iter().enumerate() {
                    let i = idx(row0 + d as i32);
                    if side == 0 {
                        cx = cx.min((lf[i] - gap).min(-gap / 2.0) - h);
                    } else {
                        cx = cx.max((rf[i] + gap).max(gap / 2.0) - l);
                    }
                }
                units[u].cx = cx;
                for (d, &(l, h)) in units[u].ctr.iter().enumerate() {
                    let i = idx(row0 + d as i32);
                    if side == 0 {
                        lf[i] = lf[i].min(cx + l);
                    } else {
                        rf[i] = rf[i].max(cx + h);
                    }
                }
            }
        }
    }
}

/// Whether the brothers and sisters of block `bi` stand left of its anchor (the anchor is last in its group), and
/// whether that is the inner side of the chart (towards the middle), where the spine has to make room for them.
fn sibling_side(units: &[Unit], blocks: &[Block], bi: usize) -> Option<(bool, bool)> {
    let b = &blocks[bi];
    let side = b.side?;
    let k = b.group.iter().position(|&u| u == b.anchor)?;
    if b.group.len() < 2 {
        return None;
    }
    let _ = units;
    let left = k == b.group.len() - 1;
    Some((left, left == (side == 1)))
}

/// The width of every unit and the contour of everything below it, bottom-up: children are always created after
/// their owners.
fn fill_contours(units: &mut [Unit], m: &Metrics) {
    let gap = m.gap_x;
    let uw = |u: &Unit| m.node_w * (1.0 + u.partners as f32) + m.couple_gap * u.partners as f32;
    for i in (0..units.len()).rev() {
        let w = uw(&units[i]);
        let ch = units[i].children.clone();
        let own = (-w / 2.0, w / 2.0);
        if ch.is_empty() {
            units[i].ctr = vec![own];
            continue;
        }
        let (places, acc) = {
            let items: Vec<&Contour> = ch.iter().map(|&c| &units[c].ctr).collect();
            pack(&items, gap)
        };
        // The parent stands over the middle of its first and last child, or over the end nearest the
        // centred person when the children hang away from it.
        let (first, last) = (places[0], places[places.len() - 1]);
        let mid = match units[i].lean {
            l if l < 0 => last,
            l if l > 0 => first,
            _ => (first + last) / 2.0,
        };
        units[i].coff = places.iter().map(|p| p - mid).collect();
        let mut ctr = vec![own];
        ctr.extend(acc.iter().map(|&(lo, hi)| (lo - mid, hi - mid)));
        units[i].ctr = ctr;
    }
}

/// The partner links of every unit, and its families (parents and children), from the final box positions.
fn unit_edges(nodes: &[Node], units: &[Unit]) -> (Vec<Edge>, Vec<Family>) {
    let mut edges = Vec::new();
    let mut fams: Vec<Family> = Vec::new();
    for u in units {
        for k in 1..=u.partners {
            let to = u.person + k;
            let a = nodes[(to - 1) as usize].rect;
            let b = nodes[to as usize].rect;
            edges.push(Edge {
                from: to - 1,
                to,
                kind: EdgeKind::Partner,
                pts: vec![a.right(), a.cy(), b.x, b.cy()],
            });
        }
        let mut by_parents: HashMap<Vec<u32>, usize> = HashMap::new();
        for &c in &u.children {
            let cu = &units[c];
            let parents = match cu.attach {
                Some(pn) => vec![pn - 1, pn],
                None => vec![u.person],
            };
            let at = *by_parents.entry(parents.clone()).or_insert_with(|| {
                fams.push(Family::new(parents, Vec::new(), None));
                fams.len() - 1
            });
            fams[at].children.push(cu.person);
        }
    }
    (edges, fams)
}

/// Left and right links along each row, by position.
fn link_rows(nodes: &mut [Node]) {
    for n in nodes.iter_mut() {
        n.nav.prev = None;
        n.nav.next = None;
    }
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by(|&a, &b| {
        let (na, nb) = (&nodes[a], &nodes[b]);
        na.rect
            .y
            .total_cmp(&nb.rect.y)
            .then(na.rect.x.total_cmp(&nb.rect.x))
            .then(a.cmp(&b))
    });
    for w in order.windows(2) {
        if nodes[w[0]].rect.y == nodes[w[1]].rect.y {
            nodes[w[0]].nav.next = Some(w[1] as u32);
            nodes[w[1]].nav.prev = Some(w[0] as u32);
        }
    }
}

/// The chart built around several people: the chosen people side by side in one row, each with their partners and all
/// their descendants below (packed so nothing overlaps, unrelated families side by side at the normal gap), and their
/// ancestors above, one generation at a time. A couple stands side by side (one card and the partner gap) over the
/// middle of the children it has on the chart: over its one child, or over the midpoint of several. A couple moves
/// away from the middle of the row only as far as needed to clear the couples of its row. Everyone is placed once;
/// a person reached by two lines stands where the first line put them and the second line gets a link to them.
/// No brothers, sisters or other collateral relatives. `generations` is the number of rows above and below.
pub fn around_many(
    src: &dyn ChartSource,
    chosen: &[Key],
    generations: u8,
    m: &Metrics,
    max_nodes: usize,
) -> Layout {
    around_many_kin(src, chosen, generations, ManyKin::Line, m, max_nodes)
}

/// How much of the wider family [`around_many_kin`] shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManyKin {
    /// Only the lines.
    Line,
    /// Also the brothers and sisters of every chosen person, with partners and all descendants. Brothers and
    /// sisters who are chosen themselves share one group.
    Siblings,
    /// Also the brothers and sisters of every ancestor on the lines, with partners and all descendants, hung on the
    /// outer side of their line.
    All,
}

/// [`around_many`] with the wider family of `kin`: the chosen people and (when asked) their brothers and sisters
/// stand in the chosen row, each sibling group in birth order with its families below; the parents of a group of
/// chosen people are centred over the midpoint of the group's chosen people. With [`ManyKin::All`] the brothers and
/// sisters of the ancestors are then hung outward, nearest generation first: the families left of the middle of the
/// chosen row go to the left, the others to the right, each clearing row by row everything already placed. Everyone
/// is drawn once; a relative reached by two ways appears where the first put them and the second gets a link.
pub fn around_many_kin(
    src: &dyn ChartSource,
    chosen: &[Key],
    generations: u8,
    kin: ManyKin,
    m: &Metrics,
    max_nodes: usize,
) -> Layout {
    let first = around_many_kin_ordered(src, chosen, generations, kin, m, max_nodes);
    // Brothers and sisters with families of a line ancestor cannot always stand on the outer side whatever the
    // order of the chosen row: with two or three chosen people try the other orders (chosen partners stay
    // adjacent) and keep the one with the fewest crossing lines, then the shortest lines; ties keep the given order.
    if kin != ManyKin::All || !(2..=3).contains(&chosen.len()) {
        return first;
    }
    let score = |l: &Layout| {
        let q = crate::quality(l);
        (q.line_crossings, q.line_length)
    };
    let mut best = (score(&first), first);
    if best.0.0 == 0 {
        return best.1;
    }
    let apart = |order: &[Key]| {
        // Partners among the chosen people must stand next to each other.
        order.iter().enumerate().all(|(i, &k)| {
            src.unions(k).iter().all(|(p, _)| {
                let Some(p) = p else { return true };
                match order.iter().position(|x| x == p) {
                    Some(j) => i.abs_diff(j) == 1 || order.len() == 2,
                    None => true,
                }
            })
        })
    };
    let mut orders: Vec<Vec<Key>> = Vec::new();
    let mut idx: Vec<usize> = (0..chosen.len()).collect();
    permute(&mut idx, 0, &mut |p| {
        let o: Vec<Key> = p.iter().map(|&i| chosen[i]).collect();
        if o != chosen && apart(&o) {
            orders.push(o);
        }
    });
    for o in orders {
        let l = around_many_kin_ordered(src, &o, generations, kin, m, max_nodes);
        let sc = score(&l);
        if sc.0 < best.0.0 || (sc.0 == best.0.0 && sc.1 < best.0.1 - 0.5) {
            best = (sc, l);
        }
    }
    best.1
}

fn permute(v: &mut Vec<usize>, k: usize, f: &mut dyn FnMut(&[usize])) {
    if k == v.len() {
        f(v);
        return;
    }
    for i in k..v.len() {
        v.swap(k, i);
        permute(v, k + 1, f);
        v.swap(k, i);
    }
}

fn around_many_kin_ordered(
    src: &dyn ChartSource,
    chosen: &[Key],
    generations: u8,
    kin: ManyKin,
    m: &Metrics,
    max_nodes: usize,
) -> Layout {
    // One chosen person with every relative: the all-relatives layout puts each ancestor's brothers and sisters on the
    // outer side of the ancestor's couple, which the several-people layout below cannot do.
    // A chosen partner without known parents or siblings adds nothing the one-person layout does not draw.
    let lone = |k: Key| {
        src.parents(k).iter().all(Option::is_none) && siblings(src, k).iter().all(|s| s.key == k)
    };
    let partners_of_first = chosen.len() > 1
        && chosen[1..]
            .iter()
            .all(|&k| lone(k) && src.unions(chosen[0]).iter().any(|(p, _)| *p == Some(k)));
    if kin == ManyKin::All && (chosen.len() == 1 || partners_of_first) {
        let o = RelativesOptions {
            generations,
            aunts: false,
        };
        return lay_out(src, chosen[0], &o, true, true, true, m, max_nodes);
    }
    let m = &partner_room(m);
    let with_sibs = kin != ManyKin::Line;
    let g = generations.max(1);
    let mut b = Builder {
        src,
        max_nodes: max_nodes.max(1),
        rect: Rect {
            x: 0.0,
            y: 0.0,
            w: m.node_w,
            h: m.node_h,
        },
        nodes: Vec::new(),
        visited: HashMap::new(),
        units: Vec::new(),
        pending: Vec::new(),
        blocks: Vec::new(),
        truncated: false,
        siblings: false,
        all: false,
    };
    // A chosen person who is an ancestor of another one goes first: the other one is then drawn as a descendant.
    let mut ancestors: std::collections::HashSet<Key> = std::collections::HashSet::new();
    for &c in chosen {
        let mut seen: std::collections::HashSet<Key> = std::collections::HashSet::from([c]);
        let mut todo = vec![c];
        while let Some(p) = todo.pop() {
            for q in src.parents(p).into_iter().flatten() {
                if seen.insert(q) {
                    ancestors.insert(q);
                    todo.push(q);
                }
            }
        }
    }
    let mut order: Vec<Key> = chosen
        .iter()
        .copied()
        .filter(|k| ancestors.contains(k))
        .collect();
    order.extend(chosen.iter().copied().filter(|k| !ancestors.contains(k)));
    let chosen_set: std::collections::HashSet<Key> = chosen.iter().copied().collect();
    // The units of the chosen row, one group per chosen person (or, flagged, the brothers and sisters of a chosen
    // person who stands as somebody's partner); the groups go left to right in this order.
    let mut groups: Vec<(Vec<usize>, bool)> = Vec::new();
    // Brothers and sisters who are not chosen themselves.
    let mut sib_roots: Vec<usize> = Vec::new();
    // Brothers and sisters that hang from no parent, per set: joined by a bar.
    let mut bars: Vec<Vec<u32>> = Vec::new();
    'roots: for key in order {
        if let Some(&node) = b.visited.get(&key) {
            // Already drawn. A chosen partner (of a chosen person or of a brother or sister) still brings his or
            // her own brothers and sisters, on the outer side of the group that holds the partner.
            if !with_sibs {
                continue;
            }
            let owner = groups.iter().position(|g| {
                g.0.iter().any(|&u| {
                    let un = &b.units[u];
                    (un.person..=un.person + un.partners).contains(&node)
                })
            });
            let Some(owner) = owner else { continue };
            let mut at = owner + 1;
            while at < groups.len() && groups[at].1 {
                at += 1;
            }
            let mut made = Vec::new();
            let mut bar = vec![node];
            for s in siblings(src, key) {
                if s.key == key || b.visited.contains_key(&s.key) || chosen_set.contains(&s.key) {
                    continue;
                }
                let Some(u) = b.make_unit(
                    s.key,
                    0,
                    Rel::Sibling { half: s.half },
                    Some(Rel::Partner(PartnerOf::Sibling)),
                    None,
                ) else {
                    break 'roots;
                };
                b.expand(u, Tree::Kin(1));
                sib_roots.push(u);
                made.push(u);
                if s.barred() {
                    bar.push(b.units[u].person);
                }
            }
            bars.push(bar);
            if !made.is_empty() {
                groups.insert(at, (made, true));
            }
            continue;
        }
        groups.push((Vec::new(), false));
        // A chosen person whose partner is chosen too stands last in the group, next to the partner who follows
        // the unit; the partner's own brothers and sisters then go on that side of the pair.
        let partner_chosen = |k: Key| {
            src.unions(k).iter().any(|(p, _)| {
                p.is_some_and(|p| p != k && chosen_set.contains(&p) && !b.visited.contains_key(&p))
            })
        };
        let mut bar_keys: Vec<Key> = vec![key];
        let group: Vec<(Key, Rel, bool)> = if with_sibs {
            let mut sibs = siblings(src, key);
            // Also when it is a chosen brother or sister who has the chosen partner.
            let last = if partner_chosen(key) {
                sibs.iter().position(|s| s.key == key)
            } else {
                sibs.iter()
                    .position(|s| chosen_set.contains(&s.key) && partner_chosen(s.key))
            };
            if let Some(i) = last {
                let me = sibs.remove(i);
                sibs.push(me);
            }
            bar_keys.extend(sibs.iter().filter(|s| s.barred()).map(|s| s.key));
            sibs.into_iter()
                .map(|s| {
                    if chosen_set.contains(&s.key) {
                        (s.key, Rel::Focus, true)
                    } else {
                        (s.key, Rel::Sibling { half: s.half }, false)
                    }
                })
                .collect()
        } else {
            vec![(key, Rel::Focus, true)]
        };
        let mut bar: Vec<u32> = Vec::new();
        for (k, rel, is_chosen) in group {
            if b.visited.contains_key(&k) {
                continue;
            }
            let pr = if is_chosen {
                PartnerOf::Focus
            } else {
                PartnerOf::Sibling
            };
            let Some(u) = b.make_unit(k, 0, rel, Some(Rel::Partner(pr)), None) else {
                break 'roots;
            };
            if is_chosen {
                b.expand(u, Tree::Focus(g));
            } else {
                b.expand(u, Tree::Kin(1));
                sib_roots.push(u);
            }
            if bar_keys.contains(&k) {
                bar.push(b.units[u].person);
            }
            if let Some(g) = groups.last_mut() {
                g.0.push(u);
            }
        }
        bars.push(bar);
    }
    let roots: Vec<usize> = groups.into_iter().flat_map(|g| g.0).collect();
    if roots.is_empty() {
        return Layout::finish(Vec::new(), Vec::new(), 0, b.truncated);
    }
    fill_contours(&mut b.units, m);
    let gap = m.gap_x;
    let nw = m.node_w;
    let step_y = m.node_h + m.gap_y * 4.0;
    let pstep = nw + m.couple_gap;
    // The chosen people's units side by side, then every descendant under its parent.
    let (places, _) = {
        let items: Vec<&Contour> = roots.iter().map(|&r| &b.units[r].ctr).collect();
        pack(&items, gap)
    };
    for (&r, &x) in roots.iter().zip(&places) {
        b.units[r].cx = x;
    }
    let root_nodes: Vec<u32> = roots
        .iter()
        .flat_map(|&r| b.units[r].person..=b.units[r].person + b.units[r].partners)
        .collect();
    place_units(&mut b.units, &mut b.nodes, 0, m);

    // The ancestors, a generation at a time. The frontier is the people whose parents come next, by centre.
    let mut frontier: Vec<(u32, f32)> = Vec::new();
    for &c in chosen {
        if let Some(&node) = b.visited.get(&c)
            && root_nodes.contains(&node)
            && !frontier.iter().any(|f| f.0 == node)
        {
            let r = b.nodes[node as usize].rect;
            frontier.push((node, r.x + r.w / 2.0));
        }
    }
    let mut links: Vec<(u32, u32)> = Vec::new();
    let mut couples: Vec<(u32, u32)> = Vec::new();
    // Every ancestor drawn on the lines.
    let mut spine: Vec<u32> = Vec::new();
    // The planar order of the lines: every card's chain of digits (see `arrange_ancestors`) and every couple of
    // parents with the children it hangs over.
    let mut chain: HashMap<u32, Vec<i32>> = HashMap::new();
    for (i, &r) in roots.iter().enumerate() {
        for j in 0..=b.units[r].partners {
            chain.insert(b.units[r].person + j, vec![i as i32, 2 * j as i32]);
        }
    }
    let mut spine_groups: Vec<SpineGroup> = Vec::new();
    let mut root_of: HashMap<u32, usize> = HashMap::new();
    let mut unit_key: HashMap<usize, Vec<i32>> = HashMap::new();
    for (i, &r) in roots.iter().enumerate() {
        for j in 0..=b.units[r].partners {
            root_of.insert(b.units[r].person + j, i);
        }
    }
    let mut row = 0i32;
    while !frontier.is_empty() {
        frontier.sort_by(|a, b| a.1.total_cmp(&b.1));
        // Children with the same parents form one group.
        type Kids = Vec<(u32, f32)>;
        let mut groups: Vec<([Option<Key>; 2], Kids)> = Vec::new();
        for &(node, cx) in &frontier {
            let ps = src.parents(b.nodes[node as usize].key);
            if ps.iter().all(Option::is_none) {
                continue;
            }
            if row <= -i32::from(g) {
                b.nodes[node as usize].more_up = true;
                continue;
            }
            match groups.iter_mut().find(|grp| grp.0 == ps) {
                Some(grp) => grp.1.push((node, cx)),
                None => groups.push((ps, vec![(node, cx)])),
            }
        }
        if groups.is_empty() {
            break;
        }
        row -= 1;
        let y = row as f32 * step_y;
        struct Plan {
            fresh: Vec<Key>,
            c: f32,
            w: f32,
        }
        let mut plans: Vec<Plan> = Vec::new();
        for (ps, kids) in &groups {
            let lo = kids.iter().map(|k| k.1).fold(f32::INFINITY, f32::min);
            let hi = kids.iter().map(|k| k.1).fold(f32::NEG_INFINITY, f32::max);
            let mut fresh: Vec<Key> = ps
                .iter()
                .flatten()
                .copied()
                .filter(|k| !b.visited.contains_key(k))
                .collect();
            if b.nodes.len() + fresh.len() > b.max_nodes {
                b.truncated = true;
                for k in kids {
                    b.nodes[k.0 as usize].more_up = true;
                }
                fresh.clear();
            }
            let w = match fresh.len() {
                0 => 0.0,
                1 => nw,
                _ => nw + pstep,
            };
            plans.push(Plan {
                fresh,
                c: (lo + hi) / 2.0,
                w,
            });
        }
        // Place from the middle of the row outward: the group nearest the middle where it wants to be, the next
        // one outside it only as far as needed to clear it.
        let targets: Vec<f32> = plans
            .iter()
            .filter(|p| !p.fresh.is_empty())
            .map(|p| p.c)
            .collect();
        let mid = (targets.iter().copied().fold(f32::INFINITY, f32::min)
            + targets.iter().copied().fold(f32::NEG_INFINITY, f32::max))
            / 2.0;
        let mut at: Vec<f32> = plans.iter().map(|p| p.c).collect();
        let mut by_target: Vec<usize> = (0..plans.len())
            .filter(|&i| !plans[i].fresh.is_empty())
            .collect();
        by_target.sort_by(|&a, &c| plans[a].c.total_cmp(&plans[c].c));
        let (mut lf, mut rf) = (f32::INFINITY, f32::NEG_INFINITY);
        let centre: Vec<usize> = by_target
            .iter()
            .copied()
            .filter(|&i| (plans[i].c - mid).abs() < 0.5)
            .collect();
        for &i in &centre {
            lf = lf.min(at[i] - plans[i].w / 2.0);
            rf = rf.max(at[i] + plans[i].w / 2.0);
        }
        let left: Vec<usize> = by_target
            .iter()
            .rev()
            .copied()
            .filter(|&i| plans[i].c < mid - 0.5)
            .collect();
        for i in left {
            let limit = (lf - gap).min(mid - gap / 2.0);
            let hi = at[i] + plans[i].w / 2.0;
            at[i] -= (hi - limit).max(0.0);
            lf = lf.min(at[i] - plans[i].w / 2.0);
        }
        let right: Vec<usize> = by_target
            .iter()
            .copied()
            .filter(|&i| plans[i].c > mid + 0.5)
            .collect();
        for i in right {
            let limit = (rf + gap).max(mid + gap / 2.0);
            let lo = at[i] - plans[i].w / 2.0;
            at[i] += (limit - lo).max(0.0);
            rf = rf.max(at[i] + plans[i].w / 2.0);
        }
        let mut next: Vec<(u32, f32)> = Vec::new();
        for ((ps, kids), (plan, &c)) in groups.iter().zip(plans.iter().zip(&at)) {
            let xs: Vec<f32> = if plan.fresh.len() == 2 {
                vec![c - pstep / 2.0, c + pstep / 2.0]
            } else {
                vec![c]
            };
            for (&key, &cx) in plan.fresh.iter().zip(&xs) {
                let id = b.nodes.len() as u32;
                let rect = Rect {
                    x: cx - nw / 2.0,
                    y,
                    w: nw,
                    h: m.node_h,
                };
                let mut n = new_node(id, key, rect, row as i16);
                n.rel = Some(Rel::Ancestor((-row) as u8));
                b.nodes.push(n);
                b.visited.insert(key, id);
                spine.push(id);
                next.push((id, cx));
            }
            if plan.fresh.len() == 2 {
                let a = b.visited[&plan.fresh[0]];
                couples.push((a, a + 1));
            }
            if !plan.fresh.is_empty() {
                // The children: the chosen ones on the chart and the brothers and sisters of the chosen people.
                let mut kid_nodes: Vec<u32> = kids.iter().map(|k| k.0).collect();
                for &r in &roots {
                    let person = b.units[r].person;
                    if sib_roots.contains(&r)
                        && src.parents(b.nodes[person as usize].key) == *ps
                        && !kid_nodes.contains(&person)
                    {
                        kid_nodes.push(person);
                    }
                }
                kid_nodes.sort_by(|x, y| chain.get(x).cmp(&chain.get(y)));
                let kmin = kid_nodes
                    .first()
                    .and_then(|k| chain.get(k))
                    .cloned()
                    .unwrap_or_default();
                let left = 2 * kmin.first().copied().unwrap_or(0) < roots.len() as i32 - 1;
                let digits: &[i32] = match (plan.fresh.len(), left) {
                    (2, _) => &[0, 2],
                    (_, true) => &[0],
                    _ => &[2],
                };
                let ids: Vec<u32> = plan.fresh.iter().map(|k| b.visited[k]).collect();
                for (&id, &d) in ids.iter().zip(digits) {
                    let mut c = kmin.clone();
                    c.push(d);
                    chain.insert(id, c);
                }
                let mut key = kmin;
                key.push(1);
                // A unit whose own card is a child stands among the children, between the two parents' sides.
                for (n, k) in kid_nodes.iter().enumerate() {
                    if let Some(&ri) = root_of.get(k)
                        && b.units[roots[ri]].person == *k
                    {
                        let mut ok = key.clone();
                        ok.push(n as i32);
                        unit_key.entry(ri).or_insert(ok);
                    }
                }
                spine_groups.push(SpineGroup {
                    ids,
                    row,
                    key,
                    kids: kids.iter().map(|k| k.0).collect(),
                    order: kid_nodes,
                });
            }
            for p in ps.iter().flatten() {
                if let Some(&pn) = b.visited.get(p) {
                    for k in kids {
                        links.push((pn, k.0));
                    }
                }
            }
        }
        frontier = next;
    }

    // The brothers and sisters of the chosen people hang from the parents too.
    for &u in &sib_roots {
        let node = b.units[u].person;
        for p in src
            .parents(b.nodes[node as usize].key)
            .into_iter()
            .flatten()
        {
            if let Some(&pn) = b.visited.get(&p) {
                links.push((pn, node));
            }
        }
    }

    if kin == ManyKin::All {
        arrange_ancestors(
            &mut b,
            &spine_groups,
            &chain,
            &unit_key,
            &roots,
            &mut links,
            &mut bars,
            m,
        );
    }

    let Builder {
        mut nodes,
        units,
        truncated,
        ..
    } = b;
    {
        let mut seen = std::collections::HashSet::new();
        links.retain(|l| seen.insert(*l));
    }
    // Keep every coordinate non-negative: the canvas bounds start at the origin.
    let min_x = nodes.iter().map(|n| n.rect.x).fold(f32::INFINITY, f32::min);
    let min_y = nodes.iter().map(|n| n.rect.y).fold(f32::INFINITY, f32::min);
    for n in &mut nodes {
        n.rect.x -= min_x;
        n.rect.y -= min_y;
    }
    let (mut edges, mut fams) = unit_edges(&nodes, &units);
    for &(a, b2) in &couples {
        let (ra, rb) = (nodes[a as usize].rect, nodes[b2 as usize].rect);
        edges.push(Edge {
            from: a,
            to: b2,
            kind: EdgeKind::Partner,
            pts: vec![ra.right(), ra.cy(), rb.x, rb.cy()],
        });
    }
    let mut parents_of: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut child_order: Vec<u32> = Vec::new();
    for &(p, c) in &links {
        let ps = parents_of.entry(c).or_insert_with(|| {
            child_order.push(c);
            Vec::new()
        });
        if !ps.contains(&p) {
            ps.push(p);
        }
        nodes[c as usize].nav.parent.get_or_insert(p);
        nodes[p as usize].nav.child.get_or_insert(c);
    }
    let mut by_parents: HashMap<Vec<u32>, usize> = HashMap::new();
    for c in child_order {
        let mut ps = parents_of.remove(&c).unwrap_or_default();
        ps.sort_by(|&a, &b| {
            nodes[a as usize]
                .rect
                .x
                .total_cmp(&nodes[b as usize].rect.x)
        });
        let at = *by_parents.entry(ps.clone()).or_insert_with(|| {
            fams.push(Family::new(ps, Vec::new(), None));
            fams.len() - 1
        });
        fams[at].children.push(c);
    }
    for bar in &bars {
        fams.push(Family::barred(bar.clone()));
    }
    let routed = route(&mut nodes, &mut edges, fams, m);
    edges.extend(routed);
    for v in 0..nodes.len() {
        if let Some(orig) = nodes[v].dup_of {
            edges.push(dup_edge(&nodes, v as u32, orig, m.gap_x));
        }
    }
    link_rows(&mut nodes);
    let root = units[roots[0]].person;
    Layout::finish(nodes, edges, root, truncated)
}

/// Set the boxes of every unit from `from` on from its centre, and the centres of its children. The units whose
/// parents are not in that range need their centre set beforehand.
fn place_units(units: &mut [Unit], nodes: &mut [Node], from: usize, m: &Metrics) {
    let step_y = m.node_h + m.gap_y * 4.0;
    let pstep = m.node_w + m.couple_gap;
    for i in from..units.len() {
        let w =
            m.node_w * (1.0 + units[i].partners as f32) + m.couple_gap * units[i].partners as f32;
        let x0 = units[i].cx - w / 2.0;
        let y = units[i].row as f32 * step_y;
        let p = units[i].person;
        for k in 0..=units[i].partners {
            let n = &mut nodes[(p + k) as usize];
            n.rect.x = x0 + k as f32 * pstep;
            n.rect.y = y;
        }
        let ch = units[i].children.clone();
        for (k, &c) in ch.iter().enumerate() {
            units[c].cx = units[i].cx + units[i].coff[k];
        }
    }
}

/// One couple (or lone parent) of ancestors on the lines of [`around_many_kin`]: its cards, their row, the order key
/// of the couple and the children it hangs over.
struct SpineGroup {
    ids: Vec<u32>,
    row: i32,
    key: Vec<i32>,
    /// The chosen children, which the couple is centred over.
    kids: Vec<u32>,
    /// All the children on the chart, the chosen people's brothers and sisters too, left to right.
    order: Vec<u32>,
}

/// Where a body of [`arrange_ancestors`] wants to stand.
enum Goal {
    /// Where it started.
    Stay,
    /// Over the middle of these cards.
    Over(Vec<u32>),
    /// Under a couple of parents: the body of the couple and the offset of the child card in this body.
    Under(usize, f32),
    /// Right beside its neighbour on the inner side: a brother or sister on the left hugs the one to its right.
    Hug(bool),
}

/// A rigid piece of the all-kin chart of several people: a unit with its descendants, or a couple of ancestors.
struct Body {
    row0: i32,
    ctr: Vec<(f32, f32)>,
    key: Vec<i32>,
    x: f32,
    base: f32,
    goal: Goal,
}

/// The extent of a contour over all its rows.
fn span(ctr: &[(f32, f32)]) -> (f32, f32) {
    ctr.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |a, c| {
        (a.0.min(c.0), a.1.max(c.1))
    })
}

/// All-kin chart of several people: the brothers and sisters of every ancestor on the lines, with partners and all
/// descendants, on the outer side of the ancestor's couple (a left partner's to the left, a right partner's to the
/// right), and the whole of the lines placed again to make room.
///
/// Every piece (a chosen unit with its descendants, a couple of ancestors, a brother or sister with the descendants)
/// gets an order key, so that sorting the pieces of any row by key gives the planar left-to-right order of that row:
/// a card's chain is its first child's chain plus 0 (father or left lone parent) or 2 (mother or right lone parent);
/// a couple's key is its first child's chain plus 1; the brothers and sisters of a card are keyed by the card's chain
/// plus 1 and their distance from it, negative on the left. The pieces of a row then keep that order and clear each
/// other in every row they share; couples stay centred over their children as far as that allows.
#[allow(clippy::too_many_arguments)]
fn arrange_ancestors(
    b: &mut Builder<'_>,
    groups: &[SpineGroup],
    chain: &HashMap<u32, Vec<i32>>,
    unit_key: &HashMap<usize, Vec<i32>>,
    roots: &[usize],
    links: &mut Vec<(u32, u32)>,
    bars: &mut Vec<Vec<u32>>,
    m: &Metrics,
) {
    let nw = m.node_w;
    let pstep = nw + m.couple_gap;
    let gap = m.gap_x;
    let uw = |u: &Unit| nw * (1.0 + u.partners as f32) + m.couple_gap * u.partners as f32;
    let mut spine: Vec<u32> = groups.iter().flat_map(|g| g.ids.iter().copied()).collect();
    spine.sort_by_key(|&id| (std::cmp::Reverse(b.nodes[id as usize].r#gen), id));
    // The brothers and sisters of every ancestor: unit, order key, side, ancestor.
    let mut sibs: Vec<(usize, Vec<i32>, bool, u32)> = Vec::new();
    for id in spine {
        let Some(ch) = chain.get(&id) else { continue };
        let left = ch.last() == Some(&0);
        let key = b.nodes[id as usize].key;
        let row = i32::from(b.nodes[id as usize].r#gen);
        let gens = u8::try_from(-row).unwrap_or(u8::MAX);
        let up = gens.saturating_add(1);
        let mut made: Vec<usize> = Vec::new();
        let mut bar = vec![id];
        for s in siblings(b.src, key) {
            if s.key == key {
                continue;
            }
            let parents: Vec<u32> = b
                .src
                .parents(s.key)
                .into_iter()
                .flatten()
                .filter_map(|p| b.visited.get(&p).copied())
                .collect();
            if let Some(&n) = b.visited.get(&s.key) {
                // Drawn already: it only gets its link.
                if i32::from(b.nodes[n as usize].r#gen) == row {
                    links.extend(parents.iter().map(|&pn| (pn, n)));
                    if s.barred() {
                        bar.push(n);
                    }
                }
                continue;
            }
            let Some(u) = b.make_unit(
                s.key,
                row,
                sibling_rel(gens, s.half),
                Some(kin_partner(up, 1)),
                None,
            ) else {
                break;
            };
            b.expand(u, Tree::Kin(up));
            let person = b.units[u].person;
            links.extend(parents.iter().map(|&pn| (pn, person)));
            if s.barred() {
                bar.push(person);
            }
            made.push(u);
        }
        bars.push(bar);
        let k = made.len() as i32;
        for (i, u) in made.into_iter().enumerate() {
            let mut kk = ch.clone();
            kk.push(1);
            kk.push(if left { i as i32 - k } else { i as i32 + 1 });
            sibs.push((u, kk, left, id));
        }
    }
    fill_contours(&mut b.units, m);
    let ukey = |i: usize| {
        unit_key
            .get(&i)
            .cloned()
            .unwrap_or_else(|| vec![i as i32, 1])
    };
    // A brother or sister with a family, on the side of the chosen row's parents that faces a partner of the chosen
    // row (the children of those parents have a chosen partner beside them there), cannot stand between the two
    // partners: its family goes outside the whole chart instead, on that side.
    let mut inner: HashMap<(i32, i32), (bool, bool)> = HashMap::new();
    for g in groups.iter().filter(|g| g.row == -1) {
        let mut kids: Vec<&Vec<i32>> = g.order.iter().filter_map(|k| chain.get(k)).collect();
        kids.sort();
        let (Some(first), Some(last)) = (kids.first(), kids.last()) else {
            continue;
        };
        let partners = |c: &Vec<i32>| {
            roots
                .get(c[0] as usize)
                .map_or(0, |&r| b.units[r].partners as i32)
        };
        inner.insert(
            (first[0], first[1]),
            (first[1] > 0, last[1] / 2 < partners(last)),
        );
    }
    for sib in &mut sibs {
        let Some(ch) = chain.get(&sib.3) else {
            continue;
        };
        if b.units[sib.0].ctr.len() < 2 || ch.len() < 3 {
            continue;
        }
        let Some(&(il, ir)) = inner.get(&(ch[0], ch[1])) else {
            continue;
        };
        if (sib.2 && il) || (!sib.2 && ir) {
            let mut kk = vec![if sib.2 { -1 } else { i32::MAX }];
            kk.extend_from_slice(&sib.1);
            sib.1 = kk;
        }
    }

    // The pieces: chosen units, couples of ancestors, brothers and sisters.
    let mut bodies: Vec<Body> = Vec::new();
    let mut at: HashMap<u32, (usize, f32)> = HashMap::new();
    for (i, &r) in roots.iter().enumerate() {
        let w = uw(&b.units[r]);
        for k in 0..=b.units[r].partners {
            at.insert(
                b.units[r].person + k,
                (bodies.len(), -w / 2.0 + nw / 2.0 + k as f32 * pstep),
            );
        }
        let x = b.units[r].cx;
        bodies.push(Body {
            row0: b.units[r].row,
            ctr: b.units[r].ctr.clone(),
            key: {
                let mut k = ukey(i);
                k.push(0);
                k
            },
            x,
            base: x,
            goal: Goal::Stay,
        });
    }
    for g in groups {
        let w = if g.ids.len() > 1 { nw + pstep } else { nw };
        let first = b.nodes[g.ids[0] as usize].rect;
        let last = b.nodes[g.ids[g.ids.len() - 1] as usize].rect;
        let x = (first.x + last.right()) / 2.0;
        for (k, &id) in g.ids.iter().enumerate() {
            at.insert(id, (bodies.len(), -w / 2.0 + nw / 2.0 + k as f32 * pstep));
        }
        bodies.push(Body {
            row0: g.row,
            ctr: vec![(-w / 2.0, w / 2.0)],
            key: g.key.clone(),
            x,
            base: x,
            goal: Goal::Over(g.kids.clone()),
        });
    }
    // A chosen person whose couple of parents is centred over them alone follows the couple when it has to move.
    for (gi, g) in groups.iter().enumerate() {
        if g.row != -1 || g.kids.len() != 1 {
            continue;
        }
        let Some(&(ub, off)) = at.get(&g.kids[0]) else {
            continue;
        };
        if (off + (uw(&b.units[roots[ub]]) - nw) / 2.0).abs() < 0.01 {
            bodies[ub].goal = Goal::Under(roots.len() + gi, off);
        }
    }
    // First guess for the brothers and sisters: side by side outward from the ancestor, nearest first.
    let mut edge: HashMap<(u32, bool), f32> = HashMap::new();
    let mut guess: Vec<f32> = vec![0.0; sibs.len()];
    let mut walk: Vec<usize> = (0..sibs.len()).collect();
    walk.sort_by(|&p, &q| {
        let (a, c) = (&sibs[p], &sibs[q]);
        a.3.cmp(&c.3)
            .then(if a.2 { c.1.cmp(&a.1) } else { a.1.cmp(&c.1) })
    });
    for i in walk {
        let (u, _, left, a) = (sibs[i].0, 0, sibs[i].2, sibs[i].3);
        let r = b.nodes[a as usize].rect;
        let (lo, hi) = span(&b.units[u].ctr);
        let e = edge
            .entry((a, left))
            .or_insert(if left { r.x - gap } else { r.right() + gap });
        if left {
            guess[i] = *e - hi;
            *e = guess[i] + lo - gap;
        } else {
            guess[i] = *e - lo;
            *e = guess[i] + hi + gap;
        }
    }
    let first_sib = bodies.len();
    for (i, (u, key, left, _)) in sibs.iter().enumerate() {
        bodies.push(Body {
            row0: b.units[*u].row,
            ctr: b.units[*u].ctr.clone(),
            key: key.clone(),
            x: guess[i],
            base: guess[i],
            goal: Goal::Hug(*left),
        });
    }
    solve_order(&mut bodies, &at, gap);

    // The result back into the cards and units.
    for g in groups {
        for &id in &g.ids {
            let (bi, off) = at[&id];
            b.nodes[id as usize].rect.x = bodies[bi].x + off - nw / 2.0;
        }
    }
    for (i, &r) in roots.iter().enumerate() {
        b.units[r].cx = bodies[i].x;
    }
    for (i, (u, ..)) in sibs.iter().enumerate() {
        b.units[*u].cx = bodies[first_sib + i].x;
    }
    place_units(&mut b.units, &mut b.nodes, 0, m);
}

/// Place the bodies: the pieces of every row keep the order of their keys and clear each other by `gap` in every row
/// they share. Start from the left with everything pushed right just far enough, then let each piece move towards
/// where it wants to stand, between its neighbours, sweeping left to right and back until nothing moves.
fn solve_order(bodies: &mut [Body], at: &HashMap<u32, (usize, f32)>, gap: f32) {
    let n = bodies.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &c| bodies[a].key.cmp(&bodies[c].key));
    let mut rows: std::collections::BTreeMap<i32, Vec<usize>> = std::collections::BTreeMap::new();
    for &i in &order {
        for d in 0..bodies[i].ctr.len() {
            rows.entry(bodies[i].row0 + d as i32).or_default().push(i);
        }
    }
    let mut cons: HashMap<(usize, usize), f32> = HashMap::new();
    for (&r, list) in &rows {
        for w in list.windows(2) {
            let (a, c) = (w[0], w[1]);
            let need = bodies[a].ctr[(r - bodies[a].row0) as usize].1
                - bodies[c].ctr[(r - bodies[c].row0) as usize].0
                + gap;
            let e = cons.entry((a, c)).or_insert(need);
            *e = e.max(need);
        }
    }
    let mut preds: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
    let mut succs: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
    for (&(a, c), &w) in &cons {
        preds[c].push((a, w));
        succs[a].push((c, w));
    }
    for &j in &order {
        let lo = preds[j]
            .iter()
            .map(|&(i, w)| bodies[i].x + w)
            .fold(f32::NEG_INFINITY, f32::max);
        bodies[j].x = bodies[j].x.max(lo);
    }
    let centre = |bodies: &[Body], c: u32| at.get(&c).map(|&(bi, off)| bodies[bi].x + off);
    for sweep in 0..400 {
        let mut moved = 0.0f32;
        let seq: Vec<usize> = if sweep % 2 == 0 {
            order.clone()
        } else {
            order.iter().rev().copied().collect()
        };
        for j in seq {
            let lo = preds[j]
                .iter()
                .map(|&(i, w)| bodies[i].x + w)
                .fold(f32::NEG_INFINITY, f32::max);
            let hi = succs[j]
                .iter()
                .map(|&(k, w)| bodies[k].x - w)
                .fold(f32::INFINITY, f32::min);
            let want = match &bodies[j].goal {
                Goal::Stay => bodies[j].base,
                Goal::Over(kids) => {
                    let cs: Vec<f32> = kids.iter().filter_map(|&k| centre(bodies, k)).collect();
                    if cs.is_empty() {
                        bodies[j].x
                    } else {
                        let l = cs.iter().copied().fold(f32::INFINITY, f32::min);
                        let h = cs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                        (l + h) / 2.0
                    }
                }
                Goal::Under(g, off) => bodies[*g].x - off,
                Goal::Hug(true) if hi.is_finite() => hi,
                Goal::Hug(false) if lo.is_finite() => lo,
                Goal::Hug(_) => bodies[j].x,
            };
            let x = if lo > hi { lo } else { want.clamp(lo, hi) };
            moved = moved.max((x - bodies[j].x).abs());
            bodies[j].x = x;
        }
        if moved < 0.01 {
            break;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lay_out(
    src: &dyn ChartSource,
    root: Key,
    o: &RelativesOptions,
    siblings: bool,
    centred: bool,
    all: bool,
    m: &Metrics,
    max_nodes: usize,
) -> Layout {
    let m = &partner_room(m);
    let o = RelativesOptions {
        generations: o.generations.max(1),
        aunts: o.aunts,
    };
    let mut b = Builder {
        src,
        max_nodes: max_nodes.max(1),
        rect: Rect {
            x: 0.0,
            y: 0.0,
            w: m.node_w,
            h: m.node_h,
        },
        nodes: Vec::new(),
        visited: HashMap::new(),
        units: Vec::new(),
        pending: Vec::new(),
        blocks: Vec::new(),
        truncated: false,
        siblings,
        all,
    };
    let top = -i32::from(o.generations);
    b.make_block(root, 0, &o, None);
    let mut bi = 0;
    while bi < b.blocks.len() {
        let cur = bi;
        bi += 1;
        let (row, xnode) = (b.blocks[cur].row, b.units[b.blocks[cur].anchor].person);
        if b.nodes[xnode as usize].dup_of.is_some() {
            continue;
        }
        let ps = src.parents(b.nodes[xnode as usize].key);
        if ps.iter().all(Option::is_none) {
            continue;
        }
        if row <= top {
            b.nodes[xnode as usize].more_up = true;
            continue;
        }
        for (slot, p) in ps.iter().enumerate() {
            let Some(pk) = *p else { continue };
            let side = b.blocks[cur].side.or(Some(slot));
            match b.make_block(pk, row - 1, &o, side) {
                Some(nb) => {
                    b.blocks[cur].slots[slot] = Some(nb);
                    b.blocks[cur].ups.push(nb);
                }
                None => b.nodes[xnode as usize].more_up = true,
            }
        }
    }

    if all {
        for bi in 0..b.blocks.len() {
            b.fill_group(bi);
        }
    }

    let Builder {
        mut nodes,
        mut units,
        mut blocks,
        truncated,
        ..
    } = b;

    // Widths, bottom-up: children and ancestor blocks are always created after their owners.
    let gap = m.gap_x;
    let uw = |u: &Unit| m.node_w * (1.0 + u.partners as f32) + m.couple_gap * u.partners as f32;
    fill_contours(&mut units, m);
    if all {
        solve_all(&mut units, &mut blocks, m);
    }
    let old_order: Vec<usize> = if all {
        Vec::new()
    } else {
        (0..blocks.len()).rev().collect()
    };
    for i in old_order {
        let ups = blocks[i].ups.clone();
        let up_w: f32 = ups.iter().map(|&u| blocks[u].w).sum::<f32>()
            + gap * ups.len().saturating_sub(1) as f32;
        let group = blocks[i].group.clone();
        let (places, acc) = {
            let items: Vec<&Contour> = group.iter().map(|&u| &units[u].ctr).collect();
            pack(&items, gap)
        };
        let glo = acc.iter().map(|c| c.0).fold(f32::INFINITY, f32::min);
        let ghi = acc.iter().map(|c| c.1).fold(f32::NEG_INFINITY, f32::max);
        // One person alone in the group (the chart of one line): the person's box, not the whole family
        // below, is what the parents stand over, so the group is made symmetric about the box.
        let pv = -(uw(&units[group[0]]) - m.node_w) / 2.0;
        let (glo, ghi, pivot) = if !siblings && group.len() == 1 {
            let half = (pv - glo).max(ghi - pv);
            (pv - half, pv + half, pv)
        } else {
            (glo, ghi, 0.0)
        };
        let gw = ghi - glo;
        let gplace: Vec<f32> = places.iter().map(|p| p - glo).collect();
        let anchor_k = group.iter().position(|&u| u == blocks[i].anchor);
        let (w, offs, goff, acx);
        if centred && siblings {
            // The anchor's box, not the middle of the whole group, stands under the parents: brothers
            // and sisters (and their families) spread to either side and the block widens to hold them.
            let pv = -(uw(&units[blocks[i].anchor]) - m.node_w) / 2.0;
            let ag = anchor_k.map_or(gw / 2.0, |k| gplace[k] + pv);
            let mut x = 0.0;
            let mut centres = Vec::new();
            for &u in &ups {
                centres.push(x + blocks[u].acx);
                x += blocks[u].w + gap;
            }
            let t0 = if centres.is_empty() {
                ag
            } else {
                centres.iter().sum::<f32>() / centres.len() as f32
            };
            let s = (ag - t0).max(0.0);
            let mut o = Vec::new();
            let mut x = s;
            for &u in &ups {
                o.push(x);
                x += blocks[u].w + gap;
            }
            offs = o;
            goff = (t0 + s - ag).max(0.0);
            w = (up_w + s).max(goff + gw);
            acx = goff + ag;
        } else {
            let wv = up_w.max(gw);
            let mut x = (wv - up_w) / 2.0;
            let mut o = Vec::new();
            let mut centres = Vec::new();
            for &u in &ups {
                o.push(x);
                centres.push(x + blocks[u].acx);
                x += blocks[u].w + gap;
            }
            let target = if centres.is_empty() {
                wv / 2.0
            } else {
                centres.iter().sum::<f32>() / centres.len() as f32
            };
            goff = (target - gw / 2.0).clamp(0.0, (wv - gw).max(0.0));
            acx = anchor_k.map_or(0.0, |k| goff + gplace[k] + pivot);
            w = wv;
            offs = o;
        }
        let blk = &mut blocks[i];
        blk.gplace = gplace;
        blk.w = w;
        blk.up_off = offs;
        blk.goff = goff;
        blk.acx = acx;
    }

    // Absolute positions, top-down.
    blocks[0].left = 0.0;
    for i in 0..blocks.len() {
        if all {
            continue;
        }
        let left = blocks[i].left;
        let ups = blocks[i].ups.clone();
        for (k, &u) in ups.iter().enumerate() {
            blocks[u].left = left + blocks[i].up_off[k];
        }
        for (k, &u) in blocks[i].group.clone().iter().enumerate() {
            units[u].cx = left + blocks[i].goff + blocks[i].gplace[k];
        }
    }
    let step_y = m.node_h + m.gap_y * 4.0;
    let has_cousins = units.iter().any(|u| u.rel_cousin);
    let step_p = m.node_w + m.couple_gap;
    for i in 0..units.len() {
        let ch = units[i].children.clone();
        let w = uw(&units[i]);
        let x0 = units[i].cx - w / 2.0;
        // Cousins get a row of their own between the parents' row and the centred person's row.
        let y = if units[i].rel_cousin {
            0.0
        } else if has_cousins && units[i].row >= 0 {
            (units[i].row + 1) as f32 * step_y
        } else {
            units[i].row as f32 * step_y
        };
        let p = units[i].person;
        for k in 0..=units[i].partners {
            let n = &mut nodes[(p + k) as usize];
            n.rect.x = x0 + k as f32 * step_p;
            n.rect.y = y;
        }
        for (k, &c) in ch.iter().enumerate() {
            units[c].cx = units[i].cx + units[i].coff[k];
        }
    }
    // Keep every coordinate non-negative: the canvas bounds start at the origin.
    let min_x = nodes.iter().map(|n| n.rect.x).fold(f32::INFINITY, f32::min);
    let min_y = nodes.iter().map(|n| n.rect.y).fold(f32::INFINITY, f32::min);
    for n in &mut nodes {
        n.rect.x -= min_x;
        n.rect.y -= min_y;
    }

    let (mut edges, mut fams) = unit_edges(&nodes, &units);
    // Parents to their group: one connector per family (the parents that the group hangs from), so a half sibling
    // does not look like a full one.
    // With cousins, the bus into the centred person's row runs below the cousin row, so a parent's
    // vertical line passes that row at the parent's own x, never over a cousin.
    let cousin_bottom = nodes
        .iter()
        .filter(|n| n.rel == Some(Rel::Cousin))
        .map(|n| n.rect.bottom())
        .fold(f32::NEG_INFINITY, f32::max);
    let mut by_parents: HashMap<Vec<u32>, usize> = HashMap::new();
    for blk in &blocks {
        for &r in &blk.group {
            let parents: Vec<u32> = (0..2)
                .filter(|&s| units[r].hang[s])
                .filter_map(|s| blk.slots[s])
                .map(|ub| units[blocks[ub].anchor].person)
                .collect();
            if parents.is_empty() {
                continue;
            }
            let zone_top = (has_cousins && units[r].row == 0).then_some(cousin_bottom);
            let at = *by_parents.entry(parents.clone()).or_insert_with(|| {
                fams.push(Family::new(parents, Vec::new(), zone_top));
                fams.len() - 1
            });
            fams[at].children.push(units[r].person);
        }
    }
    for blk in &blocks {
        let bar: Vec<u32> = blk
            .group
            .iter()
            .filter(|&&r| r == blk.anchor || units[r].hang == [false, false])
            .map(|&r| units[r].person)
            .collect();
        fams.push(Family::barred(bar));
    }
    let routed = route(&mut nodes, &mut edges, fams, m);
    edges.extend(routed);
    for v in 0..nodes.len() {
        if let Some(orig) = nodes[v].dup_of {
            edges.push(dup_edge(&nodes, v as u32, orig, m.gap_x));
        }
    }

    // Keyboard links: up to a parent, down to a child, left and right along the row.
    for blk in &blocks {
        let first_up = |hang: Hang| {
            (0..2)
                .filter(|&s| hang[s])
                .find_map(|s| blk.slots[s])
                .map(|ub| units[blocks[ub].anchor].person)
        };
        for &r in &blk.group {
            let p = units[r].person;
            nodes[p as usize].nav.parent = first_up(units[r].hang);
        }
        let anchor = units[blk.anchor].person;
        for &ub in &blk.ups {
            let up = units[blocks[ub].anchor].person;
            if nodes[up as usize].nav.child.is_none() {
                nodes[up as usize].nav.child = Some(anchor);
            }
        }
    }
    for u in &units {
        for k in 1..=u.partners {
            let (parent, child) = {
                let p = &nodes[u.person as usize].nav;
                (p.parent, p.child)
            };
            let n = &mut nodes[(u.person + k) as usize].nav;
            n.parent = parent;
            n.child = child;
        }
    }
    link_rows(&mut nodes);
    Layout::finish(nodes, edges, 0, truncated)
}
