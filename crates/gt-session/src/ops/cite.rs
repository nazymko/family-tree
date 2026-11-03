//! Notes, sources and citations, and the owner lookup they share with media.

use super::{Planner, invalid, new_common, parse_date, text};
use crate::command::{
    CitationArg, CitationInput, CitationPatch, NoteInput, OwnerKind, OwnerRef, SourceInput,
    SourceRef,
};
use crate::dto::NoteKindDto;
use crate::error::ApiError;
use gt_core::ids::new_id;
use gt_core::model::*;
use uuid::Uuid;

/// The lists of a record that notes, citations and media links can be added to.
pub(super) struct Fields<'a> {
    pub notes: Option<&'a mut Vec<Uuid>>,
    pub citations: Option<&'a mut Vec<Uuid>>,
    pub media: Option<&'a mut Vec<MediaLink>>,
}

fn common_fields(c: &mut Common) -> Fields<'_> {
    Fields {
        notes: Some(&mut c.notes),
        citations: Some(&mut c.citations),
        media: Some(&mut c.media),
    }
}

/// Runs `f` on the owner's lists and stores the owner again. Returns what `f` returned and
/// the person to focus (the person, or the principal of an event).
pub(super) fn with_owner<R>(
    p: &mut Planner,
    target: OwnerRef,
    f: impl FnOnce(&mut Fields) -> Result<R, ApiError>,
) -> Result<(R, Option<Uuid>), ApiError> {
    let now = p.now;
    match target.kind {
        OwnerKind::Person => {
            let mut x: Person = p.get(target.id)?;
            let r = f(&mut common_fields(&mut x.common))?;
            x.common.changed = Some(now);
            p.put(x);
            Ok((r, Some(target.id)))
        }
        OwnerKind::Family => {
            let mut x: Family = p.get(target.id)?;
            let r = f(&mut common_fields(&mut x.common))?;
            x.common.changed = Some(now);
            p.put(x);
            Ok((r, None))
        }
        OwnerKind::Event => {
            let mut x: Event = p.get(target.id)?;
            let r = f(&mut common_fields(&mut x.common))?;
            x.common.changed = Some(now);
            let principal = x.participants.iter().find_map(|q| match q.target {
                ParticipantRef::Person(id) if q.role == Role::Principal => Some(id),
                _ => None,
            });
            p.put(x);
            if let Some(id) = principal {
                p.bump_person(id);
            }
            Ok((r, principal))
        }
        OwnerKind::Media => {
            let mut x: Media = p.get(target.id)?;
            let r = f(&mut common_fields(&mut x.common))?;
            x.common.changed = Some(now);
            p.put(x);
            Ok((r, None))
        }
        OwnerKind::Citation => {
            let mut x: Citation = p.get(target.id)?;
            let r = f(&mut common_fields(&mut x.common))?;
            x.common.changed = Some(now);
            p.put(x);
            Ok((r, None))
        }
        OwnerKind::Name => {
            let pid = p
                .tree
                .persons
                .values()
                .find(|q| q.names.iter().any(|n| n.id == target.id))
                .map(|q| q.common.id)
                .ok_or_else(|| ApiError::not_found("name", target.id))?;
            let mut x: Person = p.get(pid)?;
            let Some(name) = x.names.iter_mut().find(|n| n.id == target.id) else {
                return Err(ApiError::not_found("name", target.id));
            };
            let r = f(&mut Fields {
                notes: Some(&mut name.notes),
                citations: Some(&mut name.citations),
                media: None,
            })?;
            x.common.changed = Some(now);
            p.put(x);
            Ok((r, Some(pid)))
        }
        OwnerKind::ChildLink => Err(invalid(
            "Notes and citations cannot be attached to this record.",
        )),
    }
}

