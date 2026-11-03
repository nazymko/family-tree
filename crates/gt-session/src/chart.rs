//! Chart windows and the outline: pedigree / descendants layouts from `gt-layout` held per open
//! chart, served as viewport windows. At most two charts stay open (least recently used goes).

use crate::dto::{
    ChartEdge, ChartKind, ChartMeta, ChartNav, ChartNode, ChartRequest, ChartSize, ChartWindow,
    EdgeKindDto, OutlineItem, Rect, SexKind,
};
use crate::error::{ApiError, ErrorCode};
use crate::index::{Links, PersonIndex};
use crate::relation::relation_label;
use crate::session::Session;
use gt_core::tree::Tree;
use gt_layout::{ChartSource, EdgeKind, Key, Layout, Metrics, Rel, RelativesOptions};
use std::cell::Cell;
use std::collections::HashMap;

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

struct OpenChart {
    id: u32,
    kind: ChartKind,
    version: u64,
    layout: Layout,
    /// Partner boxes of each descendants node, in order.
    partners: HashMap<u32, Vec<u32>>,
    /// Partner node -> the person it sits beside.
    partner_owner: HashMap<u32, u32>,
    /// Relatives chart: parent edges as node -> parents and node -> children, in edge order.
    parents_of: HashMap<u32, Vec<u32>>,
    kids_of: HashMap<u32, Vec<u32>>,
    last_used: Cell<u64>,
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
            ChartKind::Relatives => {
                let o = RelativesOptions {
                    generations: u8::try_from(req.generations.clamp(1, 6)).unwrap_or(6),
                    aunts: req.aunts_uncles.unwrap_or(true),
                };
                gt_layout::relatives(&src, root, &o, &m, MAX_NODES)
            }
        };
        let charts = &mut self.charts;
        while charts.open.len() >= MAX_CHARTS {
            let oldest = charts
                .open
                .iter()
                .enumerate()
                .min_by_key(|(_, c)| c.last_used.get())
                .map_or(0, |(i, _)| i);
            charts.open.remove(oldest);
        }
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
            layout,
            partners,
            partner_owner,
            parents_of,
            kids_of,
            last_used: Cell::new(0),
        };
        charts.touch(&chart);
        charts.open.push(chart);
        Ok(meta)
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
        let nodes = nodes
            .into_iter()
            .map(|n| {
                let s = if text {
                    self.index.summaries.get(n.key as usize)
                } else {
                    None
                };
                let relation = match (c.kind, s, n.rel) {
                    (ChartKind::Relatives, Some(s), Some(r)) => Some(relation_label(r, s.sex)),
                    _ => None,
                };
                let label = s.map_or_else(String::new, |s| {
                    let mut l = s.name.clone();
                    if !s.lifespan.is_empty() {
                        l = format!("{l}, {}", s.lifespan);
                    }
                    let role = if let Some(r) = &relation {
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
                ChartNode {
                    id: n.id,
                    person: s.map(|s| s.person_ref()),
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
        let item = |id: u32| -> Result<OutlineItem, ApiError> {
            let n = node(id)?;
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
            Ok(OutlineItem {
                node_id: id,
                person: person_of(id)?,
                label,
                has_children: RelGroup::ALL
                    .iter()
                    .any(|g| !relatives_group(c, id, *g).is_empty()),
            })
        };
        let Some(p) = parent else {
            return Ok(vec![item(c.layout.root)?]);
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
                })
                .collect());
        }
        let owner = (p & !GROUP_BASE) >> 3;
        let g = RelGroup::from_index(p & 7).ok_or_else(|| ApiError::not_found("chart node", p))?;
        node(owner)?;
        relatives_group(c, owner, g)
            .into_iter()
            .take(limit)
            .map(item)
            .collect()
    }

    pub fn chart_close(&mut self, chart: u32) {
        self.charts.open.retain(|c| c.id != chart);
    }
}
