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
pub(crate) fn enum_id<T: Serialize>(v: &T) -> String {
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
        EventKind::Missing => "Went missing".to_string(),
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
    pub fn place_option(&self, id: Uuid) -> Option<PlaceOption> {
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

    /// How many citations cite the source, without copying the source's text.
    pub fn source_citation_count(&self, id: Uuid) -> Option<u32> {
        self.tree.sources.contains_key(&id).then(|| {
            self.links
                .citations_of_source
                .get(&id)
                .copied()
                .unwrap_or(0)
        })
    }

    pub fn source_dto(&self, id: Uuid) -> Option<SourceDto> {
        let s = self.tree.sources.get(&id)?;
        Some(SourceDto {
            id,
            title: s.title.clone(),
            author: s.author.clone(),
            publication: s.publication.clone(),
            abbreviation: s.abbreviation.clone(),
            text: s.text.clone(),
            kind: s.kind.as_ref().map(|k| k.id()),
            issuer: s.issuer.clone(),
            doc_number: s.doc_number.clone(),
            doc_date: s.doc_date.as_ref().map(date_dto),
            archive: s
                .repositories
                .first()
                .and_then(|r| self.tree.repositories.get(&r.repository))
                .map(|r| r.name.clone()),
            call_number: s.repositories.first().and_then(|r| r.call_number.clone()),
            citation_count: self
                .links
                .citations_of_source
                .get(&id)
                .copied()
                .unwrap_or(0),
            media: self.media_dtos(&s.common.media),
        })
    }

    pub fn citation_dto(&self, id: Uuid) -> Option<CitationDto> {
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
            media: self.media_dtos(&c.common.media),
        })
    }

    pub fn citations_dto(&self, ids: &[Uuid]) -> Vec<CitationDto> {
        ids.iter().filter_map(|c| self.citation_dto(*c)).collect()
    }

    pub fn note_dtos(&self, ids: &[Uuid]) -> Vec<NoteDto> {
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
                    media: self.media_dtos(&n.common.media),
                })
            })
            .collect()
    }

    pub fn media_dtos(&self, links: &[MediaLink]) -> Vec<MediaLinkDto> {
        let from_sources = self.source_texts_of(links);
        let groups = self.tree.version_index();
        links
            .iter()
            .filter_map(|l| {
                let mut media = self.media_dto(l.media, &from_sources)?;
                media.versions = self.version_dtos_in(&groups, l.media);
                Some(MediaLinkDto {
                    media,
                    primary: l.primary,
                    clip: l.clip.as_ref().map(clip_dto),
                })
            })
            .collect()
    }

    /// Every file of the version group of `media`, the primary first, then by pixel count, largest
    /// first; empty for a file with no other versions.
    pub fn version_dtos(&self, media: Uuid) -> Vec<MediaDto> {
        self.version_dtos_in(&self.tree.version_index(), media)
    }

    fn version_dtos_in(
        &self,
        groups: &std::collections::HashMap<Uuid, Vec<Uuid>>,
        media: Uuid,
    ) -> Vec<MediaDto> {
        let primary = self.tree.version_primary(media);
        let Some(rest) = groups.get(&primary) else {
            return Vec::new();
        };
        let group: Vec<Uuid> = std::iter::once(primary)
            .chain(rest.iter().copied())
            .collect();
        let links: Vec<MediaLink> = group
            .iter()
            .map(|id| MediaLink {
                media: *id,
                region: None,
                primary: false,
                clip: None,
            })
            .collect();
        let from_sources = self.source_texts_of(&links);
        let mut out: Vec<MediaDto> = group
            .iter()
            .filter_map(|id| self.media_dto(*id, &from_sources))
            .collect();
        let pixels =
            |m: &MediaDto| u64::from(m.width.unwrap_or(0)) * u64::from(m.height.unwrap_or(0));
        if let Some(rest) = out.get_mut(1..) {
            rest.sort_by_key(|m| std::cmp::Reverse(pixels(m)));
        }
        out
    }

    /// One media record as the app shows it, without its versions.
    fn media_dto(
        &self,
        id: Uuid,
        from_sources: &std::collections::HashMap<Uuid, Vec<String>>,
    ) -> Option<MediaDto> {
        let md = self.tree.media.get(&id)?;
        Some(MediaDto {
            id,
            title: md.title.clone(),
            mime: md.mime.clone(),
            original_name: md.original_name.clone(),
            date: md.date.as_ref().map(date_dto),
            description: md.description.clone(),
            place: md.place.and_then(|p| self.place_option(p)),
            resolved: self.media_file_exists(&md.path),
            width: self.media_dims(id, &md.path).0,
            height: self.media_dims(id, &md.path).1,
            sha256: md.sha256.clone(),
            transcription: md.transcription.iter().map(text_region_dto).collect(),
            rotation: md.shown_rotation(),
            duration_ms: md
                .mime
                .as_deref()
                .is_some_and(|t| t.starts_with("audio/"))
                .then(|| self.media_length(id))
                .flatten()
                .map(|len| len.ms),
            web_links: crate::web_links::collect(
                md.description.iter().map(String::as_str).chain(
                    from_sources
                        .get(&id)
                        .into_iter()
                        .flatten()
                        .map(String::as_str),
                ),
            ),
            versions: Vec::new(),
        })
    }

    /// The texts of every source that holds one of these media (title, publication, text, call
    /// numbers), by media id, in source-id order. One pass over the sources per call, and none
    /// when nothing is linked; sources with no media of these are skipped without reading their text.
    fn source_texts_of(&self, links: &[MediaLink]) -> std::collections::HashMap<Uuid, Vec<String>> {
        let mut out: std::collections::HashMap<Uuid, Vec<String>> = Default::default();
        if links.is_empty() {
            return out;
        }
        let wanted: std::collections::HashSet<Uuid> = links.iter().map(|l| l.media).collect();
        let mut sources: Vec<_> = self
            .tree
            .sources
            .iter()
            .filter(|(_, s)| s.common.media.iter().any(|l| wanted.contains(&l.media)))
            .collect();
        sources.sort_by_key(|(id, _)| **id);
        for (_, s) in sources {
            let texts: Vec<String> = [&s.title, &s.publication, &s.text]
                .into_iter()
                .flatten()
                .cloned()
                .chain(s.repositories.iter().filter_map(|r| r.call_number.clone()))
                .collect();
            for l in &s.common.media {
                if wanted.contains(&l.media) {
                    out.entry(l.media)
                        .or_default()
                        .extend(texts.iter().cloned());
                }
            }
        }
        out
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
            godparents: self.godparent_dtos(&e.godparents),
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
        return Err(ApiError::sibling_marker(id));
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
        godchildren: s.godchildren(id),
        changed: p.common.changed.unwrap_or(0),
    })
}

/// A clip as the app shows it.
pub fn clip_dto(c: &gt_core::model::Clip) -> crate::dto::ClipDto {
    crate::dto::ClipDto {
        id: c.id,
        start_ms: c.start_ms,
        end_ms: c.end_ms,
        label: c.label.clone(),
    }
}

/// A stored piece of transcription as the app shows it.
pub fn text_region_dto(r: &gt_core::model::TextRegion) -> crate::dto::TextRegionDto {
    crate::dto::TextRegionDto {
        id: r.id,
        area: r.area.as_ref().map(|a| crate::command::TextAreaInput {
            x: a.x,
            y: a.y,
            w: a.w,
            h: a.h,
        }),
        rotation: r.rotation,
        text: r.text.clone(),
        start_ms: r.time.map(|t| t.start_ms),
        end_ms: r.time.map(|t| t.end_ms),
        speaker: r.speaker.clone(),
    }
}
