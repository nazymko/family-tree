//! Lookups for the editors: date preview, place and source suggestions, last citation.

use crate::dto::{CitationDto, DatePreview, DateStatus, DayOrder, PlaceOption, SourceDto};
use crate::index::fold;
use crate::session::Session;
use gt_core::date::calendar::gregorian_ymd;
use gt_core::date::{Date, DateValue, GenDate};
use gt_store::collation::collate;
use std::collections::HashMap;
use uuid::Uuid;

pub const MAX_PLACES: usize = 20;
pub const MAX_SOURCES: usize = 200;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Gregorian (astronomical, 1 BCE = 0) year of a Julian Day Number.
pub fn year_of_jdn(jdn: i32) -> i32 {
    gregorian_ymd(jdn).0
}

fn year_label(y: i32) -> String {
    if y <= 0 {
        format!("{} BCE", 1i64 - i64::from(y))
    } else {
        y.to_string()
    }
}

fn full_date(jdn: i32) -> String {
    let (y, m, d) = gregorian_ymd(jdn);
    let mon = MONTHS
        .get(usize::from(m).wrapping_sub(1))
        .copied()
        .unwrap_or("");
    format!("{d} {mon} {}", year_label(y))
}

/// "1845 to 1855", or full dates when the span is under two years. `None` for a single day
/// or an open-ended date.
fn range_text(g: &GenDate) -> Option<String> {
    let (lo, hi) = (g.sort_min?, g.sort_max?);
    if lo >= hi {
        return None;
    }
    if i64::from(hi) - i64::from(lo) < 730 {
        Some(format!("{} to {}", full_date(lo), full_date(hi)))
    } else {
        Some(format!(
            "{} to {}",
            year_label(year_of_jdn(lo)),
            year_label(year_of_jdn(hi))
        ))
    }
}

/// `NN/NN/YYYY` (also `.` or `-`) with both leading parts at most 12.
fn is_ambiguous_numeric(input: &str) -> bool {
    input.split_whitespace().any(|tok| {
        let tok = tok.trim_matches(|c: char| !c.is_ascii_digit());
        let parts: Vec<&str> = tok.split(['/', '.', '-']).collect();
        let [a, b, y] = parts.as_slice() else {
            return false;
        };
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
        let month_like = |s: &str| {
            digits(s) && s.len() <= 2 && s.parse::<u8>().is_ok_and(|n| (1..=12).contains(&n))
        };
        month_like(a) && month_like(b) && y.len() == 4 && digits(y)
    })
}

fn has_dual_year(v: &DateValue) -> bool {
    use DateValue::*;
    let dual = |d: &Date| d.dual_year.is_some();
    match v {
        Exact(d) | About(d) | Calculated(d) | Estimated(d) | Before(d) | After(d) | From(d)
        | To(d) | Interpreted(d) => dual(d),
        Between(a, b) | FromTo(a, b) => dual(a) || dual(b),
        PhraseOnly | Unparsed => false,
    }
}

/// Parses `input` for live feedback. Never fails: text that is not a date comes back as
/// `Unparsed` with a warning.
pub fn parse_date_preview(input: &str, order: DayOrder) -> DatePreview {
    let core_order = match order {
        DayOrder::Dmy => gt_core::DayOrder::Dmy,
        DayOrder::Mdy => gt_core::DayOrder::Mdy,
    };
    let g = GenDate::parse(input, core_order);
    let mut warnings = Vec::new();
    let status = match g.value {
        DateValue::PhraseOnly => DateStatus::Phrase,
        DateValue::Unparsed => {
            warnings
                .push("Not understood as a date. It will be saved exactly as you typed it.".into());
            DateStatus::Unparsed
        }
        _ => DateStatus::Ok,
    };
    if status == DateStatus::Ok {
        if is_ambiguous_numeric(input) {
            let name = match order {
                DayOrder::Dmy => "DMY",
                DayOrder::Mdy => "MDY",
            };
            warnings.push(format!("Day and month are ambiguous; read as {name}"));
        }
        if has_dual_year(&g.value) {
            warnings.push("Double year: the first year is Old Style, the second New Style.".into());
        }
    }
    DatePreview {
        raw: input.to_string(),
        understood_as: g.display(),
        range: if status == DateStatus::Ok {
            range_text(&g)
        } else {
            None
        },
        status,
        warnings,
    }
}

/// A place's name segments, leaf first.
struct Chain {
    id: Uuid,
    names: Vec<String>,
}

fn place_name(p: &gt_core::model::Place) -> String {
    p.names
        .first()
        .map(|n| n.name.clone())
        .or_else(|| p.raw.clone())
        .unwrap_or_default()
}

impl Session {
    /// Number of events referencing each place or any of its descendants.
    fn place_use_counts(&self) -> HashMap<Uuid, u32> {
        let mut direct: HashMap<Uuid, u32> = HashMap::new();
        for e in self.tree.events.values() {
            if let Some(p) = e.place {
                *direct.entry(p).or_default() += 1;
            }
        }
        let mut total: HashMap<Uuid, u32> = HashMap::new();
        for (id, n) in direct {
            let mut next = Some(id);
            // The limit only guards against a parent cycle.
            for _ in 0..16 {
                let Some(pid) = next else { break };
                *total.entry(pid).or_default() += n;
                next = self.tree.places.get(&pid).and_then(|p| p.parent);
            }
        }
        total
    }

