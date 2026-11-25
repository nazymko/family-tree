//! Loose text folding so "Іванівка", "Ivanivka", "Iwaniwka" and "IVANIVKA" meet.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Ukrainian national transliteration (2010), with the Russian letters that appear in old names.
fn translit(c: char) -> Option<&'static str> {
    Some(match c {
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "h",
        'ґ' => "g",
        'д' => "d",
        'е' => "e",
        'є' => "ie",
        'ж' => "zh",
        'з' => "z",
        'и' => "y",
        'і' => "i",
        'ї' => "i",
        'й' => "i",
        'к' => "k",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'с' => "s",
        'т' => "t",
        'у' => "u",
        'ф' => "f",
        'х' => "kh",
        'ц' => "ts",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "shch",
        'ю' => "iu",
        'я' => "ia",
        'ы' => "y",
        'э' => "e",
        'ё' => "e",
        'ь' | 'ъ' | '\'' | '’' | 'ʼ' | '`' => "",
        'ł' => "l",
        'đ' => "d",
        'ø' => "o",
        'ß' => "ss",
        'æ' => "ae",
        _ => return None,
    })
}

/// True when the text has a Cyrillic letter.
pub fn has_cyrillic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}

/// True for a name that is clearly Ukrainian (has one of і ї є ґ).
pub fn is_ukrainian(s: &str) -> bool {
    s.chars()
        .any(|c| matches!(c.to_lowercase().next(), Some('і' | 'ї' | 'є' | 'ґ')))
}

/// Lower case, no diacritics, Cyrillic to Latin, then a loose phonetic reduction: y j to i,
/// h kh to g, w to v, x to ks, c q to k, doubled letters collapse. Words are separated by one space.
pub fn fold(s: &str) -> String {
    let mut latin = String::with_capacity(s.len() + 4);
    for c in s.nfkd().filter(|c| !is_combining_mark(*c)) {
        for lc in c.to_lowercase() {
            match translit(lc) {
                Some(t) => latin.push_str(t),
                None if lc.is_alphanumeric() => latin.push(lc),
                None => latin.push(' '),
            }
        }
    }
    let latin = latin.replace("shch", "sh").replace("kh", "h");
    let mut out = String::with_capacity(latin.len());
    let mut last = '\0';
    for c in latin.chars() {
        if c == 'x' {
            out.push_str("ks");
            last = 's';
            continue;
        }
        let c = match c {
            'y' | 'j' => 'i',
            'w' => 'v',
            'h' => 'g',
            'q' | 'c' => 'k',
            other => other,
        };
        if c == last && c != ' ' {
            continue;
        }
        if c == ' ' && (last == ' ' || out.is_empty()) {
            continue;
        }
        out.push(c);
        last = c;
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Words that name a kind of place or division, not the place; dropped before comparing hierarchy parts.
const GENERIC: &[&str] = &[
    "oblast",
    "obl",
    "oblasti",
    "raion",
    "raon",
    "region",
    "district",
    "county",
    "gmina",
    "powiat",
    "voivodeship",
    "province",
    "krai",
    "guberniia",
    "gubernia",
    "uezd",
    "volost",
    "selo",
    "village",
    "town",
    "city",
    "derevnia",
    "mistechko",
    "selishche",
    "of",
    "the",
    "rural",
    "urban",
    "parish",
    "parafiia",
];

/// `fold` without the generic words and one-letter abbreviations ("с.", "м.", "обл.").
pub fn fold_name(s: &str) -> String {
    let f = fold(s);
    let words: Vec<&str> = f
        .split(' ')
        .filter(|w| w.chars().count() > 1 && !GENERIC.contains(w))
        .collect();
    if words.is_empty() {
        return f;
    }
    let words: Vec<&str> = words.into_iter().map(strip_adjective).collect();
    words.join(" ")
}

/// "kivska" and "kiv" are the same place: the adjective endings of division names (-ська, -ський, -ське) go.
fn strip_adjective(w: &str) -> &str {
    for suffix in ["ska", "ske", "ski", "skoi"] {
        if let Some(stem) = w.strip_suffix(suffix)
            && stem.chars().count() >= 3
        {
            return stem;
        }
    }
    w
}

/// True when `a` and `b` differ by at most one inserted, deleted or replaced letter (or one swap).
pub fn within_one_edit(a: &str, b: &str) -> bool {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let mut i = 0;
    while i < a.len() && i < b.len() && a.get(i) == b.get(i) {
        i += 1;
    }
    if i == a.len() && i == b.len() {
        return true;
    }
    let ra = a.get(i + 1..).unwrap_or_default();
    let rb = b.get(i + 1..).unwrap_or_default();
    match a.len().cmp(&b.len()) {
        std::cmp::Ordering::Equal => {
            ra == rb
                || (a.get(i) == b.get(i + 1)
                    && a.get(i + 1) == b.get(i)
                    && a.get(i + 2..) == b.get(i + 2..))
        }
        std::cmp::Ordering::Greater => ra == b.get(i..).unwrap_or_default(),
        std::cmp::Ordering::Less => a.get(i..).unwrap_or_default() == rb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spellings_meet() {
        let a = fold("Іванівка");
        assert_eq!(a, fold("Ivanivka"));
        assert_eq!(a, fold("IWANIWKA"));
        assert_eq!(fold("Kyiv"), fold("Kyjiv"));
        assert_eq!(fold("Харків"), fold("Kharkiv"));
        assert_eq!(fold("Zhytomyr"), fold("Житомир"));
    }

    #[test]
    fn diacritics_and_punctuation_go() {
        assert_eq!(fold("Łódź"), "lodz");
        assert_eq!(fold("Bila-Tserkva"), fold("bila tserkva"));
        assert_eq!(fold("  Mala   Vyska "), "mala viska");
        assert_eq!(fold("Дніпро"), fold("Dnipro"));
        assert_eq!(fold("Кам'янець"), fold("Kamianets"));
    }

    #[test]
    fn generic_words_dropped() {
        assert_eq!(fold_name("Київська область"), fold_name("Kyiv oblast"));
        assert_eq!(fold_name("с. Іванівка"), fold_name("Ivanivka"));
        assert_eq!(fold_name("oblast"), "oblast");
    }

    #[test]
    fn one_edit() {
        assert!(within_one_edit("ivanivka", "ivanivka"));
        assert!(within_one_edit("ivanivka", "ivanivkaa"));
        assert!(within_one_edit("ivanivka", "ivanivla"));
        assert!(within_one_edit("ivanivka", "ianivka"));
        assert!(within_one_edit("ivanivka", "ivnaivka"));
        assert!(!within_one_edit("ivanivka", "ivanovkaxx"));
        assert!(!within_one_edit("ivanivka", "petrivka"));
    }

    #[test]
    fn ukrainian_detection() {
        assert!(is_ukrainian("Київ"));
        assert!(!is_ukrainian("Киев"));
        assert!(has_cyrillic("Киев"));
        assert!(!has_cyrillic("Kyiv"));
    }
}
