//! What exactly changed between two versions of a record: for a person its sex and every field of
//! every name, for any other kind every field it has, with ids worded through [`Words`].
//!
//! Values come out as stored (no trimming, no reformatting). A few are tokens the UI words in
//! its own language: `kind` (`birth`, `married`, ..., `other:<text>`), `sex` (`male`, `female`,
//! `intersex`, `unknown`, `other:<text>`), `preferred` and `private` (`yes` or `no`), `proof`
//! (`proven`, `disproven`), and `citations`, `notes`, `unknown` (a count).

use gt_core::model::{
    Assertion, ChildLink, Citation, Common, Event, Evidence, ExtIdKind, ExternalId, Family, Media,
    MediaLink, Name, NameKind, Note, Participant, ParticipantRef, Person, Place, PlaceName, Proof,
    RepoRef, Repository, Sex, Source,
};
use gt_store::Record;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::person_detail::enum_id;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FieldChangeDto {
    /// Stable id: for a record other than a person its own field (`text`, `eventKind`, `partner1`,
    /// `child`, `participant`, `page`, ...; see `rows`). For a person: `sex`, or a name field: `kind`, `preferred`, `proof`, `confidence`, `private`,
    /// `prefix`, `given`, `call`, `surnamePrefix`, `surnames`, `patronymic`, `suffix`,
    /// `nickname`, `lang`, `date`, `raw`, `citations`, `notes`, `unknown`, or `position`
    /// (where the name stands among the names kept; the first is the main one).
    pub field: String,
    /// The part of the record this row is about, in words (a participant's or a child's name). Several
    /// names (the partners of a family) are joined with [`LIST_SEP`] so the UI words the "and".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub of: Option<String>,
    /// Which name, counted from 1, when the person has more than one; `None` for `sex`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name_number: Option<u32>,
    /// The kind of that name (a `kind` token), to tell names apart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name_kind: Option<String>,
    /// The id of that name, so the UI can open it in the name editor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name_id: Option<Uuid>,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// Joins the names in a row's `of` (the partners of a family); the UI puts its own "and" between them.
pub const LIST_SEP: char = '\u{1f}';

/// What stands for a record that was deleted and cannot be read any more (a place, a source ...).
/// The UI words it in its own language; it never appears in stored data.
pub const REMOVED: &str = "\u{1}removed";

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

type Who = (Option<u32>, Option<String>, Option<Uuid>);

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
            of: None,
            name_number: None,
            name_kind: None,
            name_id: None,
            before: Some(sex_token(before_sex)),
            after: Some(sex_token(after_sex)),
        });
    }
    let several = before_names.len() > 1 || after_names.len() > 1;
    let who = |pos: usize, n: &Name| -> Who {
        (
            several.then(|| u32::try_from(pos + 1).unwrap_or(u32::MAX)),
            several.then(|| kind_token(&n.kind)),
            Some(n.id),
        )
    };
    let push = |out: &mut Vec<FieldChangeDto>,
                field: &str,
                at: &Who,
                b: Option<String>,
                a: Option<String>| {
        out.push(FieldChangeDto {
            field: field.into(),
            of: None,
            name_number: at.0,
            name_kind: at.1.clone(),
            name_id: at.2,
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

/// How ids read in a field change: a person's name, a place's name, a source's title. Whoever
/// implements it also finds records that have since been deleted.
pub trait Words {
    fn person(&self, id: Uuid) -> String;
    fn family(&self, id: Uuid) -> String;
    fn place(&self, id: Uuid) -> String;
    fn source(&self, id: Uuid) -> String;
    fn repository(&self, id: Uuid) -> String;
    fn citation(&self, id: Uuid) -> String;
    fn note(&self, id: Uuid) -> String;
    fn media(&self, id: Uuid) -> String;
}

/// One field of a record: `key` tells apart rows of the same field (one per child, per participant).
struct Row {
    field: &'static str,
    of: Option<String>,
    key: Option<Uuid>,
    value: Option<String>,
}

fn row(field: &'static str, value: Option<String>) -> Row {
    Row {
        field,
        of: None,
        key: None,
        value,
    }
}

fn text(field: &'static str, v: &Option<String>) -> Row {
    row(field, v.clone().filter(|s| !s.is_empty()))
}

fn tri(field: &'static str, v: Option<bool>) -> Row {
    row(field, v.map(yes))
}

fn join(list: Vec<String>) -> Option<String> {
    (!list.is_empty()).then(|| list.join("; "))
}

/// Fields every record has. The destructure has no `..`.
fn common_rows(c: &Common, w: &dyn Words) -> Vec<Row> {
    let Common {
        id: _,
        private,
        changed: _,
        citations,
        notes,
        media,
        ext_ids,
        unknown,
    } = c;
    let mut out = vec![
        row("private", Some(yes(*private))),
        row(
            "citations",
            join(citations.iter().map(|i| w.citation(*i)).collect()),
        ),
        row("notes", join(notes.iter().map(|i| w.note(*i)).collect())),
        row(
            "media",
            join(media.iter().map(|l| w.media(l.media)).collect()),
        ),
        row("extIds", join(ext_ids.iter().map(ext_id).collect())),
        row("unknown", Some(unknown.len().to_string())),
    ];
    for MediaLink {
        media,
        region,
        primary,
    } in media
    {
        let name = w.media(*media);
        out.push(Row {
            field: "mediaPrimary",
            of: Some(name.clone()),
            key: Some(*media),
            value: Some(yes(*primary)),
        });
        out.push(Row {
            field: "mediaRegion",
            of: Some(name),
            key: Some(*media),
            value: region
                .as_ref()
                .map(|r| format!("{:.3}, {:.3}, {:.3}, {:.3}", r.x, r.y, r.w, r.h)),
        });
    }
    out
}

/// An external id as `scheme: value`.
fn ext_id(e: &ExternalId) -> String {
    let scheme = match &e.kind {
        ExtIdKind::Xref => "xref".to_string(),
        ExtIdKind::Uid => "uid".to_string(),
        ExtIdKind::FamilySearch => "familysearch".to_string(),
        ExtIdKind::AncestryApid => "ancestry".to_string(),
        ExtIdKind::Exid { scheme } | ExtIdKind::Other(scheme) => scheme.clone(),
    };
    format!("{scheme}: {}", e.value)
}

fn assertion_rows(a: &Assertion) -> Vec<Row> {
    // Whether it is the preferred one is not worth a row for anything but a name.
    let Assertion {
        preferred: _,
        proof,
        confidence,
    } = a;
    vec![
        row(
            "proof",
            proof.as_ref().map(|p| {
                match p {
                    Proof::Proven => "proven",
                    Proof::Disproven => "disproven",
                }
                .to_string()
            }),
        ),
        row("confidence", confidence.map(|c| c.to_string())),
    ]
}

fn note_rows(n: &Note, w: &dyn Words) -> Vec<Row> {
    let Note {
        common,
        kind,
        text: body,
        lang,
        shared,
    } = n;
    let mut out = vec![
        row("noteKind", Some(enum_id(kind))),
        row("text", Some(body.clone()).filter(|s| !s.is_empty())),
        text("lang", lang),
        row("shared", Some(yes(*shared))),
    ];
    out.extend(common_rows(common, w));
    out
}

fn event_rows(e: &Event, w: &dyn Words) -> Vec<Row> {
    let Event {
        common,
        kind,
        date,
        place,
        description,
        age,
        cause,
        negative,
        assertion,
        participants,
    } = e;
    let mut out = vec![
        row("eventKind", Some(enum_id(kind))),
        row("when", date.as_ref().map(|d| d.display())),
        row("place", place.map(|p| w.place(p))),
        text("description", description),
        text("age", age),
        text("cause", cause),
        row("negative", Some(yes(*negative))),
    ];
    out.extend(assertion_rows(assertion));
    for Participant { target, role } in participants {
        let (id, who) = match target {
            ParticipantRef::Person(id) => (*id, w.person(*id)),
            ParticipantRef::Family(id) => (*id, w.family(*id)),
        };
        out.push(Row {
            field: "participant",
            of: Some(who),
            key: Some(id),
            value: Some(enum_id(role)),
        });
    }
    out.extend(common_rows(common, w));
    out
}

fn family_rows(f: &Family, w: &dyn Words) -> Vec<Row> {
    let Family {
        common,
        partner1,
        partner2,
        kind,
        children,
    } = f;
    let mut out = vec![
        row("partner1", partner1.map(|p| w.person(p))),
        row("partner2", partner2.map(|p| w.person(p))),
        row("unionKind", Some(enum_id(kind))),
    ];
    for c in children {
        let ChildLink {
            person,
            rel1,
            rel2,
            preferred,
            assertion,
            citations,
            unknown,
        } = c;
        let name = w.person(*person);
        let mut mine = vec![
            row("childRel1", Some(enum_id(rel1))),
            row("childRel2", Some(enum_id(rel2))),
            row("childPreferred", Some(yes(*preferred))),
        ];
        mine.extend(assertion_rows(assertion));
        mine.push(row(
            "childCitations",
            join(citations.iter().map(|i| w.citation(*i)).collect()),
        ));
        mine.push(row("childUnknown", Some(unknown.len().to_string())));
        out.push(Row {
            field: "child",
            of: None,
            key: Some(*person),
            value: Some(name.clone()),
        });
        out.extend(mine.into_iter().map(|r| Row {
            of: Some(name.clone()),
            key: Some(*person),
            ..r
        }));
    }
    out.extend(common_rows(common, w));
    out
}

fn citation_rows(c: &Citation, w: &dyn Words) -> Vec<Row> {
    let Citation {
        common,
        source,
        page,
        date,
        text: quote,
        quality,
        evidence:
            Evidence {
                original,
                primary_info,
                direct,
            },
    } = c;
    let mut out = vec![
        row("source", source.map(|s| w.source(s))),
        text("page", page),
        row("when", date.as_ref().map(|d| d.display())),
        text("text", quote),
        row("quality", quality.map(|q| q.to_string())),
        tri("original", *original),
        tri("primaryInfo", *primary_info),
        tri("direct", *direct),
    ];
    out.extend(common_rows(common, w));
    out
}

fn source_rows(s: &Source, w: &dyn Words) -> Vec<Row> {
    let Source {
        common,
        title,
        author,
        publication,
        abbreviation,
        text: body,
        repositories,
    } = s;
    let mut out = vec![
        text("title", title),
        text("author", author),
        text("publication", publication),
        text("abbreviation", abbreviation),
        text("text", body),
    ];
    for RepoRef {
        repository,
        call_number,
    } in repositories
    {
        let name = w.repository(*repository);
        out.push(Row {
            field: "repository",
            of: None,
            key: Some(*repository),
            value: Some(match call_number {
                Some(n) if !n.is_empty() => format!("{name}, {n}"),
                _ => name,
            }),
        });
    }
    out.extend(common_rows(common, w));
    out
}

fn place_rows(p: &Place, w: &dyn Words) -> Vec<Row> {
    let Place {
        common,
        names,
        parent,
        kind,
        lat,
        lon,
        raw,
    } = p;
    let named = names
        .iter()
        .map(|n| {
            let PlaceName {
                name,
                lang,
                date: _,
            } = n;
            match lang {
                Some(l) if !l.is_empty() => format!("{name} ({l})"),
                _ => name.clone(),
            }
        })
        .collect();
    let mut out = vec![
        row("placeNames", join(named)),
        row("parent", parent.map(|i| w.place(i))),
        text("placeKind", kind),
        row("lat", lat.map(|v| v.to_string())),
        row("lon", lon.map(|v| v.to_string())),
        text("placeRaw", raw),
    ];
    out.extend(common_rows(common, w));
    out
}

fn repository_rows(r: &Repository, w: &dyn Words) -> Vec<Row> {
    let Repository {
        common,
        name,
        address,
        urls,
    } = r;
    let mut out = vec![
        row("repoName", Some(name.clone()).filter(|s| !s.is_empty())),
        text("address", address),
        row("urls", join(urls.clone())),
    ];
    out.extend(common_rows(common, w));
    out
}

fn media_rows(m: &Media, w: &dyn Words) -> Vec<Row> {
    let Media {
        common,
        path,
        original_name,
        mime,
        title,
        date,
        description,
        sha256,
    } = m;
    let mut out = vec![
        row("path", Some(path.clone()).filter(|s| !s.is_empty())),
        text("originalName", original_name),
        text("mime", mime),
        text("title", title),
        row("when", date.as_ref().map(|d| d.display())),
        text("description", description),
        text("sha256", sha256),
    ];
    out.extend(common_rows(common, w));
    out
}

/// Every field of a record other than a person; a person's are the sex and name fields above.
fn rows(rec: &Record, w: &dyn Words) -> Vec<Row> {
    match rec {
        Record::Person(_) => Vec::new(),
        Record::Family(f) => family_rows(f, w),
        Record::Event(e) => event_rows(e, w),
        Record::Place(p) => place_rows(p, w),
        Record::Source(s) => source_rows(s, w),
        Record::Repository(r) => repository_rows(r, w),
        Record::Citation(c) => citation_rows(c, w),
        Record::Media(m) => media_rows(m, w),
        Record::Note(n) => note_rows(n, w),
    }
}

fn dto(r: &Row, b: Option<String>, a: Option<String>) -> FieldChangeDto {
    FieldChangeDto {
        field: r.field.into(),
        of: r.of.clone(),
        name_number: None,
        name_kind: None,
        name_id: None,
        before: b,
        after: a,
    }
}

/// The field changes of a logged change. A record that appears lists the fields it fills against
/// an empty side, and one that goes the reverse; a flag that is off is not worth a row then.
/// A person only has them while it exists on both sides.
pub fn record_field_changes(
    before: Option<&Record>,
    after: Option<&Record>,
    w: &dyn Words,
) -> Vec<FieldChangeDto> {
    match (before, after) {
        (Some(Record::Person(b)), Some(Record::Person(a))) => person_field_changes(b, a),
        (Some(Record::Person(_)), _) | (_, Some(Record::Person(_))) | (None, None) => Vec::new(),
        _ => {
            let one_sided = before.is_none() || after.is_none();
            let was = before.map(|r| rows(r, w)).unwrap_or_default();
            let now = after.map(|r| rows(r, w)).unwrap_or_default();
            let off = |r: &Row| {
                one_sided
                    && match r.value.as_deref() {
                        Some("no") => true,
                        Some("0") => matches!(r.field, "unknown" | "childUnknown"),
                        _ => false,
                    }
            };
            let same = |a: &Row, b: &Row| a.field == b.field && a.key == b.key;
            let mut out = Vec::new();
            for r in &now {
                let old = was
                    .iter()
                    .find(|o| same(o, r))
                    .and_then(|o| o.value.clone());
                if old != r.value && !off(r) {
                    out.push(dto(r, old, r.value.clone()));
                }
            }
            for r in &was {
                if !now.iter().any(|n| same(n, r)) && r.value.is_some() && !off(r) {
                    out.push(dto(r, r.value.clone(), None));
                }
            }
            out
        }
    }
}
#[cfg(test)]
#[path = "field_diff_tests.rs"]
mod tests;