    fn chain_of(&self, id: Uuid) -> Chain {
        let mut names = Vec::new();
        let mut next = Some(id);
        for _ in 0..16 {
            let Some(p) = next.and_then(|p| self.tree.places.get(&p)) else {
                break;
            };
            let n = place_name(p);
            if !n.is_empty() {
                names.push(n);
            }
            next = p.parent;
        }
        Chain { id, names }
    }

    /// Places whose leaf name starts with `prefix` come first, then places with an ancestor
    /// that does; within a group by use count, then ICU order. `limit` is clamped to 20.
    /// Computed from the tree on each call, so it always reflects the latest edit.
    pub fn suggest_places(&self, prefix: &str, limit: usize) -> Vec<PlaceOption> {
        let limit = limit.min(MAX_PLACES);
        if limit == 0 {
            return Vec::new();
        }
        let q = fold(prefix.trim());
        let uses = self.place_use_counts();
        let mut hits: Vec<(u8, u32, String, Chain)> = Vec::new();
        for id in self.tree.places.keys() {
            let chain = self.chain_of(*id);
            let full = chain.names.join(", ");
            let tier = if q.is_empty() {
                0
            } else {
                let folded: Vec<String> = chain.names.iter().map(|n| fold(n)).collect();
                if folded.first().is_some_and(|n| n.starts_with(&q)) || fold(&full).starts_with(&q)
                {
                    0
                } else if folded.iter().skip(1).any(|n| n.starts_with(&q)) {
                    1
                } else {
                    continue;
                }
            };
            hits.push((tier, uses.get(id).copied().unwrap_or(0), full, chain));
        }
        hits.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(b.1.cmp(&a.1))
                .then_with(|| collate(&a.2, &b.2))
                .then(a.3.id.cmp(&b.3.id))
        });
        hits.into_iter()
            .take(limit)
            .map(|(_, _, full, c)| PlaceOption {
                id: c.id,
                name: c.names.first().cloned().unwrap_or_default(),
                full,
                kind: self.tree.places.get(&c.id).and_then(|p| p.kind.clone()),
            })
            .collect()
    }

    /// Sources whose title starts with `prefix` first, then those with a title, abbreviation
    /// or author word that does; by citation count, then ICU order. `limit` is clamped to 200.
    /// An empty prefix lists every source.
    pub fn suggest_sources(&self, prefix: &str, limit: usize) -> Vec<SourceDto> {
        let limit = limit.min(MAX_SOURCES);
        let q = fold(prefix.trim());
        let word_prefix = |text: &str| {
            let t = fold(text);
            t.starts_with(&q) || t.split_whitespace().any(|w| w.starts_with(&q))
        };
        let mut hits: Vec<(u8, u32, String, Uuid)> = Vec::new();
        for (id, s) in &self.tree.sources {
            let title = s.title.as_deref().unwrap_or("");
            let tier = if q.is_empty() || fold(title).starts_with(&q) {
                0
            } else if word_prefix(title)
                || s.abbreviation.as_deref().is_some_and(word_prefix)
                || s.author.as_deref().is_some_and(word_prefix)
            {
                1
            } else {
                continue;
            };
            let count = self.links.citations_of_source.get(id).copied().unwrap_or(0);
            hits.push((tier, count, title.to_string(), *id));
        }
        hits.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(b.1.cmp(&a.1))
                .then_with(|| collate(&a.2, &b.2))
                .then(a.3.cmp(&b.3))
        });
        hits.into_iter()
            .take(limit)
            .filter_map(|(_, _, _, id)| self.source_dto(id))
            .collect()
    }

    /// The most recently changed citation (ties and unstamped ones: the latest in the tree),
    /// to pre-fill "same source as last time".
    pub fn last_citation(&self) -> Option<CitationDto> {
        let id = self
            .tree
            .citations
            .iter()
            .enumerate()
            .max_by_key(|(i, (_, c))| (c.common.changed.unwrap_or(i64::MIN), *i))
            .map(|(_, (id, _))| *id)?;
        self.citation_dto(id)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn year_of_jdn_known_days() {
        // 2000-01-01 is JDN 2451545; JDN 1721426 is 0001-01-01; the day before is 1 BCE (year 0).
        assert_eq!(year_of_jdn(2_451_545), 2000);
        assert_eq!(year_of_jdn(1_721_426), 1);
        assert_eq!(year_of_jdn(1_721_425), 0);
    }

    #[test]
    fn places_after_edit_include_new_chain() {
        use gt_core::model::{Common, Place, PlaceName};
        let dir = tempfile::tempdir().unwrap();
        let proj = dir.path().join("p");
        gt_store::Project::create(&proj).unwrap();
        let mut s = Session::open(&proj, crate::SessionClock::fixed(2_460_000, 0)).unwrap();
        assert!(s.suggest_places("zug", 5).is_empty());
        let mk = |name: &str, parent: Option<Uuid>| {
            let id = Uuid::now_v7();
            let place = Place {
                common: Common {
                    id,
                    private: false,
                    changed: None,
                    citations: vec![],
                    notes: vec![],
                    media: vec![],
                    ext_ids: vec![],
                    unknown: vec![],
                },
                names: vec![PlaceName {
                    name: name.into(),
                    lang: None,
                    date: None,
                }],
                parent,
                kind: None,
                lat: None,
                lon: None,
                raw: None,
            };
            (id, place)
        };
        let (cid, country) = mk("Switzerland", None);
        let (tid, town) = mk("Zug", Some(cid));
        s.tree.places.insert(cid, country);
        s.tree.places.insert(tid, town);
        s.rebuild();
        let got = s.suggest_places("zug", 5);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].full, "Zug, Switzerland");
    }
}
