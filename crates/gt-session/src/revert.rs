//! Undoing one logged change against whatever its record has become since, fact by fact.
//!
//! `revert_change` works on the JSON form of the records, so it needs no list of fields: a field
//! the change left alone stays, a field still as the change wrote it goes back, and a field
//! someone has changed since stays and is reported. Rule by rule:
//!
//! 1. Current equals `after` (ignoring `common.changed`): exactly `before`, so that an undo with
//!    nothing else in between is byte for byte the old state.
//! 2. A created record that has changed since stays, and goes into `kept`.
//! 3. An updated record is merged field by field. `changed` is never merged: the caller stamps it.
//! 4. Arrays match their items by `id`, a `ChildLink` by `person`, anything else by whole value.
//!    An item the change added goes, unless it names a kept record; an item it removed comes back
//!    at its old index; other items stay in their current order. An item matched by whole value
//!    cannot be edited, only replaced: if a replacement has gone since and the old item is not
//!    back, the list has moved on from what the change wrote, and it stays as it is.
//! 5. A scalar naming a kept record stays (`partner1` on a family).
//! 6. A deleted record comes back, unless something has put it back already.
//!
//! `settle_kept` finds what must stay before anything is written: a record someone changed, what
//! a remaining record still points at, and what holds a kept record in place. `dangling_writes`
//! finds the opposite case: a write that would point at a record that has gone since.

use gt_core::model::*;
use gt_core::tree::Tree;
use gt_store::{Change, Record};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// What undoing a change comes to.
#[derive(Clone, Debug, PartialEq)]
pub enum Reverted {
    /// The record becomes this (`None`: it goes).
    Write(Option<Record>),
    /// Nothing to do.
    Unchanged,
    /// Part of the change cannot be undone: this is the partial merge, keeping that part.
    Conflict(Option<Record>),
}

/// The change undone on `current`. See the module comment for the rules; `kept` holds the records
/// that stay whatever else happens (`settle_kept` fills it).
pub fn revert_change(ch: &Change, current: Option<&Record>, kept: &HashSet<Uuid>) -> Reverted {
    let Some(cur) = current else {
        return match (&ch.before, &ch.after) {
            (Some(b), None) => Reverted::Write(Some(b.clone())),
            (None, _) => Reverted::Unchanged,
            // The record it changed has gone since: nothing is left to put back.
            (Some(_), Some(_)) => Reverted::Conflict(None),
        };
    };
    let Some(after) = &ch.after else {
        return Reverted::Unchanged;
    };
    let keep = || Reverted::Conflict(Some(cur.clone()));
    let (Some((cur_json, stamp)), Some((after_json, _))) = (stripped(cur), stripped(after)) else {
        return keep();
    };
    let same = cur_json == after_json;
    let Some(before) = &ch.before else {
        let stays = kept.contains(&ch.id) || holds_kept(after, kept);
        return if same && !stays {
            Reverted::Write(None)
        } else {
            keep()
        };
    };
    if same && !mentions(&after_json, kept) {
        return Reverted::Write(Some(before.clone()));
    }
    let Some((before_json, _)) = stripped(before) else {
        return keep();
    };
    let mut m = Merge {
        kept,
        conflict: false,
    };
    let merged = m.value("", &before_json, &after_json, &cur_json);
    if !m.conflict && merged == cur_json {
        return Reverted::Unchanged;
    }
    if !m.conflict && merged == before_json {
        return Reverted::Write(Some(before.clone()));
    }
    let Some(rec) = restamped(merged, stamp) else {
        return keep();
    };
    if m.conflict {
        Reverted::Conflict(Some(rec))
    } else {
        Reverted::Write(Some(rec))
    }
}

/// The record's JSON without `common.changed`, and the value taken out.
fn stripped(r: &Record) -> Option<(Value, Value)> {
    let mut v = serde_json::to_value(r).ok()?;
    let stamp = common_json(&mut v)?.remove("changed")?;
    Some((v, stamp))
}

fn restamped(mut v: Value, stamp: Value) -> Option<Record> {
    common_json(&mut v)?.insert("changed".into(), stamp);
    serde_json::from_value(v).ok()
}

