//! Public data types shared by both layouts.

use crate::grid::Grid;

/// Dense person index chosen by the caller.
pub type Key = u32;

/// Read access to the family structure the charts are drawn from.
pub trait ChartSource {
    /// Preferred parents, upper slot first (callers put the male parent first when only one is male).
    fn parents(&self, p: Key) -> [Option<Key>; 2];
    /// Sortable birth date of `p` (earlier is smaller), `None` when unknown. Used to order siblings.
    fn birth_sort(&self, _p: Key) -> Option<i32> {
        None
    }
    /// Partnerships of `p` in display order: (partner or None, children in birth order).
    fn unions(&self, p: Key) -> Vec<(Option<Key>, Vec<Key>)>;
    /// Every family in which `p` is a child: its partners (either may be unknown, both may be: a family with only
    /// children) and its children. `unions` cannot reach a family without partners, nor the families beside the
    /// preferred one; the brothers and sisters of `p` are the other children of all of them.
    fn child_families(&self, _p: Key) -> Vec<([Option<Key>; 2], Vec<Key>)> {
        Vec::new()
    }
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
            node_h: 66.0,
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

/// Whose partner a partner box is (relatives chart only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartnerOf {
    Focus,
    Sibling,
    AuntUncle,
    /// A descendant of the focus person, this many generations down.
    Descendant(u8),
    /// A partner of a more distant relative (see [`Rel::Kin`]).
    Kin {
        up: u8,
        down: u8,
    },
}

/// How a person on the relatives chart is related to the centred person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rel {
    Focus,
    /// Parent is 1, grandparent 2, and so on.
    Ancestor(u8),
    /// Child is 1, grandchild 2, and so on.
    Descendant(u8),
    Sibling {
        half: bool,
    },
    /// A brother or sister of a parent.
    AuntUncle {
        half: bool,
    },
    /// A child of a brother or sister.
    NieceNephew,
    /// A child of a parent's brother or sister.
    Cousin,
    /// Any other blood relative on the chart with all relatives: `up` generations from the centred person
    /// to the nearest common ancestor, then `down` generations from that ancestor to this person.
    Kin {
        up: u8,
        down: u8,
    },
    Partner(PartnerOf),
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
    /// Relationship to the centred person (relatives chart only).
    pub rel: Option<Rel>,
    pub nav: Nav,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    Parent,
    Partner,
    Dup,
    /// Brothers and sisters of a family without partners, joined by a short bar above their row.
    Sibling,
}

/// An orthogonal polyline: `pts` is `x0, y0, x1, y1, ...`.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub from: u32,
    pub to: u32,
    pub kind: EdgeKind,
    pub pts: Vec<f32>,
}

/// Where the oldest generation of a chart is drawn: the side the ancestors sit on (for a chart of descendants, the
/// side of the person it starts from).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Top,
    Bottom,
    Left,
    Right,
}

impl Direction {
    /// True when generations follow the x axis (ancestors on the left or on the right).
    pub fn is_horizontal(self) -> bool {
        matches!(self, Direction::Left | Direction::Right)
    }
}

impl Metrics {
    /// The sizes to lay a chart out with so that, once turned by [`Layout::orient`] from its `native` direction
    /// to `to`, the cards keep their real size and the spacing fits the new direction: a card is wider than tall,
    /// so a chart turned a quarter has its generation axis along the card width and its sibling axis along the
    /// card height. The layout code sees a chart in its own native direction with the card sides swapped.
    pub fn for_direction(&self, native: Direction, to: Direction) -> Metrics {
        if native.is_horizontal() == to.is_horizontal() {
            return *self;
        }
        if native.is_horizontal() {
            // Native generations along x, wanted along y: the generation step is a card height plus a line gap
            // as wide as a vertical chart's, the siblings are spaced like the columns of a vertical chart.
            Metrics {
                node_w: self.node_h,
                node_h: self.node_w,
                gap_x: self.gap_y * 4.0,
                gap_y: self.gap_x,
                couple_gap: self.couple_gap,
            }
        } else {
            // Native generations along y, wanted along x: a generation step as wide as a pedigree column,
            // siblings one card height apart with room for the lines between them.
            Metrics {
                node_w: self.node_h,
                node_h: self.node_w,
                gap_x: self.gap_y * 2.0,
                gap_y: self.gap_x / 4.0,
                couple_gap: self.couple_gap,
            }
        }
    }
}

