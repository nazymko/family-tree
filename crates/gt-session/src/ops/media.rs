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