/// `{"Person": {"common": {..}, ..}}` -> the `common` object.
fn common_json(v: &mut Value) -> Option<&mut Map<String, Value>> {
    v.as_object_mut()?
        .values_mut()
        .next()?
        .get_mut("common")?
        .as_object_mut()
}

/// Whether the JSON names a kept record.
fn mentions(v: &Value, kept: &HashSet<Uuid>) -> bool {
    if kept.is_empty() {
        return false;
    }
    match v {
        Value::String(s) => Uuid::parse_str(s).is_ok_and(|u| kept.contains(&u)),
        Value::Array(a) => a.iter().any(|x| mentions(x, kept)),
        Value::Object(o) => o.values().any(|x| mentions(x, kept)),
        _ => false,
    }
}

struct Merge<'a> {
    kept: &'a HashSet<Uuid>,
    conflict: bool,
}

impl Merge<'_> {
    /// One field: `a` -> `b` is the change to undo, `c` the value now.
    fn value(&mut self, key: &str, b: &Value, a: &Value, c: &Value) -> Value {
        if b == a || c == b {
            return c.clone();
        }
        if c == a && !mentions(a, self.kept) {
            return b.clone();
        }
        match (b, a, c) {
            // A date is one fact: its parts mean nothing apart. So is a list of surnames, below.
            (Value::Object(bo), Value::Object(ao), Value::Object(co))
                if key != "date" && same_keys(bo, co) && same_keys(ao, co) =>
            {
                let walked = co.iter().map(|(k, cv)| {
                    let null = Value::Null;
                    let (bv, av) = (bo.get(k).unwrap_or(&null), ao.get(k).unwrap_or(&null));
                    (k.clone(), self.value(k, bv, av, cv))
                });
                Value::Object(walked.collect())
            }
            (Value::Array(ba), Value::Array(aa), Value::Array(ca)) if key != "surnames" => {
                self.array(ba, aa, ca)
            }
            // A value naming a kept record, or changed since: either way it stays.
            _ => {
                self.conflict = true;
                c.clone()
            }
        }
    }

    fn array(&mut self, b: &[Value], a: &[Value], c: &[Value]) -> Value {
        let find = |list: &[Value], id: &Value| list.iter().position(|x| identity(x) == *id);
        // Lists hold one shape of item, so a replaced item means the whole list is by whole value.
        let replaced = |x: &Value, other: &[Value]| identity(x) == *x && find(other, x).is_none();
        let replacement_gone = a.iter().any(|x| replaced(x, b) && find(c, x).is_none());
        let old_item_gone = b.iter().any(|x| replaced(x, a) && find(c, x).is_none());
        if replacement_gone && old_item_gone {
            self.conflict = true;
            return Value::Array(c.to_vec());
        }
        let mut out: Vec<Value> = Vec::with_capacity(c.len());
        for item in c {
            let id = identity(item);
            match (find(b, &id), find(a, &id)) {
                (None, Some(i)) => {
                    let added = a.get(i).unwrap_or(item);
                    if item != added || mentions(added, self.kept) {
                        self.conflict = true;
                        out.push(item.clone());
                    }
                }
                (Some(i), Some(j)) => {
                    let (bv, av) = (b.get(i).unwrap_or(item), a.get(j).unwrap_or(item));
                    out.push(self.value("", bv, av, item));
                }
                _ => out.push(item.clone()),
            }
        }
        // Items in both whose change cannot be undone, because someone has removed them since.
        let gone_changed = b.iter().any(|x| {
            let id = identity(x);
            find(c, &id).is_none() && find(a, &id).is_some_and(|j| a.get(j) != Some(x))
        });
        self.conflict |= gone_changed;
        for (i, item) in b.iter().enumerate() {
            let id = identity(item);
            if find(a, &id).is_none() && find(&out, &id).is_none() {
                out.insert(i.min(out.len()), item.clone());
            }
        }
        self.undo_reorder(b, a, &mut out);
        Value::Array(out)
    }

    /// If the change reordered the items that both sides have, and they are still in that order,
    /// they go back to the old order in the slots they now occupy.
    fn undo_reorder(&mut self, b: &[Value], a: &[Value], out: &mut [Value]) {
        let ids = |list: &[Value]| list.iter().map(identity).collect::<Vec<_>>();
        let (bi, ai) = (ids(b), ids(a));
        let shared_b: Vec<&Value> = bi.iter().filter(|x| ai.contains(x)).collect();
        let shared_a: Vec<&Value> = ai.iter().filter(|x| bi.contains(x)).collect();
        if shared_a == shared_b {
            return;
        }
        let slots: Vec<usize> = (0..out.len())
            .filter(|i| {
                out.get(*i)
                    .is_some_and(|x| shared_a.contains(&&identity(x)))
            })
            .collect();
        let now: Vec<Value> = slots
            .iter()
            .filter_map(|i| out.get(*i).map(identity))
            .collect();
        if now.iter().collect::<Vec<_>>() == shared_b {
            return;
        }
        if now.iter().collect::<Vec<_>>() != shared_a {
            self.conflict = true;
            return;
        }
        let items: HashMap<String, Value> = slots
            .iter()
            .filter_map(|i| out.get(*i))
            .map(|x| (identity(x).to_string(), x.clone()))
            .collect();
        for (slot, id) in slots.iter().zip(&shared_b) {
            if let (Some(place), Some(item)) = (out.get_mut(*slot), items.get(&id.to_string())) {
                *place = item.clone();
            }
        }
    }
}

