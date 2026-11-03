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

fn place_name(p: &gt_core::model::Place) -> String {
    p.names
        .first()
        .map(|n| n.name.clone())
        .or_else(|| p.raw.clone())
        .unwrap_or_default()
}

/// One place with its chain pre-folded for matching.
#[derive(Debug)]
struct PlaceEntry {
    option: PlaceOption,
    /// Folded leaf name.
    leaf: String,
    /// Folded names of the ancestors, parent first.
    ancestors: Vec<String>,
    /// Folded `full`.
    full: String,
    /// Events at the place or any descendant.
    uses: u32,
}

/// Place suggestions index, built when the session opens and after every rebuild.
#[derive(Debug, Default)]
pub struct PlaceIndex {
    entries: Vec<PlaceEntry>,
}

impl PlaceIndex {
    pub fn build(tree: &gt_core::tree::Tree) -> PlaceIndex {
        let mut direct: HashMap<Uuid, u32> = HashMap::new();
        for e in tree.events.values() {
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
                next = tree.places.get(&pid).and_then(|p| p.parent);
            }
        }
        let mut folded_names: HashMap<Uuid, String> = HashMap::new();
        let mut entries = Vec::with_capacity(tree.places.len());
        for (id, place) in &tree.places {
            let mut names = Vec::new();
            let mut next = Some(*id);
            for _ in 0..16 {
                let Some(p) = next.and_then(|p| tree.places.get(&p)) else {
                    break;
                };
                let n = place_name(p);
                if !n.is_empty() {
                    names.push((next.unwrap_or(*id), n));
                }
                next = p.parent;
            }
            let mut fold_of = |(pid, n): &(Uuid, String)| {
                folded_names.entry(*pid).or_insert_with(|| fold(n)).clone()
            };
            let folded: Vec<String> = names.iter().map(&mut fold_of).collect();
            let strs: Vec<&str> = names.iter().map(|(_, n)| n.as_str()).collect();
            let full = strs.join(", ");
            entries.push(PlaceEntry {
                option: PlaceOption {
                    id: *id,
                    name: strs.first().map(|s| (*s).to_string()).unwrap_or_default(),
                    full: full.clone(),
                    kind: place.kind.clone(),
                },
                leaf: folded.first().cloned().unwrap_or_default(),
                ancestors: folded.iter().skip(1).cloned().collect(),
                full: folded.join(", "),
                uses: total.get(id).copied().unwrap_or(0),
            });
        }
        PlaceIndex { entries }
    }

    /// Leaf-prefix matches first, then ancestor-prefix matches; each by use count, then ICU order.
    pub fn suggest(&self, prefix: &str, limit: usize) -> Vec<PlaceOption> {
        let limit = limit.min(MAX_PLACES);
        if limit == 0 {
            return Vec::new();
        }
        let q = fold(prefix.trim());
        let mut hits: Vec<(u8, &PlaceEntry)> = Vec::new();
        for e in &self.entries {
            if q.is_empty() || e.leaf.starts_with(&q) || e.full.starts_with(&q) {
                hits.push((0, e));
            } else if e.ancestors.iter().any(|a| a.starts_with(&q)) {
                hits.push((1, e));
            }
        }
        hits.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(b.1.uses.cmp(&a.1.uses))
                .then_with(|| collate(&a.1.option.full, &b.1.option.full))
                .then(a.1.option.id.cmp(&b.1.option.id))
        });
        hits.into_iter()
            .take(limit)
            .map(|(_, e)| e.option.clone())
            .collect()
    }
}

impl Session {
    /// Places whose leaf name starts with `prefix` come first, then places with an ancestor
    /// that does; within a group by use count, then ICU order. `limit` is clamped to 20.
    pub fn suggest_places(&self, prefix: &str, limit: usize) -> Vec<PlaceOption> {
        self.place_index.suggest(prefix, limit)
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
