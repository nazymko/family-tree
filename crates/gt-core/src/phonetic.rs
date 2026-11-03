//! Phonetic keys for surname matching: Soundex and Daitch-Mokotoff.
//!
//! Names are folded to plain ASCII letters first (`Müller` -> `Muller`).
//! Names without any Latin letter yield no key.

use rphonetic::{DaitchMokotoffSoundex, Encoder, Soundex};
use std::sync::OnceLock;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

/// Fold to uppercase ASCII letters only: strip diacritics, expand common
/// ligatures and drop everything that is not a Latin letter.
fn fold(name: &str) -> String {
    let mut out = String::new();
    for c in name.nfd().filter(|c| !is_combining_mark(*c)) {
        match c {
            'ß' => out.push_str("SS"),
            'æ' | 'Æ' => out.push_str("AE"),
            'œ' | 'Œ' => out.push_str("OE"),
            'ø' | 'Ø' => out.push('O'),
            'ł' | 'Ł' => out.push('L'),
            'đ' | 'Đ' => out.push('D'),
            'þ' | 'Þ' => out.push_str("TH"),
            c if c.is_ascii_alphabetic() => out.push(c.to_ascii_uppercase()),
            _ => {}
        }
    }
    out
}

/// American Soundex code (`Robert` -> `R163`), or `None` if the name has no Latin letters.
pub fn soundex(name: &str) -> Option<String> {
    let folded = fold(name);
    if folded.is_empty() {
        return None;
    }
    Some(Soundex::default().encode(&folded))
}

/// Daitch-Mokotoff codes (several when a name branches), empty if the name has no Latin letters.
pub fn daitch_mokotoff(name: &str) -> Vec<String> {
    static ENCODER: OnceLock<DaitchMokotoffSoundex> = OnceLock::new();
    let folded = fold(name);
    if folded.is_empty() {
        return Vec::new();
    }
    let encoder = ENCODER.get_or_init(DaitchMokotoffSoundex::default);
    let mut codes = encoder.inner_soundex(&folded, true);
    codes.sort();
    codes.dedup();
    codes
}
