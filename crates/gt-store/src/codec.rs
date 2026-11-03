//! Column encodings shared by save and load.

use crate::StoreError;
use gt_core::Uuid;
use gt_core::date::GenDate;
use gt_core::model::ExtIdKind;
use gt_core::raw::RawNode;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub(crate) fn corrupt(msg: impl Into<String>) -> StoreError {
    StoreError::Corrupt(msg.into())
}

pub(crate) fn parse_uuid(s: &str) -> Result<Uuid, StoreError> {
    Uuid::parse_str(s).map_err(|_| corrupt(format!("bad uuid {s:?}")))
}

pub(crate) fn opt_uuid(s: Option<String>) -> Result<Option<Uuid>, StoreError> {
    s.as_deref().map(parse_uuid).transpose()
}

fn snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn camel(s: &str) -> String {
    s.split('_')
        .map(|w| {
            let mut cs = w.chars();
            cs.next()
                .map(|f| f.to_uppercase().collect::<String>() + cs.as_str())
                .unwrap_or_default()
        })
        .collect()
}

/// `Birth` -> `birth`, `CivilUnion` -> `civil_union`, `Other("x")` -> `other:x`.
pub(crate) fn enum_text<T: Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => snake(&s),
        Ok(serde_json::Value::Object(m)) => match m.into_iter().next() {
            Some((k, serde_json::Value::String(x))) => format!("{}:{x}", snake(&k)),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

pub(crate) fn enum_parse<T: DeserializeOwned>(s: &str) -> Result<T, StoreError> {
    let value = match s.split_once(':') {
        Some((k, x)) => {
            let mut m = serde_json::Map::new();
            m.insert(camel(k), serde_json::Value::String(x.to_string()));
            serde_json::Value::Object(m)
        }
        None => serde_json::Value::String(camel(s)),
    };
    serde_json::from_value(value).map_err(|_| corrupt(format!("bad enum value {s:?}")))
}

pub(crate) fn opt_enum_parse<T: DeserializeOwned>(
    s: Option<String>,
) -> Result<Option<T>, StoreError> {
    s.as_deref().map(enum_parse).transpose()
}

pub(crate) fn ext_kind_cols(k: &ExtIdKind) -> (&'static str, Option<&str>) {
    match k {
        ExtIdKind::Xref => ("xref", None),
        ExtIdKind::Uid => ("uid", None),
        ExtIdKind::FamilySearch => ("familysearch", None),
        ExtIdKind::AncestryApid => ("ancestry_apid", None),
        ExtIdKind::Exid { scheme } => ("exid", Some(scheme)),
        ExtIdKind::Other(n) => ("other", Some(n)),
    }
}

pub(crate) fn ext_kind_parse(kind: &str, scheme: Option<String>) -> Result<ExtIdKind, StoreError> {
    Ok(match kind {
        "xref" => ExtIdKind::Xref,
        "uid" => ExtIdKind::Uid,
        "familysearch" => ExtIdKind::FamilySearch,
        "ancestry_apid" => ExtIdKind::AncestryApid,
        "exid" => ExtIdKind::Exid {
            scheme: scheme.unwrap_or_default(),
        },
        "other" => ExtIdKind::Other(scheme.unwrap_or_default()),
        _ => return Err(corrupt(format!("bad ext_id kind {kind:?}"))),
    })
}

pub(crate) fn unknown_json(nodes: &[RawNode]) -> Result<Option<String>, StoreError> {
    if nodes.is_empty() {
        Ok(None)
    } else {
        Ok(Some(
            serde_json::to_string(nodes).map_err(|e| corrupt(e.to_string()))?,
        ))
    }
}

pub(crate) fn unknown_parse(s: Option<String>) -> Result<Vec<RawNode>, StoreError> {
    match s {
        None => Ok(Vec::new()),
        Some(j) => serde_json::from_str(&j).map_err(|e| corrupt(e.to_string())),
    }
}

/// The four columns of a date: raw text, parsed JSON, earliest and latest day number.
pub(crate) struct DateCols {
    pub raw: Option<String>,
    pub json: Option<String>,
    pub min: Option<i32>,
    pub max: Option<i32>,
}

pub(crate) fn date_cols(d: &Option<GenDate>) -> Result<DateCols, StoreError> {
    Ok(match d {
        None => DateCols {
            raw: None,
            json: None,
            min: None,
            max: None,
        },
        Some(d) => DateCols {
            raw: Some(d.raw.clone()),
            json: Some(serde_json::to_string(d).map_err(|e| corrupt(e.to_string()))?),
            min: d.sort_min,
            max: d.sort_max,
        },
    })
}

pub(crate) fn date_parse(json: Option<String>) -> Result<Option<GenDate>, StoreError> {
    match json {
        None => Ok(None),
        Some(j) => serde_json::from_str(&j)
            .map(Some)
            .map_err(|e| corrupt(e.to_string())),
    }
}
