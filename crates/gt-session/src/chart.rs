//! Chart windows and the outline: pedigree / descendants layouts from `gt-layout` held per open
//! chart, served as viewport windows. At most two charts stay open (least recently used goes).

use crate::dto::{
    ChartEdge, ChartKind, ChartLocation, ChartMeta, ChartNav, ChartNode, ChartRequest, ChartSize,
    ChartWindow, EdgeKindDto, Kinship, KinshipKind, OutlineGroupKind, OutlineItem, PersonRef, Rect,
    RoleDto, RoleKind, SexKind, TreeDirection, WholeBlockDto, WholeMeta,
};
use crate::error::{ApiError, ErrorCode};
use crate::index::{Links, PersonIndex};
use crate::kinship::{Kin, KinMemo};
use crate::relation::{relation_label, relation_role};
use crate::session::Session;
use gt_core::tree::Tree;
use gt_layout::{
    Axis, BlockKind, ChartSource, EdgeKind, Key, Layout, Metrics, Rel, RelativesOptions,
    WholeExtras, WholeFamily, WholeInput,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use uuid::Uuid;

/// Node cap per chart.
pub const MAX_NODES: usize = 20_000;
const MAX_CHARTS: usize = 2;
const MAX_OUTLINE: usize = 100;
/// Below this zoom the canvas draws dots only, so the window carries no text.
const TEXT_ZOOM: f32 = 0.25;
const STALE: &str = "The tree changed since this chart was opened. Reopen the chart.";

/// `gt-layout` input over a loaded tree; keys are person-index slots.
pub struct TreeSource<'a> {
    pub tree: &'a Tree,
    pub links: &'a Links,
    pub index: &'a PersonIndex,
}

impl TreeSource<'_> {
    fn sex(&self, k: Key) -> Option<SexKind> {
        self.index.summaries.get(k as usize).map(|s| s.sex)
    }
}

impl ChartSource for TreeSource<'_> {
    /// Preferred parent family (a child link marked preferred, else the first); the male parent
    /// goes first when exactly one is male, a lone mother takes the second slot.
    fn parents(&self, p: Key) -> [Option<Key>; 2] {
        let Some(s) = self.index.summaries.get(p as usize) else {
            return [None, None];
        };
        let Some(links) = self.links.child_in.get(&s.id) else {
            return [None, None];
        };
        let pick = links
            .iter()
            .find(|(f, i)| {
                self.tree
                    .families
                    .get(f)
                    .and_then(|f| f.children.get(*i))
                    .is_some_and(|c| c.preferred)
            })
            .or_else(|| links.first());
        let Some(fam) = pick.and_then(|(f, _)| self.tree.families.get(f)) else {
            return [None, None];
        };
        let mut a = fam.partner1.and_then(|id| self.index.slot(id));
        let mut b = fam.partner2.and_then(|id| self.index.slot(id));
        let male = |k: Option<Key>| k.is_some_and(|k| self.sex(k) == Some(SexKind::Male));
        if male(b) && !male(a) {
            std::mem::swap(&mut a, &mut b);
        }
        if b.is_none() && a.is_some_and(|k| self.sex(k) == Some(SexKind::Female)) {
            b = a.take();
        }
        [a, b]
    }

    fn birth_sort(&self, p: Key) -> Option<i32> {
        self.index
            .summaries
            .get(p as usize)
            .and_then(|s| s.birth_sort)
    }

    fn unions(&self, p: Key) -> Vec<(Option<Key>, Vec<Key>)> {
        let Some(s) = self.index.summaries.get(p as usize) else {
            return Vec::new();
        };
        let Some(fids) = self.links.partner_in.get(&s.id) else {
            return Vec::new();
        };
        fids.iter()
            .filter_map(|f| self.tree.families.get(f))
            .map(|fam| {
                let partner = if fam.partner1 == Some(s.id) {
                    fam.partner2
                } else {
                    fam.partner1
                };
                let mut kids: Vec<Key> = fam
                    .children
                    .iter()
                    .filter_map(|c| self.index.slot(c.person))
                    .collect();
                // Birth order; undated children keep their listed order after the dated ones.
                kids.sort_by_key(|k| {
                    self.index
                        .summaries
                        .get(*k as usize)
                        .and_then(|s| s.birth_sort)
                        .unwrap_or(i32::MAX)
                });
                (partner.and_then(|id| self.index.slot(id)), kids)
            })
            .collect()
    }
}