fn same_keys(a: &Map<String, Value>, b: &Map<String, Value>) -> bool {
    a.len() == b.len() && a.keys().all(|k| b.contains_key(k))
}

/// What makes two array items the same item.
fn identity(item: &Value) -> Value {
    match item.as_object() {
        Some(o) if o.contains_key("id") => o.get("id").cloned().unwrap_or(Value::Null),
        // `ChildLink` has no id.
        Some(o) if o.contains_key("person") => o.get("person").cloned().unwrap_or(Value::Null),
        _ => item.clone(),
    }
}

// ---- references between records -----------------------------------------------------------

pub(crate) fn common_of(r: &Record) -> &Common {
    match r {
        Record::Person(x) => &x.common,
        Record::Family(x) => &x.common,
        Record::Event(x) => &x.common,
        Record::Place(x) => &x.common,
        Record::Source(x) => &x.common,
        Record::Repository(x) => &x.common,
        Record::Citation(x) => &x.common,
        Record::Media(x) => &x.common,
        Record::Note(x) => &x.common,
    }
}

fn stamp_changed(r: &mut Record, now: i64) {
    let c = match r {
        Record::Person(x) => &mut x.common,
        Record::Family(x) => &mut x.common,
        Record::Event(x) => &mut x.common,
        Record::Place(x) => &mut x.common,
        Record::Source(x) => &mut x.common,
        Record::Repository(x) => &mut x.common,
        Record::Citation(x) => &mut x.common,
        Record::Media(x) => &mut x.common,
        Record::Note(x) => &mut x.common,
    };
    c.changed = Some(now);
}

fn common_refs(c: &Common, f: &mut dyn FnMut(Uuid)) {
    c.citations.iter().for_each(|x| f(*x));
    c.notes.iter().for_each(|x| f(*x));
    c.media.iter().for_each(|x| f(x.media));
}

fn person_refs(p: &Person, f: &mut dyn FnMut(Uuid)) {
    common_refs(&p.common, f);
    for n in &p.names {
        n.citations.iter().for_each(|x| f(*x));
        n.notes.iter().for_each(|x| f(*x));
    }
}

fn family_refs(x: &Family, f: &mut dyn FnMut(Uuid)) {
    common_refs(&x.common, f);
    x.partner1.iter().chain(&x.partner2).for_each(|p| f(*p));
    for c in &x.children {
        f(c.person);
        c.citations.iter().for_each(|x| f(*x));
    }
}

/// What a record holds on to: its notes, citations and pictures, and for a family its members.
/// If one of these has to stay, so does the record, or it would be left with no place in the tree.
fn holding_refs(r: &Record, f: &mut dyn FnMut(Uuid)) {
    match r {
        Record::Person(x) => person_refs(x, f),
        Record::Family(x) => family_refs(x, f),
        other => common_refs(common_of(other), f),
    }
}

