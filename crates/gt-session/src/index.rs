//! In-memory lookups built once per open: record links, one summary per person, and lazily
//! built sort orders.

use crate::dates::Side;
use crate::dto::{NameOrder, PersonRef, PersonRow, SexKind, SortKey};
use crate::names::{
    BIRTH_KINDS, DEATH_KINDS, compose, pick_event, preferred_name, side_of, surname_of,
};
use gt_core::model::ParticipantRef;
use gt_core::tree::Tree;
use gt_store::{ChangeSet, Record};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;
use uuid::Uuid;

/// Reverse lookups over a `Tree`, so no UI read scans every event or family.
#[derive(Debug, Default, PartialEq)]
pub struct Links {
    /// Person -> events they take part in (any role), in tree order.
    pub events_of: HashMap<Uuid, Vec<Uuid>>,
    /// Family -> events it is a participant of.
    pub events_of_family: HashMap<Uuid, Vec<Uuid>>,
    /// Person -> families where they are a partner.
    pub partner_in: HashMap<Uuid, Vec<Uuid>>,
    /// Person -> `(family, index in the family's child list)`.
    pub child_in: HashMap<Uuid, Vec<(Uuid, usize)>>,
    /// Source -> number of citations that point at it.
    pub citations_of_source: HashMap<Uuid, u32>,
}

impl Links {
    pub fn build(tree: &Tree) -> Links {
        let mut l = Links::default();
        for (eid, e) in &tree.events {
            for p in &e.participants {
                match p.target {
                    ParticipantRef::Person(id) => {
                        let v = l.events_of.entry(id).or_default();
                        if v.last() != Some(eid) {
                            v.push(*eid);
                        }
                    }
                    ParticipantRef::Family(id) => {
                        let v = l.events_of_family.entry(id).or_default();
                        if v.last() != Some(eid) {
                            v.push(*eid);
                        }
                    }
                }
            }
        }
        for (fid, f) in &tree.families {
            for p in [f.partner1, f.partner2].into_iter().flatten() {
                let v = l.partner_in.entry(p).or_default();
                if v.last() != Some(fid) {
                    v.push(*fid);
                }
            }
            for (i, c) in f.children.iter().enumerate() {
                l.child_in.entry(c.person).or_default().push((*fid, i));
            }
        }
        for c in tree.citations.values() {
            if let Some(s) = c.source {
                *l.citations_of_source.entry(s).or_default() += 1;
            }
        }
        l
    }

