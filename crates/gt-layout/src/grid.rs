//! Uniform-grid spatial index: nodes and edge bounding boxes bucketed by 512-unit cells.

use std::collections::HashMap;

use crate::types::{Edge, Node, Rect};

const CELL: f32 = 512.0;

type Cell = (i32, i32);

#[derive(Debug, Clone, Default)]
pub(crate) struct Grid {
    nodes: HashMap<Cell, Vec<u32>>,
    edges: HashMap<Cell, Vec<u32>>,
    node_boxes: Vec<Rect>,
    edge_boxes: Vec<Option<Rect>>,
}

fn cell(v: f32) -> i32 {
    (v / CELL).floor() as i32
}

fn range(r: &Rect) -> (Cell, Cell) {
    ((cell(r.x), cell(r.y)), (cell(r.x + r.w), cell(r.y + r.h)))
}

fn bucket(map: &mut HashMap<Cell, Vec<u32>>, r: &Rect, id: u32) {
    let ((x0, y0), (x1, y1)) = range(r);
    for cx in x0..=x1 {
        for cy in y0..=y1 {
            map.entry((cx, cy)).or_default().push(id);
        }
    }
}

fn edge_box(e: &Edge) -> Option<Rect> {
    let mut it = e.pts.chunks_exact(2);
    let first = it.next()?;
    let (mut x0, mut y0, mut x1, mut y1) = (first[0], first[1], first[0], first[1]);
    for p in it {
        x0 = x0.min(p[0]);
        x1 = x1.max(p[0]);
        y0 = y0.min(p[1]);
        y1 = y1.max(p[1]);
    }
    Some(Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    })
}

fn collect(map: &HashMap<Cell, Vec<u32>>, r: &Rect, out: &mut Vec<u32>) {
    let ((x0, y0), (x1, y1)) = range(r);
    let cells = (i64::from(x1) - i64::from(x0) + 1) * (i64::from(y1) - i64::from(y0) + 1);
    if cells > map.len() as i64 {
        for (&(cx, cy), ids) in map {
            if cx >= x0 && cx <= x1 && cy >= y0 && cy <= y1 {
                out.extend_from_slice(ids);
            }
        }
    } else {
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                if let Some(ids) = map.get(&(cx, cy)) {
                    out.extend_from_slice(ids);
                }
            }
        }
    }
    out.sort_unstable();
    out.dedup();
}

impl Grid {
    pub(crate) fn build(nodes: &[Node], edges: &[Edge]) -> Grid {
        let mut g = Grid::default();
        for n in nodes {
            bucket(&mut g.nodes, &n.rect, n.id);
            g.node_boxes.push(n.rect);
        }
        for (i, e) in edges.iter().enumerate() {
            let b = edge_box(e);
            if let Some(b) = &b {
                bucket(&mut g.edges, b, i as u32);
            }
            g.edge_boxes.push(b);
        }
        g
    }

    /// Ids of nodes and edges intersecting `r`, ascending.
    pub(crate) fn query(&self, r: &Rect) -> (Vec<u32>, Vec<u32>) {
        let mut n = Vec::new();
        collect(&self.nodes, r, &mut n);
        n.retain(|&i| self.node_boxes[i as usize].overlaps(r));
        let mut e = Vec::new();
        collect(&self.edges, r, &mut e);
        e.retain(|&i| self.edge_boxes[i as usize].is_some_and(|b| b.touches(r)));
        (n, e)
    }
}
