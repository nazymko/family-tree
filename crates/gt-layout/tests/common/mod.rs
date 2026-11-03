#![allow(dead_code)]

use std::collections::HashMap;

use gt_layout::{ChartSource, Key};

pub type Union = (Option<Key>, Vec<Key>);

#[derive(Default)]
pub struct MapSource {
    pub parents: HashMap<Key, [Option<Key>; 2]>,
    pub unions: HashMap<Key, Vec<Union>>,
}

impl ChartSource for MapSource {
    fn parents(&self, p: Key) -> [Option<Key>; 2] {
        self.parents.get(&p).copied().unwrap_or([None, None])
    }
    fn unions(&self, p: Key) -> Vec<Union> {
        self.unions.get(&p).cloned().unwrap_or_default()
    }
}

impl MapSource {
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
