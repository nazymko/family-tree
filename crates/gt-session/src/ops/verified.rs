//! Verified corrections by an agent: a change to data someone else wrote, with the record that
//! shows the right value cited on what changed. Each plan is one change set, so one undo takes
//! back the change and its citation together. The agent gate (`agent/verified.rs`) checks the
//! evidence and the batch; these only plan.

use super::cite::{add_citation, save_note};
use super::family::{link_child, unlink_child};
use super::places::place_for_text;
use super::{Planner, invalid, new_common, text};
use crate::command::{CitationArg, CitationInput, NoteInput, OwnerKind, OwnerRef, SourceRef};
use crate::dto::{NameKindDto, NoteKindDto, ParentRelId};
use crate::error::ApiError;
use gt_core::date::{DayOrder, GenDate};
use gt_core::ids::new_id;
use gt_core::model::*;
use gt_store::ChangeSet;
use uuid::Uuid;

/// The record the change rests on, as the citation will say it.
pub(crate) struct Cited<'a> {
    pub source: Uuid,
    pub page: &'a str,
    pub quote: &'a str,
    pub quality: Option<u8>,
}

fn cite(p: &mut Planner, owner: OwnerRef, c: &Cited) -> Result<(), ApiError> {
    let arg = CitationArg::New(CitationInput {
        source: SourceRef::Id(c.source),
        page: Some(c.page.to_string()),
        date_raw: None,
        text: Some(c.quote.to_string()),
        quality: c.quality,
        evidence: None,
    });
    add_citation(p, owner, &arg, None)?;
    Ok(())
}

/// The change set's label: it is where the log keeps the reason.
fn shown(label: &str, reason: &str) -> String {
    let one_line: String = reason.split_whitespace().collect::<Vec<_>>().join(" ");
    format!(
        "{label}: {}",
        one_line.chars().take(200).collect::<String>()
    )
}

/// What a name correction asks for; absent fields stay, empty text clears.
#[derive(Debug, Clone, Default)]
pub struct NamePatch {
    pub given: Option<String>,
    pub patronymic: Option<String>,
    pub surnames: Option<Vec<String>>,
    pub kind: Option<NameKindDto>,
    pub make_preferred: bool,
}

fn kind_of(k: NameKindDto, keep: &NameKind) -> NameKind {
    match k {
        NameKindDto::Birth => NameKind::Birth,
        NameKindDto::Married => NameKind::Married,
        NameKindDto::Aka => NameKind::Aka,
        NameKindDto::Religious => NameKind::Religious,
        NameKindDto::Nickname => NameKind::Nickname,
        NameKindDto::Immigrant => NameKind::Immigrant,
        NameKindDto::Professional => NameKind::Professional,
        NameKindDto::AsRecorded => NameKind::AsRecorded,
        NameKindDto::Other => match keep {
            NameKind::Other(t) => NameKind::Other(t.clone()),
            _ => NameKind::Other(String::new()),
        },
    }
}

/// `None`: the name already is what the patch says.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_name_correction(
    tree: &gt_core::tree::Tree,
    person: Uuid,
    name: Option<Uuid>,
    patch: &NamePatch,
    c: &Cited,
    reason: &str,
    now: i64,
    order: DayOrder,
) -> Result<Option<(ChangeSet, Option<Uuid>)>, ApiError> {
    let mut p = Planner::new(tree, now, order);
    let mut who: Person = p.real_person(person)?;
    let idx = match name {
        Some(id) => who
            .names
            .iter()
            .position(|n| n.id == id)
            .ok_or_else(|| ApiError::not_found("name", id))?,
        None => who
            .names
            .iter()
            .position(|n| n.assertion.preferred)
            .unwrap_or(0),
    };
    let old = who
        .names
        .get(idx)
        .cloned()
        .ok_or_else(|| invalid("This person has no name to correct."))?;
    let mut new = old.clone();
    if let Some(g) = &patch.given {
        new.given = text(Some(g));
    }
    if let Some(g) = &patch.patronymic {
        new.patronymic = text(Some(g));
    }
    if let Some(s) = &patch.surnames {
        new.surnames = s
            .iter()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
    }
    if new.given.is_none() && new.surnames.is_empty() && new.patronymic.is_none() {
        return Err(invalid(
            "A name needs a given name, a patronymic or a surname.",
        ));
    }
    if let Some(k) = patch.kind {
        new.kind = kind_of(k, &old.kind);
    }
    let text_changed =
        new.given != old.given || new.patronymic != old.patronymic || new.surnames != old.surnames;
    if text_changed {
        // The written form no longer matches the parts; the history row keeps it.
        new.raw = None;
    }
    let promote = patch.make_preferred && !old.assertion.preferred;
    if new == old && !promote {
        return Ok(None);
    }
    // The main name's old text stays, as an alternative name, with the citations that backed it.
    let keep_old = text_changed && old.assertion.preferred;
    let mut copy = None;
    if keep_old {
        let mut kept = old.clone();
        kept.id = new_id();
        kept.kind = NameKind::AsRecorded;
        kept.assertion.preferred = false;
        new.citations = Vec::new();
        copy = Some(kept);
    }
    let name_id = new.id;
    who.names[idx] = new;
    if let Some(kept) = copy {
        who.names.push(kept);
    }
    if patch.make_preferred || old.assertion.preferred {
        for n in &mut who.names {
            n.assertion.preferred = n.id == name_id;
        }
    }
    who.common.changed = Some(now);
    p.put(who);
    cite(
        &mut p,
        OwnerRef {
            kind: OwnerKind::Name,
            id: name_id,
        },
        c,
    )?;
    Ok(Some((
        p.finish(shown("Correct name", reason)),
        Some(person),
    )))
}

