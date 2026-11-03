//! GEDCOM and human-readable output for dates.

use super::calendar::{Calendar, months_in_year};
use super::{Date, DateValue, GedcomVersion, GenDate};

pub(super) const GREGORIAN_CODES: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];
pub(super) const HEBREW_CODES: [&str; 13] = [
    "TSH", "CSH", "KSL", "TVT", "SHV", "ADR", "ADS", "NSN", "IYR", "SVN", "TMZ", "AAV", "ELL",
];
pub(super) const FRENCH_CODES: [&str; 13] = [
    "VEND", "BRUM", "FRIM", "NIVO", "PLUV", "VENT", "GERM", "FLOR", "PRAI", "MESS", "THER", "FRUC",
    "COMP",
];

const GREGORIAN_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const HEBREW_NAMES: [&str; 13] = [
    "Tishrei", "Cheshvan", "Kislev", "Tevet", "Shevat", "Adar I", "Adar II", "Nisan", "Iyar",
    "Sivan", "Tamuz", "Av", "Elul",
];
const FRENCH_NAMES: [&str; 13] = [
    "Vendemiaire",
    "Brumaire",
    "Frimaire",
    "Nivose",
    "Pluviose",
    "Ventose",
    "Germinal",
    "Floreal",
    "Prairial",
    "Messidor",
    "Thermidor",
    "Fructidor",
    "Sansculottides",
];

fn month_str<'a>(codes: &'a [&'a str], m: u8) -> &'a str {
    usize::from(m)
        .checked_sub(1)
        .and_then(|i| codes.get(i))
        .copied()
        .unwrap_or("")
}

fn month_codes(cal: Calendar) -> &'static [&'static str] {
    match cal {
        Calendar::Gregorian | Calendar::Julian => &GREGORIAN_CODES,
        Calendar::Hebrew => &HEBREW_CODES,
        Calendar::FrenchRepublican => &FRENCH_CODES,
    }
}

fn gedcom_date(d: &Date, v: GedcomVersion) -> String {
    let mut parts: Vec<String> = Vec::new();
    match (d.calendar, v) {
        (Calendar::Gregorian, _) => {}
        (Calendar::Julian, GedcomVersion::V551) => parts.push("@#DJULIAN@".into()),
        (Calendar::Hebrew, GedcomVersion::V551) => parts.push("@#DHEBREW@".into()),
        (Calendar::FrenchRepublican, GedcomVersion::V551) => parts.push("@#DFRENCH R@".into()),
        (Calendar::Julian, GedcomVersion::V70) => parts.push("JULIAN".into()),
        (Calendar::Hebrew, GedcomVersion::V70) => parts.push("HEBREW".into()),
        (Calendar::FrenchRepublican, GedcomVersion::V70) => parts.push("FRENCH_R".into()),
    }
    if let Some(day) = d.day {
        parts.push(day.to_string());
    }
    if let Some(m) = d.month {
        parts.push(month_str(month_codes(d.calendar), m).to_string());
    }
    match (v, d.dual_year) {
        (GedcomVersion::V551, Some(dual)) => parts.push(format!("{}/{:02}", d.year, dual % 100)),
        (GedcomVersion::V70, Some(dual)) => parts.push(dual.to_string()),
        _ => parts.push(d.year.to_string()),
    }
    if d.bce {
        parts.push(
            match v {
                GedcomVersion::V551 => "B.C.",
                GedcomVersion::V70 => "BCE",
            }
            .into(),
        );
    }
    parts.join(" ")
}

fn has_dual(value: &DateValue) -> bool {
    use DateValue::*;
    match value {
        Exact(d) | About(d) | Calculated(d) | Estimated(d) | Before(d) | After(d) | From(d)
        | To(d) | Interpreted(d) => d.dual_year.is_some(),
        Between(a, b) | FromTo(a, b) => a.dual_year.is_some() || b.dual_year.is_some(),
        PhraseOnly | Unparsed => false,
    }
}

