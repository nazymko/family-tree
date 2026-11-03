//! Descendant chart: Walker / Buchheim linear-time tidy tree over couple boxes.
//!
//! A unit is one person plus every partner drawn to the right of them; the children of all
//! of the person's unions hang below the unit, grouped per union in order. The tree
//! algorithm (Buchheim, Jünger, Leipert 2002) works on units of varying width.

use std::collections::{HashMap, VecDeque};

use crate::types::{
    ChartSource, Edge, EdgeKind, Key, Layout, Metrics, Node, Rect, dup_edge, link_children,
    new_node,
};

struct Unit {
    /// Node id of the person; their partners are the next `partners` node ids.
    person: u32,
    partners: u32,
    depth: usize,
    parent: Option<usize>,
    /// Partner node whose union produced this unit (None: the person's own bottom edge).
    attach: Option<u32>,
    children: Vec<usize>,
}

type Pending = Vec<(Option<u32>, Vec<Key>)>;

struct Builder<'a> {
    src: &'a dyn ChartSource,
    max_nodes: usize,
    rect: Rect,
    nodes: Vec<Node>,
    visited: HashMap<Key, u32>,
    units: Vec<Unit>,
    /// Unions of each unit not yet expanded: (partner node, children).
    pending: Vec<Pending>,
    truncated: bool,
}

impl Builder<'_> {
    fn add_node(&mut self, key: Key, depth: usize) -> u32 {
        let id = self.nodes.len() as u32;
        let mut n = new_node(id, key, self.rect, depth as i16);
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
        depth: usize,
        parent: Option<usize>,
        attach: Option<u32>,
    ) -> Option<usize> {
        if self.nodes.len() >= self.max_nodes {
            self.truncated = true;
            return None;
        }
        let person = self.add_node(key, depth);
        let mut partners = 0;
        let mut pend: Pending = Vec::new();
        if self.nodes[person as usize].dup_of.is_none() {
            for (partner, children) in self.src.unions(key) {
                let mut at = None;
                if let Some(pk) = partner {
                    if self.nodes.len() >= self.max_nodes {
                        self.truncated = true;
                    } else {
                        at = Some(self.add_node(pk, depth));
                        partners += 1;
                    }
                }
                pend.push((at, children));
            }
        }
        self.pending.push(pend);
        self.units.push(Unit {
            person,
            partners,
            depth,
            parent,
            attach,
            children: Vec::new(),
        });
        Some(self.units.len() - 1)
    }
}

/// Lay out the descendants of `root` over `generations` rows (the root is row 0).
pub fn descendants(
    src: &dyn ChartSource,
    root: Key,
    generations: u8,
    m: &Metrics,
    max_nodes: usize,
) -> Layout {
    let generations = usize::from(generations.max(1));
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
        truncated: false,
    };
    b.make_unit(root, 0, None, None);
    let mut queue = VecDeque::from([0usize]);
    while let Some(u) = queue.pop_front() {
        let unions = std::mem::take(&mut b.pending[u]);
        let person = b.units[u].person;
        let depth = b.units[u].depth;
        if unions.iter().all(|(_, c)| c.is_empty()) {
            continue;
        }
        if depth + 1 >= generations {
            b.nodes[person as usize].more_down = true;
            continue;
        }
        let mut kid_nodes = Vec::new();
        'unions: for (attach, children) in unions {
            for key in children {
                let Some(cu) = b.make_unit(key, depth + 1, Some(u), attach) else {
                    b.nodes[person as usize].more_down = true;
                    break 'unions;
                };
                b.units[u].children.push(cu);
                kid_nodes.push(b.units[cu].person);
                queue.push_back(cu);
            }
        }
        link_children(&mut b.nodes, person, &kid_nodes);
    }

    let Builder {
        mut nodes,
        units,
        truncated,
        ..
    } = b;

    let widths: Vec<f64> = units
        .iter()
        .map(|u| {
            let p = f64::from(u.partners);
            f64::from(m.node_w) * (1.0 + p) + f64::from(m.couple_gap) * p
        })
        .collect();
    let centres = tidy(&units, &widths, f64::from(m.gap_x));
    let min_left = centres
        .iter()
        .zip(&widths)
        .map(|(c, w)| c - w / 2.0)
        .fold(f64::INFINITY, f64::min);

    let step_y = m.node_h + m.gap_y * 4.0;
    let step_p = m.node_w + m.couple_gap;
    for (i, u) in units.iter().enumerate() {
        let left = (centres[i] - widths[i] / 2.0 - min_left) as f32;
        let y = u.depth as f32 * step_y;
        for k in 0..=u.partners {
            let n = &mut nodes[(u.person + k) as usize];
            n.rect.x = left + k as f32 * step_p;
            n.rect.y = y;
        }
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
    for v in 0..nodes.len() {
        if let Some(orig) = nodes[v].dup_of {
            edges.push(dup_edge(&nodes, v as u32, orig, m.gap_x));
        }
    }
    Layout::finish(nodes, edges, 0, truncated)
}

