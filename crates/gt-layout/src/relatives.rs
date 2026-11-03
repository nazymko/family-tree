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
    /// A parent's sibling: no children shown.
    Aunt,
}

impl Tree {
    fn limit(self) -> u8 {
        match self {
            Tree::Focus(n) => n,
            Tree::Niece => 1,
            Tree::Aunt => 0,
        }
    }
}

struct Unit {
    /// Node id of the person; partners are the next `partners` node ids.
    person: u32,
    partners: u32,
    row: i32,
    /// Partner node whose union produced this unit (None: the parent's own bottom edge).
    attach: Option<u32>,
    children: Vec<usize>,
    /// Group roots only: the parents this person descends from.
    hang: Hang,
    /// Width of the unit's subtree, and the unit centre inside it.
    sw: f32,
    ucx: f32,
    /// Absolute left edge of the subtree.
    left: f32,
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
            let full = if other.is_some() {
                partner == other
            } else {
                partner.is_none()
            };
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
            attach,
            children: Vec::new(),
            hang: [true, true],
            sw: 0.0,
            ucx: 0.0,
            left: 0.0,
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
                if !matches!(tree, Tree::Aunt) {
                    self.nodes[person as usize].more_down = true;
                }
                continue;
            }
            let row = self.units[u].row + 1;
            let step = (depth + 1) as u8;
            let (rel, partner_rel) = match tree {
                Tree::Focus(_) => (
                    Rel::Descendant(step),
                    Some(Rel::Partner(PartnerOf::Descendant(step))),
                ),
                _ => (Rel::NieceNephew, None),
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

    fn make_block(&mut self, key: Key, row: i32, o: &RelativesOptions) -> Option<usize> {
        if self.nodes.len() >= self.max_nodes {
            self.truncated = true;
            return None;
        }
        let dup = self.visited.contains_key(&key);
        let focus = row == 0;
        let with_sibs = !dup && (focus || (row == -1 && o.aunts));
        let sibs = if with_sibs {
            siblings(self.src, key)
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
        self.blocks.push(Block {
            row,
            anchor,
            group,
            slots: [None, None],
            ups: Vec::new(),
            up_off: Vec::new(),
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
    b.make_block(root, 0, &o);
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
            match b.make_block(pk, row - 1, &o) {
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
        if ch.is_empty() {
            units[i].sw = w;
            units[i].ucx = w / 2.0;
            continue;
        }
        let cw: f32 = ch.iter().map(|&c| units[c].sw).sum::<f32>() + gap * (ch.len() - 1) as f32;
        let sw = w.max(cw);
        let start = (sw - cw) / 2.0;
        let last_off = start + cw - units[ch[ch.len() - 1]].sw;
        let mid = (start + units[ch[0]].ucx + last_off + units[ch[ch.len() - 1]].ucx) / 2.0;
        units[i].sw = sw;
        units[i].ucx = mid.clamp(w / 2.0, sw - w / 2.0);
    }
    for i in (0..blocks.len()).rev() {
        let ups = blocks[i].ups.clone();
        let up_w: f32 = ups.iter().map(|&u| blocks[u].w).sum::<f32>()
            + gap * ups.len().saturating_sub(1) as f32;
        let group = blocks[i].group.clone();
        let gw: f32 = group.iter().map(|&u| units[u].sw).sum::<f32>()
            + gap * group.len().saturating_sub(1) as f32;
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
        let mut gx = goff;
        let mut acx = 0.0;
        for &u in &group {
            if u == blocks[i].anchor {
                acx = gx + units[u].ucx;
            }
            gx += units[u].sw + gap;
        }
        let blk = &mut blocks[i];
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
        let mut gx = left + blocks[i].goff;
        for &u in &blocks[i].group.clone() {
            units[u].left = gx;
            gx += units[u].sw + gap;
        }
    }
    let step_y = m.node_h + m.gap_y * 4.0;
    let step_p = m.node_w + m.couple_gap;
    for i in 0..units.len() {
        let ch = units[i].children.clone();
        let w = uw(&units[i]);
        let x0 = units[i].left + units[i].ucx - w / 2.0;
        let y = units[i].row as f32 * step_y;
        let p = units[i].person;
        for k in 0..=units[i].partners {
            let n = &mut nodes[(p + k) as usize];
            n.rect.x = x0 + k as f32 * step_p;
            n.rect.y = y;
        }
        if !ch.is_empty() {
            let cw: f32 =
                ch.iter().map(|&c| units[c].sw).sum::<f32>() + gap * (ch.len() - 1) as f32;
            let mut cx = units[i].left + (units[i].sw - cw) / 2.0;
            for &c in &ch {
                units[c].left = cx;
                cx += units[c].sw + gap;
            }
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
                let mid = a.bottom() + m.gap_y * k;
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
        na.r#gen
            .cmp(&nb.r#gen)
            .then(na.rect.x.total_cmp(&nb.rect.x))
            .then(a.cmp(&b))
    });
    for w in order.windows(2) {
        if nodes[w[0]].r#gen == nodes[w[1]].r#gen {
            nodes[w[0]].nav.next = Some(w[1] as u32);
            nodes[w[1]].nav.prev = Some(w[0] as u32);
        }
    }
    Layout::finish(nodes, edges, 0, truncated)
}
