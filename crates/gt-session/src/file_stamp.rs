//! The date and time a phone recorder writes into a file name, and the readable title, date and
//! description a new recording gets from it. Pure text work: no file is opened.
//!
//! Recognised, always strictly (a name that is ambiguous or does not hold a real date gives
//! nothing, never a guess):
//! - `Voice 261111_090049`, `Voice_261111_090049`: YYMMDD_HHMMSS (two-digit year, only in this
//!   shape);
//! - `VID20261031202924`, `REC_20261031_202924`, `AUD-20261031-WA0003`, `20261031_202924`:
//!   YYYYMMDD with an optional time;
//! - `2026-10-31 20.29.24`: YYYY-MM-DD with an optional time.
//!
//! When a name holds two dates (`VID20261031202924_02112026`, an export date after the capture
//! date) the first one counts.

use crate::dto::Language;

/// A date, and when the name has one a time of day, read from a file name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    /// Hour, minute, second.
    pub time: Option<(u8, u8, u8)>,
}

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

impl FileStamp {
    /// The date as the app stores dates (GEDCOM-like): `11 NOV 2026`.
    pub fn gedcom_date(&self) -> String {
        let month = MONTHS
            .get(usize::from(self.month).saturating_sub(1))
            .copied()
            .unwrap_or("JAN");
        format!("{} {month} {}", self.day, self.year)
    }

    /// `11.11.2026`.
    pub fn dotted(&self) -> String {
        format!("{:02}.{:02}.{}", self.day, self.month, self.year)
    }

    /// `09:00:49`, when the name has a time.
    pub fn time_text(&self) -> Option<String> {
        self.time.map(|(h, m, s)| format!("{h:02}:{m:02}:{s:02}"))
    }
}

/// What a new recording gets from its file name where the owner or agent gave nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordingDefaults {
    pub stamp: FileStamp,
    /// For the media record's date field, e.g. `11 NOV 2026`.
    pub date_raw: String,
    /// `Recording 11.11.2026 (Voice 261111_090049)`, in the interface language.
    pub title: String,
    /// The time of day, which the date field cannot hold; `None` when the name has no time.
    pub description: Option<String>,
}

/// The year of a Unix time in milliseconds (UTC).
pub fn year_of_unix_ms(ms: i64) -> i32 {
    let z = ms.div_euclid(86_400_000) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    i32::try_from(if month <= 2 { y + 1 } else { y }).unwrap_or(2000)
}

/// The title, date and description a recording named `original_name` gets, or `None` when the
/// name holds no date. `current_year` bounds what counts as plausible.
pub fn recording_defaults(
    original_name: &str,
    current_year: i32,
    language: Language,
) -> Option<RecordingDefaults> {
    let stamp = stamp_from_file_name(original_name, current_year)?;
    let stem = stem_of(original_name);
    let date = stamp.dotted();
    let (title, description) = if language == Language::Uk {
        (
            format!("Запис розмови {date} ({stem})"),
            stamp
                .time_text()
                .map(|t| format!("Записано о {t} (за назвою файлу).")),
        )
    } else {
        (
            format!("Recording {date} ({stem})"),
            stamp
                .time_text()
                .map(|t| format!("Recorded at {t} (from the file name).")),
        )
    };
    Some(RecordingDefaults {
        date_raw: stamp.gedcom_date(),
        stamp,
        title,
        description,
    })
}

/// The file name without its extension (a last dot followed by letters and digits that are not
/// all digits).
fn stem_of(name: &str) -> &str {
    let name = name.trim();
    match name.rfind('.') {
        Some(i) => {
            let ext = &name[i + 1..];
            let is_ext = !ext.is_empty()
                && ext.len() <= 5
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
                && !ext.chars().all(|c| c.is_ascii_digit());
            if is_ext { &name[..i] } else { name }
        }
        None => name,
    }
}

