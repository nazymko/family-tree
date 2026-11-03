//! Genealogical dates: tolerant parsing, sort ranges and GEDCOM formatting.
//!
//! # Sort ranges (a design call)
//!
//! Every [`GenDate`] carries `sort_min` / `sort_max` Julian Day Numbers so that
//! imprecise dates can be ordered. `d.min` / `d.max` are `first_jdn` / `last_jdn`
//! of the date at its written precision (a year covers 1 Jan .. 31 Dec); when a
//! dual year is set, the computation uses `dual_year`.
//!
//! | Value | sort_min | sort_max |
//! |---|---|---|
//! | Exact, Interpreted | d.min | d.max |
//! | About, Calculated, Estimated | d.min - 5 years | d.max + 5 years |
//! | Before(d) | none | d.max |
//! | After(d) | d.min | none |
//! | Between(a,b), FromTo(a,b) | a.min | b.max |
//! | From(d) | d.min | none |
//! | To(d) | none | d.max |
//! | PhraseOnly, Unparsed | none | none |

pub mod calendar;
mod format;
mod parse;

use calendar::{Calendar, first_jdn, last_jdn};
use serde::{Deserialize, Serialize};

/// A single (possibly partial) calendar date as written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Date {
    pub calendar: Calendar,
    /// Year as written, at least 1 (see `bce`).
    pub year: i32,
    pub bce: bool,
    pub month: Option<u8>,
    pub day: Option<u8>,
    /// `1731/32` is stored as `year = 1731`, `dual_year = Some(1732)`.
    pub dual_year: Option<i32>,
}

impl Date {
    /// Astronomical year (1 BCE = 0) used for calendar arithmetic. A dual year
    /// takes precedence over the year as written.
    fn astro_year(&self) -> i32 {
        if self.bce {
            1i32.saturating_sub(self.year)
        } else {
            self.dual_year.unwrap_or(self.year)
        }
    }

    /// First and last JDN covered by this date, `None` if it is invalid.
    pub fn jdn_range(&self) -> Option<(i32, i32)> {
        let y = self.astro_year();
        Some((
            first_jdn(self.calendar, y, self.month, self.day)?,
            last_jdn(self.calendar, y, self.month, self.day)?,
        ))
    }

    /// `jdn_range` widened by five calendar years on each side.
    fn jdn_range_fuzzy(&self) -> Option<(i32, i32)> {
        const FALLBACK_DAYS: i32 = 1826; // 5 * 365.25, for shifted dates that do not exist
        let (min, max) = self.jdn_range()?;
        let y = self.astro_year();
        let lo = first_jdn(self.calendar, y.saturating_sub(5), self.month, self.day)
            .unwrap_or_else(|| min.saturating_sub(FALLBACK_DAYS));
        let hi = last_jdn(self.calendar, y.saturating_add(5), self.month, self.day)
            .unwrap_or_else(|| max.saturating_add(FALLBACK_DAYS));
        Some((lo, hi))
    }
}

/// The meaning of a GEDCOM `DATE` value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DateValue {
    Exact(Date),
    About(Date),
    Calculated(Date),
    Estimated(Date),
    Before(Date),
    After(Date),
    Between(Date, Date),
    From(Date),
    To(Date),
    FromTo(Date, Date),
    Interpreted(Date),
    PhraseOnly,
    Unparsed,
}

/// A genealogical date: the original text, its parsed meaning and sort range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenDate {
    /// The input exactly as given.
    pub raw: String,
    pub value: DateValue,
    pub phrase: Option<String>,
    pub sort_min: Option<i32>,
    pub sort_max: Option<i32>,
}

/// How to read `NN/NN/YYYY`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DayOrder {
    Dmy,
    Mdy,
}

/// Target GEDCOM version for output (re-exported by `gt-gedcom`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GedcomVersion {
    V551,
    V70,
}

impl GenDate {
    /// Parse any text; never fails (unrecognised text becomes `Unparsed`).
    pub fn parse(input: &str, order: DayOrder) -> GenDate {
        parse::parse(input, order)
    }

    /// GEDCOM text for the `DATE` line plus, for 7.0, the `PHRASE` text.
    pub fn to_gedcom(&self, v: GedcomVersion) -> (String, Option<String>) {
        format::to_gedcom(self, v)
    }

    /// Human-readable form, e.g. `about 1850`, `12 Mar 1901`.
    pub fn display(&self) -> String {
        format::display(self)
    }

    /// Key for ordering: `sort_min`, else `sort_max`.
    pub fn sort_key(&self) -> Option<i32> {
        self.sort_min.or(self.sort_max)
    }

    pub(crate) fn from_value(raw: &str, value: DateValue, phrase: Option<String>) -> GenDate {
        let (sort_min, sort_max) = sort_range(&value);
        GenDate {
            raw: raw.to_string(),
            value,
            phrase,
            sort_min,
            sort_max,
        }
    }
}

fn sort_range(value: &DateValue) -> (Option<i32>, Option<i32>) {
    use DateValue::*;
    let min = |d: &Date| d.jdn_range().map(|r| r.0);
    let max = |d: &Date| d.jdn_range().map(|r| r.1);
    match value {
        Exact(d) | Interpreted(d) => (min(d), max(d)),
        About(d) | Calculated(d) | Estimated(d) => match d.jdn_range_fuzzy() {
            Some((lo, hi)) => (Some(lo), Some(hi)),
            None => (None, None),
        },
        Before(d) | To(d) => (None, max(d)),
        After(d) | From(d) => (min(d), None),
        Between(a, b) | FromTo(a, b) => (min(a), max(b)),
        PhraseOnly | Unparsed => (None, None),
    }
}
