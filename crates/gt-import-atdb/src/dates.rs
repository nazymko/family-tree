//! `ValuesDates` rows -> `GenDate`, by building GEDCOM date text and using the shared parser.

use gt_core::DayOrder;
use gt_core::date::{DateValue, GenDate};
use gt_gedcom::import::canonical_date_text;

/// One `ValuesDates` row.
#[derive(Debug, Clone, Default)]
pub struct DateRow {
    pub d: Option<i64>,
    pub m: Option<i64>,
    pub y: Option<i64>,
    pub d2: Option<i64>,
    pub m2: Option<i64>,
    pub y2: Option<i64>,
    pub calendar: Option<i64>,
    pub calendar2: Option<i64>,
    pub ty: Option<i64>,
}

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// GEDCOM text for one date half; `None` when the half is empty. Out-of-range numbers make the
/// text plain digits that the parser reports as unparsed, so nothing is lost.
fn part(d: Option<i64>, m: Option<i64>, y: Option<i64>) -> Option<String> {
    if d.is_none() && m.is_none() && y.is_none() {
        return None;
    }
    let month = m
        .and_then(|m| usize::try_from(m).ok()?.checked_sub(1))
        .and_then(|i| MONTHS.get(i));
    let day_ok = d.filter(|d| (1..=31).contains(d));
    let year_ok = y.filter(|y| (1..=9999).contains(y));
    let valid = (m.is_none() || month.is_some())
        && (d.is_none() || day_ok.is_some())
        && (y.is_none() || year_ok.is_some());
    if !valid {
        let f = |v: Option<i64>| v.map_or("?".to_string(), |v| v.to_string());
        return Some(format!("{}.{}.{}", f(d), f(m), f(y)));
    }
    let mut out: Vec<String> = Vec::new();
    if let Some(d) = day_ok {
        out.push(d.to_string());
    }
    if let Some(m) = month {
        out.push((*m).to_string());
    }
    if let Some(y) = year_ok {
        out.push(y.to_string());
    }
    Some(out.join(" "))
}

/// What the importer could not interpret in a row; each string becomes an `unmapped_tags` key.
pub fn caveats(r: &DateRow) -> Vec<String> {
    let mut v = Vec::new();
    if let Some(t) = r.ty.filter(|t| *t != 0) {
        v.push(format!("date qualifier {t} (read as an exact date)"));
    }
    for c in [r.calendar, r.calendar2].into_iter().flatten() {
        v.push(format!("date calendar {c} (read as Gregorian)"));
    }
    v
}

/// The date, plus the text of an unparsed one for the report.
pub fn to_date(r: &DateRow) -> Option<(GenDate, bool)> {
    let first = part(r.d, r.m, r.y);
    let second = part(r.d2, r.m2, r.y2);
    let text = match (first, second) {
        (Some(a), Some(b)) => format!("BET {a} AND {b}"),
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => return None,
    };
    let mut g = GenDate::parse(&text, DayOrder::Dmy);
    let unparsed = matches!(g.value, DateValue::Unparsed);
    if !unparsed {
        g.raw = canonical_date_text(&g);
    }
    Some((g, unparsed))
}
