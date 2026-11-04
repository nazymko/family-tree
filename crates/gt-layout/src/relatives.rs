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
}

impl Tree {
    fn limit(self) -> u8 {
        match self {
            Tree::Focus(n) => n,
            Tree::Niece => 1,
            Tree::Aunt => 1,
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
        let with_sibs = !dup && (focus || (row == -1 && o.aunts));
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
        });
        Some(self.blocks.len() - 1)
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
            match b.make_block(pk, row - 1, &o, Some(slot)) {
                Some(nb) => {
                    b.blocks[cur].slots[slot] = Some(nb);
                    b.blocks[cur].ups.push(nb);
                }
                None => b.nodes[xnode as usize].more_up = true,
            }
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
    for i in (0..blocks.len()).rev() {
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
        let gw = ghi - glo;
        let gplace: Vec<f32> = places.iter().map(|p| p - glo).collect();
        let w = up_w.max(gw);
        let mut x = (w - up_w) / 2.0;
        let mut offs = Vec::new();
        let mut centres = Vec::new();
        for &u in &ups {
            offs.push(x);
            centres.push(x + blocks[u].acx);
            x += blocks[u].w + gap;
        }
        let target = if centres.is_empty() {
            w / 2.0
        } else {
            centres.iter().sum::<f32>() / centres.len() as f32
        };
        let goff = (target - gw / 2.0).clamp(0.0, (w - gw).max(0.0));
        let acx = group
            .iter()
            .position(|&u| u == blocks[i].anchor)
            .map_or(0.0, |k| goff + gplace[k]);
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

    let mut edges = Vec::new();
    for u in &units {
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
        for &c in &u.children {
            let cu = &units[c];
            let (sx, sy) = match cu.attach {
                Some(pn) => {
                    let a = nodes[(pn - 1) as usize].rect;
                    let b = nodes[pn as usize].rect;
                    ((a.right() + b.x) / 2.0, b.cy())
                }
                None => {
                    let a = nodes[u.person as usize].rect;
                    (a.cx(), a.bottom())
                }
            };
            let t = nodes[cu.person as usize].rect;
            let mid = nodes[u.person as usize].rect.bottom() + m.gap_y * 2.0;
            edges.push(Edge {
                from: u.person,
                to: cu.person,
                kind: EdgeKind::Parent,
                pts: vec![sx, sy, sx, mid, t.cx(), mid, t.cx(), t.y],
            });
        }
    }
    // Parents to their group: one bar per family, drawn at its own height so a half sibling
    // does not look like a full one.
    // With cousins, the bar into the centred person's row runs below the cousin row, so a parent's
    // vertical line passes that row at the parent's own x, never over a cousin.
    let cousin_bottom = nodes
        .iter()
        .filter(|n| n.rel == Some(Rel::Cousin))
        .map(|n| n.rect.bottom())
        .fold(f32::NEG_INFINITY, f32::max);
    for blk in &blocks {
        for &r in &blk.group {
            let t = nodes[units[r].person as usize].rect;
            let hung: Vec<usize> = (0..2)
                .filter(|&s| units[r].hang[s] && blk.slots[s].is_some())
                .collect();
            let k = match (hung.as_slice(), blk.slots) {
                ([0], [_, Some(_)]) => 1.2,
                ([1], [Some(_), _]) => 2.8,
                _ => 2.0,
            };
            for s in hung {
                let Some(ub) = blk.slots[s] else { continue };
                let pn = units[blocks[ub].anchor].person;
                let a = nodes[pn as usize].rect;
                let top = if has_cousins && units[r].row == 0 {
                    cousin_bottom
                } else {
                    a.bottom()
                };
                let mid = top + m.gap_y * k;
                edges.push(Edge {
                    from: pn,
                    to: units[r].person,
                    kind: EdgeKind::Parent,
                    pts: vec![a.cx(), a.bottom(), a.cx(), mid, t.cx(), mid, t.cx(), t.y],
                });
            }
        }
    }
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
    for n in &mut nodes {
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
    Layout::finish(nodes, edges, 0, truncated)
}
