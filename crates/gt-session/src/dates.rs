//! Date and lifespan text for the UI.

use crate::dto::{DateDto, DateStatus};
use gt_core::date::{Date, DateValue, GenDate};

pub fn date_dto(d: &GenDate) -> DateDto {
    DateDto {
        raw: d.raw.clone(),
        display: d.display(),
        status: match d.value {
            DateValue::PhraseOnly => DateStatus::Phrase,
            DateValue::Unparsed => DateStatus::Unparsed,
            _ => DateStatus::Ok,
        },
        sort_min: d.sort_min,
        sort_max: d.sort_max,
    }
}

fn year_text(d: &Date) -> String {
    if d.bce {
        format!("{} BCE", d.year)
    } else {
        d.year.to_string()
    }
}

/// The year as the record states it, with a qualifier: "1968", "c. 1968", "bef. 1968",
/// "aft. 1968". A range shows its first stated year with "c."; text that is not a date is "?".
pub fn stated_year(d: &GenDate) -> String {
    use DateValue::*;
    match &d.value {
        Exact(x) | Interpreted(x) => year_text(x),
        About(x) | Calculated(x) | Estimated(x) => format!("c. {}", year_text(x)),
        Before(x) | To(x) => format!("bef. {}", year_text(x)),
        After(x) | From(x) => format!("aft. {}", year_text(x)),
        Between(a, _) | FromTo(a, _) => format!("c. {}", year_text(a)),
        PhraseOnly | Unparsed => "?".to_string(),
    }
}

/// What a birth-like or death-like event tells about one end of a lifespan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Side {
    /// No such event.
    Absent,
    /// The event exists but its date is missing or unreadable.
    Undated,
    /// The stated year, e.g. "c. 1968".
    Year(String),
}

impl Side {
    pub fn of(date: Option<&GenDate>) -> Side {
        match date.map(stated_year) {
            None => Side::Undated,
            Some(y) if y == "?" => Side::Undated,
            Some(y) => Side::Year(y),
        }
    }
}

/// "c. 1968 – 2021" (en dash), "b. 1970" while no death is recorded, "d. 1921" when only a
/// death year is known, "" when no year is known at all.
pub fn lifespan_text(birth: &Side, death: &Side) -> String {
    use Side::*;
    match (birth, death) {
        (Year(b), Year(d)) => format!("{b} \u{2013} {d}"),
        (Year(b), Undated) => format!("{b} \u{2013} ?"),
        (Year(b), Absent) => format!("b. {b}"),
        (Undated, Year(d)) => format!("? \u{2013} {d}"),
        (Absent, Year(d)) => format!("d. {d}"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gt_core::DayOrder;

    fn year(s: &str) -> String {
        stated_year(&GenDate::parse(s, DayOrder::Dmy))
    }

    #[test]
    fn qualifiers() {
        assert_eq!(year("1968"), "1968");
        assert_eq!(year("12 Mar 1968"), "1968");
        assert_eq!(year("abt 1968"), "c. 1968");
        assert_eq!(year("est 1968"), "c. 1968");
        assert_eq!(year("cal 1968"), "c. 1968");
        assert_eq!(year("bef 1968"), "bef. 1968");
        assert_eq!(year("aft 1968"), "aft. 1968");
        assert_eq!(year("bet 1960 and 1970"), "c. 1960");
        assert_eq!(year("from 1960 to 1970"), "c. 1960");
        assert_eq!(year("(somewhere)"), "?");
        assert_eq!(year("not a date at all"), "?");
    }

    #[test]
    fn lifespan_shapes() {
        let y = |s: &str| Side::Year(s.into());
        assert_eq!(
            lifespan_text(&y("c. 1968"), &y("2021")),
            "c. 1968 \u{2013} 2021"
        );
        assert_eq!(lifespan_text(&y("1970"), &Side::Absent), "b. 1970");
        assert_eq!(lifespan_text(&Side::Absent, &y("1921")), "d. 1921");
        assert_eq!(lifespan_text(&y("1850"), &Side::Undated), "1850 \u{2013} ?");
        assert_eq!(lifespan_text(&Side::Undated, &y("1921")), "? \u{2013} 1921");
        assert_eq!(lifespan_text(&Side::Absent, &Side::Absent), "");
        assert_eq!(lifespan_text(&Side::Undated, &Side::Absent), "");
    }
}