    /// Brings the lookups in line with `tree` after `cs` was applied to it, touching only the
    /// records `cs` names. The result equals `Links::build(tree)`.
    pub fn apply(&mut self, tree: &Tree, cs: &ChangeSet) {
        let mut events: HashSet<Uuid> = HashSet::new();
        let mut families: HashSet<Uuid> = HashSet::new();
        // Everything an old or new version of a touched record pointed at.
        let mut person_keys: HashSet<Uuid> = HashSet::new();
        let mut family_keys: HashSet<Uuid> = HashSet::new();
        let mut partner_keys: HashSet<Uuid> = HashSet::new();
        let mut child_keys: HashSet<Uuid> = HashSet::new();
        for ch in &cs.changes {
            for rec in [&ch.before, &ch.after].into_iter().flatten() {
                match rec {
                    Record::Event(e) => {
                        events.insert(e.common.id);
                        for p in &e.participants {
                            match p.target {
                                ParticipantRef::Person(id) => person_keys.insert(id),
                                ParticipantRef::Family(id) => family_keys.insert(id),
                            };
                        }
                    }
                    Record::Family(f) => {
                        families.insert(f.common.id);
                        partner_keys.extend([f.partner1, f.partner2].into_iter().flatten());
                        child_keys.extend(f.children.iter().map(|c| c.person));
                    }
                    _ => {}
                }
            }
            let source_of = |r: &Option<Record>| match r {
                Some(Record::Citation(c)) => c.source,
                _ => None,
            };
            let (old, new) = (source_of(&ch.before), source_of(&ch.after));
            if old != new {
                if let Some(s) = old {
                    let n = self.citations_of_source.entry(s).or_default();
                    *n = n.saturating_sub(1);
                    if *n == 0 {
                        self.citations_of_source.remove(&s);
                    }
                }
                if let Some(s) = new {
                    *self.citations_of_source.entry(s).or_default() += 1;
                }
            }
        }
        // Take the touched records out of every list they were in, then put them back where the
        // tree now has them.
        for id in &person_keys {
            if let Some(v) = self.events_of.get_mut(id) {
                v.retain(|e| !events.contains(e));
            }
        }
        for id in &family_keys {
            if let Some(v) = self.events_of_family.get_mut(id) {
                v.retain(|e| !events.contains(e));
            }
        }
        for id in &partner_keys {
            if let Some(v) = self.partner_in.get_mut(id) {
                v.retain(|f| !families.contains(f));
            }
        }
        for id in &child_keys {
            if let Some(v) = self.child_in.get_mut(id) {
                v.retain(|(f, _)| !families.contains(f));
            }
        }
        for eid in &events {
            let Some(e) = tree.events.get(eid) else {
                continue;
            };
            for p in &e.participants {
                let (map, id) = match p.target {
                    ParticipantRef::Person(id) => {
                        person_keys.insert(id);
                        (&mut self.events_of, id)
                    }
                    ParticipantRef::Family(id) => {
                        family_keys.insert(id);
                        (&mut self.events_of_family, id)
                    }
                };
                let v = map.entry(id).or_default();
                if !v.contains(eid) {
                    v.push(*eid);
                }
            }
        }
        for fid in &families {
            let Some(f) = tree.families.get(fid) else {
                continue;
            };
            for p in [f.partner1, f.partner2].into_iter().flatten() {
                partner_keys.insert(p);
                let v = self.partner_in.entry(p).or_default();
                if !v.contains(fid) {
                    v.push(*fid);
                }
            }
            for (i, c) in f.children.iter().enumerate() {
                child_keys.insert(c.person);
                self.child_in.entry(c.person).or_default().push((*fid, i));
            }
        }
        // Lists are in tree order, and a list left empty has no entry.
        let ev_pos = |e: &Uuid| tree.events.get_index_of(e).unwrap_or(usize::MAX);
        let fam_pos = |f: &Uuid| tree.families.get_index_of(f).unwrap_or(usize::MAX);
        for id in &person_keys {
            if let Some(v) = self.events_of.get_mut(id) {
                v.sort_by_key(ev_pos);
                if v.is_empty() {
                    self.events_of.remove(id);
                }
            }
        }
        for id in &family_keys {
            if let Some(v) = self.events_of_family.get_mut(id) {
                v.sort_by_key(ev_pos);
                if v.is_empty() {
                    self.events_of_family.remove(id);
                }
            }
        }
        for id in &partner_keys {
            if let Some(v) = self.partner_in.get_mut(id) {
                v.sort_by_key(fam_pos);
                if v.is_empty() {
                    self.partner_in.remove(id);
                }
            }
        }
        for id in &child_keys {
            if let Some(v) = self.child_in.get_mut(id) {
                v.sort_by_key(|(f, i)| (fam_pos(f), *i));
                if v.is_empty() {
                    self.child_in.remove(id);
                }
            }
        }
    }
}

/// Lower-case text without accents, for diacritic-insensitive matching.
pub fn fold(s: &str) -> String {
    s.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

/// Which letter heading a surname belongs under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LetterClass {
    Latin(char),
    OtherScripts,
    NoSurname,
}

impl LetterClass {
    pub fn of(surname: &str) -> LetterClass {
        let Some(first) = surname.trim().chars().next() else {
            return LetterClass::NoSurname;
        };
        let base = match first {
            'Ø' | 'ø' => 'O',
            'Ł' | 'ł' => 'L',
            'Đ' | 'đ' => 'D',
            'Æ' | 'æ' | 'Œ' | 'œ' => 'A',
            'ß' => 'S',
            'Ħ' | 'ħ' => 'H',
            'ı' => 'I',
            c => c.nfd().next().unwrap_or(c),
        };
        let upper: char = base.to_uppercase().next().unwrap_or(base);
        let latin = upper.is_ascii_uppercase()
            || (matches!(upper as u32, 0xC0..=0x24F | 0x1E00..=0x1EFF) && upper.is_alphabetic());
        if latin {
            LetterClass::Latin(upper)
        } else {
            LetterClass::OtherScripts
        }
    }

