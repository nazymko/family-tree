//! Pedigree chart: root at the left, one column per generation, Ahnentafel numbered.

use std::collections::{HashMap, VecDeque};

use crate::types::{
    ChartSource, Edge, EdgeKind, Key, Layout, Metrics, Node, Rect, dup_edge, link_children,
    new_node,
};

/// Lay out the ancestors of `root` over `generations` columns (the root is column 0).
///
/// Breadth-first, so a person met twice (pedigree collapse) keeps their shallowest
/// occurrence and the later one becomes a dup node.
pub fn pedigree(
    src: &dyn ChartSource,
    root: Key,
    generations: u8,
    m: &Metrics,
    max_nodes: usize,
) -> Layout {
    let generations = usize::from(generations.max(1));
    let max_nodes = max_nodes.max(1);
    let rect = Rect {
        x: 0.0,
        y: 0.0,
        w: m.node_w,
        h: m.node_h,
    };
    let mut first = new_node(0, root, rect, 0);
    first.ahnen = Some(1);
    let mut nodes: Vec<Node> = vec![first];
    let mut kids: Vec<Vec<u32>> = vec![Vec::new()];
    let mut visited: HashMap<Key, u32> = HashMap::from([(root, 0)]);
    let mut truncated = false;
    let mut queue: VecDeque<u32> = VecDeque::from([0]);

    while let Some(id) = queue.pop_front() {
        let (key, r#gen, ahnen) = {
            let n = &nodes[id as usize];
            (n.key, n.r#gen, n.ahnen)
        };
        let ps = src.parents(key);
        if ps.iter().all(Option::is_none) {
            continue;
        }
        if r#gen as usize + 1 >= generations {
            nodes[id as usize].more_up = true;
            continue;
        }
        for (slot, p) in ps.iter().enumerate() {
            let Some(pk) = *p else { continue };
            if nodes.len() >= max_nodes {
                truncated = true;
                nodes[id as usize].more_up = true;
                continue;
            }
            let new_id = nodes.len() as u32;
            let mut n = new_node(new_id, pk, rect, r#gen + 1);
            n.ahnen = ahnen
                .and_then(|a| a.checked_mul(2))
                .map(|a| a + slot as u64);
            match visited.get(&pk) {
                Some(&first) => n.dup_of = Some(first),
                None => {
                    visited.insert(pk, new_id);
                    queue.push_back(new_id);
                }
            }
            nodes.push(n);
            kids.push(Vec::new());
            kids[id as usize].push(new_id);
        }
    }

    // Vertical placement: leaves take consecutive rows in left-to-right order, every
    // other person sits midway between their first and last parent.
    let step_y = m.node_h + m.gap_y;
    let step_x = m.node_w + m.gap_x;
    let mut y = vec![0.0f32; nodes.len()];
    let mut row = 0u32;
    let mut stack = vec![0u32];
    while let Some(v) = stack.pop() {
        let k = &kids[v as usize];
        if k.is_empty() {
            y[v as usize] = row as f32 * step_y;
            row += 1;
        } else {
            stack.extend(k.iter().rev());
        }
    }
    for v in (0..nodes.len()).rev() {
        if let (Some(&a), Some(&b)) = (kids[v].first(), kids[v].last()) {
            y[v] = (y[a as usize] + y[b as usize]) / 2.0;
        }
    }
    for (n, y) in nodes.iter_mut().zip(&y) {
        n.rect.x = f32::from(n.r#gen) * step_x;
        n.rect.y = *y;
    }

    let mut edges = Vec::new();
    for v in 0..nodes.len() {
        link_children(&mut nodes, v as u32, &kids[v]);
        let a = nodes[v].rect;
        for &k in &kids[v] {
            let b = nodes[k as usize].rect;
            let mx = a.right() + m.gap_x / 2.0;
            edges.push(Edge {
                from: v as u32,
                to: k,
                kind: EdgeKind::Parent,
                pts: vec![a.right(), a.cy(), mx, a.cy(), mx, b.cy(), b.x, b.cy()],
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
