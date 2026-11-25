//! Godparents as the app and the agents read them: on their event, and from the godparent's side
//! (who they stood godparent for).

use crate::dates::date_dto;
use crate::dto::{GodchildDto, GodparentDto};
use crate::person_detail::event_kind_id;
use crate::session::Session;
use gt_core::model::{Event, Godparent, ParticipantRef, Role};
use uuid::Uuid;

impl Session {
    /// The godparents of an event, in order. A linked person who cannot be shown is left out.
    pub fn godparent_dtos(&self, list: &[Godparent]) -> Vec<GodparentDto> {
        list.iter()
            .filter_map(|g| {
                let person = match g.person {
                    Some(id) => Some(self.person_ref(id)?),
                    None => None,
                };
                Some(GodparentDto {
                    id: g.id,
                    role: g.role.id().to_string(),
                    person,
                    name: g.name.clone(),
                    note: g.note.clone(),
                })
            })
            .collect()
    }

    /// Everyone `person` stood godparent for, oldest event first.
    pub fn godchildren(&self, person: Uuid) -> Vec<GodchildDto> {
        let mut found: Vec<(&Event, &Godparent)> = self
            .tree
            .events
            .values()
            .flat_map(|e| {
                e.godparents
                    .iter()
                    .filter(move |g| g.person == Some(person))
                    .map(move |g| (e, g))
            })
            .collect();
        found.sort_by_key(|(e, _)| {
            e.date
                .as_ref()
                .and_then(|d| d.sort_key())
                .unwrap_or(i32::MAX)
        });
        found
            .into_iter()
            .flat_map(|(e, g)| {
                principals(e).filter_map(move |child| {
                    Some(GodchildDto {
                        event: e.common.id,
                        kind: event_kind_id(&e.kind),
                        date: e.date.as_ref().map(date_dto),
                        person: self.person_ref(child)?,
                        role: g.role.id().to_string(),
                    })
                })
            })
            .collect()
    }
}

fn principals(e: &Event) -> impl Iterator<Item = Uuid> + '_ {
    e.participants.iter().filter_map(|p| match p.target {
        ParticipantRef::Person(id) if p.role == Role::Principal => Some(id),
        _ => None,
    })
}
