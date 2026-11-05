//! Who a change reaches: a note, event or citation belongs to people, and a stale editor watches
//! people. A change to a person does not reach that person's relatives.

use crate::session::Session;
use gt_core::model::{Common, Family, ParticipantRef};
use gt_core::tree::Tree;
use std::collections::HashSet;
use uuid::Uuid;

fn holds(common: &Common, set: &HashSet<Uuid>) -> bool {
    common.citations.iter().any(|x| set.contains(x))
        || common.notes.iter().any(|x| set.contains(x))
        || common.media.iter().any(|x| set.contains(&x.media))
}

/// The records that show a record in `set`, and are not in it yet.
fn owners_of(tree: &Tree, set: &HashSet<Uuid>) -> Vec<Uuid> {
    let has = |x: &Uuid| set.contains(x);
    let persons = tree.persons.iter().filter(|(_, p)| {
        holds(&p.common, set)
            || p.names
                .iter()
                .any(|n| n.citations.iter().any(has) || n.notes.iter().any(has))
    });
    let families = tree.families.iter().filter(|(_, f)| {
        holds(&f.common, set) || f.children.iter().any(|c| c.citations.iter().any(has))
    });
    let events = tree
        .events
        .iter()
        .filter(|(_, e)| holds(&e.common, set) || e.place.iter().any(has));
    let places = tree
        .places
        .iter()
        .filter(|(_, x)| holds(&x.common, set) || x.parent.iter().any(has));
    let sources = tree.sources.iter().filter(|(_, x)| {
        holds(&x.common, set) || x.repositories.iter().any(|r| has(&r.repository))
    });
    let citations = tree
        .citations
        .iter()
        .filter(|(_, x)| holds(&x.common, set) || x.source.iter().any(has));
    let repositories = tree
        .repositories
        .iter()
        .filter(|(_, x)| holds(&x.common, set));
    let media = tree.media.iter().filter(|(_, x)| holds(&x.common, set));
    let notes = tree.notes.iter().filter(|(_, x)| holds(&x.common, set));

    persons
        .map(|(id, _)| *id)
        .chain(families.map(|(id, _)| *id))
        .chain(events.map(|(id, _)| *id))
        .chain(places.map(|(id, _)| *id))
        .chain(sources.map(|(id, _)| *id))
        .chain(citations.map(|(id, _)| *id))
        .chain(repositories.map(|(id, _)| *id))
        .chain(media.map(|(id, _)| *id))
        .chain(notes.map(|(id, _)| *id))
        .filter(|id| !set.contains(id))
        .collect()
}

fn partners(f: &Family) -> impl Iterator<Item = Uuid> + '_ {
    f.partner1.iter().chain(&f.partner2).copied()
}

impl Session {
    /// `changed`, then every person those records belong to. A record reaches whatever shows it: a
    /// note its owner, a citation whoever cites it, a source the users of its citations, a place
    /// the people at its events. Ids the tree does not know are kept and reach nobody.
    pub fn touched_with_people(&self, changed: &[Uuid]) -> Vec<Uuid> {
        let tree = &self.tree;
        let mut reached: HashSet<Uuid> = changed.iter().copied().collect();
        // Chains are short (a note on a citation on an event), so this settles in a few rounds.
        loop {
            let owners = owners_of(tree, &reached);
            if owners.is_empty() {
                break;
            }
            reached.extend(owners);
        }

        let mut out: Vec<Uuid> = Vec::new();
        let mut seen: HashSet<Uuid> = HashSet::new();
        let mut add = |id: Uuid| {
            if seen.insert(id) {
                out.push(id);
            }
        };
        changed.iter().for_each(|id| add(*id));
        // The maps keep insertion order, so the list comes out the same each time.
        tree.persons
            .keys()
            .filter(|id| reached.contains(*id))
            .for_each(|id| add(*id));
        for f in tree
            .families
            .values()
            .filter(|f| reached.contains(&f.common.id))
        {
            partners(f).for_each(&mut add);
            f.children.iter().for_each(|c| add(c.person));
        }
        for e in tree
            .events
            .values()
            .filter(|e| reached.contains(&e.common.id))
        {
            for p in &e.participants {
                match p.target {
                    ParticipantRef::Person(id) => add(id),
                    ParticipantRef::Family(id) => {
                        if let Some(f) = tree.families.get(&id) {
                            partners(f).for_each(&mut add);
                        }
                    }
                }
            }
        }
        out
    }
}
