//! Reading the answer of the AI picture reader. No network code lives here: this is only the
//! defensive parser, so it can be tested with fixed text.
//!
//! The model is asked for strict JSON, but its answer can be wrapped in prose or code fences, cut
//! off at the output limit, or contain entries of the wrong shape. The parser pulls out each
//! object of the `people` and `relations` arrays one at a time, keeps the good ones and counts the
//! rest in `skipped`. It never fails because of one bad entry.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use ts_rs::TS;

/// More people than this in one answer are dropped (and counted as skipped).
pub const MAX_AI_PEOPLE: usize = 500;

/// One person the reader found on the picture. The box is in 0..1 of the picture size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AiPerson {
    pub id: String,
    pub given: String,
    pub surname: String,
    pub patronymic: String,
    /// `male`, `female` or `unknown`; absent when the reader did not say.
    pub sex: Option<String>,
    pub birth: Option<String>,
    pub death: Option<String>,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// 0..1, how sure the reader is.
    pub confidence: f64,
}

/// A link the reader saw. For `parent`, `from` is the parent and `to` the child.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AiRelation {
    /// `parent` or `partner`.
    pub kind: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AiReadResult {
    pub people: Vec<AiPerson>,
    pub relations: Vec<AiRelation>,
    /// Entries that were malformed, repeated, over the cap or pointed at nobody.
    pub skipped: u32,
    /// The answer ended before its closing brackets (so some entries may be missing).
    pub truncated: bool,
}

/// Finds the complete `{...}` objects inside the array that follows `"key"`. Returns them and
/// whether the array was cut off before its `]`.
fn array_objects<'a>(text: &'a str, key: &str) -> (Vec<&'a str>, bool) {
    let needle = format!("\"{key}\"");
    let Some(at) = text.find(&needle) else {
        return (Vec::new(), false);
    };
    let rest = &text[at + needle.len()..];
    let Some(open) = rest.find('[') else {
        return (Vec::new(), true);
    };
    // Only whitespace and a colon may sit between the key and the bracket.
    if !rest[..open].chars().all(|c| c.is_whitespace() || c == ':') {
        return (Vec::new(), false);
    }
    let body = &rest[open + 1..];
    let bytes = body.as_bytes();
    let (mut out, mut depth, mut start) = (Vec::new(), 0usize, 0usize);
    let (mut in_str, mut esc) = (false, false);
    for (i, &b) in bytes.iter().enumerate() {
        if in_str {
            if esc {
                esc = false;
            } else if b == 0x5c {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => {
                if depth == 0 {
                    start = i;
                }
                depth += 1;
            }
            b'}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    out.push(&body[start..=i]);
                }
            }
            b']' if depth == 0 => return (out, false),
            _ => {}
        }
    }
    (out, true)
}

fn text_of(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => {
            let t = s.trim();
            (!t.is_empty()).then(|| t.chars().take(200).collect())
        }
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn unit(v: Option<&Value>) -> Option<f64> {
    let n = v?.as_f64()?;
    n.is_finite().then_some(n.clamp(0.0, 1.0))
}

fn person_of(v: &Value) -> Option<AiPerson> {
    let o = v.as_object()?;
    let id = text_of(o.get("id"))?;
    let given = text_of(o.get("given")).unwrap_or_default();
    let surname = text_of(o.get("surname")).unwrap_or_default();
    if given.is_empty() && surname.is_empty() {
        return None;
    }
    let b = o.get("box")?.as_object()?;
    let (x, y) = (unit(b.get("x"))?, unit(b.get("y"))?);
    let (w, h) = (unit(b.get("w"))?, unit(b.get("h"))?);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let sex = text_of(o.get("sex"))
        .map(|s| s.to_lowercase())
        .filter(|s| matches!(s.as_str(), "male" | "female" | "unknown"));
    Some(AiPerson {
        id,
        given,
        surname,
        patronymic: text_of(o.get("patronymic")).unwrap_or_default(),
        sex,
        birth: text_of(o.get("birth")),
        death: text_of(o.get("death")),
        x,
        y,
        w: w.min(1.0 - x).max(0.001),
        h: h.min(1.0 - y).max(0.001),
        confidence: unit(o.get("confidence")).unwrap_or(0.5),
    })
}

/// Parses the model's text. See the module comment for what is tolerated.
pub fn parse_ai_reply(text: &str) -> AiReadResult {
    let (raw_people, cut_people) = array_objects(text, "people");
    let (raw_rels, cut_rels) = array_objects(text, "relations");
    let mut skipped = 0u32;
    let mut people: Vec<AiPerson> = Vec::new();
    let mut ids = HashSet::new();
    for raw in raw_people {
        let parsed = serde_json::from_str::<Value>(raw)
            .ok()
            .and_then(|v| person_of(&v));
        match parsed {
            Some(p) if people.len() < MAX_AI_PEOPLE && ids.insert(p.id.clone()) => people.push(p),
            _ => skipped += 1,
        }
    }
    let mut relations = Vec::new();
    for raw in raw_rels {
        let rel = serde_json::from_str::<Value>(raw).ok().and_then(|v| {
            let o = v.as_object()?;
            let kind = text_of(o.get("kind"))?.to_lowercase();
            let (from, to) = (text_of(o.get("from"))?, text_of(o.get("to"))?);
            (matches!(kind.as_str(), "parent" | "partner")
                && from != to
                && ids.contains(&from)
                && ids.contains(&to))
            .then_some(AiRelation { kind, from, to })
        });
        match rel {
            Some(r) if !relations.contains(&r) => relations.push(r),
            _ => skipped += 1,
        }
    }
    AiReadResult {
        people,
        relations,
        skipped,
        truncated: cut_people || cut_rels,
    }
}
