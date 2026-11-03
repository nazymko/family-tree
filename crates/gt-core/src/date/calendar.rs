//! Calendar conversions to Julian Day Numbers (integer, noon-based).
//!
//! Years are astronomical in this module (1 BCE = year 0).

/// Calendars supported by GEDCOM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Calendar {
    Gregorian,
    Julian,
    Hebrew,
    FrenchRepublican,
}

/// Years outside this range are rejected (keeps every JDN inside `i32`).
const MAX_ABS_YEAR: i32 = 1_000_000;

/// JDN of the Hebrew epoch (1 Tishri, year 1) minus the elapsed-days origin.
/// Reingold & Dershowitz: epoch = Julian 7 October 3761 BCE.
fn hebrew_epoch() -> i64 {
    julian_jdn(-3760, 10, 7)
}

/// JDN of 1 Vendemiaire, year 1 (22 September 1792 Gregorian).
fn french_epoch() -> i64 {
    gregorian_jdn(1792, 9, 22)
}

fn gregorian_jdn(y: i64, m: i64, d: i64) -> i64 {
    let a = (14 - m).div_euclid(12);
    let y2 = y + 4800 - a;
    let m2 = m + 12 * a - 3;
    d + (153 * m2 + 2).div_euclid(5) + 365 * y2 + y2.div_euclid(4) - y2.div_euclid(100)
        + y2.div_euclid(400)
        - 32045
}

fn julian_jdn(y: i64, m: i64, d: i64) -> i64 {
    let a = (14 - m).div_euclid(12);
    let y2 = y + 4800 - a;
    let m2 = m + 12 * a - 3;
    d + (153 * m2 + 2).div_euclid(5) + 365 * y2 + y2.div_euclid(4) - 32083
}

fn gregorian_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn julian_leap(y: i64) -> bool {
    y % 4 == 0
}

// ---------- Hebrew (Reingold & Dershowitz) ----------

fn hebrew_leap(y: i64) -> bool {
    (7 * y + 1).rem_euclid(19) < 7
}

fn hebrew_elapsed_days(y: i64) -> i64 {
    let months = (235 * y - 234).div_euclid(19);
    let parts = 12084 + 13753 * months;
    let days = 29 * months + parts.div_euclid(25920);
    if (3 * (days + 1)).rem_euclid(7) < 3 {
        days + 1
    } else {
        days
    }
}

fn hebrew_correction(y: i64) -> i64 {
    let ny0 = hebrew_elapsed_days(y - 1);
    let ny1 = hebrew_elapsed_days(y);
    let ny2 = hebrew_elapsed_days(y + 1);
    if ny2 - ny1 == 356 {
        2
    } else if ny1 - ny0 == 382 {
        1
    } else {
        0
    }
}

fn hebrew_new_year(y: i64) -> i64 {
    hebrew_epoch() + hebrew_elapsed_days(y) + hebrew_correction(y)
}

fn hebrew_year_days(y: i64) -> i64 {
    hebrew_new_year(y + 1) - hebrew_new_year(y)
}

/// Map a GEDCOM Hebrew month (TSH=1 .. ELL=13) to the R&D number (Nisan=1).
/// ADR (6) exists only in leap years; ADS (7) is Adar II in a leap year and
/// plain Adar otherwise.
fn hebrew_rd_month(y: i64, m: u8) -> Option<i64> {
    let leap = hebrew_leap(y);
    Some(match m {
        1 => 7,
        2 => 8,
        3 => 9,
        4 => 10,
        5 => 11,
        6 if leap => 12,
        7 if leap => 13,
        7 => 12,
        8 => 1,
        9 => 2,
        10 => 3,
        11 => 4,
        12 => 5,
        13 => 6,
        _ => return None,
    })
}

fn hebrew_rd_month_len(y: i64, rd_month: i64) -> i64 {
    let ylen = hebrew_year_days(y);
    match rd_month {
        2 | 4 | 6 | 10 | 13 => 29,
        12 if !hebrew_leap(y) => 29,
        8 if !matches!(ylen, 355 | 385) => 29,
        9 if matches!(ylen, 353 | 383) => 29,
        _ => 30,
    }
}

