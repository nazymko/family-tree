//! attachMedia, detachMedia, deleteMedia, setPrimaryMedia, updateMedia.

use super::cite::with_owner;
use super::{Planner, invalid, parse_date, text};
use crate::command::OwnerRef;
use crate::error::ApiError;
use gt_core::date::DayOrder;
use gt_core::model::*;
use gt_core::tree::Tree;
use gt_store::ChangeSet;
use std::collections::HashMap;
use uuid::Uuid;

pub(super) fn attach_media(
    p: &mut Planner,
    target: OwnerRef,
    media: &[Uuid],
) -> Result<Option<Uuid>, ApiError> {
    for m in media {
        p.get::<Media>(*m)?;
    }
    let (_, focus) = with_owner(p, target, |f| {
        let links = f
            .media
            .as_mut()
            .ok_or_else(|| invalid("Media cannot be attached to this record."))?;
        for m in media {
            if !links.iter().any(|l| l.media == *m) {
                links.push(MediaLink {
                    media: *m,
                    region: None,
                    primary: links.is_empty(),
                });
            }
        }
        Ok(())
    })?;
    Ok(focus)
}

pub(super) fn detach_media(
    p: &mut Planner,
    target: OwnerRef,
    media: Uuid,
) -> Result<Option<Uuid>, ApiError> {
    let (_, focus) = with_owner(p, target, |f| {
        let links = f
            .media
            .as_mut()
            .ok_or_else(|| invalid("Media cannot be attached to this record."))?;
        let i = links
            .iter()
            .position(|l| l.media == media)
            .ok_or_else(|| ApiError::not_found("media link", media))?;
        let gone = links.remove(i);
        if gone.primary
            && !links.iter().any(|l| l.primary)
            && let Some(first) = links.first_mut()
        {
            first.primary = true;
        }
        Ok(())
    })?;
    Ok(focus)
}

/// Takes the link out of one record; the next link becomes primary if that one was.
fn drop_link(common: &mut Common, media: Uuid, now: i64) {
    let Some(i) = common.media.iter().position(|l| l.media == media) else {
        return;
    };
    let gone = common.media.remove(i);
    if gone.primary
        && !common.media.iter().any(|l| l.primary)
        && let Some(first) = common.media.first_mut()
    {
        first.primary = true;
    }
    common.changed = Some(now);
}

/// Deletes the media record and takes it off every record it is linked to. The file stays in the project's content
/// store: undo needs it, and the same content may be attached again.
pub(super) fn delete_media(p: &mut Planner, id: Uuid) -> Result<Option<Uuid>, ApiError> {
    let media: Media = p.get(id)?;
    macro_rules! sweep {
        ($($field:ident),*) => {$(
            let hits: Vec<_> = p
                .tree
                .$field
                .values()
                .filter(|x| x.common.media.iter().any(|l| l.media == id))
                .cloned()
                .collect();
            for mut x in hits {
                drop_link(&mut x.common, id, p.now);
                p.put(x);
            }
        )*};
    }
    sweep!(
        persons,
        families,
        events,
        places,
        sources,
        repositories,
        citations,
        notes
    );
    p.abandon(&media.common.citations, &media.common.notes, true);
    p.remove::<Media>(id);
    Ok(None)
}

pub(super) fn set_primary(
    p: &mut Planner,
    person: Uuid,
    media: Uuid,
) -> Result<Option<Uuid>, ApiError> {
    let mut x: Person = p.real_person(person)?;
    if !x.common.media.iter().any(|l| l.media == media) {
        return Err(ApiError::not_found("media link", media));
    }
    for l in &mut x.common.media {
        l.primary = l.media == media;
    }
    x.common.changed = Some(p.now);
    p.put(x);
    Ok(Some(person))
}

pub(super) fn update_media(
    p: &mut Planner,
    id: Uuid,
    title: Option<&str>,
    description: Option<&str>,
    date_raw: Option<&str>,
) -> Result<Option<Uuid>, ApiError> {
    let mut m: Media = p.get(id)?;
    if let Some(t) = title {
        m.title = text(Some(t));
    }
    if let Some(d) = description {
        m.description = text(Some(d));
    }
    if let Some(d) = date_raw {
        m.date = parse_date(Some(d), p.order);
    }
    m.common.changed = Some(p.now);
    p.put(m);
    Ok(None)
}

/// The most pieces one transcription holds.
pub const TRANSCRIPTION_PARTS_MAX: usize = 2000;
/// Bytes of the text of one piece.
pub const TRANSCRIPTION_PART_MAX: usize = 64 * 1024;
/// Bytes of the text of a whole transcription.
pub const TRANSCRIPTION_TEXT_MAX: usize = 1024 * 1024;
/// The smallest width or height of an area, as a fraction of the picture.
const AREA_MIN: f32 = 0.001;

