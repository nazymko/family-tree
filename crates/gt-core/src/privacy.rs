//! Privacy filter shared by every export (spec §6.1): clone, prune, then
//! garbage-collect whatever became unreferenced.
use crate::living::{LivingRule, compute_living};
use crate::model::*;
use crate::tree::Tree;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LivingMode {
    Exclude,
    Placeholder,
    IncludeAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivacyPolicy {
    pub living: LivingMode,
    pub include_private: bool,
    pub rule: LivingRule,
}

impl Default for PrivacyPolicy {
    fn default() -> Self {
        Self {
            living: LivingMode::Placeholder,
            include_private: false,
            rule: LivingRule::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PrivacySummary {
    /// Living persons hidden from the output (replaced or removed).
    pub hidden_persons: usize,
    /// Of those, persons kept only as a "Living" placeholder.
    pub placeholder_persons: usize,
    /// Records dropped for any other reason (private, emptied, unreferenced), excluding
    /// persons removed because they are living.
    pub removed_records: usize,
    /// (note, hidden person) where a kept note names a hidden person.
    pub mention_warnings: Vec<(Uuid, Uuid)>,
    /// Kept media that is also linked from a hidden person.
    pub group_media: Vec<Uuid>,
}

fn total(t: &Tree) -> usize {
    t.persons.len()
        + t.families.len()
        + t.events.len()
        + t.places.len()
        + t.sources.len()
        + t.repositories.len()
        + t.citations.len()
        + t.media.len()
        + t.notes.len()
}

fn common_refs(c: &Common, notes: &mut Vec<Uuid>, cits: &mut Vec<Uuid>, media: &mut Vec<Uuid>) {
    notes.extend(&c.notes);
    cits.extend(&c.citations);
    media.extend(c.media.iter().map(|m| m.media));
}

/// Remove ids that no longer exist from every reference list.
fn scrub_dangling(t: &mut Tree) {
    let notes: HashSet<Uuid> = t.notes.keys().copied().collect();
    let cits: HashSet<Uuid> = t.citations.keys().copied().collect();
    let media: HashSet<Uuid> = t.media.keys().copied().collect();
    let places: HashSet<Uuid> = t.places.keys().copied().collect();
    let sources: HashSet<Uuid> = t.sources.keys().copied().collect();
    let repos: HashSet<Uuid> = t.repositories.keys().copied().collect();
    let fix = |c: &mut Common| {
        c.notes.retain(|i| notes.contains(i));
        c.citations.retain(|i| cits.contains(i));
        c.media.retain(|m| media.contains(&m.media));
    };
    t.persons.values_mut().for_each(|r| {
        fix(&mut r.common);
        for n in &mut r.names {
            n.notes.retain(|i| notes.contains(i));
            n.citations.retain(|i| cits.contains(i));
        }
    });
    t.families.values_mut().for_each(|r| {
        fix(&mut r.common);
        for c in &mut r.children {
            c.citations.retain(|i| cits.contains(i));
        }
    });
    t.events.values_mut().for_each(|r| {
        fix(&mut r.common);
        if r.place.is_some_and(|p| !places.contains(&p)) {
            r.place = None;
        }
    });
    t.places.values_mut().for_each(|r| {
        fix(&mut r.common);
        if r.parent.is_some_and(|p| !places.contains(&p)) {
            r.parent = None;
        }
    });
    t.sources.values_mut().for_each(|r| {
        fix(&mut r.common);
        r.repositories.retain(|x| repos.contains(&x.repository));
    });
    t.repositories.values_mut().for_each(|r| fix(&mut r.common));
    t.citations.values_mut().for_each(|r| {
        fix(&mut r.common);
        if r.source.is_some_and(|s| !sources.contains(&s)) {
            r.source = None;
        }
    });
    t.media.values_mut().for_each(|r| fix(&mut r.common));
    t.notes.values_mut().for_each(|r| fix(&mut r.common));
}

/// Keep only records reachable from persons, families and events.
fn gc(t: &mut Tree) {
    let (mut notes, mut cits, mut media) = (vec![], vec![], vec![]);
    let (mut places, mut sources, mut repos) = (vec![], vec![], vec![]);
    for p in t.persons.values() {
        common_refs(&p.common, &mut notes, &mut cits, &mut media);
        for n in &p.names {
            notes.extend(&n.notes);
            cits.extend(&n.citations);
        }
    }
    for f in t.families.values() {
        common_refs(&f.common, &mut notes, &mut cits, &mut media);
        for c in &f.children {
            cits.extend(&c.citations);
        }
    }
    for e in t.events.values() {
        common_refs(&e.common, &mut notes, &mut cits, &mut media);
        places.extend(e.place);
    }
    let mut keep: [HashSet<Uuid>; 6] = Default::default(); // notes cits media places sources repos
    loop {
        let before: usize = keep.iter().map(HashSet::len).sum();
        keep[0].extend(notes.drain(..));
        keep[1].extend(cits.drain(..));
        keep[2].extend(media.drain(..));
        keep[3].extend(places.drain(..));
        keep[4].extend(sources.drain(..));
        keep[5].extend(repos.drain(..));
        for id in &keep[0] {
            if let Some(n) = t.notes.get(id) {
                common_refs(&n.common, &mut notes, &mut cits, &mut media);
            }
        }
        for id in &keep[1] {
            if let Some(c) = t.citations.get(id) {
                common_refs(&c.common, &mut notes, &mut cits, &mut media);
                sources.extend(c.source);
            }
        }
        for id in &keep[2] {
            if let Some(m) = t.media.get(id) {
                common_refs(&m.common, &mut notes, &mut cits, &mut media);
            }
        }
        for id in &keep[3] {
            if let Some(p) = t.places.get(id) {
                common_refs(&p.common, &mut notes, &mut cits, &mut media);
                places.extend(p.parent);
            }
        }
        for id in &keep[4] {
            if let Some(s) = t.sources.get(id) {
                common_refs(&s.common, &mut notes, &mut cits, &mut media);
                repos.extend(s.repositories.iter().map(|r| r.repository));
            }
        }
        for id in &keep[5] {
            if let Some(r) = t.repositories.get(id) {
                common_refs(&r.common, &mut notes, &mut cits, &mut media);
            }
        }
        let grew = notes.iter().any(|i| !keep[0].contains(i))
            || cits.iter().any(|i| !keep[1].contains(i))
            || media.iter().any(|i| !keep[2].contains(i))
            || places.iter().any(|i| !keep[3].contains(i))
            || sources.iter().any(|i| !keep[4].contains(i))
            || repos.iter().any(|i| !keep[5].contains(i));
        if !grew && before == keep.iter().map(HashSet::len).sum::<usize>() {
            break;
        }
    }
    t.notes.retain(|id, _| keep[0].contains(id));
    t.citations.retain(|id, _| keep[1].contains(id));
    t.media.retain(|id, _| keep[2].contains(id));
    t.places.retain(|id, _| keep[3].contains(id));
    t.sources.retain(|id, _| keep[4].contains(id));
    t.repositories.retain(|id, _| keep[5].contains(id));
}

fn full_names(p: &Person) -> Vec<String> {
    p.names
        .iter()
        .filter_map(|n| {
            let given = n.given.as_deref()?.trim();
            let sur = n.surnames.join(" ");
            let sur = sur.trim();
            (!given.is_empty() && !sur.is_empty()).then(|| format!("{given} {sur}").to_lowercase())
        })
        .collect()
}

pub fn filter(tree: &Tree, policy: &PrivacyPolicy, today_jdn: i32) -> (Tree, PrivacySummary) {
    let living = compute_living(tree, &policy.rule, today_jdn);
    let hidden: HashSet<Uuid> = if policy.living == LivingMode::IncludeAll {
        HashSet::new()
    } else {
        living
            .iter()
            .filter(|(_, l)| **l)
            .map(|(id, _)| *id)
            .collect()
    };
    let placeholder = policy.living == LivingMode::Placeholder;
    let drop_private = !policy.include_private;
    let mut t = tree.clone();
    let mut summary = PrivacySummary {
        hidden_persons: hidden.len(),
        placeholder_persons: if placeholder { hidden.len() } else { 0 },
        ..PrivacySummary::default()
    };

    // Mention warnings and group media are computed from the original tree.
    let hidden_names: Vec<(Uuid, Vec<String>)> = hidden
        .iter()
        .filter_map(|id| tree.persons.get(id).map(|p| (*id, full_names(p))))
        .collect();
    let hidden_media: HashSet<Uuid> = hidden
        .iter()
        .filter_map(|id| tree.persons.get(id))
        .flat_map(|p| p.common.media.iter().map(|m| m.media))
        .collect();

    // 1. Private records go regardless of mode.
    if drop_private {
        t.persons.retain(|_, r| !r.common.private);
        t.families.retain(|_, r| !r.common.private);
        t.events.retain(|_, r| !r.common.private);
        t.places.retain(|_, r| !r.common.private);
        t.sources.retain(|_, r| !r.common.private);
        t.repositories.retain(|_, r| !r.common.private);
        t.citations.retain(|_, r| !r.common.private);
        t.media.retain(|_, r| !r.common.private);
        t.notes.retain(|_, r| !r.common.private);
        for p in t.persons.values_mut() {
            p.names.retain(|n| !n.private);
        }
    }

    // 2/3. Living persons: replace or remove.
    let hidden_removed = |id: &Uuid| hidden.contains(id) && !placeholder;
    let hidden_removed_count = t.persons.keys().filter(|id| hidden_removed(id)).count();
    if placeholder {
        for id in &hidden {
            if let Some(p) = t.persons.get_mut(id) {
                p.sex = Sex::Unknown;
                p.names = vec![Name {
                    id: p.names.first().map_or(*id, |n| n.id),
                    kind: NameKind::Birth,
                    assertion: Assertion {
                        preferred: true,
                        proof: None,
                        confidence: None,
                    },
                    private: false,
                    prefix: None,
                    given: Some("Living".to_string()),
                    call: None,
                    surname_prefix: None,
                    surnames: vec![],
                    patronymic: None,
                    suffix: None,
                    nickname: None,
                    lang: None,
                    date: None,
                    raw: None,
                    citations: vec![],
                    notes: vec![],
                    unknown: vec![],
                }];
                p.common.citations.clear();
                p.common.notes.clear();
                p.common.media.clear();
                p.common.ext_ids.clear();
                p.common.unknown.clear();
            }
        }
    } else {
        t.persons.retain(|id, _| !hidden.contains(id));
    }

    // Families: drop vanished people, then empty families.
    let alive: HashSet<Uuid> = t.persons.keys().copied().collect();
    for f in t.families.values_mut() {
        for slot in [&mut f.partner1, &mut f.partner2] {
            if slot.is_some_and(|p| !alive.contains(&p)) {
                *slot = None;
            }
        }
        f.children.retain(|c| alive.contains(&c.person));
    }
    t.families
        .retain(|_, f| f.partner1.is_some() || f.partner2.is_some() || !f.children.is_empty());

    // Families with a hidden partner lose their own events too (dates would leak).
    let hidden_families: HashSet<Uuid> = tree
        .families
        .iter()
        .filter(|(_, f)| {
            [f.partner1, f.partner2]
                .into_iter()
                .flatten()
                .any(|p| hidden.contains(&p))
        })
        .map(|(id, _)| *id)
        .collect();

    // Events: strip participants that are gone, drop events left without a principal.
    let families: HashSet<Uuid> = t.families.keys().copied().collect();
    let mut dropped = vec![];
    for (id, e) in &mut t.events {
        let n = e.participants.len();
        e.participants.retain(|p| match p.target {
            ParticipantRef::Person(x) => alive.contains(&x) && !hidden.contains(&x),
            ParticipantRef::Family(f) => families.contains(&f) && !hidden_families.contains(&f),
        });
        let touched = e.participants.len() != n;
        let has_principal = e.participants.iter().any(|p| p.role == Role::Principal);
        if touched && !has_principal {
            dropped.push(*id);
        }
    }
    for id in dropped {
        t.events.shift_remove(&id);
    }

    // Dangling references, then reachability GC.
    scrub_dangling(&mut t);
    gc(&mut t);
    scrub_dangling(&mut t);

    // Mention warnings over the notes that survived.
    for (nid, note) in &t.notes {
        let text = note.text.to_lowercase();
        for (pid, names) in &hidden_names {
            if names.iter().any(|n| text.contains(n.as_str())) {
                summary.mention_warnings.push((*nid, *pid));
            }
        }
    }
    summary.group_media = t
        .media
        .keys()
        .filter(|m| hidden_media.contains(m))
        .copied()
        .collect();

    summary.removed_records = total(tree)
        .saturating_sub(total(&t))
        .saturating_sub(hidden_removed_count);
    (t, summary)
}
