//! How one person is related to another, as data (`Kinship`), worked out from the record links.
//!
//! The search is bounded (eight generations up, a capped number of ancestors) and reads each person's
//! ancestors once: a `KinMemo` keeps the reference person's ancestors and the answers already found, so
//! labelling a whole chart window costs one small walk per new person.
//!
//! Step, foster, guardian and sealed parent links do not count as blood or legal parents; a stepparent
//! shows up as the partner of a parent. Adopted links count as parents.

use crate::dto::{Kinship, KinshipKind, SexKind};
use crate::index::{Links, PersonIndex};
use gt_core::model::{ParentRel, UnionKind};
use gt_core::tree::Tree;
use std::collections::HashMap;
use uuid::Uuid;

/// Generations walked up from a person.
const MAX_UP: u8 = 8;
/// Ancestors kept per person (an 8-generation pedigree has 510 at most; collapse only lowers that).
const MAX_ANCESTORS: usize = 2_000;

/// Ancestor slot -> (distance in generations, family through which it was reached; nil for the person).
type Anc = HashMap<u32, (u8, Uuid)>;

/// The record data the search reads.
pub struct Kin<'a> {
    pub tree: &'a Tree,
    pub links: &'a Links,
    pub index: &'a PersonIndex,
}

/// Per reference person: their ancestors, their partners (with theirs) and the answers found so far.
pub struct KinMemo {
    from: u32,
    from_anc: Option<Anc>,
    partners: Option<Vec<(u32, bool, Anc)>>,
    out: HashMap<u32, Option<Kinship>>,
}

impl KinMemo {
    pub fn new(from: u32) -> Self {
        Self {
            from,
            from_anc: None,
            partners: None,
            out: HashMap::new(),
        }
    }

    pub fn from(&self) -> u32 {
        self.from
    }
}

/// A blood relation found between two ancestor maps.
#[derive(Clone, Copy)]
struct Blood {
    up: u8,
    down: u8,
    half: bool,
}

impl Blood {
    fn cost(self) -> u8 {
        self.up + self.down
    }
}

fn counts_as_parent(r: &ParentRel) -> bool {
    matches!(
        r,
        ParentRel::Birth | ParentRel::Adopted | ParentRel::Unknown
    )
}

