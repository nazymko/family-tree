//! Hand-written tokenizer and parser for genealogical dates. Never panics.

use super::calendar::Calendar;
use super::format::{FRENCH_CODES, GREGORIAN_CODES, HEBREW_CODES};
use super::{Date, DateValue, DayOrder, GenDate};

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(String),
    Word(String),
    Slash,
    Dash,
    Tilde,
    Question,
    Other,
}

pub(super) fn parse(input: &str, order: DayOrder) -> GenDate {
    let unparsed = || GenDate::from_value(input, DateValue::Unparsed, None);
    let text = input.trim();

    // Text wholly in parentheses: phrase only.
    if text.starts_with('(') && text.ends_with(')') && text.len() >= 2 {
        let inner = text
            .get(1..text.len() - 1)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        return GenDate::from_value(input, DateValue::PhraseOnly, inner);
    }

    // Split off a trailing "(phrase)".
    let (head, phrase) = match text.find('(') {
        Some(open) => {
            let Some(close) = text.rfind(')') else {
                return unparsed();
            };
            if close < open || !text.get(close + 1..).is_some_and(|t| t.trim().is_empty()) {
                return unparsed();
            }
            let inner = text
                .get(open + 1..close)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            (text.get(..open).unwrap_or(""), inner)
        }
        None => (text, None),
    };

    let toks = lex(head);
    let mut p = Parser {
        t: &toks,
        i: 0,
        order,
    };
    match p.value() {
        Some(value) if p.i == toks.len() => GenDate::from_value(input, value, phrase),
        _ => unparsed(),
    }
}

fn lex(s: &str) -> Vec<Tok> {
    let upper = s.to_uppercase().replace("@#DFRENCH R@", "@#DFRENCH_R@");
    let mut out = Vec::new();
    let mut chars = upper.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            let mut n = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() {
                    n.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            out.push(Tok::Num(n));
        } else if c.is_alphabetic() || matches!(c, '.' | '_' | '@' | '#' | '\'') {
            let mut w = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_alphabetic() || matches!(d, '.' | '_' | '@' | '#' | '\'') {
                    if d != '.' {
                        w.push(d);
                    }
                    chars.next();
                } else {
                    break;
                }
            }
            if !w.is_empty() {
                out.push(Tok::Word(w));
            }
        } else {
            chars.next();
            match c {
                c if c.is_whitespace() || c == ',' => {}
                '/' => out.push(Tok::Slash),
                '-' | '\u{2013}' | '\u{2014}' => out.push(Tok::Dash),
                '~' => out.push(Tok::Tilde),
                '?' => out.push(Tok::Question),
                _ => out.push(Tok::Other),
            }
        }
    }
    out
}

struct Parser<'a> {
    t: &'a [Tok],
    i: usize,
    order: DayOrder,
}

fn num(s: &str) -> Option<i32> {
    if s.len() > 7 {
        return None;
    }
    s.parse().ok()
}

fn calendar_word(w: &str) -> Option<Option<Calendar>> {
    // Some(Some(c)) = calendar; Some(None) = unsupported calendar; None = not a calendar word.
    let name = w
        .strip_prefix("@#D")
        .and_then(|r| r.strip_suffix('@'))
        .unwrap_or(w);
    let known = match name {
        "GREGORIAN" => Some(Calendar::Gregorian),
        "JULIAN" => Some(Calendar::Julian),
        "HEBREW" => Some(Calendar::Hebrew),
        "FRENCH_R" | "FRENCH" => Some(Calendar::FrenchRepublican),
        _ => None,
    };
    if known.is_some() {
        Some(known)
    } else if name.len() != w.len() {
        Some(None) // an escape for a calendar we do not support (ROMAN, UNKNOWN)
    } else {
        None
    }
}

fn english_month(w: &str) -> Option<u8> {
    let full = [
        "JANUARY",
        "FEBRUARY",
        "MARCH",
        "APRIL",
        "MAY",
        "JUNE",
        "JULY",
        "AUGUST",
        "SEPTEMBER",
        "OCTOBER",
        "NOVEMBER",
        "DECEMBER",
    ];
    if w == "SEPT" {
        return Some(9);
    }
    GREGORIAN_CODES
        .iter()
        .position(|c| *c == w)
        .or_else(|| full.iter().position(|c| *c == w))
        .and_then(|i| u8::try_from(i + 1).ok())
}

