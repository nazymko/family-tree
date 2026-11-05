//! The family view: parents, partners, children and siblings of one person.

use crate::dto::{
    ChildDto, FamilyChildDto, FamilyDto, FamilyViewDto, ParentSetDto, PartnershipDto,
};
use crate::error::ApiError;
use crate::names::{BIRTH_KINDS, DEATH_KINDS, pick_event};
use crate::person_detail::{rel_id, union_id};
use crate::session::Session;
use gt_core::model::{Event, EventKind};
use std::collections::HashSet;
use uuid::Uuid;

fn event_text(s: &Session, tag: &str, e: Option<&Event>) -> Option<String> {
    let e = e?;
    let place = e
        .place
        .and_then(|p| s.place_option(p))
        .map(|p| p.name)
        .filter(|n| !n.is_empty());
    let bits: Vec<String> = [e.date.as_ref().map(|d| d.display()), place]
        .into_iter()
        .flatten()
        .filter(|b| !b.is_empty())
        .collect();
    (!bits.is_empty()).then(|| format!("{tag} {}", bits.join(", ")))
}

fn family_events(s: &Session, family: Uuid) -> Vec<&Event> {
    s.links
        .events_of_family
        .get(&family)
        .map(|v| v.iter().filter_map(|e| s.tree.events.get(e)).collect())
        .unwrap_or_default()
}

/// "m. 1 Jun 1878, Hamburg", or `None` without a dated or placed marriage.
fn union_summary(s: &Session, family: Uuid) -> Option<String> {
    let marriage = family_events(s, family)
        .into_iter()
        .find(|e| e.kind == EventKind::Marriage && !e.negative);
    event_text(s, "m.", marriage)
}

fn birth_key(s: &Session, id: Uuid) -> i32 {
    s.index
        .get(id)
        .and_then(|x| x.birth_sort)
        .unwrap_or(i32::MAX)
}

pub fn parent_sets(s: &Session, id: Uuid) -> Vec<ParentSetDto> {
    let mut out: Vec<ParentSetDto> = Vec::new();
    for (fid, idx) in s.links.child_in.get(&id).into_iter().flatten() {
        let Some(f) = s.tree.families.get(fid) else {
            continue;
        };
        let Some(link) = f.children.get(*idx) else {
            continue;
        };
        out.push(ParentSetDto {
            family_id: *fid,
            parents: (
                f.partner1.and_then(|p| s.person_ref(p)),
                f.partner2.and_then(|p| s.person_ref(p)),
            ),
            rel: (rel_id(&link.rel1), rel_id(&link.rel2)),
            preferred: link.preferred,
            summary: union_summary(s, *fid),
        });
    }
    // Preferred first; the sort is stable so the rest keep tree order.
    out.sort_by_key(|p| !p.preferred);
    out
}

pub fn partnerships(s: &Session, id: Uuid) -> Vec<PartnershipDto> {
    let mut out = Vec::new();
    for fid in s.links.partner_in.get(&id).into_iter().flatten() {
        let Some(f) = s.tree.families.get(fid) else {
            continue;
        };
        let other = if f.partner1 == Some(id) {
            f.partner2
        } else {
            f.partner1
        };
        let mut children: Vec<(i32, ChildDto)> = Vec::with_capacity(f.children.len());
        for c in &f.children {
            let Some(cs) = s.index.get(c.person) else {
                continue;
            };
            children.push((
                cs.birth_sort.unwrap_or(i32::MAX),
                ChildDto {
                    person: cs.person_ref(),
                    rel: (rel_id(&c.rel1), rel_id(&c.rel2)),
                    birth: cs.birth.clone(),
                },
            ));
        }
        // Birth order; undated children keep their listed order after the dated ones.
        children.sort_by_key(|(k, _)| *k);
        let mut events = family_events(s, *fid);
        events.sort_by_key(|e| {
            e.date
                .as_ref()
                .and_then(|d| d.sort_key())
                .unwrap_or(i32::MAX)
        });
        out.push(PartnershipDto {
            family_id: *fid,
            partner: other.filter(|o| *o != id).and_then(|o| s.person_ref(o)),
            focus_second: (f.partner1 != Some(id)).then_some(true),
            kind: union_id(&f.kind),
            summary: union_summary(s, *fid),
            events: events.into_iter().map(|e| s.event_dto(e)).collect(),
            children: children.into_iter().map(|(_, c)| c).collect(),
        });
    }
    out
}

fn summary(s: &Session, id: Uuid) -> String {
    let events: Vec<&Event> = s
        .links
        .events_of
        .get(&id)
        .map(|v| v.iter().filter_map(|e| s.tree.events.get(e)).collect())
        .unwrap_or_default();
    let birth = pick_event(events.iter().copied(), id, &BIRTH_KINDS);
    let death = pick_event(events.iter().copied(), id, &DEATH_KINDS);
    [event_text(s, "b.", birth), event_text(s, "d.", death)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn family_view(s: &Session, id: Uuid) -> Result<FamilyViewDto, ApiError> {
    let focus = s
        .person_ref(id)
        .ok_or_else(|| ApiError::not_found("person", id))?;
    if focus.sibling_marker {
        return Err(ApiError::sibling_marker(id));
    }
    let mut seen: HashSet<Uuid> = HashSet::from([id]);
    let mut siblings: Vec<Uuid> = Vec::new();
    for (fid, _) in s.links.child_in.get(&id).into_iter().flatten() {
        let Some(f) = s.tree.families.get(fid) else {
            continue;
        };
        for c in &f.children {
            if seen.insert(c.person) {
                siblings.push(c.person);
            }
        }
    }
    siblings.sort_by_key(|p| birth_key(s, *p));
    Ok(FamilyViewDto {
        focus,
        focus_summary: summary(s, id),
        parent_sets: parent_sets(s, id),
        partnerships: partnerships(s, id),
        siblings: siblings
            .into_iter()
            .filter_map(|p| s.person_ref(p))
            .collect(),
    })
}

pub fn family(s: &Session, id: Uuid) -> Result<FamilyDto, ApiError> {
    let f = s
        .tree
        .families
        .get(&id)
        .ok_or_else(|| ApiError::not_found("family", id))?;
    let mut children: Vec<(i32, FamilyChildDto)> = Vec::with_capacity(f.children.len());
    for c in &f.children {
        let Some(cs) = s.index.get(c.person) else {
            continue;
        };
        children.push((
            cs.birth_sort.unwrap_or(i32::MAX),
            FamilyChildDto {
                child: ChildDto {
                    person: cs.person_ref(),
                    rel: (rel_id(&c.rel1), rel_id(&c.rel2)),
                    birth: cs.birth.clone(),
                },
                preferred: c.preferred,
            },
        ));
    }
    children.sort_by_key(|(k, _)| *k);
    let mut events = family_events(s, id);
    events.sort_by_key(|e| {
        e.date
            .as_ref()
            .and_then(|d| d.sort_key())
            .unwrap_or(i32::MAX)
    });
    Ok(FamilyDto {
        id,
        partners: (
            f.partner1.and_then(|p| s.person_ref(p)),
            f.partner2.and_then(|p| s.person_ref(p)),
        ),
        kind: union_id(&f.kind),
        summary: union_summary(s, id),
        events: events.into_iter().map(|e| s.event_dto(e)).collect(),
        children: children.into_iter().map(|(_, c)| c).collect(),
    })
}
