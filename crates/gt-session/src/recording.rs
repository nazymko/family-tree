//! A sound recording as the full player shows it: the file's details, the sources that hold it,
//! and every clip of it with the record it is linked to.

use crate::command::{OwnerKind, OwnerRef};
use crate::dto::{ClipDto, MediaDto, PersonRef};
use crate::error::ApiError;
use crate::person_detail::{clip_dto, event_kind_id};
use crate::session::Session;
use gt_core::model::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecordingDto {
    pub media: MediaDto,
    /// The sources the whole recording is attached to.
    pub sources: Vec<RecordingSourceDto>,
    /// Every clip of the recording, in time order.
    pub clips: Vec<RecordingClipDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecordingSourceDto {
    pub id: Uuid,
    pub title: Option<String>,
}

/// One clip and where it is linked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecordingClipDto {
    pub clip: ClipDto,
    pub target: OwnerRef,
    /// Words from the record itself, never from the app: a note's first line, a source's title
    /// and page, a family's partners. Absent for a person or event (see `person`, `eventKind`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    /// For an event: its kind, as `EventKindId` names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub event_kind: Option<String>,
    /// The person whose page shows it: the person, an event's principal, a family's first
    /// partner, or the person holding the note or citation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub person: Option<PersonRef>,
}

/// Characters of a note's text that name it in a list.
const NOTE_TITLE_CHARS: usize = 80;

impl Session {
    /// The full player's view of a recording. Never reads the file beyond its headers.
    pub fn get_recording(&self, media: Uuid) -> Result<RecordingDto, ApiError> {
        let m = self
            .tree
            .media
            .get(&media)
            .ok_or_else(|| ApiError::not_found("media", media))?;
        let whole = MediaLink {
            media,
            region: None,
            primary: false,
            clip: None,
        };
        let dto = self
            .media_dtos(std::slice::from_ref(&whole))
            .pop()
            .map(|l| l.media)
            .ok_or_else(|| ApiError::not_found("media", m.common.id))?;
        let mut sources: Vec<RecordingSourceDto> = self
            .tree
            .sources
            .values()
            .filter(|s| s.common.media.iter().any(|l| l.media == media))
            .map(|s| RecordingSourceDto {
                id: s.common.id,
                title: s.title.clone(),
            })
            .collect();
        sources.sort_by(|a, b| (&a.title, a.id).cmp(&(&b.title, b.id)));
        let mut clips = Vec::new();
        for (target, links) in self.clip_holders() {
            for l in links.iter().filter(|l| l.media == media) {
                if let Some(c) = &l.clip {
                    clips.push(self.recording_clip(c, target));
                }
            }
        }
        clips.sort_by_key(|c| (c.clip.start_ms, c.clip.end_ms, c.clip.id));
        Ok(RecordingDto {
            media: dto,
            sources,
            clips,
        })
    }

    /// The clip `clip` on `target`, as it stands.
    pub fn get_recording_clip(&self, target: OwnerRef, clip: Uuid) -> Option<Clip> {
        self.owner_links(target)?
            .iter()
            .find_map(|l| l.clip.as_ref().filter(|c| c.id == clip).cloned())
    }

    /// Every record that can hold a clip, with its media links.
    pub(crate) fn clip_holders(&self) -> Vec<(OwnerRef, &[MediaLink])> {
        let t = &self.tree;
        let mut out: Vec<(OwnerRef, &[MediaLink])> = Vec::new();
        let mut add = |kind: OwnerKind, c: &'_ Common| {
            if c.media.iter().any(|l| l.clip.is_some()) {
                out.push((OwnerRef { kind, id: c.id }, &[]));
            }
        };
        for x in t.persons.values() {
            add(OwnerKind::Person, &x.common);
        }
        for x in t.families.values() {
            add(OwnerKind::Family, &x.common);
        }
        for x in t.events.values() {
            add(OwnerKind::Event, &x.common);
        }
        for x in t.citations.values() {
            add(OwnerKind::Citation, &x.common);
        }
        for x in t.notes.values() {
            add(OwnerKind::Note, &x.common);
        }
        out.into_iter()
            .filter_map(|(target, _)| Some((target, self.owner_links(target)?)))
            .collect()
    }

    pub(crate) fn recording_clip(&self, c: &Clip, target: OwnerRef) -> RecordingClipDto {
        let t = &self.tree;
        let mut title = None;
        let mut event_kind = None;
        let person = match target.kind {
            OwnerKind::Person => Some(target.id),
            OwnerKind::Event => t.events.get(&target.id).and_then(|e| {
                event_kind = Some(event_kind_id(&e.kind));
                principal(e)
            }),
            OwnerKind::Family => t.families.get(&target.id).and_then(|f| {
                let names: Vec<String> = [f.partner1, f.partner2]
                    .into_iter()
                    .flatten()
                    .filter_map(|p| self.person_ref(p).map(|r| r.name))
                    .collect();
                title = (!names.is_empty()).then(|| names.join(" & "));
                f.partner1.or(f.partner2)
            }),
            OwnerKind::Note => {
                title = t.notes.get(&target.id).map(|n| note_title(&n.text));
                self.holder_person(target.id, true)
            }
            OwnerKind::Citation => {
                title = t.citations.get(&target.id).map(|ci| {
                    let source = ci
                        .source
                        .and_then(|s| t.sources.get(&s))
                        .and_then(|s| s.title.clone());
                    [source, ci.page.clone()]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(", ")
                });
                self.holder_person(target.id, false)
            }
            _ => None,
        };
        RecordingClipDto {
            clip: clip_dto(c),
            target,
            title: title.filter(|t| !t.is_empty()),
            event_kind,
            person: person.and_then(|p| self.person_ref(p)),
        }
    }

    /// The person whose page shows a note (`note`) or citation: the person holding it, or the
    /// principal of the event, or the first partner of the family, that holds it.
    fn holder_person(&self, id: Uuid, note: bool) -> Option<Uuid> {
        let t = &self.tree;
        let held = |c: &Common| {
            if note {
                c.notes.contains(&id)
            } else {
                c.citations.contains(&id)
            }
        };
        let by_name = |p: &Person| {
            p.names.iter().any(|n| {
                if note {
                    n.notes.contains(&id)
                } else {
                    n.citations.contains(&id)
                }
            })
        };
        if let Some(p) = t.persons.values().find(|p| held(&p.common) || by_name(p)) {
            return Some(p.common.id);
        }
        if let Some(e) = t.events.values().find(|e| held(&e.common)) {
            return principal(e);
        }
        if let Some(f) = t.families.values().find(|f| held(&f.common)) {
            return f.partner1.or(f.partner2);
        }
        // A note on a citation: the person holding the citation.
        if note && let Some(c) = t.citations.values().find(|c| held(&c.common)) {
            return self.holder_person(c.common.id, false);
        }
        None
    }
}

fn principal(e: &Event) -> Option<Uuid> {
    e.participants.iter().find_map(|q| match q.target {
        ParticipantRef::Person(id) if q.role == Role::Principal => Some(id),
        _ => None,
    })
}

/// A note's first line, cut to a short length.
fn note_title(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default();
    let mut out: String = line.chars().take(NOTE_TITLE_CHARS).collect();
    if line.chars().count() > NOTE_TITLE_CHARS {
        out.push('…');
    }
    out
}
