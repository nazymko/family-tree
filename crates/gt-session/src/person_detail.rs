//! Everything the person editor shows, and the record-to-DTO conversions it shares with the
//! family view.

use crate::dates::date_dto;
use crate::dto::*;
use crate::error::ApiError;
use crate::names::{compose, sex_kind};
use crate::session::Session;
use gt_core::model::{
    self as m, Event, EventKind, MediaLink, NameKind, NoteKind, ParentRel, ParticipantRef, Role,
    UnionKind,
};
use serde::Serialize;
use uuid::Uuid;

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

/// `Birth` -> `birth`, `MarriageBanns` -> `marriage_banns`, `Custom("x")` -> `custom:x`.
fn enum_id<T: Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => snake(&s),
        Ok(serde_json::Value::Object(map)) => match map.into_iter().next() {
            Some((k, serde_json::Value::String(x))) => format!("{}:{x}", snake(&k)),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

pub fn event_kind_id(k: &EventKind) -> String {
    enum_id(k)
}

pub fn event_label(k: &EventKind) -> String {
    let capitalise = |t: &str| {
        let mut cs = t.chars();
        cs.next()
            .map(|f| f.to_uppercase().collect::<String>() + cs.as_str())
            .unwrap_or_default()
    };
    match k {
        EventKind::Custom(t) => {
            let t = t.trim();
            if t.is_empty() {
                "Event".to_string()
            } else {
                capitalise(t)
            }
        }
        EventKind::Military => "Military service".to_string(),
        other => capitalise(&event_kind_id(other).replace('_', " ")),
    }
}

/// Facts describe a state (job, residence), not a moment.
pub fn is_fact(k: &EventKind) -> bool {
    matches!(
        k,
        EventKind::Occupation
            | EventKind::Education
            | EventKind::Religion
            | EventKind::Nationality
            | EventKind::Title
            | EventKind::Residence
            | EventKind::PhysicalDescription
    )
}

fn role_text(r: &Role) -> String {
    match r {
        Role::Other(t) => t.clone(),
        other => enum_id(other),
    }
}

pub fn rel_id(r: &ParentRel) -> ParentRelId {
    match r {
        ParentRel::Birth => ParentRelId::Birth,
        ParentRel::Adopted => ParentRelId::Adopted,
        ParentRel::Step => ParentRelId::Step,
        ParentRel::Foster => ParentRelId::Foster,
        ParentRel::Guardian => ParentRelId::Guardian,
        ParentRel::Sealed => ParentRelId::Sealed,
        ParentRel::Unknown => ParentRelId::Unknown,
        ParentRel::Other(_) => ParentRelId::Other,
    }
}

pub fn union_id(k: &UnionKind) -> UnionKindId {
    match k {
        UnionKind::Married => UnionKindId::Married,
        UnionKind::CivilUnion => UnionKindId::CivilUnion,
        UnionKind::Unmarried => UnionKindId::Unmarried,
        UnionKind::Unknown => UnionKindId::Unknown,
        UnionKind::Other(_) => UnionKindId::Other,
    }
}

impl Session {
    pub(crate) fn place_option(&self, id: Uuid) -> Option<PlaceOption> {
        let place = self.tree.places.get(&id)?;
        let name_of = |p: &m::Place| {
            p.names
                .first()
                .map(|n| n.name.clone())
                .or_else(|| p.raw.clone())
                .unwrap_or_default()
        };
        let mut parts = vec![name_of(place)];
        let mut next = place.parent;
        // A place chain is short; the limit only guards against a parent cycle.
        for _ in 0..16 {
            let Some(parent) = next.and_then(|p| self.tree.places.get(&p)) else {
                break;
            };
            parts.push(name_of(parent));
            next = parent.parent;
        }
        parts.retain(|p| !p.is_empty());
        Some(PlaceOption {
            id,
            name: parts.first().cloned().unwrap_or_default(),
            full: parts.join(", "),
            kind: place.kind.clone(),
        })
    }

    pub(crate) fn source_dto(&self, id: Uuid) -> Option<SourceDto> {
        let s = self.tree.sources.get(&id)?;
        Some(SourceDto {
            id,
            title: s.title.clone(),
            author: s.author.clone(),
            publication: s.publication.clone(),
            abbreviation: s.abbreviation.clone(),
            text: s.text.clone(),
            citation_count: self
                .links
                .citations_of_source
                .get(&id)
                .copied()
                .unwrap_or(0),
        })
    }

    pub(crate) fn citation_dto(&self, id: Uuid) -> Option<CitationDto> {
        let c = self.tree.citations.get(&id)?;
        Some(CitationDto {
            id,
            source: c.source.and_then(|s| self.source_dto(s)),
            page: c.page.clone(),
            date: c.date.as_ref().map(date_dto),
            text: c.text.clone(),
            quality: c.quality.map(|q| q.min(3)),
            evidence: EvidenceDto {
                original: c.evidence.original,
                primary_info: c.evidence.primary_info,
                direct: c.evidence.direct,
            },
        })
    }

    pub(crate) fn citations_dto(&self, ids: &[Uuid]) -> Vec<CitationDto> {
        ids.iter().filter_map(|c| self.citation_dto(*c)).collect()
    }

    pub(crate) fn note_dtos(&self, ids: &[Uuid]) -> Vec<NoteDto> {
        ids.iter()
            .filter_map(|id| {
                let n = self.tree.notes.get(id)?;
                Some(NoteDto {
                    id: *id,
                    kind: match n.kind {
                        NoteKind::General => NoteKindDto::General,
                        NoteKind::Story => NoteKindDto::Story,
                        NoteKind::Research => NoteKindDto::Research,
                        NoteKind::Transcription => NoteKindDto::Transcription,
                        NoteKind::Conclusion => NoteKindDto::Conclusion,
                        NoteKind::Other(_) => NoteKindDto::Other,
                    },
                    text: n.text.clone(),
                    shared: n.shared,
                    lang: n.lang.clone(),
                })
            })
            .collect()
    }

    pub(crate) fn media_dtos(&self, links: &[MediaLink]) -> Vec<MediaLinkDto> {
        links
            .iter()
            .filter_map(|l| {
                let md = self.tree.media.get(&l.media)?;
                Some(MediaLinkDto {
                    media: MediaDto {
                        id: l.media,
                        title: md.title.clone(),
                        mime: md.mime.clone(),
                        original_name: md.original_name.clone(),
                        date: md.date.as_ref().map(date_dto),
                        description: md.description.clone(),
                        resolved: self.media_file_exists(&md.path),
                        width: self.media_dims(l.media, &md.path).0,
                        height: self.media_dims(l.media, &md.path).1,
                        sha256: md.sha256.clone(),
                    },
                    primary: l.primary,
                })
            })
            .collect()
    }

    /// Whether a stored media path names a file inside `<project>/media`. The path is checked
    /// lexically first (no absolute, UNC, drive or `..` forms); only then is the disk touched.
    pub(crate) fn media_file_exists(&self, stored: &str) -> bool {
        let rel = std::path::Path::new(stored);
        if stored.is_empty()
            || rel.has_root()
            || rel.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::Prefix(_) | std::path::Component::RootDir
                )
            })
        {
            return false;
        }
        let candidate = self.project.dir.join(rel);
        let roots = [self.project.dir.join("media")];
        gt_gedcom::is_lexically_under(&candidate, &roots) && candidate.is_file()
    }

    pub(crate) fn event_dto(&self, e: &Event) -> EventDto {
        EventDto {
            id: e.common.id,
            kind: event_kind_id(&e.kind),
            label: event_label(&e.kind),
            is_fact: is_fact(&e.kind),
            date: e.date.as_ref().map(date_dto),
            place: e.place.and_then(|p| self.place_option(p)),
            description: e.description.clone(),
            age: e.age.clone(),
            cause: e.cause.clone(),
            negative: e.negative,
            private: e.common.private,
            preferred: e.assertion.preferred,
            participants: e
                .participants
                .iter()
                .filter_map(|p| match p.target {
                    ParticipantRef::Person(id) => Some(ParticipantDto {
                        person: self.person_ref(id)?,
                        role: role_text(&p.role),
                    }),
                    ParticipantRef::Family(_) => None,
                })
                .collect(),
            citations: self.citations_dto(&e.common.citations),
            notes: self.note_dtos(&e.common.notes),
            media: self.media_dtos(&e.common.media),
        }
    }

    fn name_dto(&self, n: &m::Name) -> NameDto {
        let (kind, kind_text) = match &n.kind {
            NameKind::Birth => (NameKindDto::Birth, None),
            NameKind::Married => (NameKindDto::Married, None),
            NameKind::Aka => (NameKindDto::Aka, None),
            NameKind::Religious => (NameKindDto::Religious, None),
            NameKind::Nickname => (NameKindDto::Nickname, None),
            NameKind::Immigrant => (NameKindDto::Immigrant, None),
            NameKind::Professional => (NameKindDto::Professional, None),
            NameKind::Other(t) => (NameKindDto::Other, Some(t.clone())),
        };
        NameDto {
            id: n.id,
            kind,
            kind_text,
            preferred: n.assertion.preferred,
            private: n.private,
            display: compose(n, self.settings.name_order, true),
            prefix: n.prefix.clone(),
            given: n.given.clone(),
            call: n.call.clone(),
            surname_prefix: n.surname_prefix.clone(),
            surnames: n.surnames.clone(),
            patronymic: n.patronymic.clone(),
            suffix: n.suffix.clone(),
            nickname: n.nickname.clone(),
            lang: n.lang.clone(),
            date: n.date.as_ref().map(date_dto),
            citations: self.citations_dto(&n.citations),
        }
    }
}

