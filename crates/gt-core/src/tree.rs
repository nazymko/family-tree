use crate::model::*;
use crate::raw::RawNode;
use indexmap::IndexMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct TreeHeader {
    pub producer: Option<String>,
    pub source_version: Option<String>,
    pub unknown: Vec<RawNode>,
}

/// In-memory tree; `IndexMap` keeps insertion order (stable export order).
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Tree {
    pub header: TreeHeader,
    pub persons: IndexMap<Uuid, Person>,
    pub families: IndexMap<Uuid, Family>,
    pub events: IndexMap<Uuid, Event>,
    pub places: IndexMap<Uuid, Place>,
    pub sources: IndexMap<Uuid, Source>,
    pub repositories: IndexMap<Uuid, Repository>,
    pub citations: IndexMap<Uuid, Citation>,
    pub media: IndexMap<Uuid, Media>,
    pub notes: IndexMap<Uuid, Note>,
}

fn strip(c: &mut Common) {
    c.changed = None;
    c.ext_ids.retain(|e| e.kind != ExtIdKind::Xref);
}

impl Tree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Events in which the person participates directly, with their role.
    /// A copy without the unnamed-siblings markers and their child links (what an export sees).
    pub fn without_sibling_markers(&self) -> Tree {
        let mut t = self.clone();
        t.persons.retain(|_, p| !p.sibling_marker);
        for f in t.families.values_mut() {
            f.children.retain(|c| {
                !self
                    .persons
                    .get(&c.person)
                    .is_some_and(|p| p.sibling_marker)
            });
        }
        t
    }

    pub fn events_of(&self, person: Uuid) -> impl Iterator<Item = (&Event, &Role)> {
        self.events.values().flat_map(move |e| {
            e.participants
                .iter()
                .filter(move |p| p.target == ParticipantRef::Person(person))
                .map(move |p| (e, &p.role))
        })
    }

    pub fn families_as_partner(&self, person: Uuid) -> impl Iterator<Item = &Family> {
        self.families
            .values()
            .filter(move |f| f.partner1 == Some(person) || f.partner2 == Some(person))
    }

    pub fn families_as_child(&self, person: Uuid) -> impl Iterator<Item = (&Family, &ChildLink)> {
        self.families.values().flat_map(move |f| {
            f.children
                .iter()
                .filter(move |c| c.person == person)
                .map(move |c| (f, c))
        })
    }

    /// Copy with `changed` cleared and `ExtIdKind::Xref` ids removed; used by round-trip tests.
    pub fn normalized_for_compare(&self) -> Tree {
        let mut t = self.clone();
        t.persons.values_mut().for_each(|r| strip(&mut r.common));
        t.families.values_mut().for_each(|r| strip(&mut r.common));
        t.events.values_mut().for_each(|r| strip(&mut r.common));
        t.places.values_mut().for_each(|r| strip(&mut r.common));
        t.sources.values_mut().for_each(|r| strip(&mut r.common));
        t.repositories
            .values_mut()
            .for_each(|r| strip(&mut r.common));
        t.citations.values_mut().for_each(|r| strip(&mut r.common));
        t.media.values_mut().for_each(|r| strip(&mut r.common));
        t.notes.values_mut().for_each(|r| strip(&mut r.common));
        t
    }
}