fn hebrew_month_start(y: i64, m: u8) -> Option<i64> {
    let target = hebrew_rd_month(y, m)?;
    let mut jdn = hebrew_new_year(y);
    // Walk the months of the year in order: Tishri(7)..last, then Nisan(1)..Elul(6).
    let last = if hebrew_leap(y) { 13 } else { 12 };
    let order = (7..=last).chain(1..=6);
    for rd in order {
        if rd == target {
            return Some(jdn);
        }
        jdn += hebrew_rd_month_len(y, rd);
    }
    None
}

// ---------- French Republican ----------

fn french_leap(y: i64) -> bool {
    if y <= 14 {
        matches!(y, 3 | 7 | 11)
    } else {
        // Romme rule.
        gregorian_leap(y)
    }
}

/// Number of leap years in `1..y` (strictly before `y`).
fn french_leaps_before(y: i64) -> i64 {
    let hist = [3, 7, 11].iter().filter(|&&l| l < y).count() as i64;
    if y <= 15 {
        return hist;
    }
    let f = |n: i64| n / 4 - n / 100 + n / 400;
    hist + (f(y - 1) - f(14))
}

fn french_year_start(y: i64) -> i64 {
    french_epoch() + 365 * (y - 1) + french_leaps_before(y)
}

// ---------- public API ----------

/// Number of months in the year (Hebrew leap year = 13; French = 13 incl.
/// the complementary days).
pub fn months_in_year(cal: Calendar, year: i32) -> u8 {
    match cal {
        Calendar::Gregorian | Calendar::Julian => 12,
        Calendar::Hebrew => {
            if hebrew_leap(i64::from(year)) {
                13
            } else {
                12
            }
        }
        Calendar::FrenchRepublican => 13,
    }
}

/// Days in the given month (GEDCOM numbering), or `None` if the month does not exist.
pub fn days_in_month(cal: Calendar, year: i32, month: u8) -> Option<u8> {
    if year.abs() > MAX_ABS_YEAR {
        return None;
    }
    let y = i64::from(year);
    match cal {
        Calendar::Gregorian | Calendar::Julian => {
            let leap = if cal == Calendar::Gregorian {
                gregorian_leap(y)
            } else {
                julian_leap(y)
            };
            Some(match month {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                2 if leap => 29,
                2 => 28,
                _ => return None,
            })
        }
        Calendar::Hebrew => {
            if y < 1 {
                return None;
            }
            let rd = hebrew_rd_month(y, month)?;
            u8::try_from(hebrew_rd_month_len(y, rd)).ok()
        }
        Calendar::FrenchRepublican => {
            if y < 1 {
                return None;
            }
            match month {
                1..=12 => Some(30),
                13 => Some(if french_leap(y) { 6 } else { 5 }),
                _ => None,
            }
        }
    }
}

fn first_month(_cal: Calendar) -> u8 {
    1
}

fn last_month(cal: Calendar) -> u8 {
    match cal {
        Calendar::Gregorian | Calendar::Julian => 12,
        Calendar::Hebrew | Calendar::FrenchRepublican => 13,
    }
}

fn jdn_of(cal: Calendar, year: i32, month: u8, day: u8) -> Option<i32> {
    let dim = days_in_month(cal, year, month)?;
    if day == 0 || day > dim {
        return None;
    }
    let y = i64::from(year);
    let (m, d) = (i64::from(month), i64::from(day));
    let jdn = match cal {
        Calendar::Gregorian => gregorian_jdn(y, m, d),
        Calendar::Julian => julian_jdn(y, m, d),
        Calendar::Hebrew => hebrew_month_start(y, month)? + d - 1,
        Calendar::FrenchRepublican => french_year_start(y) + 30 * (m - 1) + d - 1,
    };
    i32::try_from(jdn).ok()
}

/// First JDN covered by the (possibly partial) date. Missing day -> first day
/// of the month; missing month -> first month of the year. `None` if invalid.
pub fn first_jdn(cal: Calendar, year: i32, month: Option<u8>, day: Option<u8>) -> Option<i32> {
    let m = month.unwrap_or_else(|| first_month(cal));
    jdn_of(cal, year, m, day.unwrap_or(1))
}

/// Last JDN covered by the (possibly partial) date. Missing day -> last day
/// of the month; missing month -> last month of the year. `None` if invalid.
pub fn last_jdn(cal: Calendar, year: i32, month: Option<u8>, day: Option<u8>) -> Option<i32> {
    let m = month.unwrap_or_else(|| last_month(cal));
    let d = match day {
        Some(d) => d,
        None => days_in_month(cal, year, m)?,
    };
    jdn_of(cal, year, m, d)
}