fn note_kind(k: NoteKindDto, keep: Option<&NoteKind>) -> NoteKind {
    match k {
        NoteKindDto::General => NoteKind::General,
        NoteKindDto::Story => NoteKind::Story,
        NoteKindDto::Research => NoteKind::Research,
        NoteKindDto::Transcription => NoteKind::Transcription,
        NoteKindDto::Conclusion => NoteKind::Conclusion,
        NoteKindDto::Other => match keep {
            Some(NoteKind::Other(t)) => NoteKind::Other(t.clone()),
            _ => NoteKind::Other(String::new()),
        },
    }
}

pub(super) fn save_note(
    p: &mut Planner,
    target: OwnerRef,
    input: &NoteInput,
) -> Result<Option<Uuid>, ApiError> {
    if input.text.trim().is_empty() {
        return Err(invalid("Write some text for the note."));
    }
    let (note_id, note) = match input.id {
        Some(id) => {
            let mut n: Note = p.get(id)?;
            n.kind = note_kind(input.kind, Some(&n.kind));
            n.text = input.text.clone();
            n.common.changed = Some(p.now);
            (id, n)
        }
        None => {
            let n = Note {
                common: new_common(p.now),
                kind: note_kind(input.kind, None),
                text: input.text.clone(),
                lang: None,
                shared: false,
            };
            (n.common.id, n)
        }
    };
    let (_, focus) = with_owner(p, target, |f| {
        let Some(notes) = f.notes.as_mut() else {
            return Err(invalid("Notes cannot be attached to this record."));
        };
        if input.id.is_some() {
            if !notes.contains(&note_id) {
                return Err(ApiError::not_found("note", note_id));
            }
        } else {
            notes.push(note_id);
        }
        Ok(())
    })?;
    p.put(note);
    Ok(focus)
}

pub(super) fn delete_note(
    p: &mut Planner,
    target: OwnerRef,
    note: Uuid,
) -> Result<Option<Uuid>, ApiError> {
    let (_, focus) = with_owner(p, target, |f| {
        let notes = f
            .notes
            .as_mut()
            .ok_or_else(|| invalid("Notes cannot be attached to this record."))?;
        let i = notes
            .iter()
            .position(|n| *n == note)
            .ok_or_else(|| ApiError::not_found("note", note))?;
        notes.remove(i);
        Ok(())
    })?;
    p.abandon(&[], &[note], false);
    Ok(focus)
}

fn set_opt(slot: &mut Option<String>, v: &Option<String>) {
    if let Some(v) = v {
        *slot = text(Some(v));
    }
}

pub(super) fn save_source(p: &mut Planner, input: &SourceInput) -> Result<Option<Uuid>, ApiError> {
    match input.id {
        Some(id) => {
            let mut s: Source = p.get(id)?;
            set_opt(&mut s.title, &input.title);
            set_opt(&mut s.author, &input.author);
            set_opt(&mut s.publication, &input.publication);
            set_opt(&mut s.abbreviation, &input.abbreviation);
            set_opt(&mut s.text, &input.text);
            s.common.changed = Some(p.now);
            p.put(s);
        }
        None => {
            let Some(title) = text(input.title.as_deref()) else {
                return Err(invalid("Enter a source title."));
            };
            p.put(Source {
                common: new_common(p.now),
                title: Some(title),
                author: text(input.author.as_deref()),
                publication: text(input.publication.as_deref()),
                abbreviation: text(input.abbreviation.as_deref()),
                text: text(input.text.as_deref()),
                repositories: Vec::new(),
            });
        }
    }
    Ok(None)
}

fn source_for(p: &mut Planner, source: &SourceRef) -> Result<Uuid, ApiError> {
    match source {
        SourceRef::Id(id) => p.get::<Source>(*id).map(|_| *id),
        SourceRef::New(n) => {
            let Some(title) = text(Some(&n.title)) else {
                return Err(invalid("Enter a source title."));
            };
            if let Some(found) = p.tree.sources.values().find(|s| {
                s.title
                    .as_deref()
                    .is_some_and(|t| t.trim().eq_ignore_ascii_case(&title))
            }) {
                return Ok(found.common.id);
            }
            let s = Source {
                common: new_common(p.now),
                title: Some(title),
                author: None,
                publication: None,
                abbreviation: None,
                text: None,
                repositories: Vec::new(),
            };
            let id = s.common.id;
            p.put(s);
            Ok(id)
        }
    }
}