/// An area of the picture, checked and pulled inside it.
fn checked_area(a: &crate::command::TextAreaInput) -> Result<Region, ApiError> {
    let all = [a.x, a.y, a.w, a.h];
    if all.iter().any(|v| !v.is_finite()) {
        return Err(invalid(
            "An area of the picture needs x, y, w and h as numbers.",
        ));
    }
    let x = a.x.clamp(0.0, 1.0);
    let y = a.y.clamp(0.0, 1.0);
    let w = a.w.min(1.0 - x);
    let h = a.h.min(1.0 - y);
    if a.x < -0.01 || a.y < -0.01 || a.x > 1.0 || a.y > 1.0 || w < AREA_MIN || h < AREA_MIN {
        return Err(invalid(
            "An area must lie on the picture: x, y, w and h are fractions from 0 to 1 of its width \
             and height, from the top left corner, and w and h are more than 0.",
        ));
    }
    Ok(Region { x, y, w, h })
}

/// Checks a whole transcription and gives every new piece its id.
pub(crate) fn checked_transcription(
    media: &Media,
    regions: &[crate::command::TextRegionInput],
) -> Result<Vec<TextRegion>, ApiError> {
    let mime = media.mime.as_deref().unwrap_or_default();
    if mime.starts_with("audio/") {
        return Err(invalid("A sound recording has no picture to transcribe."));
    }
    if regions.len() > TRANSCRIPTION_PARTS_MAX {
        return Err(invalid(format!(
            "A transcription holds at most {TRANSCRIPTION_PARTS_MAX} pieces."
        )));
    }
    let total: usize = regions.iter().map(|r| r.text.len()).sum();
    if total > TRANSCRIPTION_TEXT_MAX {
        return Err(invalid(
            "A transcription holds up to 1 MiB of text. Put the rest on a source.",
        ));
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(regions.len());
    for r in regions {
        if r.text.len() > TRANSCRIPTION_PART_MAX {
            return Err(invalid(
                "One piece of a transcription holds up to 64 KiB. Split it into several.",
            ));
        }
        let rotation = r.rotation.unwrap_or(0);
        if !TEXT_ROTATIONS.contains(&rotation) {
            return Err(invalid("rotation must be 0, 90, 180 or 270."));
        }
        let area = match &r.area {
            Some(_) if mime == "application/pdf" => {
                return Err(invalid(
                    "A document has no picture to mark: give its text without an area.",
                ));
            }
            Some(a) => Some(checked_area(a)?),
            None => None,
        };
        let id = r.id.unwrap_or_else(gt_core::ids::new_id);
        if !seen.insert(id) {
            return Err(invalid("Two pieces of the transcription have the same id."));
        }
        out.push(TextRegion {
            id,
            area,
            rotation,
            text: r.text.clone(),
        });
    }
    Ok(out)
}

pub(super) fn set_transcription(
    p: &mut Planner,
    id: Uuid,
    regions: &[crate::command::TextRegionInput],
) -> Result<Option<Uuid>, ApiError> {
    let mut m: Media = p.get(id)?;
    m.transcription = checked_transcription(&m, regions)?;
    m.common.changed = Some(p.now);
    p.put(m);
    Ok(None)
}

/// Links new media records to `target` in one change set. Content already stored as a media
/// record (same sha256) is reused rather than duplicated. Per input, the linked media id, or
/// `None` when that content is already attached to the target.
pub(crate) fn plan_attach(
    tree: &Tree,
    target: OwnerRef,
    staged: Vec<Media>,
    now: i64,
    order: DayOrder,
) -> Result<(ChangeSet, Vec<Option<Uuid>>), ApiError> {
    let mut p = Planner::new(tree, now, order);
    let mut by_sha: HashMap<String, Uuid> = tree
        .media
        .values()
        .filter_map(|m| Some((m.sha256.clone()?, m.common.id)))
        .collect();
    let mut fresh: Vec<Media> = Vec::new();
    // Existing records the content matched, with the type the file's own bytes show.
    let mut reused: Vec<(Uuid, Option<String>)> = Vec::new();
    let (outcomes, _) = with_owner(&mut p, target, |f| {
        let links = f
            .media
            .as_mut()
            .ok_or_else(|| invalid("Media cannot be attached to this record."))?;
        let mut out = Vec::with_capacity(staged.len());
        for m in &staged {
            let sha = m.sha256.clone().unwrap_or_default();
            if let Some(old) = by_sha.get(&sha)
                && !fresh.iter().any(|x| x.common.id == *old)
            {
                reused.push((*old, m.mime.clone()));
            }
            let id = *by_sha.entry(sha).or_insert_with(|| {
                fresh.push(m.clone());
                m.common.id
            });
            if links.iter().any(|l| l.media == id) {
                out.push(None);
            } else {
                links.push(MediaLink {
                    media: id,
                    region: None,
                    primary: links.is_empty(),
                });
                out.push(Some(id));
            }
        }
        Ok(out)
    })?;
    for m in fresh {
        p.put(m);
    }
    // An imported record may carry free text ("mp3") or nothing as its type. The bytes just
    // showed what the file is, so the record is corrected in the same, logged, change set;
    // otherwise a note would hold a recording it cannot list.
    let mut done: Vec<Uuid> = Vec::new();
    for (id, mime) in reused {
        if done.contains(&id) {
            continue;
        }
        done.push(id);
        let mut old: Media = p.get(id)?;
        if mime.is_some() && old.mime != mime {
            old.mime = mime;
            old.common.changed = Some(now);
            p.put(old);
        }
    }
    Ok((p.finish("Attach media".into()), outcomes))
}