/// Every record this one points at: if one of these went, the reference would dangle.
fn all_refs(r: &Record, f: &mut dyn FnMut(Uuid)) {
    holding_refs(r, f);
    match r {
        Record::Event(x) => event_targets(x, f),
        Record::Place(x) => x.parent.iter().for_each(|p| f(*p)),
        Record::Source(x) => x.repositories.iter().for_each(|p| f(p.repository)),
        Record::Citation(x) => x.source.iter().for_each(|p| f(*p)),
        _ => {}
    }
}

fn event_targets(e: &Event, f: &mut dyn FnMut(Uuid)) {
    e.place.iter().for_each(|p| f(*p));
    for p in &e.participants {
        match p.target {
            ParticipantRef::Person(id) | ParticipantRef::Family(id) => f(id),
        }
    }
}

fn holds_kept(r: &Record, kept: &HashSet<Uuid>) -> bool {
    let mut found = false;
    holding_refs(r, &mut |id| found |= kept.contains(&id));
    found
}

/// Runs `f` on everything in the tree that points at something, skipping `skip`.
fn scan_refs(tree: &Tree, skip: &HashMap<Uuid, Option<Record>>, f: &mut dyn FnMut(Uuid)) {
    let live = |id: &Uuid| !skip.contains_key(id);
    tree.persons
        .iter()
        .filter(|(i, _)| live(i))
        .for_each(|(_, x)| person_refs(x, f));
    tree.families
        .iter()
        .filter(|(i, _)| live(i))
        .for_each(|(_, x)| family_refs(x, f));
    for (i, x) in &tree.events {
        if live(i) {
            common_refs(&x.common, f);
            event_targets(x, f);
        }
    }
    for (i, x) in &tree.places {
        if live(i) {
            common_refs(&x.common, f);
            x.parent.iter().for_each(|p| f(*p));
        }
    }
    for (i, x) in &tree.sources {
        if live(i) {
            common_refs(&x.common, f);
            x.repositories.iter().for_each(|p| f(p.repository));
        }
    }
    for (i, x) in &tree.citations {
        if live(i) {
            common_refs(&x.common, f);
            x.source.iter().for_each(|p| f(*p));
        }
    }
    macro_rules! plain {
        ($map:ident) => {
            tree.$map
                .iter()
                .filter(|(i, _)| live(i))
                .for_each(|(_, x)| common_refs(&x.common, f));
        };
    }
    plain!(repositories);
    plain!(media);
    plain!(notes);
    for rec in skip.values().flatten() {
        all_refs(rec, f);
    }
}

fn in_tree(tree: &Tree, id: &Uuid) -> bool {
    tree.persons.contains_key(id)
        || tree.families.contains_key(id)
        || tree.events.contains_key(id)
        || tree.places.contains_key(id)
        || tree.sources.contains_key(id)
        || tree.repositories.contains_key(id)
        || tree.citations.contains_key(id)
        || tree.media.contains_key(id)
        || tree.notes.contains_key(id)
}

/// The records whose writes cannot go ahead: each would point at a record that does not exist
/// once the writes are done (a log undo has removed it since). The rest stay valid without them:
/// a record not written stays as it is, so whatever it points at cannot be removed either.
pub(crate) fn dangling_writes(tree: &Tree, writes: &[Change]) -> HashSet<Uuid> {
    let mut dropped: HashSet<Uuid> = HashSet::new();
    loop {
        let going_ahead: HashMap<Uuid, &Change> = writes
            .iter()
            .filter(|w| !dropped.contains(&w.id))
            .map(|w| (w.id, w))
            .collect();
        let exists = |id: Uuid| match going_ahead.get(&id) {
            Some(w) => w.after.is_some(),
            None => in_tree(tree, &id),
        };
        let mut more: Vec<Uuid> = Vec::new();
        for w in writes {
            if dropped.contains(&w.id) {
                if let Some(stays) = &w.before {
                    all_refs(stays, &mut |id| {
                        if going_ahead.contains_key(&id) && !exists(id) {
                            more.push(id);
                        }
                    });
                }
            } else if let Some(written) = &w.after {
                let mut missing = false;
                all_refs(written, &mut |id| missing |= !exists(id));
                if missing {
                    more.push(w.id);
                }
            }
        }
        if more.is_empty() {
            return dropped;
        }
        dropped.extend(more);
    }
}

