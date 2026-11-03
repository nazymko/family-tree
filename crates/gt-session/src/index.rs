//! In-memory lookups built once per open: record links, one summary per person, and lazily
//! built sort orders.

use crate::dates::Side;
use crate::dto::{NameOrder, PersonRef, PersonRow, SexKind, SortKey};
use crate::names::{
    BIRTH_KINDS, DEATH_KINDS, compose, pick_event, preferred_name, side_of, surname_of,
};
use gt_core::model::ParticipantRef;
use gt_core::tree::Tree;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;
use uuid::Uuid;

/// Reverse lookups over a `Tree`, so no UI read scans every event or family.
#[derive(Debug, Default)]
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

#[derive(Debug, Clone)]
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
            let events: Vec<&gt_core::model::Event> = links
                .events_of
                .get(id)
                .map(|v| v.iter().filter_map(|e| tree.events.get(e)).collect())
                .unwrap_or_default();
            let birth = pick_event(events.iter().copied(), *id, &BIRTH_KINDS);
            let death = pick_event(events.iter().copied(), *id, &DEATH_KINDS);
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
            let slot = u32::try_from(summaries.len()).unwrap_or(u32::MAX);
            slots.insert(*id, slot);
            summaries.push(PersonSummary {
                id: *id,
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
                living: living.get(id).copied().unwrap_or(true),
                private: p.common.private,
                source_count: u32::try_from(sources).unwrap_or(u32::MAX),
                changed: p.common.changed.unwrap_or(0),
                folded,
            });
        }
        PersonIndex {
            summaries,
            slots,
            orders: RefCell::new(HashMap::new()),
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
        order.sort_unstable_by_key(|&i| (keys[i as usize], i));
        order
    }
}
