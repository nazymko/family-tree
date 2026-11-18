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

    /// The primary of the version group a media record belongs to: the record itself when it is
    /// no version of another (or names one that is gone).
    pub fn version_primary(&self, media: Uuid) -> Uuid {
        match self.media.get(&media).and_then(|m| m.version_of) {
            Some(p) if p != media && self.media.contains_key(&p) => p,
            _ => media,
        }
    }

    /// Every record of the version group of `media`, the primary first, then the others in tree
    /// order. A record with no other versions is a group of one.
    pub fn version_group(&self, media: Uuid) -> Vec<Uuid> {
        let primary = self.version_primary(media);
        let mut out = vec![primary];
        out.extend(
            self.media
                .iter()
                .filter(|(id, m)| **id != primary && m.version_of == Some(primary))
                .map(|(id, _)| *id),
        );
        out
    }

    /// Every version group with more than one file: its primary, and the other files in tree
    /// order. One pass over the media, for code that asks about many records at once.
    pub fn version_index(&self) -> std::collections::HashMap<Uuid, Vec<Uuid>> {
        let mut out: std::collections::HashMap<Uuid, Vec<Uuid>> = Default::default();
        for (id, m) in &self.media {
            if let Some(p) = m
                .version_of
                .filter(|p| *p != *id && self.media.contains_key(p))
            {
                out.entry(p).or_default().push(*id);
            }
        }
        out
    }

    /// Makes the version groups sound after reading them: a `version_of` naming no record, the
    /// record itself, or a record that is a version itself is pointed at that chain's primary, or
    /// cleared when there is none. The ids that changed.
    pub fn mend_media_versions(&mut self) -> Vec<Uuid> {
        let mut fixes: Vec<(Uuid, Option<Uuid>)> = Vec::new();
        for (id, m) in &self.media {
            let Some(mut to) = m.version_of else {
                continue;
            };
            let mut seen = vec![*id];
            let root = loop {
                if seen.contains(&to) {
                    break None;
                }
                match self.media.get(&to) {
                    None => break None,
                    Some(t) => match t.version_of {
                        None => break Some(to),
                        Some(next) => {
                            seen.push(to);
                            to = next;
                        }
                    },
                }
            };
            if root != m.version_of {
                fixes.push((*id, root));
            }
        }
        for (id, to) in &fixes {
            if let Some(m) = self.media.get_mut(id) {
                m.version_of = *to;
            }
        }
        fixes.into_iter().map(|(id, _)| id).collect()
    }

    /// Copy with `changed` cleared and `ExtIdKind::Xref` ids removed; used by round-trip tests.
    pub fn normalized_for_compare(&self) -> Tree {
        let mut t = self.clone();
        t.persons.values_mut().for_each(|r| strip(&mut r.common));
        t.families.values_mut().for_each(|r| strip(&mut r.common));
        t.events.values_mut().for_each(|r| {
            strip(&mut r.common);
            // Their ids are made anew by an import.
            r.godparents.iter_mut().for_each(|g| g.id = Uuid::nil());
        });
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