/// Outline row ids from here up are group headings ("Siblings"), not chart nodes.
pub const GROUP_BASE: u32 = 1 << 31;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RelGroup {
    Parents,
    Siblings,
    Partners,
    Children,
    AuntsUncles,
}

impl RelGroup {
    const ALL: [RelGroup; 5] = [
        RelGroup::Parents,
        RelGroup::Siblings,
        RelGroup::Partners,
        RelGroup::Children,
        RelGroup::AuntsUncles,
    ];

    fn index(self) -> u32 {
        Self::ALL.iter().position(|g| *g == self).unwrap_or(0) as u32
    }

    fn from_index(i: u32) -> Option<Self> {
        Self::ALL.get(i as usize).copied()
    }

    fn kind(self) -> OutlineGroupKind {
        match self {
            RelGroup::Parents => OutlineGroupKind::Parents,
            RelGroup::Siblings => OutlineGroupKind::Siblings,
            RelGroup::Partners => OutlineGroupKind::Partners,
            RelGroup::Children => OutlineGroupKind::Children,
            RelGroup::AuntsUncles => OutlineGroupKind::AuntsUncles,
        }
    }

    fn title(self) -> &'static str {
        match self {
            RelGroup::Parents => "Parents",
            RelGroup::Siblings => "Siblings",
            RelGroup::Partners => "Partners",
            RelGroup::Children => "Children",
            RelGroup::AuntsUncles => "Aunts and uncles",
        }
    }
}

/// The nodes in `group` of `id` on a relatives chart, in display order.
fn relatives_group(c: &OpenChart, id: u32, group: RelGroup) -> Vec<u32> {
    let on_row = |pred: &dyn Fn(Rel) -> bool| -> Vec<u32> {
        if id != c.layout.root {
            return Vec::new();
        }
        let mut v: Vec<&gt_layout::Node> = c
            .layout
            .nodes
            .iter()
            .filter(|n| n.rel.is_some_and(pred) && n.dup_of.is_none())
            .collect();
        v.sort_by(|a, b| a.rect.x.total_cmp(&b.rect.x));
        v.into_iter().map(|n| n.id).collect()
    };
    match group {
        RelGroup::Parents => c.parents_of.get(&id).cloned().unwrap_or_default(),
        RelGroup::Partners => c.partners.get(&id).cloned().unwrap_or_default(),
        RelGroup::Children => c.kids_of.get(&id).cloned().unwrap_or_default(),
        RelGroup::Siblings => on_row(&|r| matches!(r, Rel::Sibling { .. })),
        RelGroup::AuntsUncles => on_row(&|r| matches!(r, Rel::AuntUncle { .. })),
    }
}

/// Where each node is first met when the outline is read top to bottom: `(owner node, group index)`.
/// A node met again elsewhere is only a reference there, so the outline is a tree and cannot loop.
fn first_places(c: &OpenChart) -> HashMap<u32, (u32, u32)> {
    fn walk(c: &OpenChart, id: u32, seen: &mut HashMap<u32, (u32, u32)>) {
        // Claim everyone listed under this person first, so a mother is listed under "Parents" of the
        // person she is the parent of, not under her partner; then go into the newly claimed people.
        let mut fresh = Vec::new();
        for g in RelGroup::ALL {
            for m in relatives_group(c, id, g) {
                if let std::collections::hash_map::Entry::Vacant(v) = seen.entry(m) {
                    v.insert((id, g.index()));
                    fresh.push(m);
                }
            }
        }
        for m in fresh {
            walk(c, m, seen);
        }
    }
    let mut seen = HashMap::new();
    seen.insert(c.layout.root, (u32::MAX, 0));
    walk(c, c.layout.root, &mut seen);
    seen
}