pub fn person_detail(s: &Session, id: Uuid) -> Result<PersonDetail, ApiError> {
    let p = s
        .tree
        .persons
        .get(&id)
        .ok_or_else(|| ApiError::not_found("person", id))?;
    if p.sibling_marker {
        return Err(ApiError::not_found("person", id));
    }
    let person_ref = s
        .person_ref(id)
        .ok_or_else(|| ApiError::not_found("person", id))?;
    let mut events: Vec<&Event> = s
        .links
        .events_of
        .get(&id)
        .map(|v| v.iter().filter_map(|e| s.tree.events.get(e)).collect())
        .unwrap_or_default();
    // Dated events in date order; undated ones keep their place after them.
    events.sort_by_key(|e| {
        e.date
            .as_ref()
            .and_then(|d| d.sort_key())
            .unwrap_or(i32::MAX)
    });
    let (sex, sex_text) = sex_kind(&p.sex);
    Ok(PersonDetail {
        person_ref,
        sex,
        sex_text,
        living_override: p.living_override.as_ref().map(|o| match o {
            m::LivingOverride::Living => LivingOverrideDto::Living,
            m::LivingOverride::Deceased => LivingOverrideDto::Deceased,
        }),
        living_computed: s.living_computed.get(&id).copied().unwrap_or(true),
        private: p.common.private,
        names: p.names.iter().map(|n| s.name_dto(n)).collect(),
        events: events.into_iter().map(|e| s.event_dto(e)).collect(),
        parent_sets: crate::family_view::parent_sets(s, id),
        partnerships: crate::family_view::partnerships(s, id),
        notes: s.note_dtos(&p.common.notes),
        media: s.media_dtos(&p.common.media),
        citations: s.citations_dto(&p.common.citations),
        changed: p.common.changed.unwrap_or(0),
    })
}
