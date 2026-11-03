//! Gramps dates -> `GenDate`. A GEDCOM date phrase is built from the Gramps structure and parsed
//! by `GenDate::parse`, so the result is exactly what the GEDCOM importer yields for that text.

use gt_core::DayOrder;
use gt_core::date::{DateValue, GenDate};
use gt_gedcom::ImportReport;
use gt_gedcom::import::canonical_date_text;
use roxmltree::Node;
use std::cell::OnceCell;

const GREGORIAN: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];
const HEBREW: [&str; 13] = [
    "TSH", "CSH", "KSL", "TVT", "SHV", "ADR", "ADS", "NSN", "IYR", "SVN", "TMZ", "AAV", "ELL",
];
const FRENCH: [&str; 13] = [
    "VEND", "BRUM", "FRIM", "NIVO", "PLUV", "VENT", "GERM", "FLOR", "PRAI", "MESS", "THER", "FRUC",
    "COMP",
];

pub(crate) fn is_date_tag(name: &str) -> bool {
    matches!(name, "dateval" | "daterange" | "datespan" | "datestr")
}

/// Line numbers by byte offset: the newline index is built once, on the first lookup.
pub(crate) struct LineIndex<'t> {
    text: &'t str,
    starts: OnceCell<Vec<usize>>,
}

impl<'t> LineIndex<'t> {
    pub(crate) fn new(text: &'t str) -> Self {
        Self {
            text,
            starts: OnceCell::new(),
        }
    }

    /// 1-based line of a byte offset.
    pub(crate) fn row(&self, offset: usize) -> u32 {
        let starts = self.starts.get_or_init(|| {
            let mut v = vec![0usize];
            v.extend(
                self.text
                    .bytes()
                    .enumerate()
                    .filter(|(_, b)| *b == b'\n')
                    .map(|(i, _)| i + 1),
            );
            v
        });
        u32::try_from(starts.partition_point(|s| *s <= offset)).unwrap_or(u32::MAX)
    }
}

/// Distinct report keys beyond this fold into `other`; key text is cut to `KEY_CHARS`.
const MAX_KEYS: usize = 300;
const KEY_CHARS: usize = 60;
const MAX_UNPARSED: usize = 1000;

pub(crate) fn count_key(report: &mut ImportReport, key: &str) {
    let key: String = key.chars().take(KEY_CHARS).collect();
    let key = if report.unmapped_tags.contains_key(&key) || report.unmapped_tags.len() < MAX_KEYS {
        key
    } else {
        "other".to_string()
    };
    *report.unmapped_tags.entry(key).or_insert(0) += 1;
}

fn count(report: &mut ImportReport, key: &str) {
    count_key(report, key);
}

enum PartErr {
    /// `0000-00-00`: no date at all.
    Empty,
    /// A calendar or value we cannot express.
    Unsupported(String),
}

/// One Gramps `YYYY-MM-DD` (zero = unknown part) as GEDCOM date text.
fn part(val: &str, cal: &str) -> Result<String, PartErr> {
    let val = val.trim();
    let (neg, body) = match val.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, val),
    };
    let nums: Vec<u32> = body
        .split('-')
        .map(|s| s.trim().parse::<u32>())
        .collect::<Result<_, _>>()
        .map_err(|_| PartErr::Unsupported(format!("value {val}")))?;
    let (y, m, d) = match nums.as_slice() {
        [y] => (*y, 0, 0),
        [y, m] => (*y, *m, 0),
        [y, m, d] => (*y, *m, *d),
        _ => return Err(PartErr::Unsupported(format!("value {val}"))),
    };
    if y == 0 && m == 0 && d == 0 {
        return Err(PartErr::Empty);
    }
    if y == 0 {
        return Err(PartErr::Unsupported("year-less date".to_string()));
    }
    let (prefix, table): (&str, &[&str]) = match cal.trim().to_ascii_lowercase().as_str() {
        "" | "gregorian" => ("", &GREGORIAN),
        "julian" => ("@#DJULIAN@ ", &GREGORIAN),
        "hebrew" => ("@#DHEBREW@ ", &HEBREW),
        "french republican" | "french" => ("@#DFRENCH R@ ", &FRENCH),
        other => return Err(PartErr::Unsupported(format!("calendar {other}"))),
    };
    let mut out = String::from(prefix);
    if m >= 1 {
        let Some(code) = usize::try_from(m).ok().and_then(|i| table.get(i - 1)) else {
            return Err(PartErr::Unsupported("month out of range".to_string()));
        };
        if d >= 1 {
            out.push_str(&format!("{d} "));
        }
        out.push_str(code);
        out.push(' ');
    }
    out.push_str(&y.to_string());
    if neg {
        out.push_str(" BCE");
    }
    Ok(out)
}

/// The first date child of `parent`, if any. Further date children are counted as unmapped.
pub(crate) fn read_date(
    lines: &LineIndex,
    parent: Node,
    order: DayOrder,
    report: &mut ImportReport,
) -> Option<GenDate> {
    let mut out = None;
    for c in parent.children().filter(Node::is_element) {
        let name = c.tag_name().name();
        if !is_date_tag(name) {
            continue;
        }
        if out.is_some() {
            count(
                report,
                &format!("{}/{name}(extra)", parent.tag_name().name()),
            );
            continue;
        }
        out = build(c, name, order, report, lines);
    }
    out
}