fn code_month(codes: &[&str], w: &str) -> Option<u8> {
    codes
        .iter()
        .position(|c| *c == w)
        .and_then(|i| u8::try_from(i + 1).ok())
}

/// Month word in the given (or any) calendar. Returns the month and the
/// calendar it implies.
fn month_word(w: &str, cal: Option<Calendar>) -> Option<(u8, Calendar)> {
    match cal {
        Some(c @ (Calendar::Gregorian | Calendar::Julian)) => english_month(w).map(|m| (m, c)),
        Some(Calendar::Hebrew) => code_month(&HEBREW_CODES, w).map(|m| (m, Calendar::Hebrew)),
        Some(Calendar::FrenchRepublican) => {
            code_month(&FRENCH_CODES, w).map(|m| (m, Calendar::FrenchRepublican))
        }
        None => english_month(w)
            .map(|m| (m, Calendar::Gregorian))
            .or_else(|| code_month(&HEBREW_CODES, w).map(|m| (m, Calendar::Hebrew)))
            .or_else(|| code_month(&FRENCH_CODES, w).map(|m| (m, Calendar::FrenchRepublican))),
    }
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }

    fn peek_at(&self, n: usize) -> Option<&Tok> {
        self.t.get(self.i + n)
    }

    fn word_is(&self, words: &[&str]) -> bool {
        matches!(self.peek(), Some(Tok::Word(w)) if words.contains(&w.as_str()))
    }

    fn value(&mut self) -> Option<DateValue> {
        use DateValue::*;
        if matches!(self.peek(), Some(Tok::Tilde)) {
            self.i += 1;
            return Some(About(self.date()?));
        }
        let kw = match self.peek() {
            Some(Tok::Word(w)) => w.clone(),
            _ => String::new(),
        };
        let v = match kw.as_str() {
            "ABT" | "ABOUT" | "C" | "CA" | "CIRCA" => {
                self.i += 1;
                About(self.date()?)
            }
            "CAL" | "CALCULATED" => {
                self.i += 1;
                Calculated(self.date()?)
            }
            "EST" | "ESTIMATED" => {
                self.i += 1;
                Estimated(self.date()?)
            }
            "BEF" | "BEFORE" => {
                self.i += 1;
                Before(self.date()?)
            }
            "AFT" | "AFTER" => {
                self.i += 1;
                After(self.date()?)
            }
            "INT" => {
                self.i += 1;
                Interpreted(self.date()?)
            }
            "TO" => {
                self.i += 1;
                To(self.date()?)
            }
            "FROM" => {
                self.i += 1;
                let a = self.date()?;
                if self.word_is(&["TO"]) {
                    self.i += 1;
                    FromTo(a, self.date()?)
                } else {
                    From(a)
                }
            }
            "BET" | "BETWEEN" => {
                self.i += 1;
                let a = self.date()?;
                if !self.word_is(&["AND"]) {
                    return None;
                }
                self.i += 1;
                Between(a, self.date()?)
            }
            _ => {
                let a = self.date()?;
                if matches!(self.peek(), Some(Tok::Dash)) {
                    // "1900-1910": two bare years.
                    self.i += 1;
                    let b = self.date()?;
                    if a.month.is_some() || b.month.is_some() {
                        return None;
                    }
                    Between(a, b)
                } else if matches!(self.peek(), Some(Tok::Question)) {
                    self.i += 1;
                    About(a)
                } else {
                    Exact(a)
                }
            }
        };
        // A trailing "?" after a keyword form is tolerated and ignored.
        if matches!(self.peek(), Some(Tok::Question)) {
            self.i += 1;
        }
        Some(v)
    }

    fn date(&mut self) -> Option<Date> {
        let mut cal: Option<Calendar> = None;
        if let Some(Tok::Word(w)) = self.peek()
            && let Some(c) = calendar_word(w)
        {
            cal = Some(c?);
            self.i += 1;
        }

        let date = self
            .iso(cal)
            .or_else(|| self.numeric(cal))
            .or_else(|| self.textual(cal))?;
        Self::validate(date)
    }

    fn validate(date: Date) -> Option<Date> {
        if date.year < 1 || date.year > 999_999 {
            return None;
        }
        date.jdn_range()?;
        Some(date)
    }

    fn plain(cal: Option<Calendar>, year: i32, month: u8, day: u8) -> Date {
        Date {
            calendar: cal.unwrap_or(Calendar::Gregorian),
            year,
            bce: false,
            month: Some(month),
            day: Some(day),
            dual_year: None,
        }
    }

    /// `YYYY-MM-DD`
    fn iso(&mut self, cal: Option<Calendar>) -> Option<Date> {
        let (
            Some(Tok::Num(y)),
            Some(Tok::Dash),
            Some(Tok::Num(m)),
            Some(Tok::Dash),
            Some(Tok::Num(d)),
        ) = (
            self.peek(),
            self.peek_at(1),
            self.peek_at(2),
            self.peek_at(3),
            self.peek_at(4),
        )
        else {
            return None;
        };
        if y.len() != 4 || m.len() > 2 || d.len() > 2 {
            return None;
        }
        let date = Self::plain(
            cal,
            num(y)?,
            u8::try_from(num(m)?).ok()?,
            u8::try_from(num(d)?).ok()?,
        );
        self.i += 5;
        Some(date)
    }

    /// `NN/NN/YYYY`, read according to the configured day order.
    fn numeric(&mut self, cal: Option<Calendar>) -> Option<Date> {
        let (
            Some(Tok::Num(a)),
            Some(Tok::Slash),
            Some(Tok::Num(b)),
            Some(Tok::Slash),
            Some(Tok::Num(y)),
        ) = (
            self.peek(),
            self.peek_at(1),
            self.peek_at(2),
            self.peek_at(3),
            self.peek_at(4),
        )
        else {
            return None;
        };
        if y.len() != 4 || a.len() > 2 || b.len() > 2 {
            return None;
        }
        let (a, b) = (u8::try_from(num(a)?).ok()?, u8::try_from(num(b)?).ok()?);
        let (day, month) = match self.order {
            DayOrder::Dmy => (a, b),
            DayOrder::Mdy => (b, a),
        };
        let date = Self::plain(cal, num(y)?, month, day);
        self.i += 5;
        Some(date)
    }

    /// `[day] [month] year [/yy] [BC]`
    fn textual(&mut self, mut cal: Option<Calendar>) -> Option<Date> {
        let mut day = None;
        let mut month = None;

        if let (Some(Tok::Num(n)), Some(Tok::Word(w))) = (self.peek(), self.peek_at(1))
            && n.len() <= 2
            && let Some((m, c)) = month_word(w, cal)
        {
            day = Some(u8::try_from(num(n)?).ok()?);
            month = Some(m);
            cal = Some(c);
            self.i += 2;
        } else if let Some(Tok::Word(w)) = self.peek()
            && let Some((m, c)) = month_word(w, cal)
        {
            month = Some(m);
            cal = Some(c);
            self.i += 1;
        }

        let Some(Tok::Num(y)) = self.peek() else {
            return None;
        };
        let year = num(y)?;
        self.i += 1;

        let mut dual_year = None;
        if let (Some(Tok::Slash), Some(Tok::Num(d2))) = (self.peek(), self.peek_at(1)) {
            let second = num(d2)?;
            let full = match d2.len() {
                2 => {
                    let candidate = year - year % 100 + second;
                    if candidate <= year {
                        candidate + 100
                    } else {
                        candidate
                    }
                }
                4 => second,
                _ => return None,
            };
            if full != year + 1 {
                return None;
            }
            dual_year = Some(full);
            self.i += 2;
        }

        let mut bce = false;
        if self.word_is(&["BC", "BCE"]) {
            bce = true;
            self.i += 1;
        }

        Some(Date {
            calendar: cal.unwrap_or(Calendar::Gregorian),
            year,
            bce,
            month,
            day,
            dual_year,
        })
    }
}