struct OpenChart {
    id: u32,
    kind: ChartKind,
    version: u64,
    /// Laid out with the ancestors at the bottom (the tree-direction setting at open time).
    reversed: bool,
    layout: Layout,
    /// Partner boxes of each descendants node, in order.
    partners: HashMap<u32, Vec<u32>>,
    /// Partner node -> the person it sits beside.
    partner_owner: HashMap<u32, u32>,
    /// Relatives chart: parent edges as node -> parents and node -> children, in edge order.
    parents_of: HashMap<u32, Vec<u32>>,
    kids_of: HashMap<u32, Vec<u32>>,
    last_used: Cell<u64>,
    /// How people are related to the reference person: the root, or whoever `chart_set_reference` named.
    kin: RefCell<Option<KinMemo>>,
    /// Whole-tree charts only.
    whole: Option<Whole>,
}

struct Whole {
    extras: WholeExtras,
    blocks: Vec<WholeBlockDto>,
}

/// Open charts and the tree revision they are checked against.
#[derive(Default)]
pub struct Charts {
    revision: u64,
    next_id: u32,
    tick: Cell<u64>,
    open: Vec<OpenChart>,
}

impl Charts {
    /// Closes the least recently used chart while two (not counting the whole tree) are open.
    fn make_room(&mut self) {
        while self.open.iter().filter(|c| c.whole.is_none()).count() >= MAX_CHARTS {
            let oldest = self
                .open
                .iter()
                .enumerate()
                .filter(|(_, c)| c.whole.is_none())
                .min_by_key(|(_, c)| c.last_used.get())
                .map_or(0, |(i, _)| i);
            self.open.remove(oldest);
        }
    }

    /// Called on every rebuild of the session data: open charts become stale.
    pub(crate) fn bump_revision(&mut self) {
        self.revision += 1;
    }

    fn touch(&self, c: &OpenChart) {
        self.tick.set(self.tick.get() + 1);
        c.last_used.set(self.tick.get());
    }

    /// The chart, marked as just used; `Stale` when the tree changed since it was opened.
    fn live(&self, id: u32) -> Result<&OpenChart, ApiError> {
        let c = self
            .open
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| ApiError::not_found("chart", id))?;
        if c.version != self.revision {
            return Err(ApiError::new(ErrorCode::Stale, STALE));
        }
        self.touch(c);
        Ok(c)
    }
}

type PartnerMaps = (HashMap<u32, Vec<u32>>, HashMap<u32, u32>);

fn partner_maps(layout: &Layout) -> PartnerMaps {
    let mut owner: HashMap<u32, u32> = HashMap::new();
    let mut out: HashMap<u32, Vec<u32>> = HashMap::new();
    // Partner edges chain person -> partner 1 -> partner 2 in node order.
    for e in layout.edges.iter().filter(|e| e.kind == EdgeKind::Partner) {
        let o = owner.get(&e.from).copied().unwrap_or(e.from);
        owner.insert(e.to, o);
        out.entry(o).or_default().push(e.to);
    }
    (out, owner)
}

