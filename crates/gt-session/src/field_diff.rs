//! What exactly changed between two versions of a person: sex and every field of every name.
//!
//! Values come out as stored (no trimming, no reformatting). A few are tokens the UI words in
//! its own language: `kind` (`birth`, `married`, ..., `other:<text>`), `sex` (`male`, `female`,
//! `intersex`, `unknown`, `other:<text>`), `preferred` and `private` (`yes` or `no`), `proof`
//! (`proven`, `disproven`), and `citations`, `notes`, `unknown` (a count).

use gt_core::model::{Assertion, Name, NameKind, Person, Proof, Sex};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FieldChangeDto {
    /// Stable id: `sex`, or a name field: `kind`, `preferred`, `proof`, `confidence`, `private`,
    /// `prefix`, `given`, `call`, `surnamePrefix`, `surnames`, `patronymic`, `suffix`,
    /// `nickname`, `lang`, `date`, `raw`, `citations`, `notes`, `unknown`, or `position`
    /// (where the name stands among the names kept; the first is the main one).
    pub field: String,
    /// Which name, counted from 1, when the person has more than one; `None` for `sex`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name_number: Option<u32>,
    /// The kind of that name (a `kind` token), to tell names apart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name_kind: Option<String>,
    pub before: Option<String>,
    pub after: Option<String>,
}

fn yes(b: bool) -> String {
    if b { "yes" } else { "no" }.to_string()
}

fn kind_token(k: &NameKind) -> String {
    match k {
        NameKind::Birth => "birth".into(),
        NameKind::Married => "married".into(),
        NameKind::Aka => "aka".into(),
        NameKind::Religious => "religious".into(),
        NameKind::Nickname => "nickname".into(),
        NameKind::Immigrant => "immigrant".into(),
        NameKind::Professional => "professional".into(),
        NameKind::Other(t) => format!("other:{t}"),
    }
}

fn sex_token(s: &Sex) -> String {
    match s {
        Sex::Male => "male".into(),
        Sex::Female => "female".into(),
        Sex::Intersex => "intersex".into(),
        Sex::Unknown => "unknown".into(),
        Sex::Other(t) => format!("other:{t}"),
    }
}

/// Every field of a name as `(id, value)`; `None` is an empty field. The destructure has no
/// `..`, so a new field of `Name` or `Assertion` does not compile until it is listed here.
fn name_fields(n: &Name) -> Vec<(&'static str, Option<String>)> {
    let Name {
        id: _,
        kind,
        assertion:
            Assertion {
                preferred,
                proof,
                confidence,
            },
        private,
        prefix,
        given,
        call,
        surname_prefix,
        surnames,
        patronymic,
        suffix,
        nickname,
        lang,
        date,
        raw,
        citations,
        notes,
        unknown,
    } = n;
    let count = |n: usize| (n > 0).then(|| n.to_string());
    vec![
        ("kind", Some(kind_token(kind))),
        ("preferred", Some(yes(*preferred))),
        (
            "proof",
            proof.as_ref().map(|p| {
                match p {
                    Proof::Proven => "proven",
                    Proof::Disproven => "disproven",
                }
                .to_string()
            }),
        ),
        ("confidence", confidence.map(|c| c.to_string())),
        ("private", Some(yes(*private))),
        ("prefix", prefix.clone()),
        ("given", given.clone()),
        ("call", call.clone()),
        ("surnamePrefix", surname_prefix.clone()),
        (
            "surnames",
            (!surnames.is_empty()).then(|| surnames.join(", ")),
        ),
        ("patronymic", patronymic.clone()),
        ("suffix", suffix.clone()),
        ("nickname", nickname.clone()),
        ("lang", lang.clone()),
        ("date", date.as_ref().map(|d| d.raw.clone())),
        ("raw", raw.clone()),
        ("citations", count(citations.len())),
        ("notes", count(notes.len())),
        ("unknown", count(unknown.len())),
    ]
}

type Who = (Option<u32>, Option<String>);

/// Fields of the person that changed from `before` to `after`, in a stable order: `sex`, then
/// each name in the new order (removed names last). A name that appears or goes lists every
/// field it fills, against an empty other side. Names are paired by id.
pub fn person_field_changes(before: &Person, after: &Person) -> Vec<FieldChangeDto> {
    // Every Person field is named, so a new one has to be decided here.
    let Person {
        common: _,
        sex: before_sex,
        names: before_names,
        living_override: _,
        sibling_marker: _,
    } = before;
    let Person {
        common: _,
        sex: after_sex,
        names: after_names,
        living_override: _,
        sibling_marker: _,
    } = after;
    let mut out: Vec<FieldChangeDto> = Vec::new();
    if before_sex != after_sex {
        out.push(FieldChangeDto {
            field: "sex".into(),
            name_number: None,
            name_kind: None,
            before: Some(sex_token(before_sex)),
            after: Some(sex_token(after_sex)),
        });
    }
    let several = before_names.len() > 1 || after_names.len() > 1;
    let who = |pos: usize, n: &Name| -> Who {
        (
            several.then(|| u32::try_from(pos + 1).unwrap_or(u32::MAX)),
            several.then(|| kind_token(&n.kind)),
        )
    };
    let push = |out: &mut Vec<FieldChangeDto>,
                field: &str,
                at: &Who,
                b: Option<String>,
                a: Option<String>| {
        out.push(FieldChangeDto {
            field: field.into(),
            name_number: at.0,
            name_kind: at.1.clone(),
            before: b,
            after: a,
        });
    };
    // The names both versions have, in each version's order: a name that moved among them.
    let kept = |names: &[Name], other: &[Name]| -> Vec<Uuid> {
        names
            .iter()
            .filter(|n| other.iter().any(|o| o.id == n.id))
            .map(|n| n.id)
            .collect()
    };
    let kept_before = kept(before_names, after_names);
    let kept_after = kept(after_names, before_names);
    for (pos, n) in after_names.iter().enumerate() {
        let at = who(pos, n);
        match before_names.iter().find(|b| b.id == n.id) {
            Some(old) => {
                for ((field, b), (_, a)) in name_fields(old).into_iter().zip(name_fields(n)) {
                    if b != a {
                        push(&mut out, field, &at, b, a);
                    }
                }
                let rank = |list: &[Uuid]| list.iter().position(|i| *i == n.id);
                let (rb, ra) = (rank(&kept_before), rank(&kept_after));
                if rb != ra {
                    let num = |r: Option<usize>| r.map(|r| (r + 1).to_string());
                    push(&mut out, "position", &at, num(rb), num(ra));
                }
            }
            None => {
                for (field, a) in name_fields(n) {
                    if a.is_some() {
                        push(&mut out, field, &at, None, a);
                    }
                }
            }
        }
    }
    for (pos, n) in before_names.iter().enumerate() {
        if after_names.iter().any(|a| a.id == n.id) {
            continue;
        }
        let at = who(pos, n);
        for (field, b) in name_fields(n) {
            if b.is_some() {
                push(&mut out, field, &at, b, None);
            }
        }
    }
    out
}

/// The field changes of a logged change: only for a person that exists on both sides.
pub fn record_field_changes(
    before: Option<&gt_store::Record>,
    after: Option<&gt_store::Record>,
) -> Vec<FieldChangeDto> {
    match (before, after) {
        (Some(gt_store::Record::Person(b)), Some(gt_store::Record::Person(a))) => {
            person_field_changes(b, a)
        }
        _ => Vec::new(),
    }
}