// ---- undoing a list of changes --------------------------------------------------------------

/// What must stay when `changes` (newest first) are undone together, on the tree as it stands:
/// created records changed since, records something remaining still points at, and records
/// holding on to any of those. Runs the undo in memory until the answer stops growing.
pub(crate) fn settle_kept(tree: &Tree, changes: &[&Change]) -> HashSet<Uuid> {
    let mut kept = HashSet::new();
    loop {
        let mut grew = false;
        let mut state: HashMap<Uuid, Option<Record>> = HashMap::new();
        for ch in changes {
            let cur = match state.get(&ch.id) {
                Some(r) => r.clone(),
                None => crate::ops::record_in(tree, ch.kind, ch.id),
            };
            match revert_change(ch, cur.as_ref(), &kept) {
                Reverted::Write(r) => {
                    state.insert(ch.id, r);
                }
                Reverted::Conflict(r) => {
                    if ch.before.is_none() && cur.is_some() {
                        grew |= kept.insert(ch.id);
                    }
                    state.insert(ch.id, r);
                }
                Reverted::Unchanged => {}
            }
        }
        let removed: HashSet<Uuid> = state
            .iter()
            .filter(|(_, r)| r.is_none())
            .map(|(id, _)| *id)
            .collect();
        if !removed.is_empty() {
            scan_refs(tree, &state, &mut |id| {
                if removed.contains(&id) {
                    grew |= kept.insert(id);
                }
            });
        }
        if !grew {
            return kept;
        }
    }
}

/// The changes that undo `changes` (newest first) on `tree`, and the ones that could not be
/// undone in full.
pub(crate) struct Undone<'a> {
    pub writes: Vec<Change>,
    pub skipped: Vec<&'a Change>,
    /// Facts really undone; a change that only moved a `changed` stamp is no fact.
    pub undone: u32,
}

impl<'a> Undone<'a> {
    /// Takes back the writes that would leave a dangling reference: those facts stay as they are
    /// and count as skipped.
    pub fn skip_dangling(&mut self, tree: &Tree, changes: &[&'a Change]) {
        let dropped = dangling_writes(tree, &self.writes);
        self.writes.retain(|w| !dropped.contains(&w.id));
        for ch in changes.iter().filter(|ch| dropped.contains(&ch.id)) {
            let already = self.skipped.iter().any(|s| std::ptr::eq(*s, *ch));
            if !already {
                self.skipped.push(ch);
                if !only_stamp_differs(ch) {
                    self.undone = self.undone.saturating_sub(1);
                }
            }
        }
    }
}

pub(crate) fn undo_on_tree<'a>(
    tree: &Tree,
    changes: &[&'a Change],
    kept: &HashSet<Uuid>,
    now: i64,
    stamp_all: bool,
) -> Undone<'a> {
    let mut out = Undone {
        writes: Vec::with_capacity(changes.len()),
        skipped: Vec::new(),
        undone: 0,
    };
    for ch in changes {
        let cur = crate::ops::record_in(tree, ch.kind, ch.id);
        let (mut after, partial) = match revert_change(ch, cur.as_ref(), kept) {
            Reverted::Unchanged => continue,
            Reverted::Write(r) => {
                if !only_stamp_differs(ch) {
                    out.undone += 1;
                }
                (r, false)
            }
            Reverted::Conflict(r) => {
                out.skipped.push(ch);
                if same_ignoring_stamp(r.as_ref(), cur.as_ref()) {
                    continue;
                }
                (r, true)
            }
        };
        if let Some(rec) = &mut after
            && (stamp_all || partial || Some(&*rec) != ch.before.as_ref())
        {
            stamp_changed(rec, now);
        }
        out.writes.push(Change {
            kind: ch.kind,
            id: ch.id,
            before: cur,
            after,
        });
    }
    out
}

fn same_ignoring_stamp(a: Option<&Record>, b: Option<&Record>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => match (stripped(a), stripped(b)) {
            (Some((x, _)), Some((y, _))) => x == y,
            _ => false,
        },
        _ => false,
    }
}

fn only_stamp_differs(ch: &Change) -> bool {
    ch.before.is_some() && same_ignoring_stamp(ch.before.as_ref(), ch.after.as_ref())
}
