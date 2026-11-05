//! Filtering and paging over the person index.

use crate::dto::{LetterOffset, LivingFilter, Page, PersonQuery, PersonRow, SortKey};
use crate::index::{PersonIndex, PersonSummary, fold};
use std::borrow::Cow;
use std::collections::HashSet;

pub const MAX_PAGE: usize = 200;
pub const MAX_SEARCH: usize = 50;

struct Filter {
    tokens: Vec<String>,
    living: LivingFilter,
    no_source: bool,
    no_birth_date: bool,
    private_only: bool,
}

impl Filter {
    fn new(q: &PersonQuery) -> Filter {
        Filter {
            tokens: fold(&q.text)
                .split_whitespace()
                .map(str::to_string)
                .collect(),
            living: q.living,
            no_source: q.no_source,
            no_birth_date: q.no_birth_date,
            private_only: q.private_only,
        }
    }

    fn is_empty(&self) -> bool {
        self.tokens.is_empty()
            && self.living == LivingFilter::Any
            && !self.no_source
            && !self.no_birth_date
            && !self.private_only
    }

    fn matches(&self, s: &PersonSummary) -> bool {
        // The unnamed-siblings marker is never listed.
        if s.sibling_marker {
            return false;
        }
        match self.living {
            LivingFilter::Living if !s.living => return false,
            LivingFilter::Deceased if s.living => return false,
            _ => {}
        }
        if (self.no_source && s.source_count > 0)
            || (self.no_birth_date && s.has_birth_date)
            || (self.private_only && !s.private)
        {
            return false;
        }
        self.tokens.iter().all(|t| s.folded.contains(t.as_str()))
    }
}

impl PersonIndex {
    /// One page of rows. `limit` is clamped to 200; an `offset` past the end gives no items.
    pub fn list(&self, q: &PersonQuery, offset: usize, limit: usize) -> Page<PersonRow> {
        let limit = limit.min(MAX_PAGE);
        let order = self.order(q.sort, q.desc);
        let filter = Filter::new(q);
        let slots: Cow<[u32]> = if filter.is_empty() && self.marker_count() == 0 {
            Cow::Borrowed(order.as_slice())
        } else {
            Cow::Owned(
                order
                    .iter()
                    .copied()
                    .filter(|&i| {
                        self.summaries
                            .get(i as usize)
                            .is_some_and(|s| filter.matches(s))
                    })
                    .collect(),
            )
        };
        let items = slots
            .iter()
            .skip(offset)
            .take(limit)
            .filter_map(|&i| self.summaries.get(i as usize))
            .map(PersonSummary::row)
            .collect();
        let letters = (q.sort == SortKey::Surname).then(|| {
            let mut seen = HashSet::new();
            let mut out = Vec::new();
            for (pos, &i) in slots.iter().enumerate() {
                let Some(s) = self.summaries.get(i as usize) else {
                    continue;
                };
                if seen.insert(s.letter) {
                    out.push(LetterOffset {
                        letter: s.letter.label(),
                        offset: u32::try_from(pos).unwrap_or(u32::MAX),
                    });
                }
            }
            out
        });
        Page {
            total: u32::try_from(slots.len()).unwrap_or(u32::MAX),
            offset: u32::try_from(offset).unwrap_or(u32::MAX),
            items,
            letters,
        }
    }
}
