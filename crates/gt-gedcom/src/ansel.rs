//! ANSEL (ANSI Z39.47 / MARC-8 Latin) decoding.

use unicode_normalization::UnicodeNormalization;

/// Map a single non-combining ANSEL byte (>= 0x80) to a char.
fn special(b: u8) -> Option<char> {
    let c = match b {
        0xA1 => '\u{0141}',
        0xA2 => '\u{00D8}',
        0xA3 => '\u{0110}',
        0xA4 => '\u{00DE}',
        0xA5 => '\u{00C6}',
        0xA6 => '\u{0152}',
        0xA7 => '\u{02B9}',
        0xA8 => '\u{00B7}',
        0xA9 => '\u{266D}',
        0xAA => '\u{00AE}',
        0xAB => '\u{00B1}',
        0xAC => '\u{01A0}',
        0xAD => '\u{01AF}',
        0xAE => '\u{02BC}',
        0xB0 => '\u{02BB}',
        0xB1 => '\u{0142}',
        0xB2 => '\u{00F8}',
        0xB3 => '\u{0111}',
        0xB4 => '\u{00FE}',
        0xB5 => '\u{00E6}',
        0xB6 => '\u{0153}',
        0xB7 => '\u{02BA}',
        0xB8 => '\u{0131}',
        0xB9 => '\u{00A3}',
        0xBA => '\u{00F0}',
        0xBC => '\u{01A1}',
        0xBD => '\u{01B0}',
        0xC0 => '\u{00B0}',
        0xC1 => '\u{2113}',
        0xC2 => '\u{2117}',
        0xC3 => '\u{00A9}',
        0xC4 => '\u{266F}',
        0xC5 => '\u{00BF}',
        0xC6 => '\u{00A1}',
        0xC7 => '\u{00DF}',
        0xC8 => '\u{20AC}',
        _ => return None,
    };
    Some(c)
}

/// Map a combining ANSEL byte (0xE0..=0xFE) to its Unicode combining mark.
fn combining(b: u8) -> Option<char> {
    let c = match b {
        0xE0 => '\u{0309}',
        0xE1 => '\u{0300}',
        0xE2 => '\u{0301}',
        0xE3 => '\u{0302}',
        0xE4 => '\u{0303}',
        0xE5 => '\u{0304}',
        0xE6 => '\u{0306}',
        0xE7 => '\u{0307}',
        0xE8 => '\u{0308}',
        0xE9 => '\u{030C}',
        0xEA => '\u{030A}',
        0xEB => '\u{FE20}',
        0xEC => '\u{FE21}',
        0xED => '\u{0315}',
        0xEE => '\u{030B}',
        0xEF => '\u{0310}',
        0xF0 => '\u{0327}',
        0xF1 => '\u{0328}',
        0xF2 => '\u{0323}',
        0xF3 => '\u{0324}',
        0xF4 => '\u{0325}',
        0xF5 => '\u{0333}',
        0xF6 => '\u{0332}',
        0xF7 => '\u{0326}',
        0xF8 => '\u{031C}',
        0xF9 => '\u{032E}',
        0xFA => '\u{FE22}',
        0xFB => '\u{FE23}',
        0xFE => '\u{0313}',
        _ => return None,
    };
    Some(c)
}

/// Decode ANSEL bytes. Combining marks precede the base letter in ANSEL; they are
/// emitted after it, then the result is NFC-normalised.
pub fn decode_ansel(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    let mut marks: Vec<char> = Vec::new();
    for &b in bytes {
        if let Some(m) = combining(b) {
            marks.push(m);
            continue;
        }
        let base = if b < 0x80 {
            char::from(b)
        } else {
            special(b).unwrap_or('\u{FFFD}')
        };
        out.push(base);
        out.extend(marks.drain(..));
    }
    out.extend(marks.drain(..));
    out.nfc().collect()
}