/// What an event correction asks for; absent fields stay, empty text clears.
#[derive(Debug, Clone, Default)]
pub struct EventPatch {
    pub date: Option<GenDate>,
    pub place_text: Option<String>,
    pub place_id: Option<Uuid>,
    pub description: Option<String>,
    pub age: Option<String>,
    pub cause: Option<String>,
}

/// `None`: the event already is what the patch says.
pub(crate) fn plan_event_correction(
    tree: &gt_core::tree::Tree,
    event: Uuid,
    patch: &EventPatch,
    c: &Cited,
    reason: &str,
    now: i64,
    order: DayOrder,
) -> Result<Option<(ChangeSet, Option<Uuid>)>, ApiError> {
    let mut p = Planner::new(tree, now, order);
    let old: Event = p.get(event)?;
    let mut e = old.clone();
    if let Some(d) = &patch.date {
        let same = old.date.as_ref().is_some_and(|o| {
            let mut as_typed = o.clone();
            as_typed.raw = d.raw.clone();
            &as_typed == d || o.raw.trim().eq_ignore_ascii_case(d.raw.trim())
        });
        if !same {
            e.date = Some(d.clone());
        }
    }
    if let Some(id) = patch.place_id {
        p.get::<Place>(id)?;
        e.place = Some(id);
    } else if let Some(t) = &patch.place_text {
        e.place = place_for_text(&mut p, t)?;
    }
    if let Some(t) = &patch.description {
        e.description = text(Some(t));
    }
    if let Some(t) = &patch.age {
        e.age = text(Some(t));
    }
    if let Some(t) = &patch.cause {
        e.cause = text(Some(t));
    }
    if e == old {
        return Ok(None);
    }
    e.common.changed = Some(now);
    let focus = e.participants.iter().find_map(|q| match q.target {
        ParticipantRef::Person(id) if q.role == Role::Principal => Some(id),
        _ => None,
    });
    p.put(e);
    cite(
        &mut p,
        OwnerRef {
            kind: OwnerKind::Event,
            id: event,
        },
        c,
    )?;
    Ok(Some((p.finish(shown("Correct event", reason)), focus)))
}

/// `None`: the note already says this.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_note_correction(
    tree: &gt_core::tree::Tree,
    note: Uuid,
    holder: OwnerRef,
    new_text: &str,
    story: Option<&str>,
    clear_story: bool,
    c: &Cited,
    reason: &str,
    now: i64,
    order: DayOrder,
) -> Result<Option<(ChangeSet, Option<Uuid>)>, ApiError> {
    let mut p = Planner::new(tree, now, order);
    let old: Note = p.get(note)?;
    let mut n = old.clone();
    n.text = new_text.to_string();
    if clear_story {
        n.story = None;
    } else if let Some(s) = story {
        n.story = Some(s.trim().to_string()).filter(|s| !s.is_empty());
    }
    if n == old {
        return Ok(None);
    }
    n.common.changed = Some(now);
    p.put(n);
    cite(&mut p, holder, c)?;
    Ok(Some((p.finish(shown("Correct note", reason)), None)))
}

/// The child's link moves from `from` to `to`; the link kind, the preferred flag and the link's
/// own citations go along, and the record's words are cited on the new link. A research note on
/// the child says what moved and why.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_child_move(
    tree: &gt_core::tree::Tree,
    child: Uuid,
    from: Uuid,
    to: Uuid,
    note: &str,
    c: &Cited,
    reason: &str,
    now: i64,
    order: DayOrder,
) -> Result<(ChangeSet, Option<Uuid>), ApiError> {
    let mut p = Planner::new(tree, now, order);
    let source: Family = p.get(from)?;
    let link = source
        .children
        .iter()
        .find(|l| l.person == child)
        .cloned()
        .ok_or_else(|| ApiError::not_found("child link", child))?;
    let target: Family = p.get(to)?;
    // The shared parent keeps their kind of link; a parent the target adds takes the same kind.
    let rel_for = |parent: Option<Uuid>| -> Option<ParentRel> {
        let parent = parent?;
        if source.partner1 == Some(parent) {
            Some(link.rel1.clone())
        } else if source.partner2 == Some(parent) {
            Some(link.rel2.clone())
        } else {
            None
        }
    };
    let fallback = rel_for(target.partner1)
        .or_else(|| rel_for(target.partner2))
        .unwrap_or(ParentRel::Birth);
    let r1 = rel_for(target.partner1).unwrap_or_else(|| fallback.clone());
    let r2 = rel_for(target.partner2).unwrap_or(fallback);
    link_child(
        &mut p,
        to,
        child,
        (ParentRelId::Birth, ParentRelId::Birth),
        link.preferred,
    )?;
    let cit = Citation {
        common: new_common(now),
        source: Some(c.source),
        page: Some(c.page.to_string()),
        date: None,
        text: Some(c.quote.to_string()),
        quality: c.quality.map(|q| q.min(3)),
        evidence: Evidence {
            original: None,
            primary_info: None,
            direct: None,
        },
    };
    let mut fam: Family = p.get(to)?;
    if let Some(l) = fam.children.iter_mut().find(|l| l.person == child) {
        l.rel1 = r1;
        l.rel2 = r2;
        l.assertion = link.assertion.clone();
        l.unknown = link.unknown.clone();
        l.citations = link.citations.clone();
        l.citations.push(cit.common.id);
    }
    fam.common.changed = Some(now);
    p.put(fam);
    p.put(cit);
    unlink_child(&mut p, from, child)?;
    save_note(
        &mut p,
        OwnerRef {
            kind: OwnerKind::Person,
            id: child,
        },
        &NoteInput {
            story: None,
            id: None,
            kind: NoteKindDto::Research,
            text: note.to_string(),
        },
    )?;
    Ok((p.finish(shown("Move child", reason)), Some(child)))
}