pub(super) fn to_gedcom(g: &GenDate, v: GedcomVersion) -> (String, Option<String>) {
    use DateValue::*;
    let f = |d: &Date| gedcom_date(d, v);
    let text = match &g.value {
        Exact(d) => f(d),
        About(d) => format!("ABT {}", f(d)),
        Calculated(d) => format!("CAL {}", f(d)),
        Estimated(d) => format!("EST {}", f(d)),
        Before(d) => format!("BEF {}", f(d)),
        After(d) => format!("AFT {}", f(d)),
        Between(a, b) => format!("BET {} AND {}", f(a), f(b)),
        From(d) => format!("FROM {}", f(d)),
        To(d) => format!("TO {}", f(d)),
        FromTo(a, b) => format!("FROM {} TO {}", f(a), f(b)),
        Interpreted(d) => match (v, &g.phrase) {
            (GedcomVersion::V551, Some(p)) => format!("INT {} ({p})", f(d)),
            (GedcomVersion::V551, None) => format!("INT {}", f(d)),
            (GedcomVersion::V70, _) => f(d),
        },
        PhraseOnly => match (v, &g.phrase) {
            (GedcomVersion::V551, Some(p)) => format!("({p})"),
            (GedcomVersion::V551, None) => "()".to_string(),
            // 7.0: no date text; the caller writes the PHRASE only.
            (GedcomVersion::V70, _) => String::new(),
        },
        Unparsed => return (g.raw.clone(), None),
    };
    let phrase = match v {
        GedcomVersion::V551 => None,
        GedcomVersion::V70 => match (&g.value, &g.phrase) {
            (Interpreted(_) | PhraseOnly, p) => p.clone(),
            (value, p) if has_dual(value) => p.clone().or_else(|| Some(g.raw.clone())),
            (_, p) => p.clone(),
        },
    };
    (text, phrase)
}

fn display_date(d: &Date) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(day) = d.day {
        parts.push(day.to_string());
    }
    if let Some(m) = d.month {
        let name = match d.calendar {
            Calendar::Gregorian | Calendar::Julian => month_str(&GREGORIAN_NAMES, m),
            Calendar::Hebrew => {
                if m == 7 && months_in_year(Calendar::Hebrew, d.year) == 12 {
                    "Adar"
                } else {
                    month_str(&HEBREW_NAMES, m)
                }
            }
            Calendar::FrenchRepublican => month_str(&FRENCH_NAMES, m),
        };
        parts.push(name.to_string());
    }
    match d.dual_year {
        Some(dual) => parts.push(format!("{}/{:02}", d.year, dual % 100)),
        None => parts.push(d.year.to_string()),
    }
    if d.bce {
        parts.push("BCE".into());
    }
    match d.calendar {
        Calendar::Gregorian => {}
        Calendar::Julian => parts.push("(Julian)".into()),
        Calendar::Hebrew => parts.push("(Hebrew)".into()),
        Calendar::FrenchRepublican => parts.push("(French Republican)".into()),
    }
    parts.join(" ")
}

pub(super) fn display(g: &GenDate) -> String {
    use DateValue::*;
    let f = display_date;
    match &g.value {
        Exact(d) => f(d),
        About(d) => format!("about {}", f(d)),
        Calculated(d) => format!("calculated {}", f(d)),
        Estimated(d) => format!("estimated {}", f(d)),
        Before(d) => format!("before {}", f(d)),
        After(d) => format!("after {}", f(d)),
        Between(a, b) => format!("between {} and {}", f(a), f(b)),
        From(d) => format!("from {}", f(d)),
        To(d) => format!("to {}", f(d)),
        FromTo(a, b) => format!("from {} to {}", f(a), f(b)),
        Interpreted(d) => match &g.phrase {
            Some(p) => format!("{} ({p})", f(d)),
            None => f(d),
        },
        PhraseOnly => g.phrase.clone().unwrap_or_else(|| g.raw.clone()),
        Unparsed => g.raw.clone(),
    }
}
