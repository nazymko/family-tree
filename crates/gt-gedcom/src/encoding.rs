//! Character-encoding detection and payload decoding.

use std::borrow::Cow;

use crate::ansel::decode_ansel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedEncoding {
    Utf8 { bom: bool },
    Utf16Le,
    Utf16Be,
    Ansel,
    Windows1252,
    Other(&'static encoding_rs::Encoding),
}

/// Split bytes into lines on CR, LF or CRLF. A terminator at EOF adds no empty line.
pub(crate) fn split_lines(bytes: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while let Some(&b) = bytes.get(i) {
        if b == b'\r' || b == b'\n' {
            lines.push(bytes.get(start..i).unwrap_or(&[]));
            if b == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                i += 1;
            }
            start = i + 1;
        }
        i += 1;
    }
    if let Some(rest) = bytes.get(start..)
        && !rest.is_empty()
    {
        lines.push(rest);
    }
    lines
}

/// Find the value of the header's `1 CHAR` line, upper-cased.
fn char_value(bytes: &[u8]) -> Option<String> {
    for line in split_lines(bytes).into_iter().take(500) {
        let t = line.trim_ascii_start();
        let mut parts = t.splitn(3, |b| b.is_ascii_whitespace());
        let level = parts.next().unwrap_or(&[]);
        let tag = parts.next().unwrap_or(&[]);
        if level == b"0" && !tag.starts_with(b"HEAD") {
            return None; // past the header
        }
        if level == b"1" && tag.eq_ignore_ascii_case(b"CHAR") {
            let rest = parts.next().unwrap_or(&[]);
            return Some(String::from_utf8_lossy(rest.trim_ascii()).to_ascii_uppercase());
        }
    }
    None
}

/// Detection order: BOM; UTF-16 by NUL-byte pattern; `1 CHAR` header value; UTF-8 validity;
/// otherwise Windows-1252. CHAR is not trusted when the bytes contradict it.
pub fn detect(bytes: &[u8]) -> DetectedEncoding {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return DetectedEncoding::Utf8 { bom: true };
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return DetectedEncoding::Utf16Le;
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return DetectedEncoding::Utf16Be;
    }
    match (bytes.first(), bytes.get(1)) {
        (Some(0), Some(b)) if *b != 0 => return DetectedEncoding::Utf16Be,
        (Some(a), Some(0)) if *a != 0 => return DetectedEncoding::Utf16Le,
        _ => {}
    }
    let multibyte_utf8 = !bytes.is_ascii() && std::str::from_utf8(bytes).is_ok();
    let fallback = if bytes.is_ascii() || multibyte_utf8 {
        DetectedEncoding::Utf8 { bom: false }
    } else {
        DetectedEncoding::Windows1252
    };
    match char_value(bytes).as_deref() {
        Some("ANSEL") if !multibyte_utf8 => DetectedEncoding::Ansel,
        Some("ANSI" | "IBMPC" | "IBM" | "WINDOWS") if !multibyte_utf8 => {
            DetectedEncoding::Windows1252
        }
        _ => fallback,
    }
}

/// UTF-16 is converted to UTF-8 bytes; a UTF-8 BOM is stripped; others pass through.
pub fn to_line_bytes<'a>(bytes: &'a [u8], enc: &DetectedEncoding) -> Cow<'a, [u8]> {
    match enc {
        DetectedEncoding::Utf16Le | DetectedEncoding::Utf16Be => {
            let e = if *enc == DetectedEncoding::Utf16Le {
                encoding_rs::UTF_16LE
            } else {
                encoding_rs::UTF_16BE
            };
            let (s, _) = e.decode_with_bom_removal(bytes);
            Cow::Owned(s.into_owned().into_bytes())
        }
        DetectedEncoding::Utf8 { bom: true } => {
            Cow::Borrowed(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes))
        }
        _ => Cow::Borrowed(bytes),
    }
}

/// Decode one payload (already passed through [`to_line_bytes`]).
pub fn decode_payload(bytes: &[u8], enc: &DetectedEncoding) -> String {
    match enc {
        DetectedEncoding::Utf8 { .. } | DetectedEncoding::Utf16Le | DetectedEncoding::Utf16Be => {
            String::from_utf8_lossy(bytes).into_owned()
        }
        DetectedEncoding::Ansel => decode_ansel(bytes),
        DetectedEncoding::Windows1252 => encoding_rs::WINDOWS_1252
            .decode_without_bom_handling(bytes)
            .0
            .into_owned(),
        DetectedEncoding::Other(e) => e.decode_without_bom_handling(bytes).0.into_owned(),
    }
}
