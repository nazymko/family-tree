//! Times in a sound recording: milliseconds from its start, written as "h:mm:ss" or "m:ss".

use crate::model::Clip;

/// "m:ss" under an hour, "h:mm:ss" from an hour; tenths are dropped unless the time has them.
pub fn format_ms(ms: u64) -> String {
    let total = ms / 1000;
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    let frac = ms % 1000;
    let base = if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    };
    if frac == 0 {
        base
    } else {
        format!("{base}.{frac:03}")
    }
}

/// A clip as one line for history rows: "1:05–1:40 label".
pub fn clip_text(c: &Clip) -> String {
    let mut out = format!("{}–{}", format_ms(c.start_ms), format_ms(c.end_ms));
    if let Some(l) = c.label.as_deref().filter(|l| !l.is_empty()) {
        out.push(' ');
        out.push_str(l);
    }
    out
}

/// Reads "h:mm:ss", "mm:ss" or "m:ss", each optionally with ".fff" seconds, or a plain number of
/// milliseconds ("65000"). Minutes and seconds after the first part must be below 60.
pub fn parse_time(text: &str) -> Option<u64> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if t.bytes().all(|b| b.is_ascii_digit()) {
        return t.parse().ok();
    }
    let parts: Vec<&str> = t.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let (last, rest) = parts.split_last()?;
    let (sec, frac) = match last.split_once('.') {
        Some((s, f)) => (s, Some(f)),
        None => (*last, None),
    };
    let digits = |s: &str| !s.is_empty() && s.len() <= 9 && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(sec) || rest.iter().any(|p| !digits(p)) {
        return None;
    }
    let s: u64 = sec.parse().ok()?;
    if s >= 60 || sec.len() != 2 {
        return None;
    }
    let ms_part = match frac {
        None => 0,
        Some(f) if (1..=3).contains(&f.len()) && f.bytes().all(|b| b.is_ascii_digit()) => {
            let v: u64 = f.parse().ok()?;
            v * 10u64.pow(3 - f.len() as u32)
        }
        Some(_) => return None,
    };
    let mut total = s;
    match rest {
        [m] => total += m.parse::<u64>().ok()? * 60,
        [h, m] => {
            let m: u64 = m.parse().ok()?;
            if m >= 60 || rest[1].len() != 2 {
                return None;
            }
            total += h.parse::<u64>().ok()? * 3600 + m * 60;
        }
        _ => return None,
    }
    total.checked_mul(1000)?.checked_add(ms_part)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_parses() {
        assert_eq!(format_ms(0), "0:00");
        assert_eq!(format_ms(65_000), "1:05");
        assert_eq!(format_ms(3_725_000), "1:02:05");
        assert_eq!(format_ms(1_500), "0:01.500");
        assert_eq!(parse_time("1:05"), Some(65_000));
        assert_eq!(parse_time("01:05"), Some(65_000));
        assert_eq!(parse_time("1:02:05"), Some(3_725_000));
        assert_eq!(parse_time("0:01.5"), Some(1_500));
        assert_eq!(parse_time("65000"), Some(65_000));
        for bad in [
            "",
            "1:5",
            "1:60",
            "1:60:00",
            "a:00",
            "1:00:00:00",
            "-1:00",
            "1:00.1234",
        ] {
            assert_eq!(parse_time(bad), None, "{bad}");
        }
        for ms in [0, 999, 65_000, 3_725_042] {
            assert_eq!(parse_time(&format_ms(ms)), Some(ms));
        }
    }
}
