//! Serialise `GedNode` trees to GEDCOM text.

use crate::import::is_pointer;
use crate::node::GedNode;

/// Longest line in 5.5.1 output, in bytes (without the line terminator).
const MAX_LINE_551: usize = 255;

pub fn write(nodes: &[GedNode], v70: bool) -> Vec<u8> {
    let mut out = String::new();
    let eol = if v70 { "\n" } else { "\r\n" };
    for n in nodes {
        write_node(&mut out, n, 0, v70, eol);
    }
    out.into_bytes()
}

fn escape(v: &str) -> String {
    if is_pointer(v) {
        v.to_string()
    } else {
        v.replace('@', "@@")
    }
}

fn push_line(
    out: &mut String,
    level: usize,
    xref: Option<&str>,
    tag: &str,
    value: &str,
    eol: &str,
) {
    out.push_str(&level.to_string());
    out.push(' ');
    if let Some(x) = xref {
        out.push_str(x);
        out.push(' ');
    }
    out.push_str(tag);
    if !value.is_empty() {
        out.push(' ');
        out.push_str(value);
    }
    out.push_str(eol);
}

/// Split `s` into pieces of at most `max` bytes (the first `first_max`), never inside a UTF-8
/// character, a doubled `@`, and preferably not next to whitespace.
fn chunks(s: &str, first_max: usize) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = s;
    let mut max = first_max;
    while !rest.is_empty() {
        if rest.len() <= max {
            out.push(rest);
            break;
        }
        let bytes = rest.as_bytes();
        let ok = |i: usize, strict: bool| {
            if !rest.is_char_boundary(i) || i == 0 {
                return false;
            }
            let (a, b) = (bytes.get(i - 1).copied(), bytes.get(i).copied());
            if a == Some(b'@') && b == Some(b'@') {
                return false;
            }
            !strict || (a != Some(b' ') && b != Some(b' '))
        };
        let top = max.min(rest.len());
        let lo = top.saturating_sub(24).max(1);
        let cut = (lo..=top)
            .rev()
            .find(|&i| ok(i, true))
            .or_else(|| (1..=top).rev().find(|&i| ok(i, false)))
            .or_else(|| (1..=rest.len()).find(|&i| rest.is_char_boundary(i)))
            .unwrap_or(rest.len());
        let (head, tail) = rest.split_at(cut);
        out.push(head);
        rest = tail;
        max = MAX_LINE_551 - 9; // "NN CONC " prefix
    }
    if out.is_empty() {
        out.push("");
    }
    out
}

fn write_node(out: &mut String, n: &GedNode, level: usize, v70: bool, eol: &str) {
    let value = n.value.as_deref().map(escape).unwrap_or_default();
    let mut lines = value.split('\n');
    let first = lines.next().unwrap_or("");
    let head_len =
        level.to_string().len() + 1 + n.xref.as_ref().map_or(0, |x| x.len() + 1) + n.tag.len() + 1;
    emit_value(
        out,
        level,
        n.xref.as_deref(),
        &n.tag,
        first,
        head_len,
        v70,
        eol,
    );
    for l in lines {
        let h = (level + 1).to_string().len() + 6;
        emit_value(out, level + 1, None, "CONT", l, h, v70, eol);
    }
    for c in &n.children {
        write_node(out, c, level + 1, v70, eol);
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_value(
    out: &mut String,
    level: usize,
    xref: Option<&str>,
    tag: &str,
    value: &str,
    head_len: usize,
    v70: bool,
    eol: &str,
) {
    if v70 {
        push_line(out, level, xref, tag, value, eol);
        return;
    }
    let pieces = chunks(value, MAX_LINE_551.saturating_sub(head_len).max(8));
    let mut it = pieces.into_iter();
    push_line(out, level, xref, tag, it.next().unwrap_or(""), eol);
    for p in it {
        push_line(out, level + 1, None, "CONC", p, eol);
    }
}