/// "Father", "Mother's father", ... from the Ahnentafel number (bits after the leading 1,
/// the root's parent first). Beyond three steps: "2x great-grandmother".
pub fn ahnen_label(n: u32) -> String {
    if n <= 1 {
        return "Self".to_string();
    }
    let steps = 31 - n.leading_zeros();
    let female = |i: u32| (n >> (steps - 1 - i)) & 1 == 1;
    if steps <= 3 {
        let mut s = String::new();
        for i in 0..steps {
            let w = if female(i) { "mother" } else { "father" };
            if i == 0 {
                s = capitalise(w);
            } else {
                s = format!("{s}'s {w}");
            }
        }
        s
    } else {
        let t = if female(steps - 1) {
            "grandmother"
        } else {
            "grandfather"
        };
        format!("{}x great-{t}", steps - 2)
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

fn not_whole() -> ApiError {
    ApiError::new(
        ErrorCode::Invalid,
        "This chart is not the whole tree. Open the whole tree first.",
    )
}

fn block_title(kind: BlockKind, surname: &str, people: u32) -> String {
    match kind {
        BlockKind::Unconnected => format!("Unconnected people ({people})"),
        BlockKind::Family if surname.is_empty() => format!("Family ({people} people)"),
        BlockKind::Family => format!("{surname} family ({people} people)"),
    }
}

fn whole_meta(c: &OpenChart) -> WholeMeta {
    let b = c.layout.bounds;
    let blocks_right = c.whole.as_ref().map_or(0.0, |w| right_of(&w.blocks));
    WholeMeta {
        meta: ChartMeta {
            chart_id: c.id,
            kind: ChartKind::Whole,
            bounds: ChartSize {
                w: (b.x + b.w).max(blocks_right),
                h: b.y + b.h,
            },
            node_count: u32::try_from(c.layout.nodes.len()).unwrap_or(u32::MAX),
            root_node: c.layout.root,
            truncated: false,
            version: c.version,
        },
        blocks: c.whole.as_ref().map_or_else(Vec::new, |w| w.blocks.clone()),
    }
}

fn right_of(blocks: &[WholeBlockDto]) -> f32 {
    blocks
        .iter()
        .map(|b| b.rect.x + b.rect.w)
        .fold(0.0, f32::max)
}

fn descendant_label(depth: i16, k: usize) -> String {
    match depth {
        ..=1 => format!("Child {k}"),
        2 => format!("Grandchild {k}"),
        3 => format!("Great-grandchild {k}"),
        d => format!("{}x great-grandchild {k}", d - 2),
    }
}

impl Session {
    /// The tree revision charts are checked against; every rebuild bumps it.
    pub fn revision(&self) -> u64 {
        self.charts.revision
    }

    /// Lays out a chart. Generations are clamped (pedigree 1..=12, descendants 1..=8) and the
    /// layout stops at 20 000 nodes. Opening a third chart closes the least recently used one.
    pub fn chart_open(&mut self, req: &ChartRequest) -> Result<ChartMeta, ApiError> {
        let root = self
            .index
            .slot(req.root)
            .ok_or_else(|| ApiError::not_found("person", req.root))?;
        let src = TreeSource {
            tree: &self.tree,
            links: &self.links,
            index: &self.index,
        };
        let m = Metrics::default();
        let reversed = self.settings.tree_direction == TreeDirection::Reversed;
        let mut layout = match req.kind {
            ChartKind::Pedigree => {
                let g = u8::try_from(req.generations.clamp(1, 12)).unwrap_or(12);
                gt_layout::pedigree(&src, root, g, &m, MAX_NODES)
            }
            ChartKind::Descendants => {
                let g = u8::try_from(req.generations.clamp(1, 8)).unwrap_or(8);
                gt_layout::descendants(&src, root, g, &m, MAX_NODES)
            }
            ChartKind::Relatives => {
                let o = RelativesOptions {
                    generations: u8::try_from(req.generations.clamp(1, 6)).unwrap_or(6),
                    aunts: req.aunts_uncles.unwrap_or(true),
                };
                gt_layout::relatives(&src, root, &o, &m, MAX_NODES)
            }
            ChartKind::Whole => {
                return Err(ApiError::new(
                    ErrorCode::Invalid,
                    "The whole tree has no starting person. Open it with the whole-tree command.",
                ));
            }
        };
        if reversed {
            // Ancestors at the bottom; the left-to-right pedigree grows leftward instead.
            layout.mirror(if req.kind == ChartKind::Pedigree {
                Axis::Horizontal
            } else {
                Axis::Vertical
            });
        }
        let charts = &mut self.charts;
        charts.make_room();
        charts.next_id += 1;
        let meta = ChartMeta {
            chart_id: charts.next_id,
            kind: req.kind,
            bounds: ChartSize {
                w: layout.bounds.x + layout.bounds.w,
                h: layout.bounds.y + layout.bounds.h,
            },
            node_count: u32::try_from(layout.nodes.len()).unwrap_or(u32::MAX),
            root_node: layout.root,
            truncated: layout.truncated,
            version: charts.revision,
        };
        let (partners, partner_owner) = partner_maps(&layout);
        let mut parents_of: HashMap<u32, Vec<u32>> = HashMap::new();
        let mut kids_of: HashMap<u32, Vec<u32>> = HashMap::new();
        if req.kind == ChartKind::Relatives {
            for e in layout.edges.iter().filter(|e| e.kind == EdgeKind::Parent) {
                parents_of.entry(e.to).or_default().push(e.from);
                kids_of.entry(e.from).or_default().push(e.to);
            }
        }
        let chart = OpenChart {
            id: meta.chart_id,
            kind: req.kind,
            version: charts.revision,
            reversed,
            layout,
            partners,
            partner_owner,
            parents_of,
            kids_of,
            last_used: Cell::new(0),
            kin: RefCell::new(Some(KinMemo::new(root))),
            whole: None,
        };
        charts.touch(&chart);
        charts.open.push(chart);
        Ok(meta)
    }

    /// Lays out every person of the project in one chart. The layout is kept for the current tree
    /// revision: asking again returns the same chart, so panning and reopening the view never
    /// lay it out twice. An edit makes it stale and the next call lays it out again.
    pub fn chart_open_whole(&mut self) -> Result<WholeMeta, ApiError> {
        let revision = self.charts.revision;
        let reversed = self.settings.tree_direction == TreeDirection::Reversed;
        if let Some(c) =
            self.charts.open.iter().find(|c| {
                c.kind == ChartKind::Whole && c.version == revision && c.reversed == reversed
            })
        {
            self.charts.touch(c);
            return Ok(whole_meta(c));
        }
        self.charts.open.retain(|c| c.kind != ChartKind::Whole);
        let summaries = &self.index.summaries;
        let input = WholeInput {
            count: summaries.len(),
            families: self
                .tree
                .families
                .values()
                .map(|f| WholeFamily {
                    partners: [f.partner1, f.partner2]
                        .map(|p| p.and_then(|id| self.index.slot(id))),
                    children: f
                        .children
                        .iter()
                        .filter_map(|c| self.index.slot(c.person))
                        .collect(),
                })
                .collect(),
            birth: summaries.iter().map(|s| s.birth_sort).collect(),
            surname: summaries.iter().map(|s| s.surname.clone()).collect(),
        };
        let gt_layout::WholeLayout { layout, extras } =
            gt_layout::whole_directed(&input, &Metrics::default(), reversed);
        let blocks = extras
            .blocks
            .iter()
            .map(|b| WholeBlockDto {
                title: block_title(b.kind, &b.surname, b.people),
                people: b.people,
                unconnected: b.kind == BlockKind::Unconnected,
                rect: Rect {
                    x: b.rect.x,
                    y: b.rect.y,
                    w: b.rect.w,
                    h: b.rect.h,
                },
                body_h: b.body_h,
            })
            .collect();
        let charts = &mut self.charts;
        charts.make_room();
        charts.next_id += 1;
        let chart = OpenChart {
            id: charts.next_id,
            kind: ChartKind::Whole,
            version: revision,
            reversed,
            layout,
            partners: HashMap::new(),
            partner_owner: HashMap::new(),
            parents_of: HashMap::new(),
            kids_of: HashMap::new(),
            last_used: Cell::new(0),
            kin: RefCell::new(None),
            whole: Some(Whole { extras, blocks }),
        };
        charts.touch(&chart);
        let meta = whole_meta(&chart);
        charts.open.push(chart);
        Ok(meta)
    }

    /// The node, its ancestors and its descendants in a whole-tree chart (the highlighted line).
    pub fn chart_lineage(&self, chart: u32, node: u32) -> Result<Vec<u32>, ApiError> {
        let c = self.charts.live(chart)?;
        let w = c.whole.as_ref().ok_or_else(not_whole)?;
        if c.layout.nodes.get(node as usize).is_none() {
            return Err(ApiError::not_found("chart node", node));
        }
        Ok(w.extras.lineage(node))
    }

    /// Where a person sits in a whole-tree chart; `None` when the person is not in the project.
    pub fn chart_locate(
        &self,
        chart: u32,
        person: Uuid,
    ) -> Result<Option<ChartLocation>, ApiError> {
        let c = self.charts.live(chart)?;
        let w = c.whole.as_ref().ok_or_else(not_whole)?;
        let located = self
            .index
            .slot(person)
            .and_then(|k| w.extras.node_of(k))
            .and_then(|id| c.layout.nodes.get(id as usize))
            .map(|n| ChartLocation {
                node_id: n.id,
                rect: Rect {
                    x: n.rect.x,
                    y: n.rect.y,
                    w: n.rect.w,
                    h: n.rect.h,
                },
            });
        Ok(located)
    }

    /// Chooses the person every box's `kinship` is worded against (default: the chart's starting person;
    /// the whole-tree chart has none until this is called). `None` removes the kinship labels.
    pub fn chart_set_reference(&self, chart: u32, person: Option<Uuid>) -> Result<(), ApiError> {
        let c = self.charts.live(chart)?;
        let slot = match person {
            Some(id) => Some(
                self.index
                    .slot(id)
                    .ok_or_else(|| ApiError::not_found("person", id))?,
            ),
            None => None,
        };
        let mut memo = c.kin.borrow_mut();
        if memo.as_ref().map(KinMemo::from) != slot {
            *memo = slot.map(KinMemo::new);
        }
        Ok(())
    }

    /// How `to` is related to `from`, or `None` when no relation is found.
    pub fn kinship_between(&self, from: Uuid, to: Uuid) -> Option<Kinship> {
        let (f, t) = (self.index.slot(from)?, self.index.slot(to)?);
        let kin = Kin {
            tree: &self.tree,
            links: &self.links,
            index: &self.index,
        };
        kin.relationship(&mut KinMemo::new(f), t)
    }

    /// Nodes and edges intersecting `rect`. Below zoom 0.25 nodes carry no person and no label.
    /// `Stale` once the tree has been rebuilt since the chart was opened.
    pub fn chart_window(
        &self,
        chart: u32,
        rect: &Rect,
        zoom: f32,
    ) -> Result<ChartWindow, ApiError> {
        let c = self.charts.live(chart)?;
        let text = zoom >= TEXT_ZOOM;
        let r = gt_layout::Rect {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: rect.h,
        };
        let (nodes, edges) = c.layout.query(&r);
        let whole = c.kind == ChartKind::Whole;
        let kin = Kin {
            tree: &self.tree,
            links: &self.links,
            index: &self.index,
        };
        let mut memo = c.kin.borrow_mut();
        let nodes = nodes
            .into_iter()
            .map(|n| {
                let s = if text || whole {
                    self.index.summaries.get(n.key as usize)
                } else {
                    None
                };
                let relation = match (c.kind, s, n.rel) {
                    (ChartKind::Relatives, Some(s), Some(r)) if text => {
                        Some(relation_label(r, s.sex))
                    }
                    _ => None,
                };
                let role_dto = s.filter(|_| text).map(|s| {
                    let sex = s.sex;
                    if whole {
                        RoleDto::new(RoleKind::Generation, i32::from(n.r#gen) + 1, sex)
                    } else if let (ChartKind::Relatives, Some(r)) = (c.kind, n.rel) {
                        relation_role(r, sex)
                    } else if c.partner_owner.contains_key(&n.id) {
                        RoleDto::new(RoleKind::Partner, 0, sex)
                    } else if n.r#gen == 0 && n.ahnen.is_none_or(|a| a == 1) {
                        RoleDto::new(RoleKind::Focus, 0, sex)
                    } else if let (ChartKind::Pedigree, Some(a)) = (c.kind, n.ahnen) {
                        RoleDto::new(RoleKind::AncestorNo, a as i32, sex)
                    } else {
                        RoleDto::new(RoleKind::Generation, i32::from(n.r#gen), sex)
                    }
                });
                let label = s.filter(|_| text).map_or_else(String::new, |s| {
                    let mut l = s.name.clone();
                    if !s.lifespan.is_empty() {
                        l = format!("{l}, {}", s.lifespan);
                    }
                    let role = if whole {
                        format!("generation {}", n.r#gen + 1)
                    } else if let Some(r) = &relation {
                        r.clone()
                    } else if c.partner_owner.contains_key(&n.id) {
                        "partner".to_string()
                    } else if n.r#gen == 0 && n.ahnen.is_none_or(|a| a == 1) {
                        "starting person".to_string()
                    } else if let (ChartKind::Pedigree, Some(a)) = (c.kind, n.ahnen) {
                        format!("ancestor {a}")
                    } else {
                        format!("generation {}", n.r#gen)
                    };
                    let tail = if n.dup_of.is_some() {
                        ", appears earlier"
                    } else {
                        ""
                    };
                    format!("{l}, {role}{tail}")
                });
                let kinship = match (&mut *memo, s) {
                    (Some(m), Some(_)) if text => kin
                        .relationship(m, n.key)
                        .filter(|k| k.kind != KinshipKind::SelfPerson),
                    _ => None,
                };
                ChartNode {
                    id: n.id,
                    person: s.map(|s| {
                        if text {
                            s.person_ref()
                        } else {
                            // Far out the canvas draws the surname on a coloured bar and nothing else.
                            PersonRef {
                                name: s.surname.clone(),
                                lifespan: String::new(),
                                ..s.person_ref()
                            }
                        }
                    }),
                    x: n.rect.x,
                    y: n.rect.y,
                    w: n.rect.w,
                    h: n.rect.h,
                    generation: n.r#gen,
                    ahnen: n.ahnen.and_then(|a| u32::try_from(a).ok()),
                    dup_of: n.dup_of,
                    more_up: n.more_up,
                    more_down: n.more_down,
                    nav: ChartNav {
                        parent: n.nav.parent,
                        child: n.nav.child,
                        prev: n.nav.prev,
                        next: n.nav.next,
                    },
                    label,
                    relation,
                    role: role_dto,
                    kinship,
                }
            })
            .collect();
        let edges = edges
            .into_iter()
            .map(|e| ChartEdge {
                from: e.from,
                to: e.to,
                kind: match e.kind {
                    EdgeKind::Parent => EdgeKindDto::Parent,
                    EdgeKind::Partner => EdgeKindDto::Partner,
                    EdgeKind::Dup => EdgeKindDto::Dup,
                },
                pts: e.pts.clone(),
            })
            .collect();
        Ok(ChartWindow { nodes, edges })
    }

    /// Screen-reader outline, one level at a time. `parent` None gives the root; otherwise the
    /// children of that node (pedigree: its parents; descendants: partners, then children).
    /// `limit` is clamped to 100.
    pub fn chart_outline(
        &self,
        chart: u32,
        parent: Option<u32>,
        limit: usize,
    ) -> Result<Vec<OutlineItem>, ApiError> {
        let c = self.charts.live(chart)?;
        if c.whole.is_some() {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "The whole tree has no outline. Use Find a person to move around it.",
            ));
        }
        let limit = limit.min(MAX_OUTLINE);
        if c.kind == ChartKind::Relatives {
            return self.relatives_outline(c, parent, limit);
        }
        let nodes = &c.layout.nodes;
        let node = |id: u32| {
            nodes
                .get(id as usize)
                .ok_or_else(|| ApiError::not_found("chart node", id))
        };
        let pedigree = c.kind == ChartKind::Pedigree;
        let sex_of = |key: u32| {
            self.index
                .summaries
                .get(key as usize)
                .map_or(SexKind::Unknown, |s| s.sex)
        };
        let mut items: Vec<(u32, String, RoleDto)> = Vec::new();
        match parent {
            None => {
                let label = if pedigree { "1 Self" } else { "Self" };
                let root = node(c.layout.root)?;
                let role = if pedigree {
                    RoleDto::new(RoleKind::AncestorNo, 1, sex_of(root.key))
                } else {
                    RoleDto::new(RoleKind::SelfPerson, 0, sex_of(root.key))
                };
                items.push((root.id, label.to_string(), role));
            }
            Some(p) => {
                let pn = node(p)?;
                for &pid in c.partners.get(&p).map_or(&[][..], Vec::as_slice) {
                    let sex = node(pid).map_or(SexKind::Unknown, |x| sex_of(x.key));
                    items.push((
                        pid,
                        "Partner".to_string(),
                        RoleDto::new(RoleKind::Partner, 0, sex),
                    ));
                }
                let mut next = pn.nav.child;
                let mut k = 0usize;
                while let Some(id) = next {
                    if items.len() >= limit {
                        break;
                    }
                    k += 1;
                    let n = node(id)?;
                    let sex = sex_of(n.key);
                    let (label, mut role) =
                        match (pedigree, n.ahnen.and_then(|a| u32::try_from(a).ok())) {
                            (true, Some(a)) => (
                                format!("{a} {}", ahnen_label(a)),
                                RoleDto::new(RoleKind::AncestorNo, a as i32, sex),
                            ),
                            _ => (
                                descendant_label(n.r#gen, k),
                                RoleDto::new(RoleKind::DescendantNo, i32::from(n.r#gen), sex),
                            ),
                        };
                    role.k = k as u32;
                    items.push((id, label, role));
                    next = n.nav.next;
                }
            }
        }
        items.truncate(limit);
        let kin = Kin {
            tree: &self.tree,
            links: &self.links,
            index: &self.index,
        };
        let mut memo = c.kin.borrow_mut();
        items
            .into_iter()
            .map(|(id, mut label, role)| {
                let n = node(id)?;
                if n.dup_of.is_some() {
                    label.push_str(" (appears earlier)");
                }
                let kinship = memo
                    .as_mut()
                    .and_then(|m| kin.relationship(m, n.key))
                    .filter(|k| k.kind != KinshipKind::SelfPerson);
                Ok(OutlineItem {
                    node_id: id,
                    person: self
                        .index
                        .summaries
                        .get(n.key as usize)
                        .map(|s| s.person_ref()),
                    label,
                    has_children: n.nav.child.is_some()
                        || n.more_up
                        || n.more_down
                        || c.partners.contains_key(&id),
                    relation: None,
                    role: Some(role),
                    kinship,
                    group: None,
                    appears_earlier: n.dup_of.is_some(),
                    also_above: false,
                })
            })
            .collect()
    }

    /// Outline of the relatives chart: the centred person, then under each person the groups
    /// Parents, Siblings, Partners, Children and Aunts and uncles, then the people in a group.
    /// Group rows get ids from `GROUP_BASE` up, packing the owner node and the group number.
    fn relatives_outline(
        &self,
        c: &OpenChart,
        parent: Option<u32>,
        limit: usize,
    ) -> Result<Vec<OutlineItem>, ApiError> {
        let nodes = &c.layout.nodes;
        let node = |id: u32| {
            nodes
                .get(id as usize)
                .ok_or_else(|| ApiError::not_found("chart node", id))
        };
        let person_of = |id: u32| {
            node(id).map(|n| {
                self.index
                    .summaries
                    .get(n.key as usize)
                    .map(|s| s.person_ref())
            })
        };
        let places = first_places(c);
        // `place` is the (owner, group) the row is listed under; `None` for the centred person.
        let item = |id: u32, place: Option<(u32, u32)>| -> Result<OutlineItem, ApiError> {
            let n = node(id)?;
            let first = place.is_none() || places.get(&id).copied() == place;
            let sex = self
                .index
                .summaries
                .get(n.key as usize)
                .map_or(SexKind::Unknown, |s| s.sex);
            let mut label = n
                .rel
                .map_or_else(String::new, |r| capitalise(&relation_label(r, sex)));
            if n.dup_of.is_some() {
                label.push_str(" (appears earlier)");
            }
            if !first {
                label.push_str(" (also shown above)");
            }
            Ok(OutlineItem {
                node_id: id,
                person: person_of(id)?,
                label,
                // Only a person with someone not listed elsewhere can be opened.
                has_children: first
                    && RelGroup::ALL.iter().any(|g| {
                        relatives_group(c, id, *g)
                            .iter()
                            .any(|m| places.get(m) == Some(&(id, g.index())))
                    }),
                relation: n
                    .rel
                    .filter(|r| *r != Rel::Focus)
                    .map(|r| relation_label(r, sex)),
                role: n.rel.map(|r| relation_role(r, sex)),
                kinship: None,
                group: None,
                appears_earlier: n.dup_of.is_some(),
                also_above: !first,
            })
        };
        let Some(p) = parent else {
            return Ok(vec![item(c.layout.root, None)?]);
        };
        if p < GROUP_BASE {
            node(p)?;
            return Ok(RelGroup::ALL
                .iter()
                .filter(|g| !relatives_group(c, p, **g).is_empty())
                .map(|g| OutlineItem {
                    node_id: GROUP_BASE | (p << 3) | g.index(),
                    person: None,
                    label: g.title().to_string(),
                    has_children: true,
                    relation: None,
                    role: None,
                    kinship: None,
                    group: Some(g.kind()),
                    appears_earlier: false,
                    also_above: false,
                })
                .collect());
        }
        let owner = (p & !GROUP_BASE) >> 3;
        let g = RelGroup::from_index(p & 7).ok_or_else(|| ApiError::not_found("chart node", p))?;
        node(owner)?;
        relatives_group(c, owner, g)
            .into_iter()
            .take(limit)
            .map(|m| item(m, Some((owner, g.index()))))
            .collect()
    }

    pub fn chart_close(&mut self, chart: u32) {
        self.charts.open.retain(|c| c.id != chart);
    }
}
