//! Public data types shared by both layouts.

use crate::grid::Grid;

/// Dense person index chosen by the caller.
pub type Key = u32;

/// Read access to the family structure the charts are drawn from.
pub trait ChartSource {
    /// Preferred parents, upper slot first (callers put the male parent first when only one is male).
    fn parents(&self, p: Key) -> [Option<Key>; 2];
    /// Partnerships of `p` in display order: (partner or None, children in birth order).
    fn unions(&self, p: Key) -> Vec<(Option<Key>, Vec<Key>)>;
}

/// Sizes in world units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub node_w: f32,
    pub node_h: f32,
    pub gap_x: f32,
    pub gap_y: f32,
    pub couple_gap: f32,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            node_w: 176.0,
            node_h: 56.0,
            gap_x: 40.0,
            gap_y: 12.0,
            couple_gap: 8.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub(crate) fn right(&self) -> f32 {
        self.x + self.w
    }
    pub(crate) fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub(crate) fn cy(&self) -> f32 {
        self.y + self.h / 2.0
    }
    pub(crate) fn cx(&self) -> f32 {
        self.x + self.w / 2.0
    }
    /// Strict overlap (touching rectangles do not intersect).
    pub(crate) fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }
    /// Overlap including shared borders, for zero-width polyline boxes.
    pub(crate) fn touches(&self, o: &Rect) -> bool {
        self.x <= o.right() && o.x <= self.right() && self.y <= o.bottom() && o.y <= self.bottom()
    }
    fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect {
            x,
            y,
            w: self.right().max(o.right()) - x,
            h: self.bottom().max(o.bottom()) - y,
        }
    }
}

/// Keyboard-navigation links between nodes (ids into `Layout::nodes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Nav {
    pub parent: Option<u32>,
    pub child: Option<u32>,
    pub prev: Option<u32>,
    pub next: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: u32,
    pub key: Key,
    pub rect: Rect,
    pub r#gen: i16,
    /// Ahnentafel number (pedigree only; `None` past 63 generations).
    pub ahnen: Option<u64>,
    /// Id of the first occurrence when this person was already placed.
    pub dup_of: Option<u32>,
    pub more_up: bool,
    pub more_down: bool,
    pub nav: Nav,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    Parent,
    Partner,
    Dup,
}

/// An orthogonal polyline: `pts` is `x0, y0, x1, y1, ...`.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub from: u32,
    pub to: u32,
    pub kind: EdgeKind,
    pub pts: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub bounds: Rect,
    pub root: u32,
    pub truncated: bool,
    grid: Grid,
}

impl Layout {
    pub(crate) fn finish(nodes: Vec<Node>, edges: Vec<Edge>, root: u32, truncated: bool) -> Self {
        let mut it = nodes.iter().map(|n| n.rect);
        let bounds = match it.next() {
            Some(first) => it.fold(first, |a, r| a.union(&r)),
            None => Rect::default(),
        };
        let grid = Grid::build(&nodes, &edges);
        Layout {
            nodes,
            edges,
            bounds,
            root,
            truncated,
            grid,
        }
    }

    /// Nodes and edges whose boxes intersect `r`, in id order.
    pub fn query(&self, r: &Rect) -> (Vec<&Node>, Vec<&Edge>) {
        let (n, e) = self.grid.query(r);
        (
            n.into_iter().map(|i| &self.nodes[i as usize]).collect(),
            e.into_iter().map(|i| &self.edges[i as usize]).collect(),
        )
    }
}

pub(crate) fn new_node(id: u32, key: Key, rect: Rect, r#gen: i16) -> Node {
    Node {
        id,
        key,
        rect,
        r#gen,
        ahnen: None,
        dup_of: None,
        more_up: false,
        more_down: false,
        nav: Nav::default(),
    }
}

/// Make `kids` the ordered children of `parent` in the navigation links.
pub(crate) fn link_children(nodes: &mut [Node], parent: u32, kids: &[u32]) {
    nodes[parent as usize].nav.child = kids.first().copied();
    for (i, &k) in kids.iter().enumerate() {
        let n = &mut nodes[k as usize].nav;
        n.parent = Some(parent);
        n.prev = i.checked_sub(1).map(|j| kids[j]);
        n.next = kids.get(i + 1).copied();
    }
}

/// Route from a duplicate to its first occurrence around the right-hand side.
pub(crate) fn dup_edge(nodes: &[Node], dup: u32, orig: u32, gap_x: f32) -> Edge {
    let a = nodes[dup as usize].rect;
    let b = nodes[orig as usize].rect;
    let mx = a.right().max(b.right()) + gap_x / 2.0;
    Edge {
        from: dup,
        to: orig,
        kind: EdgeKind::Dup,
        pts: vec![a.right(), a.cy(), mx, a.cy(), mx, b.cy(), b.right(), b.cy()],
    }
}