/// Buchheim's tidy tree over variable-width units; returns each unit's centre x.
///
/// Unit ids are in breadth-first order (parents before children), unit 0 is the root.
fn tidy(units: &[Unit], w: &[f64], gap: f64) -> Vec<f64> {
    let n = units.len();
    let mut t = Tidy {
        units,
        w,
        gap,
        x: vec![0.0; n],
        md: vec![0.0; n],
        thread: vec![None; n],
        anc: (0..n).collect(),
        change: vec![0.0; n],
        shift: vec![0.0; n],
        number: vec![0; n],
    };
    for u in units {
        for (i, &c) in u.children.iter().enumerate() {
            t.number[c] = i;
        }
    }
    let mut default_anc: Vec<usize> = units
        .iter()
        .map(|u| u.children.first().copied().unwrap_or(0))
        .collect();

    // Left-to-right post-order without recursion.
    let mut order = Vec::with_capacity(n);
    let mut stack = vec![0usize];
    while let Some(v) = stack.pop() {
        order.push(v);
        stack.extend(&units[v].children);
    }
    order.reverse();

    for &v in &order {
        let left = t.left_sibling(v);
        let ch = &units[v].children;
        if let (Some(&first), Some(&last)) = (ch.first(), ch.last()) {
            t.execute_shifts(v);
            let mid = (t.x[first] + t.x[last]) / 2.0;
            if let Some(l) = left {
                t.x[v] = t.x[l] + t.dist(v, l);
                t.md[v] = t.x[v] - mid;
            } else {
                t.x[v] = mid;
            }
        } else if let Some(l) = left {
            t.x[v] = t.x[l] + t.dist(v, l);
        }
        if let Some(p) = units[v].parent {
            default_anc[p] = t.apportion(v, default_anc[p]);
        }
    }

    // Second walk: accumulate the modifiers of all ancestors (parents precede children).
    let mut out = t.x.clone();
    let mut msum = vec![0.0; n];
    for v in 0..n {
        out[v] = t.x[v] + msum[v];
        for &c in &units[v].children {
            msum[c] = msum[v] + t.md[v];
        }
    }
    out
}

struct Tidy<'a> {
    units: &'a [Unit],
    w: &'a [f64],
    gap: f64,
    x: Vec<f64>,
    md: Vec<f64>,
    thread: Vec<Option<usize>>,
    anc: Vec<usize>,
    change: Vec<f64>,
    shift: Vec<f64>,
    number: Vec<usize>,
}

impl Tidy<'_> {
    fn dist(&self, a: usize, b: usize) -> f64 {
        (self.w[a] + self.w[b]) / 2.0 + self.gap
    }

    fn next_left(&self, v: usize) -> Option<usize> {
        self.units[v].children.first().copied().or(self.thread[v])
    }

    fn next_right(&self, v: usize) -> Option<usize> {
        self.units[v].children.last().copied().or(self.thread[v])
    }

    fn left_sibling(&self, v: usize) -> Option<usize> {
        let p = self.units[v].parent?;
        let j = self.number[v].checked_sub(1)?;
        Some(self.units[p].children[j])
    }

    fn execute_shifts(&mut self, v: usize) {
        let (mut shift, mut change) = (0.0, 0.0);
        for i in (0..self.units[v].children.len()).rev() {
            let c = self.units[v].children[i];
            self.x[c] += shift;
            self.md[c] += shift;
            change += self.change[c];
            shift += self.shift[c] + change;
        }
    }

    fn move_subtree(&mut self, wl: usize, wr: usize, shift: f64) {
        let subtrees = (self.number[wr] - self.number[wl]) as f64;
        self.change[wr] -= shift / subtrees;
        self.shift[wr] += shift;
        self.change[wl] += shift / subtrees;
        self.x[wr] += shift;
        self.md[wr] += shift;
    }

    fn apportion(&mut self, v: usize, mut default_anc: usize) -> usize {
        let Some(w) = self.left_sibling(v) else {
            return default_anc;
        };
        let Some(parent) = self.units[v].parent else {
            return default_anc;
        };
        let (mut vir, mut vor, mut vil, mut vol) = (v, v, w, self.units[parent].children[0]);
        let (mut sir, mut sor, mut sil, mut sol) =
            (self.md[v], self.md[v], self.md[vil], self.md[vol]);
        while let (Some(nr), Some(nl)) = (self.next_right(vil), self.next_left(vir)) {
            vil = nr;
            vir = nl;
            vol = self.next_left(vol).unwrap_or(vol);
            vor = self.next_right(vor).unwrap_or(vor);
            self.anc[vor] = v;
            let shift = (self.x[vil] + sil) - (self.x[vir] + sir) + self.dist(vil, vir);
            if shift > 0.0 {
                let a = if self.units[self.anc[vil]].parent == Some(parent) {
                    self.anc[vil]
                } else {
                    default_anc
                };
                self.move_subtree(a, v, shift);
                sir += shift;
                sor += shift;
            }
            sil += self.md[vil];
            sir += self.md[vir];
            sol += self.md[vol];
            sor += self.md[vor];
        }
        if let Some(nr) = self.next_right(vil)
            && self.next_right(vor).is_none()
        {
            self.thread[vor] = Some(nr);
            self.md[vor] += sil - sor;
        } else {
            if let Some(nl) = self.next_left(vir)
                && self.next_left(vol).is_none()
            {
                self.thread[vol] = Some(nl);
                self.md[vol] += sir - sol;
            }
            default_anc = v;
        }
        default_anc
    }
}
