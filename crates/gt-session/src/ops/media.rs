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
            if !links.iter().any(|l| l.media == *m && l.clip.is_none()) {
                let primary = !links.iter().any(|l| l.clip.is_none());
                links.push(MediaLink {
                    media: *m,
                    region: None,
                    primary,
                    clip: None,
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
            .position(|l| l.media == media && l.clip.is_none())
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

/// Takes every link to `media` out of one record, clips of it too; the next whole link becomes
/// primary if one that went was.
fn drop_link(common: &mut Common, media: Uuid, now: i64) {
    let was_primary = common.media.iter().any(|l| l.media == media && l.primary);
    common.media.retain(|l| l.media != media);
    if was_primary
        && !common.media.iter().any(|l| l.primary)
        && let Some(first) = common.media.iter_mut().find(|l| l.clip.is_none())
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
    // The other versions of a deleted primary stay a group, under the first of them.
    if media.version_of.is_none() {
        let rest = versions_of(p, id);
        if let Some((next, more)) = rest.split_first() {
            point_at(p, *next, None)?;
            for v in more {
                point_at(p, *v, Some(*next))?;
            }
        }
    }
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
    if !x
        .common
        .media
        .iter()
        .any(|l| l.media == media && l.clip.is_none())
    {
        return Err(ApiError::not_found("media link", media));
    }
    for l in &mut x.common.media {
        l.primary = l.media == media && l.clip.is_none();
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
    place_text: Option<&str>,
    place_id: Option<Uuid>,
) -> Result<Option<Uuid>, ApiError> {
    let mut m: Media = p.get(id)?;
    if let Some(pl) = place_id {
        p.get::<gt_core::model::Place>(pl)?;
        m.place = Some(pl);
    } else if let Some(t) = place_text {
        m.place = super::places::place_for_text(p, t)?;
    }
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
pub const TRANSCRIPTION_PART_MAX: usize = gt_core::model::TEXT_PIECE_MAX;
/// Bytes of the text of a whole transcription.
pub const TRANSCRIPTION_TEXT_MAX: usize = 1024 * 1024;
/// The smallest width or height of an area, as a fraction of the picture.
const AREA_MIN: f32 = gt_core::model::TEXT_AREA_MIN;

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
             and height, from the top left corner, and w and h are at least 0.01.",
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
    let sound = mime.starts_with("audio/");
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
        let (time, speaker) = checked_said(r, sound)?;
        if sound && (r.area.is_some() || rotation != 0) {
            return Err(invalid(
                "A sound recording has no picture: give each piece its text, and its start and \
                 end times, without an area or a rotation.",
            ));
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
            speaker,
            time,
        });
    }
    Ok(out)
}

/// When a piece of a recording's transcript is said, and by whom. A picture's pieces have neither.
fn checked_said(
    r: &crate::command::TextRegionInput,
    sound: bool,
) -> Result<(Option<TimeSpan>, Option<String>), ApiError> {
    let speaker = text(r.speaker.as_deref());
    if !sound {
        if r.start_ms.is_some() || r.end_ms.is_some() || speaker.is_some() {
            return Err(invalid(
                "Times and speakers belong to the transcript of a sound recording.",
            ));
        }
        return Ok((None, None));
    }
    if speaker
        .as_ref()
        .is_some_and(|s| s.chars().count() > SPEAKER_MAX)
    {
        return Err(invalid(format!(
            "A speaker's name holds at most {SPEAKER_MAX} characters."
        )));
    }
    let time = match (r.start_ms, r.end_ms) {
        (None, None) => None,
        (Some(_), Some(end_ms)) if end_ms > TIME_MAX_MS => {
            return Err(invalid("A time in a recording is at most 100 hours."));
        }
        (Some(start_ms), Some(end_ms)) if start_ms < end_ms => Some(TimeSpan { start_ms, end_ms }),
        (Some(_), Some(_)) => {
            return Err(invalid("A piece of a transcript must end after it starts."));
        }
        _ => {
            return Err(invalid(
                "Give a piece of a transcript both its start and its end, or neither.",
            ));
        }
    };
    Ok((time, speaker))
}

pub(super) fn set_transcription(
    p: &mut Planner,
    id: Uuid,
    regions: &[crate::command::TextRegionInput],
    known: Option<&[Uuid]>,
) -> Result<Option<Uuid>, ApiError> {
    let mut m: Media = p.get(id)?;
    let mut next = checked_transcription(&m, regions)?;
    if let Some(known) = known {
        // Pieces stored since the sender read the list stay: each after the piece it followed
        // (or first), so a save from an older view never drops them.
        let mut after: Option<Uuid> = None;
        for r in &m.transcription {
            if !known.contains(&r.id) && !next.iter().any(|n| n.id == r.id) {
                let at = after
                    .and_then(|a| next.iter().position(|n| n.id == a))
                    .map_or(0, |i| i + 1);
                next.insert(at.min(next.len()), r.clone());
            }
            after = Some(r.id);
        }
    }
    m.transcription = next;
    m.common.changed = Some(p.now);
    p.put(m);
    Ok(None)
}

/// Turns how a picture is shown. Its transcription stays exactly where it is: the areas are on
/// the picture upright by EXIF, before this turn.
pub(super) fn set_rotation(
    p: &mut Planner,
    id: Uuid,
    rotation: u16,
) -> Result<Option<Uuid>, ApiError> {
    if !TEXT_ROTATIONS.contains(&rotation) {
        return Err(invalid("rotation must be 0, 90, 180 or 270."));
    }
    let mut m: Media = p.get(id)?;
    if !can_turn(&m) {
        return Err(invalid("Only a picture can be turned."));
    }
    m.rotation = Some(rotation);
    m.common.changed = Some(p.now);
    p.put(m);
    Ok(None)
}

/// The records a clip can be linked to: where a story is told.
fn clip_target(target: OwnerRef) -> Result<(), ApiError> {
    use crate::command::OwnerKind as K;
    match target.kind {
        K::Note | K::Citation | K::Event | K::Person | K::Family => Ok(()),
        _ => Err(invalid(
            "A clip goes on a note, citation, event, person or family.",
        )),
    }
}

/// A clip's times and label, checked; the label trimmed (empty is none). The length of the
/// recording is checked by the session, which can read the file.
/// No time in a recording lies past this: 100 hours. It keeps every stored time far inside the
/// range the store can hold, also when the recording's own length could not be read.
const TIME_MAX_MS: u64 = 100 * 60 * 60 * 1000;

fn checked_clip(mut c: Clip) -> Result<Clip, ApiError> {
    if c.start_ms >= c.end_ms {
        return Err(invalid("A clip must end after it starts."));
    }
    if c.end_ms > TIME_MAX_MS {
        return Err(invalid("A time in a recording is at most 100 hours."));
    }
    c.label = text(c.label.as_deref());
    if c.label
        .as_ref()
        .is_some_and(|l| l.chars().count() > CLIP_LABEL_MAX)
    {
        return Err(invalid(format!(
            "A clip's label holds at most {CLIP_LABEL_MAX} characters."
        )));
    }
    Ok(c)
}

/// Whether the record is a sound recording, by its stored type. The session checks the file.
fn is_recording(m: &Media) -> bool {
    m.mime.as_deref().is_some_and(|t| t.starts_with("audio/"))
}

pub(super) fn add_clip(
    p: &mut Planner,
    target: OwnerRef,
    media: Uuid,
    clip: Clip,
) -> Result<Option<Uuid>, ApiError> {
    clip_target(target)?;
    let m: Media = p.get(media)?;
    if !is_recording(&m) {
        return Err(invalid("Only a sound recording has clips."));
    }
    let clip = checked_clip(clip)?;
    let (_, focus) = with_owner(p, target, |f| {
        let links = f
            .media
            .as_mut()
            .ok_or_else(|| invalid("A clip cannot be attached to this record."))?;
        let same = |l: &MediaLink| {
            l.media == media
                && l.clip.as_ref().is_some_and(|c| {
                    (c.start_ms, c.end_ms, &c.label) == (clip.start_ms, clip.end_ms, &clip.label)
                })
        };
        if links.iter().any(same) {
            return Err(invalid("That clip is already on this record."));
        }
        links.push(MediaLink {
            media,
            region: None,
            primary: false,
            clip: Some(clip),
        });
        Ok(())
    })?;
    Ok(focus)
}

/// The link of clip `id` on `target`.
fn clip_link(p: &Planner, target: OwnerRef, id: Uuid) -> Result<MediaLink, ApiError> {
    use crate::command::OwnerKind as K;
    let links = match target.kind {
        K::Person => p.get::<Person>(target.id)?.common.media,
        K::Family => p.get::<Family>(target.id)?.common.media,
        K::Event => p.get::<Event>(target.id)?.common.media,
        K::Citation => p.get::<Citation>(target.id)?.common.media,
        K::Note => p.get::<Note>(target.id)?.common.media,
        _ => return Err(ApiError::not_found("clip", id)),
    };
    links
        .iter()
        .find(|l| l.clip.as_ref().is_some_and(|c| c.id == id))
        .cloned()
        .ok_or_else(|| ApiError::not_found("clip", id))
}

pub(super) fn update_clip(
    p: &mut Planner,
    target: OwnerRef,
    clip: Clip,
    move_to: Option<OwnerRef>,
) -> Result<Option<Uuid>, ApiError> {
    let old = clip_link(p, target, clip.id)?;
    let clip = checked_clip(clip)?;
    let id = clip.id;
    let to = move_to.filter(|t| *t != target);
    match to {
        None => {
            let (_, focus) = with_owner(p, target, |f| {
                if let Some(l) = f.media.as_mut().and_then(|links| {
                    links
                        .iter_mut()
                        .find(|l| l.clip.as_ref().is_some_and(|c| c.id == id))
                }) {
                    l.clip = Some(clip);
                }
                Ok(())
            })?;
            Ok(focus)
        }
        Some(to) => {
            clip_target(to)?;
            delete_clip(p, target, id)?;
            let (_, focus) = with_owner(p, to, |f| {
                let links = f
                    .media
                    .as_mut()
                    .ok_or_else(|| invalid("A clip cannot be attached to this record."))?;
                links.push(MediaLink {
                    media: old.media,
                    region: None,
                    primary: false,
                    clip: Some(clip),
                });
                Ok(())
            })?;
            Ok(focus)
        }
    }
}

pub(super) fn delete_clip(
    p: &mut Planner,
    target: OwnerRef,
    id: Uuid,
) -> Result<Option<Uuid>, ApiError> {
    clip_link(p, target, id)?;
    let (_, focus) = with_owner(p, target, |f| {
        if let Some(links) = f.media.as_mut() {
            links.retain(|l| l.clip.as_ref().is_none_or(|c| c.id != id));
        }
        Ok(())
    })?;
    Ok(focus)
}

/// Every record whose `version_of` is `primary`, as planned so far, in tree order.
fn versions_of(p: &Planner, primary: Uuid) -> Vec<Uuid> {
    p.tree
        .media
        .keys()
        .filter(|id| **id != primary)
        .filter(|id| {
            p.find::<Media>(**id)
                .is_some_and(|m| m.version_of == Some(primary))
        })
        .copied()
        .collect()
}

/// Sets which record a media record is a version of; a record that already says so is not written.
fn point_at(p: &mut Planner, id: Uuid, to: Option<Uuid>) -> Result<(), ApiError> {
    let mut m: Media = p.get(id)?;
    if m.version_of != to {
        m.version_of = to;
        m.common.changed = Some(p.now);
        p.put(m);
    }
    Ok(())
}

/// The largest number of files in one version group.
pub const VERSION_GROUP_MAX: usize = 50;

/// Makes `others` versions of `primary`. Each file's group comes along whole, and every member
/// ends up pointing at `primary`, which points at nothing. Files, transcriptions and links stay.
pub(super) fn group_versions(
    p: &mut Planner,
    primary: Uuid,
    others: &[Uuid],
) -> Result<Option<Uuid>, ApiError> {
    let head: Media = p.get(primary)?;
    let mut members: Vec<Uuid> = Vec::new();
    let add = |id: Uuid, members: &mut Vec<Uuid>| {
        if id != primary && !members.contains(&id) {
            members.push(id);
        }
    };
    let mut roots = vec![head.version_of.unwrap_or(primary)];
    for o in others {
        let m: Media = p.get(*o)?;
        if is_sound(&m) != is_sound(&head) {
            return Err(invalid(
                "A sound recording and a picture cannot be versions of one another.",
            ));
        }
        roots.push(m.version_of.unwrap_or(*o));
    }
    for r in roots {
        add(r, &mut members);
        for v in versions_of(p, r) {
            add(v, &mut members);
        }
    }
    if members.is_empty() {
        return Err(invalid("Choose another file of the same page."));
    }
    if members.len() + 1 > VERSION_GROUP_MAX {
        return Err(invalid(format!(
            "A page can have at most {VERSION_GROUP_MAX} versions."
        )));
    }
    point_at(p, primary, None)?;
    for id in members {
        point_at(p, id, Some(primary))?;
    }
    Ok(None)
}

/// Takes `media` out of its version group. When it was the primary, `new_primary` (one of the
/// other versions), or else the first of them, becomes the primary of the rest; a group left with
/// one file is no group any more.
pub(super) fn ungroup_version(
    p: &mut Planner,
    media: Uuid,
    new_primary: Option<Uuid>,
) -> Result<Option<Uuid>, ApiError> {
    let m: Media = p.get(media)?;
    match m.version_of {
        // The primary of a group left with one file points at nothing already: a plain file.
        Some(_) => point_at(p, media, None)?,
        None => {
            let rest = versions_of(p, media);
            if rest.is_empty() {
                return Err(invalid("This file has no other versions."));
            }
            let next = match new_primary {
                Some(n) if rest.contains(&n) => n,
                Some(_) => {
                    return Err(invalid(
                        "The new primary must be one of the other versions.",
                    ));
                }
                None => rest[0],
            };
            point_at(p, next, None)?;
            for id in rest.into_iter().filter(|id| *id != next) {
                point_at(p, id, Some(next))?;
            }
        }
    }
    Ok(None)
}

fn is_sound(m: &Media) -> bool {
    m.mime.as_deref().is_some_and(|t| t.starts_with("audio/"))
}

/// Whether a media record is a picture that can be shown turned: not a recording or a document.
pub(crate) fn can_turn(m: &Media) -> bool {
    let mime = m.mime.as_deref().unwrap_or_default();
    !(mime.starts_with("audio/") || mime == "application/pdf")
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
    may_be_primary: bool,
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
            if links.iter().any(|l| l.media == id && l.clip.is_none()) {
                out.push(None);
            } else {
                links.push(MediaLink {
                    media: id,
                    region: None,
                    primary: may_be_primary && !links.iter().any(|l| l.clip.is_none()),
                    clip: None,
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
