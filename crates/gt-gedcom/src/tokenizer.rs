//! Lenient, byte-level GEDCOM tokenizer. Never panics on any input.

use crate::encoding::{DetectedEncoding, decode_payload, detect, split_lines, to_line_bytes};
use crate::node::{GedNode, TokenIssue, Tokenized};

/// A node under construction; the value stays as raw bytes until CONC/CONT are joined.
struct Pending {
    line: u32,
    level: u8,
    xref: Option<String>,
    tag: String,
    value: Option<Vec<u8>>,
    children: Vec<Pending>,
}

struct Parsed<'a> {
    level: u8,
    xref: Option<&'a [u8]>,
    tag: &'a [u8],
    value: &'a [u8],
}

fn is_ws(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

/// Parse `<ws>level<ws>[@xref@<ws>]tag[ value]`. The value starts after ONE delimiter space.
fn parse_line(line: &[u8]) -> Option<Parsed<'_>> {
    let s = line.trim_ascii_start();
    let digits = s.iter().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 || digits > 3 {
        return None;
    }
    let level: u8 = std::str::from_utf8(s.get(..digits)?).ok()?.parse().ok()?;
    let mut rest = s.get(digits..)?;
    if !rest.first().is_some_and(|&b| is_ws(b)) {
        return None;
    }
    rest = rest.trim_ascii_start();
    let mut xref = None;
    if rest.first() == Some(&b'@') {
        let end = rest.get(1..)?.iter().position(|&b| b == b'@')? + 1;
        xref = Some(rest.get(..=end)?);
        rest = rest.get(end + 1..)?;
        if !rest.is_empty() && !rest.first().is_some_and(|&b| is_ws(b)) {
            return None;
        }
        rest = rest.trim_ascii_start();
    }
    let tag_len = rest.iter().take_while(|&&b| !is_ws(b)).count();
    if tag_len == 0 {
        return None;
    }
    let tag = rest.get(..tag_len)?;
    let mut value = rest.get(tag_len..)?;
    if value.first().is_some_and(|&b| is_ws(b)) {
        value = value.get(1..)?;
    }
    Some(Parsed {
        level,
        xref,
        tag,
        value,
    })
}

/// Pop the top of the stack and attach it to its parent (or to the records).
fn close_top(stack: &mut Vec<Pending>, records: &mut Vec<Pending>) {
    if let Some(done) = stack.pop() {
        match stack.last_mut() {
            Some(parent) => parent.children.push(done),
            None => records.push(done),
        }
    }
}

/// GEDCOM 7.0: only a `@@` at the start of a line value stands for `@` (also after a CONT).
fn unescape_leading(v: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len());
    let mut line_start = true;
    let mut i = 0;
    while let Some(&b) = v.get(i) {
        out.push(b);
        if line_start && b == b'@' && v.get(i + 1) == Some(&b'@') {
            i += 1;
        }
        line_start = b == b'\n';
        i += 1;
    }
    out
}

fn unescape_at(v: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len());
    let mut i = 0;
    while let Some(&b) = v.get(i) {
        out.push(b);
        if b == b'@' && v.get(i + 1) == Some(&b'@') {
            i += 1;
        }
        i += 1;
    }
    out
}

fn raw_is_pointer(v: &[u8]) -> bool {
    let v = v.trim_ascii();
    v.len() >= 3
        && v.first() == Some(&b'@')
        && v.last() == Some(&b'@')
        && v.get(1..v.len() - 1).is_some_and(|m| !m.contains(&b'@'))
}

fn finish(p: Pending, enc: &DetectedEncoding, v70: bool) -> GedNode {
    GedNode {
        line: p.line,
        level: p.level,
        xref: p.xref,
        tag: p.tag,
        ptr: p.value.as_deref().is_some_and(raw_is_pointer),
        value: p.value.map(|v| {
            let un = if v70 {
                unescape_leading(&v)
            } else {
                unescape_at(&v)
            };
            decode_payload(&un, enc)
        }),
        children: p
            .children
            .into_iter()
            .map(|c| finish(c, enc, v70))
            .collect(),
    }
}

pub fn tokenize(bytes: &[u8]) -> Tokenized {
    let encoding = detect(bytes);
    let body = to_line_bytes(bytes, &encoding);
    // UTF-16 input has been converted to UTF-8 already.
    let payload_enc = match encoding {
        DetectedEncoding::Utf16Le | DetectedEncoding::Utf16Be => {
            DetectedEncoding::Utf8 { bom: false }
        }
        e => e,
    };
    let mut issues = Vec::new();
    let mut records: Vec<Pending> = Vec::new();
    let mut stack: Vec<Pending> = Vec::new();

    for (raw, line_no) in split_lines(&body).into_iter().zip(1u32..) {
        if raw.trim_ascii().is_empty() {
            continue;
        }
        let Some(p) = parse_line(raw) else {
            issues.push(TokenIssue::BadLine {
                line: line_no,
                text: String::from_utf8_lossy(raw).into_owned(),
            });
            continue;
        };
        while stack.last().is_some_and(|t| t.level >= p.level) {
            close_top(&mut stack, &mut records);
        }
        let is_cont = p.tag.eq_ignore_ascii_case(b"CONT");
        if p.xref.is_none() && (is_cont || p.tag.eq_ignore_ascii_case(b"CONC")) {
            match stack.last_mut() {
                Some(parent) => {
                    let v = parent.value.get_or_insert_with(Vec::new);
                    if is_cont {
                        v.push(b'\n');
                    }
                    v.extend_from_slice(p.value);
                }
                None => issues.push(TokenIssue::OrphanConc { line: line_no }),
            }
            continue;
        }
        match stack.last() {
            Some(top) if p.level > top.level.saturating_add(1) => {
                issues.push(TokenIssue::LevelJump { line: line_no });
            }
            None if p.level > 0 => issues.push(TokenIssue::LevelJump { line: line_no }),
            _ => {}
        }
        stack.push(Pending {
            line: line_no,
            level: p.level,
            xref: p.xref.map(|x| String::from_utf8_lossy(x).into_owned()),
            tag: String::from_utf8_lossy(p.tag).into_owned(),
            value: (!p.value.is_empty()).then(|| p.value.to_vec()),
            children: Vec::new(),
        });
    }
    while !stack.is_empty() {
        close_top(&mut stack, &mut records);
    }
    // `@` escaping differs: 7.0 doubles only a leading `@`, 5.5.1 every `@`.
    let v70 = records
        .iter()
        .find(|r| r.tag.eq_ignore_ascii_case("HEAD"))
        .and_then(|h| {
            h.children
                .iter()
                .find(|c| c.tag.eq_ignore_ascii_case("GEDC"))
        })
        .and_then(|g| {
            g.children
                .iter()
                .find(|c| c.tag.eq_ignore_ascii_case("VERS"))
        })
        .and_then(|v| v.value.as_deref())
        .is_some_and(|v| v.trim_ascii_start().first() == Some(&b'7'));
    Tokenized {
        encoding,
        records: records
            .into_iter()
            .map(|r| finish(r, &payload_enc, v70))
            .collect(),
        issues,
    }
}
