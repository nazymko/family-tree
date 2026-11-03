//! Chart windows and the outline: pedigree / descendants layouts from `gt-layout` held per open
//! chart, served as viewport windows. At most two charts stay open (least recently used goes).

use crate::dto::{
    ChartEdge, ChartKind, ChartLocation, ChartMeta, ChartNav, ChartNode, ChartRequest, ChartSize,
    ChartWindow, EdgeKindDto, OutlineItem, PersonRef, Rect, SexKind, WholeBlockDto, WholeMeta,
};
use crate::error::{ApiError, ErrorCode};
use crate::index::{Links, PersonIndex};
use crate::session::Session;
use gt_core::tree::Tree;
use gt_layout::{
    BlockKind, ChartSource, EdgeKind, Key, Layout, Metrics, WholeExtras, WholeFamily, WholeInput,
};
use std::cell::Cell;
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

struct OpenChart {
    id: u32,
    kind: ChartKind,
    version: u64,
    layout: Layout,
    /// Partner boxes of each descendants node, in order.
    partners: HashMap<u32, Vec<u32>>,
    /// Partner node -> the person it sits beside.
    partner_owner: HashMap<u32, u32>,
    last_used: Cell<u64>,
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
        let layout = match req.kind {
            ChartKind::Pedigree => {
                let g = u8::try_from(req.generations.clamp(1, 12)).unwrap_or(12);
                gt_layout::pedigree(&src, root, g, &m, MAX_NODES)
            }
            ChartKind::Descendants => {
                let g = u8::try_from(req.generations.clamp(1, 8)).unwrap_or(8);
                gt_layout::descendants(&src, root, g, &m, MAX_NODES)
            }
            ChartKind::Whole => {
                return Err(ApiError::new(
                    ErrorCode::Invalid,
                    "The whole tree has no starting person. Open it with the whole-tree command.",
                ));
            }
        };
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
        let chart = OpenChart {
            id: meta.chart_id,
            kind: req.kind,
            version: charts.revision,
            layout,
            partners,
            partner_owner,
            last_used: Cell::new(0),
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
        if let Some(c) = self
            .charts
            .open
            .iter()
            .find(|c| c.kind == ChartKind::Whole && c.version == revision)
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
            gt_layout::whole(&input, &Metrics::default());
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
            })
            .collect();
        let charts = &mut self.charts;
        charts.make_room();
        charts.next_id += 1;
        let chart = OpenChart {
            id: charts.next_id,
            kind: ChartKind::Whole,
            version: revision,
            layout,
            partners: HashMap::new(),
            partner_owner: HashMap::new(),
            last_used: Cell::new(0),
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
        let nodes = nodes
            .into_iter()
            .map(|n| {
                let s = if text || whole {
                    self.index.summaries.get(n.key as usize)
                } else {
                    None
                };
                let label = s.filter(|_| text).map_or_else(String::new, |s| {
                    let mut l = s.name.clone();
                    if !s.lifespan.is_empty() {
                        l = format!("{l}, {}", s.lifespan);
                    }
                    let role = if whole {
                        format!("generation {}", n.r#gen + 1)
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
        let nodes = &c.layout.nodes;
        let node = |id: u32| {
            nodes
                .get(id as usize)
                .ok_or_else(|| ApiError::not_found("chart node", id))
        };
        let pedigree = c.kind == ChartKind::Pedigree;
        let mut items: Vec<(u32, String)> = Vec::new();
        match parent {
            None => {
                let label = if pedigree { "1 Self" } else { "Self" };
                items.push((node(c.layout.root)?.id, label.to_string()));
            }
            Some(p) => {
                let pn = node(p)?;
                for &pid in c.partners.get(&p).map_or(&[][..], Vec::as_slice) {
                    items.push((pid, "Partner".to_string()));
                }
                let mut next = pn.nav.child;
                let mut k = 0usize;
                while let Some(id) = next {
                    if items.len() >= limit {
                        break;
                    }
                    k += 1;
                    let n = node(id)?;
                    let label = match (pedigree, n.ahnen.and_then(|a| u32::try_from(a).ok())) {
                        (true, Some(a)) => format!("{a} {}", ahnen_label(a)),
                        _ => descendant_label(n.r#gen, k),
                    };
                    items.push((id, label));
                    next = n.nav.next;
                }
            }
        }
        items.truncate(limit);
        items
            .into_iter()
            .map(|(id, mut label)| {
                let n = node(id)?;
                if n.dup_of.is_some() {
                    label.push_str(" (appears earlier)");
                }
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
                })
            })
            .collect()
    }

    pub fn chart_close(&mut self, chart: u32) {
        self.charts.open.retain(|c| c.id != chart);
    }
}