/// The first date (and time) in a file name, validated; see the module text for the shapes.
pub fn stamp_from_file_name(name: &str, current_year: i32) -> Option<FileStamp> {
    let stem = stem_of(name);
    let bytes = stem.as_bytes();
    let lower = stem.to_ascii_lowercase();
    // The first run of digits decides the shape.
    let start = bytes.iter().position(u8::is_ascii_digit)?;
    let run = bytes[start..]
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .count();
    let rest = &stem[start + run..];
    let raw = if lower.starts_with("voice") && run == 6 {
        // Voice 261111_090049: the two-digit year is accepted only here, with the time present.
        let time = digits_after_separator(rest, 6)?;
        let d = &stem[start..start + 6];
        let (yy, mm, dd) = (num(&d[0..2])?, num(&d[2..4])?, num(&d[4..6])?);
        (2000 + yy, mm, dd, Some(time_of(time)?))
    } else if run == 4 {
        // 2026-10-31 20.29.24
        let (sep, (month, rest)) = sep_and_digits(rest, 2)?;
        let (sep2, (day, rest)) = sep_and_digits(rest, 2)?;
        if sep != sep2
            || !matches!(sep, '-' | '.' | '_')
            || rest.starts_with(|c: char| c.is_ascii_digit())
        {
            return None;
        }
        let time = match clock_after(rest) {
            Clock::None => None,
            Clock::Is(t) => Some(t),
            Clock::Bad => return None,
        };
        (num(&stem[start..start + 4])?, num(month)?, num(day)?, time)
    } else if run == 8 || run == 14 {
        let d = &stem[start..start + run];
        let (year, mm, dd) = (num(&d[0..4])?, num(&d[4..6])?, num(&d[6..8])?);
        let time = if run == 14 {
            Some(time_of(&d[8..14])?)
        } else {
            match digits_after_separator(rest, 6) {
                Some(t) => Some(time_of(t)?),
                None => None,
            }
        };
        (year, mm, dd, time)
    } else {
        return None;
    };
    let (year, month, day, time) = raw;
    if !(2000..=current_year + 1).contains(&year) || !(1..=12).contains(&month) {
        return None;
    }
    if !(1..=days_in_month(year, month)).contains(&day) {
        return None;
    }
    Some(FileStamp {
        year,
        month: u8::try_from(month).ok()?,
        day: u8::try_from(day).ok()?,
        time,
    })
}

enum Clock {
    None,
    Is((u8, u8, u8)),
    Bad,
}

fn num(s: &str) -> Option<i32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// `HHMMSS` to a validated time.
fn time_of(six: &str) -> Option<(u8, u8, u8)> {
    if six.len() != 6 {
        return None;
    }
    let (h, m, s) = (num(&six[0..2])?, num(&six[2..4])?, num(&six[4..6])?);
    if h > 23 || m > 59 || s > 59 {
        return None;
    }
    Some((
        u8::try_from(h).ok()?,
        u8::try_from(m).ok()?,
        u8::try_from(s).ok()?,
    ))
}

/// After one separator (`_`, `-` or space), exactly `n` digits that are not followed by a digit.
fn digits_after_separator(rest: &str, n: usize) -> Option<&str> {
    let mut chars = rest.chars();
    let sep = chars.next()?;
    if !matches!(sep, '_' | '-' | ' ') {
        return None;
    }
    let tail = chars.as_str();
    let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
    (digits == n).then(|| &tail[..n])
}

/// One separator character, then exactly `n` digits: `(sep, (digits, remainder))`.
fn sep_and_digits(rest: &str, n: usize) -> Option<(char, (&str, &str))> {
    let sep = rest.chars().next()?;
    let tail = &rest[sep.len_utf8()..];
    let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
    (digits == n).then(|| (sep, (&tail[..n], &tail[n..])))
}