    pub fn label(self) -> String {
        match self {
            LetterClass::Latin(c) => c.to_string(),
            LetterClass::OtherScripts => "Other scripts".to_string(),
            LetterClass::NoSurname => "No surname".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersonSummary {
    pub id: Uuid,
    pub name: String,
    pub surname: String,
    pub given: String,
    pub lifespan: String,
    pub birth: String,
    pub death: String,
    pub birth_sort: Option<i32>,
    pub has_birth_date: bool,
    pub sex: SexKind,
    pub living: bool,
    pub private: bool,
    pub source_count: u32,
    pub changed: i64,
    pub letter: LetterClass,
    /// Every name variant, folded.
    pub folded: String,
}

impl PersonSummary {
    pub fn person_ref(&self) -> PersonRef {
        PersonRef {
            id: self.id,
            name: self.name.clone(),
            lifespan: self.lifespan.clone(),
            sex: self.sex,
            living: self.living,
            private: self.private,
        }
    }

    pub fn row(&self) -> PersonRow {
        PersonRow {
            person: self.person_ref(),
            surname: self.surname.clone(),
            given: self.given.clone(),
            birth: self.birth.clone(),
            death: self.death.clone(),
            source_count: self.source_count,
            changed: self.changed,
        }
    }
}

type OrderCache = HashMap<(SortKey, bool), Arc<Vec<u32>>>;

#[derive(Debug, Default)]
pub struct PersonIndex {
    pub summaries: Vec<PersonSummary>,
    slots: HashMap<Uuid, u32>,
    orders: RefCell<OrderCache>,
}

fn rank(values: &[&str]) -> HashMap<String, i64> {
    let mut uniq: Vec<&str> = values.iter().copied().filter(|s| !s.is_empty()).collect();
    uniq.sort_unstable();
    uniq.dedup();
    uniq.sort_by(|a, b| gt_store::collation::collate(a, b));
    let mut out = HashMap::with_capacity(uniq.len());
    let mut r = 0i64;
    for (i, s) in uniq.iter().enumerate() {
        if i > 0 && gt_store::collation::collate(uniq[i - 1], s).is_ne() {
            r += 1;
        }
        out.insert((*s).to_string(), r);
    }
    out
}

/// The index row of one person.
fn summarize(
    tree: &Tree,
    links: &Links,
    living: &HashMap<Uuid, bool>,
    order: NameOrder,
    id: Uuid,
    p: &gt_core::model::Person,
) -> PersonSummary {
    let events: Vec<&gt_core::model::Event> = links
        .events_of
        .get(&id)
        .map(|v| v.iter().filter_map(|e| tree.events.get(e)).collect())
        .unwrap_or_default();
    let birth = pick_event(events.iter().copied(), id, &BIRTH_KINDS);
    let death = pick_event(events.iter().copied(), id, &DEATH_KINDS);
    let lifespan = crate::dates::lifespan_text(&side_of(birth), &side_of(death));
    let pref = preferred_name(p);
    let surname = pref.map(surname_of).unwrap_or_default();
    let given = pref
        .and_then(|n| n.given.as_deref())
        .unwrap_or("")
        .trim()
        .to_string();
    let mut folded = String::new();
    for n in &p.names {
        for part in [&compose(n, NameOrder::GivenFirst, true), &surname_of(n)] {
            folded.push_str(&fold(part));
            folded.push(' ');
        }
        for part in [&n.nickname, &n.patronymic, &n.call].into_iter().flatten() {
            folded.push_str(&fold(part));
            folded.push(' ');
        }
    }
    let sources = p.common.citations.len()
        + p.names.iter().map(|n| n.citations.len()).sum::<usize>()
        + events
            .iter()
            .map(|e| e.common.citations.len())
            .sum::<usize>();
    PersonSummary {
        id,
        name: crate::names::display_name(p, order),
        letter: LetterClass::of(&surname),
        surname,
        given,
        lifespan,
        birth: birth
            .and_then(|e| e.date.as_ref())
            .map(|d| d.display())
            .unwrap_or_default(),
        death: death
            .and_then(|e| e.date.as_ref())
            .map(|d| d.display())
            .unwrap_or_default(),
        birth_sort: birth
            .and_then(|e| e.date.as_ref())
            .and_then(|d| d.sort_key()),
        has_birth_date: !matches!(side_of(birth), Side::Absent | Side::Undated),
        sex: crate::names::sex_kind(&p.sex).0,
        living: living.get(&id).copied().unwrap_or(true),
        private: p.common.private,
        source_count: u32::try_from(sources).unwrap_or(u32::MAX),
        changed: p.common.changed.unwrap_or(0),
        folded,
    }
}

impl PersonIndex {
    pub fn build(
        tree: &Tree,
        links: &Links,
        living: &HashMap<Uuid, bool>,
        order: NameOrder,
    ) -> PersonIndex {
        let mut summaries = Vec::with_capacity(tree.persons.len());
        let mut slots = HashMap::with_capacity(tree.persons.len());
        for (id, p) in &tree.persons {
            let slot = u32::try_from(summaries.len()).unwrap_or(u32::MAX);
            slots.insert(*id, slot);
            summaries.push(summarize(tree, links, living, order, *id, p));
        }
        PersonIndex {
            summaries,
            slots,
            orders: RefCell::new(HashMap::new()),
        }
    }

    /// Re-summarises the `dirty` persons after the tree changed. `structural`: persons were
    /// added or removed, so the slots (positions in tree order) are rebuilt around the rows
    /// that stay. Sort orders are dropped and rebuilt on next use.
    pub fn refresh(
        &mut self,
        tree: &Tree,
        links: &Links,
        living: &HashMap<Uuid, bool>,
        order: NameOrder,
        dirty: &HashSet<Uuid>,
        structural: bool,
    ) {
        self.orders.get_mut().clear();
        // Persons only appended at the end (a new person, a redo): no slot moves.
        if structural
            && tree.persons.len() > self.summaries.len()
            && self.slots.len() == self.summaries.len()
            && tree
                .persons
                .keys()
                .take(self.summaries.len())
                .zip(&self.summaries)
                .all(|(a, b)| *a == b.id)
        {
            for id in dirty {
                if let (Some(&slot), Some(p)) = (self.slots.get(id), tree.persons.get(id))
                    && let Some(row) = self.summaries.get_mut(slot as usize)
                {
                    *row = summarize(tree, links, living, order, *id, p);
                }
            }
            for (id, p) in tree.persons.iter().skip(self.summaries.len()) {
                let slot = u32::try_from(self.summaries.len()).unwrap_or(u32::MAX);
                self.slots.insert(*id, slot);
                self.summaries
                    .push(summarize(tree, links, living, order, *id, p));
            }
            return;
        }
        if !structural && self.summaries.len() == tree.persons.len() {
            for id in dirty {
                if let (Some(&slot), Some(p)) = (self.slots.get(id), tree.persons.get(id))
                    && let Some(row) = self.summaries.get_mut(slot as usize)
                {
                    *row = summarize(tree, links, living, order, *id, p);
                }
            }
            return;
        }
        let old_slots = std::mem::take(&mut self.slots);
        let mut old: Vec<Option<PersonSummary>> = std::mem::take(&mut self.summaries)
            .into_iter()
            .map(Some)
            .collect();
        self.slots.reserve(tree.persons.len());
        self.summaries.reserve(tree.persons.len());
        for (id, p) in &tree.persons {
            let kept = old_slots
                .get(id)
                .filter(|_| !dirty.contains(id))
                .and_then(|&s| old.get_mut(s as usize))
                .and_then(Option::take);
            let row = kept.unwrap_or_else(|| summarize(tree, links, living, order, *id, p));
            let slot = u32::try_from(self.summaries.len()).unwrap_or(u32::MAX);
            self.slots.insert(*id, slot);
            self.summaries.push(row);
        }
    }

    pub fn slot(&self, id: Uuid) -> Option<u32> {
        self.slots.get(&id).copied()
    }

    pub fn get(&self, id: Uuid) -> Option<&PersonSummary> {
        self.slot(id).and_then(|s| self.summaries.get(s as usize))
    }

    /// Slots in `(sort, desc)` order; built on first use. Rows with nothing to sort by
    /// (no surname, no birth date) come last in both directions.
    pub fn order(&self, sort: SortKey, desc: bool) -> Arc<Vec<u32>> {
        if let Some(o) = self.orders.borrow().get(&(sort, desc)) {
            return Arc::clone(o);
        }
        let built = Arc::new(self.build_order(sort, desc));
        self.orders
            .borrow_mut()
            .insert((sort, desc), Arc::clone(&built));
        built
    }

    fn build_order(&self, sort: SortKey, desc: bool) -> Vec<u32> {
        let s = &self.summaries;
        let text_key = |ranks: &HashMap<String, i64>, v: &str| match ranks.get(v) {
            None => i64::MAX,
            Some(r) if desc => -r,
            Some(r) => *r,
        };
        let surname_ranks = rank(&s.iter().map(|x| x.surname.as_str()).collect::<Vec<_>>());
        let given_ranks = rank(&s.iter().map(|x| x.given.as_str()).collect::<Vec<_>>());
        let keys: Vec<(i64, i64, i64)> = s
            .iter()
            .map(|x| {
                let sn = text_key(&surname_ranks, &x.surname);
                let gn = text_key(&given_ranks, &x.given);
                match sort {
                    SortKey::Surname => (sn, gn, 0),
                    SortKey::Given => (gn, sn, 0),
                    SortKey::Birth => {
                        let b = match x.birth_sort {
                            None => i64::MAX,
                            Some(v) if desc => -i64::from(v),
                            Some(v) => i64::from(v),
                        };
                        (b, sn, gn)
                    }
                    SortKey::Changed => (if desc { -x.changed } else { x.changed }, 0, 0),
                }
            })
            .collect();
        let mut order: Vec<u32> = (0..u32::try_from(s.len()).unwrap_or(u32::MAX)).collect();
        order.sort_unstable_by_key(|&i| (keys[i as usize], s[i as usize].id));
        order
    }
}