fn build_citation(p: &mut Planner, c: &CitationInput) -> Result<Citation, ApiError> {
    let source = source_for(p, &c.source)?;
    let ev = c.evidence.unwrap_or_default();
    Ok(Citation {
        common: new_common(p.now),
        source: Some(source),
        page: text(c.page.as_deref()),
        date: parse_date(c.date_raw.as_deref(), p.order),
        text: text(c.text.as_deref()),
        quality: c.quality.map(|q| q.min(3)),
        evidence: Evidence {
            original: ev.original,
            primary_info: ev.primary_info,
            direct: ev.direct,
        },
    })
}

fn copy_of(c: &Citation, now: i64) -> Citation {
    let mut copy = c.clone();
    copy.common.id = new_id();
    copy.common.changed = Some(now);
    copy.common.ext_ids.retain(|e| e.kind != ExtIdKind::Xref);
    copy
}

pub(super) fn add_citation(
    p: &mut Planner,
    target: OwnerRef,
    arg: &CitationArg,
    last: Option<&Citation>,
) -> Result<Option<Uuid>, ApiError> {
    let cit = match arg {
        CitationArg::Reuse(r) => {
            if !r.reuse_last {
                return Err(invalid("No citation to reuse."));
            }
            let last = last.ok_or_else(|| invalid("No citation to reuse."))?;
            let mut copy = copy_of(last, p.now);
            // The source may have been removed since (an undone edit): copy without it.
            if copy.source.is_some_and(|s| p.find::<Source>(s).is_none()) {
                copy.source = None;
            }
            copy
        }
        CitationArg::Clone(c) => copy_of(&p.get::<Citation>(c.clone_of)?, p.now),
        CitationArg::New(input) => build_citation(p, input)?,
    };
    let id = cit.common.id;
    let (_, focus) = with_owner(p, target, |f| {
        f.citations
            .as_mut()
            .ok_or_else(|| invalid("Citations cannot be attached to this record."))?
            .push(id);
        Ok(())
    })?;
    p.put(cit);
    Ok(focus)
}

pub(super) fn update_citation(
    p: &mut Planner,
    id: Uuid,
    patch: &CitationPatch,
) -> Result<Option<Uuid>, ApiError> {
    let mut c: Citation = p.get(id)?;
    if let Some(s) = patch.source {
        p.get::<Source>(s)?;
        c.source = Some(s);
    }
    set_opt(&mut c.page, &patch.page);
    set_opt(&mut c.text, &patch.text);
    if let Some(raw) = &patch.date_raw {
        c.date = parse_date(Some(raw), p.order);
    }
    if let Some(q) = patch.quality {
        c.quality = Some(q.min(3));
    }
    if let Some(e) = patch.evidence {
        c.evidence.original = e.original.or(c.evidence.original);
        c.evidence.primary_info = e.primary_info.or(c.evidence.primary_info);
        c.evidence.direct = e.direct.or(c.evidence.direct);
    }
    c.common.changed = Some(p.now);
    p.put(c);
    Ok(None)
}

pub(super) fn remove_citation(
    p: &mut Planner,
    target: OwnerRef,
    citation: Uuid,
) -> Result<Option<Uuid>, ApiError> {
    let (_, focus) = with_owner(p, target, |f| {
        let list = f
            .citations
            .as_mut()
            .ok_or_else(|| invalid("Citations cannot be attached to this record."))?;
        let i = list
            .iter()
            .position(|c| *c == citation)
            .ok_or_else(|| ApiError::not_found("citation", citation))?;
        list.remove(i);
        Ok(())
    })?;
    p.abandon(&[citation], &[], false);
    Ok(focus)
}