/// The clock after a dashed date: ` 20.29.24`, `_20-29-24`, ` at 20:29:24`, or nothing.
fn clock_after(rest: &str) -> Clock {
    let rest = rest
        .strip_prefix(" at ")
        .or_else(|| rest.strip_prefix(['_', ' ', '-', 'T']))
        .unwrap_or(rest);
    let Some(h) = rest.get(0..2).filter(|h| num(h).is_some()) else {
        return Clock::None;
    };
    let Some((sep, (m, tail))) = sep_and_digits(&rest[2..], 2) else {
        return Clock::Bad;
    };
    let Some((sep2, (s, tail))) = sep_and_digits(tail, 2) else {
        return Clock::Bad;
    };
    if sep != sep2
        || !matches!(sep, '.' | ':' | '-' | '_')
        || tail.starts_with(|c: char| c.is_ascii_digit())
    {
        return Clock::Bad;
    }
    match time_of(&format!("{h}{m}{s}")) {
        Some(t) => Clock::Is(t),
        None => Clock::Bad,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    type Got = Option<(i32, u8, u8, Option<(u8, u8, u8)>)>;

    fn stamp(name: &str) -> Got {
        stamp_from_file_name(name, 2026).map(|s| (s.year, s.month, s.day, s.time))
    }

    const NOV: Got = Some((2026, 11, 11, Some((9, 0, 49))));
    const OCT: Got = Some((2026, 10, 31, Some((20, 29, 24))));

    #[test]
    fn voice_style_two_digit_year() {
        assert_eq!(stamp("Voice 261111_090049.m4a"), NOV);
        assert_eq!(stamp("Voice_261111_090049.m4a"), NOV);
        assert_eq!(stamp("voice 261111_090049"), NOV);
    }

    #[test]
    fn four_digit_year_shapes() {
        assert_eq!(stamp("VID20261031202924.mp4"), OCT);
        assert_eq!(stamp("VID20261031202924_02112026.mp3"), OCT);
        assert_eq!(stamp("REC_20261031_202924.m4a"), OCT);
        assert_eq!(stamp("2026-10-31 20.29.24.m4a"), OCT);
        assert_eq!(stamp("20261031_202924.m4a"), OCT);
        assert_eq!(stamp("AUD-20261031-WA0003.m4a"), Some((2026, 10, 31, None)));
        assert_eq!(stamp("2026-10-31.m4a"), Some((2026, 10, 31, None)));
        assert_eq!(stamp("2026-10-31_20-29-24.mp3"), OCT);
    }

    #[test]
    fn the_first_date_wins() {
        assert_eq!(stamp("VID20261031202924_02112026.mp3"), OCT);
        assert_eq!(
            stamp("AUD-20250101-WA0003_20261031.m4a").map(|s| s.0),
            Some(2025)
        );
    }

    #[test]
    fn two_digit_year_needs_the_voice_pattern_and_a_time() {
        assert_eq!(stamp("261111_090049.m4a"), None);
        assert_eq!(stamp("Recording 261111_090049.m4a"), None);
        assert_eq!(stamp("Voice 261111.m4a"), None);
        assert_eq!(stamp("Voice 261111_0900.m4a"), None);
        assert_eq!(stamp("Voice 261111_09004912.m4a"), None);
    }

    #[test]
    fn invalid_parts_give_nothing() {
        assert_eq!(stamp("VID20261331202924.mp4"), None); // month 13
        assert_eq!(stamp("VID20261032202924.mp4"), None); // day 32
        assert_eq!(stamp("REC_20260231_202924.m4a"), None); // 31 February
        assert_eq!(stamp("REC_20261031_252924.m4a"), None); // hour 25
        assert_eq!(stamp("REC_20261031_206024.m4a"), None); // minute 60
        assert_eq!(stamp("REC_20261031_202961.m4a"), None); // second 61
        assert_eq!(stamp("REC_19991031_202924.m4a"), None); // year too early
        assert_eq!(stamp("REC_20281031_202924.m4a"), None); // year too late
        assert_eq!(stamp("Voice 261331_090049.m4a"), None);
        assert_eq!(stamp("2026-10-31 25.29.24.m4a"), None);
        assert_eq!(stamp("2026-10-31 20.29-24.m4a"), None);
    }

    #[test]
    fn leap_days() {
        assert_eq!(
            stamp("REC_20240229_101010.m4a").map(|s| (s.1, s.2)),
            Some((2, 29))
        );
        assert_eq!(stamp("REC_20250229_101010.m4a"), None);
    }

    #[test]
    fn names_without_a_date_or_with_odd_digit_runs_give_nothing() {
        assert_eq!(stamp("grandma story.m4a"), None);
        assert_eq!(stamp("interview 3.m4a"), None);
        assert_eq!(stamp("2026.m4a"), None);
        assert_eq!(stamp("tape_123456789.m4a"), None); // 9 digits: ambiguous
        assert_eq!(stamp("20261031202.m4a"), None); // 11 digits
        assert_eq!(stamp(""), None);
    }

    #[test]
    fn the_next_year_is_still_plausible() {
        assert_eq!(stamp("REC_20270105_101010.m4a").map(|s| s.0), Some(2027));
    }

    #[test]
    fn stored_forms() {
        let s = stamp_from_file_name("Voice 261111_090049.m4a", 2026).unwrap();
        assert_eq!(s.gedcom_date(), "11 NOV 2026");
        assert_eq!(s.dotted(), "11.11.2026");
        assert_eq!(s.time_text().as_deref(), Some("09:00:49"));
    }

    #[test]
    fn titles_follow_the_language() {
        let uk = recording_defaults("Voice 261111_090049.m4a", 2026, Language::Uk).unwrap();
        assert_eq!(uk.title, "Запис розмови 11.11.2026 (Voice 261111_090049)");
        assert_eq!(uk.date_raw, "11 NOV 2026");
        assert_eq!(
            uk.description.as_deref(),
            Some("Записано о 09:00:49 (за назвою файлу).")
        );
        let en = recording_defaults("Voice 261111_090049.m4a", 2026, Language::En).unwrap();
        assert_eq!(en.title, "Recording 11.11.2026 (Voice 261111_090049)");
        assert_eq!(
            en.description.as_deref(),
            Some("Recorded at 09:00:49 (from the file name).")
        );
        let no_time = recording_defaults("AUD-20261031-WA0003.m4a", 2026, Language::En).unwrap();
        assert_eq!(no_time.description, None);
        assert!(recording_defaults("story.m4a", 2026, Language::En).is_none());
    }

    #[test]
    fn year_of_a_unix_time() {
        assert_eq!(year_of_unix_ms(0), 1970);
        assert_eq!(year_of_unix_ms(1_793_000_000_000), 2026); // 2026-10-26
        assert_eq!(year_of_unix_ms(1_798_761_599_999), 2026); // 2026-12-31 23:59:59
        assert_eq!(year_of_unix_ms(1_798_761_600_000), 2027);
    }
}