impl Kin<'_> {
    fn sex(&self, k: u32) -> SexKind {
        self.index
            .summaries
            .get(k as usize)
            .map_or(SexKind::Unknown, |s| s.sex)
    }

    /// Parents of `k` (slot, family), over every family the person is a child in.
    fn parents(&self, k: u32) -> Vec<(u32, Uuid)> {
        let mut out = Vec::new();
        let Some(s) = self.index.summaries.get(k as usize) else {
            return out;
        };
        for (fid, i) in self.links.child_in.get(&s.id).into_iter().flatten() {
            let Some(fam) = self.tree.families.get(fid) else {
                continue;
            };
            let Some(c) = fam.children.get(*i) else {
                continue;
            };
            for (parent, rel) in [(fam.partner1, &c.rel1), (fam.partner2, &c.rel2)] {
                if let Some(slot) = parent.and_then(|id| self.index.slot(id))
                    && counts_as_parent(rel)
                {
                    out.push((slot, *fid));
                }
            }
        }
        out
    }

    /// Partners of `k` (slot, whether the union is a marriage).
    fn partners(&self, k: u32) -> Vec<(u32, bool)> {
        let mut out: Vec<(u32, bool)> = Vec::new();
        let Some(s) = self.index.summaries.get(k as usize) else {
            return out;
        };
        for fid in self.links.partner_in.get(&s.id).into_iter().flatten() {
            let Some(fam) = self.tree.families.get(fid) else {
                continue;
            };
            let other = if fam.partner1 == Some(s.id) {
                fam.partner2
            } else {
                fam.partner1
            };
            let Some(slot) = other.and_then(|id| self.index.slot(id)) else {
                continue;
            };
            // A couple counts as married unless recorded otherwise: imported families often carry no
            // marriage record at all, and "husband"/"wife" reads better than "partner" for them.
            let married = !matches!(fam.kind, UnionKind::Unmarried | UnionKind::Other(_));
            match out.iter_mut().find(|(p, _)| *p == slot) {
                Some(e) => e.1 |= married,
                None => out.push((slot, married)),
            }
        }
        out
    }

    /// `start` and its ancestors up to `MAX_UP` generations, nearest distance first.
    fn ancestors(&self, start: u32) -> Anc {
        let mut map: Anc = HashMap::new();
        map.insert(start, (0, Uuid::nil()));
        let mut frontier = vec![start];
        for d in 1..=MAX_UP {
            let mut next = Vec::new();
            for k in frontier {
                for (p, fam) in self.parents(k) {
                    if let std::collections::hash_map::Entry::Vacant(v) = map.entry(p) {
                        v.insert((d, fam));
                        next.push(p);
                    }
                }
            }
            if next.is_empty() || map.len() >= MAX_ANCESTORS {
                break;
            }
            frontier = next;
        }
        map
    }

    /// The nearest blood relation between the owners of two ancestor maps.
    fn blood(a: &Anc, b: &Anc) -> Option<Blood> {
        let mut best: Option<(u8, u8, u8, u32)> = None; // (sum, imbalance, up, ancestor)
        for (x, (d1, _)) in a {
            if let Some((d2, _)) = b.get(x) {
                let cand = (d1 + d2, d1.abs_diff(*d2), *d1, *x);
                if best.is_none_or(|b| (cand.0, cand.1, cand.3) < (b.0, b.1, b.3)) {
                    best = Some(cand);
                }
            }
        }
        let (sum, _, up, anc) = best?;
        let down = sum - up;
        let mut half = false;
        if up > 0 && down > 0 {
            let same_level = a
                .iter()
                .filter(|(x, (d1, _))| *d1 == up && b.get(*x).is_some_and(|(d2, _)| *d2 == down))
                .count();
            if same_level == 1 {
                // One shared ancestor: half relatives unless both come down through the same family.
                half = a.get(&anc).map(|e| e.1) != b.get(&anc).map(|e| e.1);
            }
        }
        Some(Blood { up, down, half })
    }

    fn kinship(kind: KinshipKind, b: Option<Blood>, sex: SexKind, via: SexKind) -> Kinship {
        Kinship {
            kind,
            up: b.map_or(0, |b| b.up),
            down: b.map_or(0, |b| b.down),
            half: b.is_some_and(|b| b.half),
            married: false,
            sex,
            via,
        }
    }

    /// How `to` is related to the memo's reference person, or `None` when no relation is found.
    pub fn relationship(&self, memo: &mut KinMemo, to: u32) -> Option<Kinship> {
        if let Some(k) = memo.out.get(&to) {
            return *k;
        }
        // The unnamed-siblings marker is nobody's relative.
        let marker = |k: u32| {
            self.index
                .summaries
                .get(k as usize)
                .is_some_and(|s| s.sibling_marker)
        };
        if marker(to) || marker(memo.from) {
            memo.out.insert(to, None);
            return None;
        }
        let k = self.compute(memo, to);
        memo.out.insert(to, k);
        k
    }

    fn compute(&self, memo: &mut KinMemo, to: u32) -> Option<Kinship> {
        let sex = self.sex(to);
        if to == memo.from {
            return Some(Self::kinship(
                KinshipKind::SelfPerson,
                None,
                sex,
                SexKind::Unknown,
            ));
        }
        if memo.from_anc.is_none() {
            memo.from_anc = Some(self.ancestors(memo.from));
        }
        if memo.partners.is_none() {
            let list = self
                .partners(memo.from)
                .into_iter()
                .map(|(p, m)| (p, m, self.ancestors(p)))
                .collect();
            memo.partners = Some(list);
        }
        let (Some(from_anc), Some(partners)) = (&memo.from_anc, &memo.partners) else {
            return None;
        };
        let to_anc = self.ancestors(to);
        // Candidates ordered by (class, cost): a parent or child, then a partner, then any other blood
        // relative (an aunt stays an aunt even when her daughter is the partner), then in-laws, nearest first.
        let mut best: Option<((u8, u8), Kinship)> = None;
        let mut offer = |class: u8, cost: u8, k: Kinship| {
            let key = (class, cost);
            if best.is_none_or(|b| key < b.0) {
                best = Some((key, k));
            }
        };
        if let Some(b) = Self::blood(from_anc, &to_anc) {
            offer(
                if b.cost() == 1 { 0 } else { 2 },
                b.cost(),
                Self::kinship(KinshipKind::Blood, Some(b), sex, SexKind::Unknown),
            );
        }
        if let Some((_, married, _)) = partners.iter().find(|(p, _, _)| *p == to) {
            let mut k = Self::kinship(KinshipKind::Partner, None, sex, SexKind::Unknown);
            k.married = *married;
            offer(1, 0, k);
        }
        // `to` is the partner of a blood relative.
        for (q, married) in self.partners(to) {
            if q == memo.from {
                continue;
            }
            if let Some(b) = Self::blood(from_anc, &self.ancestors(q)) {
                let mut k =
                    Self::kinship(KinshipKind::PartnerOfRelative, Some(b), sex, self.sex(q));
                k.married = married;
                offer(3, b.cost() * 2, k);
            }
        }
        // `to` is a blood relative of a partner.
        for (p, married, p_anc) in partners {
            if *p == to {
                continue;
            }
            if let Some(b) = Self::blood(p_anc, &to_anc) {
                let mut k =
                    Self::kinship(KinshipKind::RelativeOfPartner, Some(b), sex, self.sex(*p));
                k.married = *married;
                offer(3, b.cost() * 2 + 1, k);
            }
        }
        best.map(|b| b.1)
    }
}