fn finish(
    text: &str,
    order: DayOrder,
    report: &mut ImportReport,
    lines: &LineIndex,
    at: usize,
) -> GenDate {
    let mut g = GenDate::parse(text, order);
    if matches!(g.value, DateValue::Unparsed) {
        if report.unparsed_dates.len() < MAX_UNPARSED {
            report
                .unparsed_dates
                .push((lines.row(at), g.raw.chars().take(200).collect()));
        } else {
            count(report, "date(unparsed, over limit)");
        }
    } else {
        g.raw = canonical_date_text(&g);
    }
    g
}

fn build(
    c: Node,
    name: &str,
    order: DayOrder,
    report: &mut ImportReport,
    lines: &LineIndex,
) -> Option<GenDate> {
    let at = c.range().start;
    let cal = c.attribute("cformat").unwrap_or("Gregorian");
    if c.attribute("dualdated")
        .is_some_and(|v| matches!(v, "1" | "true"))
    {
        count(report, "date/@dualdated");
    }
    if c.attribute("newyear").is_some_and(|v| !v.is_empty()) {
        count(report, "date/@newyear");
    }
    let qword = match c.attribute("quality") {
        Some("estimated") => Some("estimated"),
        Some("calculated") => Some("calculated"),
        _ => None,
    };
    for a in ["val", "start", "stop", "from", "to"] {
        if c.attribute(a).is_some_and(day_without_month) {
            // The year (and any month) is kept; a day cannot be expressed without a month.
            count(report, "date/day without month dropped");
        }
    }
    let raw_val = c.attribute("val").unwrap_or("");
    let expr: Result<String, PartErr> = match name {
        "datestr" => {
            let t = raw_val.trim();
            if t.is_empty() {
                return None;
            }
            return Some(finish(t, order, report, lines, at));
        }
        "dateval" => dateval(c, cal, raw_val, qword),
        "daterange" | "datespan" => {
            let (a, b) = if name == "daterange" {
                ("start", "stop")
            } else {
                ("from", "to")
            };
            let pa = part(c.attribute(a).unwrap_or(""), cal);
            let pb = part(c.attribute(b).unwrap_or(""), cal);
            match (pa, pb) {
                (Ok(x), Ok(y)) => {
                    let base = if name == "daterange" {
                        format!("BET {x} AND {y}")
                    } else {
                        format!("FROM {x} TO {y}")
                    };
                    Ok(with_phrase(base, qword))
                }
                (Err(PartErr::Empty), Ok(y)) if name == "datespan" => {
                    Ok(with_phrase(format!("TO {y}"), qword))
                }
                (Ok(x), Err(PartErr::Empty)) if name == "datespan" => {
                    Ok(with_phrase(format!("FROM {x}"), qword))
                }
                (Err(PartErr::Empty), Err(PartErr::Empty)) => Err(PartErr::Empty),
                (Err(PartErr::Unsupported(w)), _) | (_, Err(PartErr::Unsupported(w))) => {
                    Err(PartErr::Unsupported(w))
                }
                _ => Err(PartErr::Unsupported(format!("incomplete {name}"))),
            }
        }
        _ => return None,
    };
    match expr {
        Ok(text) => Some(finish(&text, order, report, lines, at)),
        Err(PartErr::Empty) => None,
        Err(PartErr::Unsupported(what)) => {
            count(report, &format!("date/{what}"));
            // Keep what the file said as a phrase-only date rather than losing it.
            let shown = describe(c);
            Some(finish(&format!("({shown})"), order, report, lines, at))
        }
    }
}

fn day_without_month(v: &str) -> bool {
    let body = v.trim().trim_start_matches('-');
    let p: Vec<u32> = body
        .split('-')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    matches!(p.as_slice(), [y, 0, d] if *y > 0 && *d > 0)
}

fn with_phrase(base: String, q: Option<&str>) -> String {
    match q {
        Some(w) => format!("{base} ({w})"),
        None => base,
    }
}

fn dateval(c: Node, cal: &str, val: &str, q: Option<&str>) -> Result<String, PartErr> {
    let d = part(val, cal)?;
    let ty = c.attribute("type").unwrap_or("").to_ascii_lowercase();
    Ok(match (ty.as_str(), q) {
        ("" | "regular" | "about", Some("estimated")) => format!("EST {d}"),
        ("" | "regular" | "about", Some(_)) => format!("CAL {d}"),
        ("" | "regular", None) => d,
        ("about", None) => format!("ABT {d}"),
        ("before", q) => with_phrase(format!("BEF {d}"), q),
        ("after", q) => with_phrase(format!("AFT {d}"), q),
        ("from", q) => with_phrase(format!("FROM {d}"), q),
        ("to", q) => with_phrase(format!("TO {d}"), q),
        (other, _) => return Err(PartErr::Unsupported(format!("type {other}"))),
    })
}

/// The date attributes as readable phrase text (no parentheses, which would end the phrase).
fn describe(c: Node) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for a in c.attributes() {
        if matches!(a.name(), "val" | "start" | "stop" | "from" | "to") {
            parts.push(a.value());
        }
    }
    let mut s = parts.join(" - ");
    if let Some(cal) = c.attribute("cformat").filter(|v| *v != "Gregorian") {
        s = format!("{s} [{cal}]");
    }
    s.replace(['(', ')'], "")
}
