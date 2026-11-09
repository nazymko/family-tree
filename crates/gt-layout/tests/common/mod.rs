#![allow(dead_code)]

use std::collections::HashMap;

use gt_layout::{ChartSource, Key, Layout, quality};

pub type Union = (Option<Key>, Vec<Key>);
pub type ChildFamily = ([Option<Key>; 2], Vec<Key>);

#[derive(Default)]
pub struct MapSource {
    pub parents: HashMap<Key, [Option<Key>; 2]>,
    pub unions: HashMap<Key, Vec<Union>>,
    pub births: HashMap<Key, i32>,
    pub child_fams: HashMap<Key, Vec<ChildFamily>>,
}

impl ChartSource for MapSource {
    fn parents(&self, p: Key) -> [Option<Key>; 2] {
        self.parents.get(&p).copied().unwrap_or([None, None])
    }
    fn birth_sort(&self, p: Key) -> Option<i32> {
        self.births.get(&p).copied()
    }
    fn unions(&self, p: Key) -> Vec<Union> {
        self.unions.get(&p).cloned().unwrap_or_default()
    }
    fn child_families(&self, p: Key) -> Vec<([Option<Key>; 2], Vec<Key>)> {
        self.child_fams.get(&p).cloned().unwrap_or_default()
    }
}

impl MapSource {
    /// A family: `a` and `b` (either may be unknown) with `kids`; every kid gets both as parents.
    pub fn family(&mut self, a: Option<Key>, b: Option<Key>, kids: &[Key]) {
        for (p, other) in [(a, b), (b, a)] {
            if let Some(p) = p {
                self.unions
                    .entry(p)
                    .or_default()
                    .push((other, kids.to_vec()));
            }
        }
        for &k in kids {
            self.parents.insert(k, [a, b]);
            self.child_fams
                .entry(k)
                .or_default()
                .push(([a, b], kids.to_vec()));
        }
    }

    /// Key = Ahnentafel number; everyone below `last` has both parents.
    pub fn ahnen_tree(last: Key) -> Self {
        let mut s = MapSource::default();
        for n in 1..=last {
            if 2 * n < last {
                s.parents.insert(n, [Some(2 * n), Some(2 * n + 1)]);
            }
        }
        s
    }

    /// Partnerless unions: `children[i]` are the children of person `i`.
    pub fn plain_tree(children: &[(Key, Vec<Key>)]) -> Self {
        let mut s = MapSource::default();
        for (p, c) in children {
            s.unions.insert(*p, vec![(None, c.clone())]);
        }
        s
    }
}

/// No two families' connectors read as one: no lines on top of each other, no buses end to end on one lane, no
/// vertical lines lined up across families; and no card is crossed by a line.
pub fn assert_connectors_readable(l: &Layout) -> Layout {
    let q = quality(l);
    assert_eq!(q.box_overlaps, 0, "{q:?}");
    // The "appears earlier" links of repeated people run around the cards and may touch other lines.
    if l.nodes.iter().all(|n| n.dup_of.is_none()) {
        assert_eq!(q.lines_through_boxes, 0, "{q:?}");
        assert_eq!(q.line_overlaps, 0, "{q:?}");
    }
    assert_eq!(q.buses_outside_rows, 0, "{q:?}");
    assert_eq!(q.bus_clashes, 0, "{q:?}");
    assert_eq!(q.aligned_verticals, 0, "{q:?}");
    l.clone()
}