/// The line a layout is mirrored about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Top and bottom swap places (top-down charts grow upward instead).
    Vertical,
    /// Left and right swap places (the left-to-right pedigree grows leftward instead).
    Horizontal,
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

    /// Mirrors the whole drawing inside its own bounds: node boxes and edge polylines move, the
    /// bounds, ids and navigation links (which are logical, not visual) stay as they are.
    pub fn mirror(&mut self, axis: Axis) {
        let b = self.bounds;
        let (lo, hi) = match axis {
            Axis::Vertical => (b.y, b.y + b.h),
            Axis::Horizontal => (b.x, b.x + b.w),
        };
        for n in &mut self.nodes {
            match axis {
                Axis::Vertical => n.rect.y = lo + hi - n.rect.y - n.rect.h,
                Axis::Horizontal => n.rect.x = lo + hi - n.rect.x - n.rect.w,
            }
        }
        let from = usize::from(axis == Axis::Vertical);
        for e in &mut self.edges {
            for p in e.pts.chunks_exact_mut(2) {
                p[from] = lo + hi - p[from];
            }
        }
        self.grid = Grid::build(&self.nodes, &self.edges);
    }

    /// Swaps the x and y axes of the whole drawing: boxes (with their width and height), edge polylines and
    /// bounds. Ids and navigation links stay as they are.
    pub fn transpose(&mut self) {
        for n in &mut self.nodes {
            let r = n.rect;
            n.rect = Rect {
                x: r.y,
                y: r.x,
                w: r.h,
                h: r.w,
            };
        }
        for e in &mut self.edges {
            for p in e.pts.chunks_exact_mut(2) {
                p.swap(0, 1);
            }
        }
        let b = self.bounds;
        self.bounds = Rect {
            x: b.y,
            y: b.x,
            w: b.h,
            h: b.w,
        };
        self.grid = Grid::build(&self.nodes, &self.edges);
    }

    /// Turns a layout made in its `native` direction (laid out with [`Metrics::for_direction`]) to `to`.
    pub fn orient(&mut self, native: Direction, to: Direction) {
        let mut side = native;
        if native.is_horizontal() != to.is_horizontal() {
            self.transpose();
            // Low coordinates stay low: top becomes left, and a high side (bottom, right) the same in turn.
            side = match native {
                Direction::Top => Direction::Left,
                Direction::Left => Direction::Top,
                Direction::Bottom => Direction::Right,
                Direction::Right => Direction::Bottom,
            };
        }
        if side != to {
            self.mirror(if to.is_horizontal() {
                Axis::Horizontal
            } else {
                Axis::Vertical
            });
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
        rel: None,
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

/// Where a person with several partnerships stands among their partners (descendant charts and the relatives charts).
///
/// Rule: with one partner the partner stands to the right of the person, as always. With two or more the FIRST
/// partner stands to the LEFT of the person and the others continue to the right in order, so two partners read
/// `P1 - person - P2` with the person in the middle (the picture the whole-tree chart already draws), three read
/// `P1 - person - P2 - P3`. Member 0 is the person, member `k` the k-th partner (the k-th union with a partner).
/// Every partner's couple connector joins that partner to the PERSON, never to the neighbouring partner: a partner who
/// does not stand next to the person (the third and later) is joined by a connector dipping under the cards between.
pub(crate) fn member_col(partners: u32, member: u32) -> u32 {
    if partners < 2 {
        return member;
    }
    match member {
        0 => 1,
        1 => 0,
        k => k,
    }
}

/// Columns to the left of the person's card within the unit's box (0, or 1 with two or more partners).
pub(crate) fn lead_cols(partners: u32) -> u32 {
    member_col(partners, 0)
}

/// Offset from the centre of the unit's box to the centre of the person's card.
pub(crate) fn person_shift(partners: u32, m: &Metrics) -> f32 {
    let pstep = m.node_w + m.couple_gap;
    let w = m.node_w * (1.0 + partners as f32) + m.couple_gap * partners as f32;
    -w / 2.0 + m.node_w / 2.0 + lead_cols(partners) as f32 * pstep
}

/// The couple connector between a person (card `a`) and one partner (card `b`), and the point the children's line
/// hangs from. Next to each other: a straight line between the cards, the children hang from its middle. Apart (a
/// third partner): a line dipping `dip` under the cards in between, the children hang from the middle of the dip.
pub(crate) fn couple_link(a: Rect, b: Rect, dip: f32) -> (Vec<f32>, (f32, f32)) {
    let (l, r) = if a.x <= b.x { (a, b) } else { (b, a) };
    if (a.y - b.y).abs() < 0.5 && r.x - l.right() < l.w {
        (
            vec![l.right(), l.cy(), r.x, r.cy()],
            ((l.right() + r.x) / 2.0, l.cy()),
        )
    } else {
        let (xa, xb) = (a.cx(), b.cx());
        let y = a.bottom() + dip;
        (
            vec![xa, a.bottom(), xa, y, xb, y, xb, b.bottom()],
            ((xa + xb) / 2.0, y),
        )
    }
}

/// Puts the unions of one unit in the order of their anchors along the row, so the children of each family hang below
/// their own anchor and no two child lines cross. The anchor of a union with a partner is the couple connector (the
/// middle of the person's and the partner's columns); the anchor of a union without a partner is the person's own card.
/// With one partner (drawn to the right of the person) the partnerless families therefore come first. The sort is stable:
/// families with the same anchor keep their order, and nothing moves when every union has a partner.
pub(crate) fn order_unions<T>(person: u32, partners: u32, unions: &mut [(Option<u32>, T)]) {
    let own = member_col(partners, 0);
    unions.sort_by_key(|(at, _)| match at {
        Some(node) => own + member_col(partners, node - person),
        None => 2 * own,
    });
}
