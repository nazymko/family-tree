//! Planning edit commands. `plan` is pure: it reads a `Tree` and returns the `ChangeSet` that
//! carries out a command; it never mutates the tree and never touches the database.

mod cite;
mod delete;
pub(crate) mod draft;
mod event;
mod family;
mod media;
pub(crate) use media::plan_attach;
mod person;
pub mod places;
mod quick_add;

use crate::command::EditCommand;
use crate::error::{ApiError, ErrorCode};
use gt_core::date::{DayOrder, GenDate};
use gt_core::model::*;
use gt_core::tree::Tree;
use gt_store::{Change, ChangeSet, Record, RecordKind};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub(crate) fn invalid(msg: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::Invalid, msg)
}

pub(crate) fn conflict(msg: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::Conflict, msg)
}

/// A record type that can be read from and written to a `Tree` and a `Record`.
pub(crate) trait Rec: Clone {
    const KIND: RecordKind;
    const NAME: &'static str;
    fn rid(&self) -> Uuid;
    fn wrap(self) -> Record;
    fn unwrap(r: Record) -> Option<Self>;
    fn map(t: &Tree) -> &IndexMap<Uuid, Self>;
}

macro_rules! rec {
    ($ty:ty, $variant:ident, $field:ident, $name:literal) => {
        impl Rec for $ty {
            const KIND: RecordKind = RecordKind::$variant;
            const NAME: &'static str = $name;
            fn rid(&self) -> Uuid {
                self.common.id
            }
            fn wrap(self) -> Record {
                Record::$variant(self)
            }
            fn unwrap(r: Record) -> Option<Self> {
                match r {
                    Record::$variant(x) => Some(x),
                    _ => None,
                }
            }
            fn map(t: &Tree) -> &IndexMap<Uuid, Self> {
                &t.$field
            }
        }
    };
}
rec!(Person, Person, persons, "person");
rec!(Family, Family, families, "family");
rec!(Event, Event, events, "event");
rec!(Place, Place, places, "place");
rec!(Source, Source, sources, "source");
rec!(Repository, Repository, repositories, "repository");
rec!(Citation, Citation, citations, "citation");
rec!(Media, Media, media, "media item");
rec!(Note, Note, notes, "note");

pub(crate) fn new_common(now: i64) -> Common {
    Common {
        id: gt_core::ids::new_id(),
        private: false,
        changed: Some(now),
        citations: Vec::new(),
        notes: Vec::new(),
        media: Vec::new(),
        ext_ids: Vec::new(),
        unknown: Vec::new(),
    }
}

pub(crate) fn parse_date(raw: Option<&str>, order: DayOrder) -> Option<GenDate> {
    raw.filter(|r| !r.trim().is_empty())
        .map(|r| GenDate::parse(r, order))
}

/// Trimmed text; empty becomes `None`.
pub(crate) fn text(s: Option<&str>) -> Option<String> {
    s.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Reads the tree plus the changes made so far, and collects them into one `ChangeSet`.
pub(crate) struct Planner<'a> {
    pub tree: &'a Tree,
    pub now: i64,
    pub order: DayOrder,
    /// First-touch order; value is `(before, after)`.
    ov: IndexMap<(RecordKind, Uuid), (Option<Record>, Option<Record>)>,
    /// Citations and notes that may have lost their last reference; notes carry `keep_shared`.
    cand_citations: HashSet<Uuid>,
    cand_notes: HashMap<Uuid, bool>,
}

impl<'a> Planner<'a> {
    pub fn new(tree: &'a Tree, now: i64, order: DayOrder) -> Self {
        Planner {
            tree,
            now,
            order,
            ov: IndexMap::new(),
            cand_citations: HashSet::new(),
            cand_notes: HashMap::new(),
        }
    }

    /// Current state of a record (changes made so far included).
    pub fn find<T: Rec>(&self, id: Uuid) -> Option<T> {
        match self.ov.get(&(T::KIND, id)) {
            Some((_, after)) => after.clone().and_then(T::unwrap),
            None => T::map(self.tree).get(&id).cloned(),
        }
    }

    pub fn get<T: Rec>(&self, id: Uuid) -> Result<T, ApiError> {
        self.find(id)
            .ok_or_else(|| ApiError::not_found(T::NAME, id))
    }

    pub fn put<T: Rec>(&mut self, rec: T) {
        let id = rec.rid();
        let after = Some(rec.wrap());
        match self.ov.get_mut(&(T::KIND, id)) {
            Some(slot) => slot.1 = after,
            None => {
                let before = T::map(self.tree).get(&id).cloned().map(Rec::wrap);
                self.ov.insert((T::KIND, id), (before, after));
            }
        }
    }

    pub fn remove<T: Rec>(&mut self, id: Uuid) {
        match self.ov.get_mut(&(T::KIND, id)) {
            Some(slot) => slot.1 = None,
            None => {
                let before = T::map(self.tree).get(&id).cloned().map(Rec::wrap);
                if before.is_some() {
                    self.ov.insert((T::KIND, id), (before, None));
                }
            }
        }
    }

    /// Families written by this plan so far (their latest state).
    pub fn written_families(&self) -> impl Iterator<Item = &Family> {
        self.ov.values().filter_map(|(_, after)| match after {
            Some(Record::Family(f)) => Some(f),
            _ => None,
        })
    }

    /// Notes and citations that `keep` no longer references are deleted when `finish` finds
    /// nothing else pointing at them (shared notes are kept when `keep_shared`).
    pub fn abandon(&mut self, citations: &[Uuid], notes: &[Uuid], keep_shared: bool) {
        self.cand_citations.extend(citations.iter().copied());
        for n in notes {
            self.cand_notes.insert(*n, keep_shared);
        }
    }

    /// Records the person as changed now (their data lives in other records).
    pub fn bump_person(&mut self, id: Uuid) {
        if let Some(mut p) = self.find::<Person>(id) {
            p.common.changed = Some(self.now);
            self.put(p);
        }
    }

    fn drop_orphans(&mut self) {
        if self.cand_citations.is_empty() && self.cand_notes.is_empty() {
            return;
        }
        let mut cits: HashSet<Uuid> = HashSet::new();
        let mut notes: HashSet<Uuid> = HashSet::new();
        fn take(c: &Common, cits: &mut HashSet<Uuid>, notes: &mut HashSet<Uuid>) {
            cits.extend(c.citations.iter().copied());
            notes.extend(c.notes.iter().copied());
        }
        let skip = |k: RecordKind, id: &Uuid| self.ov.contains_key(&(k, *id));
        let mut name_refs: Vec<(&[Uuid], &[Uuid])> = Vec::new();
        let mut link_refs: Vec<&[Uuid]> = Vec::new();
        for (id, p) in &self.tree.persons {
            if !skip(RecordKind::Person, id) {
                take(&p.common, &mut cits, &mut notes);
                name_refs.extend(p.names.iter().map(|n| (&n.citations[..], &n.notes[..])));
            }
        }
        for (id, f) in &self.tree.families {
            if !skip(RecordKind::Family, id) {
                take(&f.common, &mut cits, &mut notes);
                link_refs.extend(f.children.iter().map(|c| &c.citations[..]));
            }
        }
        macro_rules! scan {
            ($map:ident, $kind:ident) => {
                for (id, r) in &self.tree.$map {
                    if !skip(RecordKind::$kind, id) {
                        take(&r.common, &mut cits, &mut notes);
                    }
                }
            };
        }
        scan!(events, Event);
        scan!(places, Place);
        scan!(sources, Source);
        scan!(repositories, Repository);
        scan!(citations, Citation);
        scan!(media, Media);
        scan!(notes, Note);
        for (c, n) in name_refs {
            cits.extend(c.iter().copied());
            notes.extend(n.iter().copied());
        }
        for c in link_refs {
            cits.extend(c.iter().copied());
        }
        for (_, after) in self.ov.values() {
            match after {
                Some(Record::Person(p)) => {
                    take(&p.common, &mut cits, &mut notes);
                    for n in &p.names {
                        cits.extend(n.citations.iter().copied());
                        notes.extend(n.notes.iter().copied());
                    }
                }
                Some(Record::Family(f)) => {
                    take(&f.common, &mut cits, &mut notes);
                    for c in &f.children {
                        cits.extend(c.citations.iter().copied());
                    }
                }
                Some(Record::Event(r)) => take(&r.common, &mut cits, &mut notes),
                Some(Record::Place(r)) => take(&r.common, &mut cits, &mut notes),
                Some(Record::Source(r)) => take(&r.common, &mut cits, &mut notes),
                Some(Record::Repository(r)) => take(&r.common, &mut cits, &mut notes),
                Some(Record::Citation(r)) => take(&r.common, &mut cits, &mut notes),
                Some(Record::Media(r)) => take(&r.common, &mut cits, &mut notes),
                Some(Record::Note(r)) => take(&r.common, &mut cits, &mut notes),
                None => {}
            }
        }
        let gone_cits: Vec<Uuid> = self
            .cand_citations
            .iter()
            .copied()
            .filter(|c| !cits.contains(c))
            .collect();
        let gone_notes: Vec<Uuid> = self
            .cand_notes
            .iter()
            .filter(|(n, keep_shared)| {
                !notes.contains(n)
                    && !(**keep_shared && self.find::<Note>(**n).is_some_and(|x| x.shared))
            })
            .map(|(n, _)| *n)
            .collect();
        for c in gone_cits {
            self.remove::<Citation>(c);
        }
        for n in gone_notes {
            self.remove::<Note>(n);
        }
    }

    pub fn finish(mut self, label: String) -> ChangeSet {
        self.drop_orphans();
        let changes: Vec<Change> = self
            .ov
            .into_iter()
            .filter(|(_, (b, a))| b != a)
            .map(|((kind, id), (before, after))| Change {
                kind,
                id,
                before,
                after,
            })
            .collect();
        ChangeSet {
            group: gt_core::ids::new_id(),
            reverts: None,
            label,
            ts: self.now,
            changes,
        }
    }
}

/// Builds the change set for `cmd`; the second value is the person to focus afterwards.
pub fn plan(
    tree: &Tree,
    cmd: &EditCommand,
    now_ms: i64,
    day_order: DayOrder,
) -> Result<(ChangeSet, Option<Uuid>), ApiError> {
    plan_with(tree, cmd, now_ms, day_order, None)
}

/// `plan`, with the session's last citation for `{reuseLast: true}`.
pub fn plan_with(
    tree: &Tree,
    cmd: &EditCommand,
    now_ms: i64,
    day_order: DayOrder,
    last_citation: Option<&Citation>,
) -> Result<(ChangeSet, Option<Uuid>), ApiError> {
    let mut p = Planner::new(tree, now_ms, day_order);
    use EditCommand as C;
    let focus = match cmd {
        C::CreatePerson { draft } => person::create_person(&mut p, draft)?,
        C::UpdatePerson {
            id,
            sex,
            sex_text,
            living_override,
            private,
        } => person::update_person(
            &mut p,
            *id,
            sex.as_ref(),
            sex_text.as_deref(),
            *living_override,
            *private,
        )?,
        C::DeletePerson { id } => delete::delete_person(&mut p, *id)?,
        C::SaveName { person, name } => person::save_name(&mut p, *person, name)?,
        C::DeleteName { person, name } => person::delete_name(&mut p, *person, *name)?,
        C::SaveEvent { owner, event } => event::save_event(&mut p, *owner, event)?,
        C::DeleteEvent { owner, event } => event::delete_event(&mut p, *owner, *event)?,
        C::QuickAdd {
            anchor,
            relation,
            draft,
            family,
            new_parent_set,
        } => quick_add::quick_add(
            &mut p,
            *anchor,
            *relation,
            draft,
            *family,
            new_parent_set.unwrap_or(false),
        )?,
        C::SaveFamily {
            id,
            partner1,
            partner2,
            kind,
        } => family::save_family(&mut p, *id, *partner1, *partner2, *kind)?,
        C::DeleteFamily { id } => family::delete_family(&mut p, *id)?,
        C::LinkChild {
            family,
            child,
            rel,
            preferred,
        } => family::link_child(&mut p, *family, *child, *rel, *preferred)?,
        C::UpdateChildLink {
            family,
            child,
            rel,
            preferred,
        } => family::update_child_link(&mut p, *family, *child, *rel, *preferred)?,
        C::UnlinkChild { family, child } => family::unlink_child(&mut p, *family, *child)?,
        C::ReorderChildren { family, order } => family::reorder_children(&mut p, *family, order)?,
        C::SaveNote { target, note } => cite::save_note(&mut p, *target, note)?,
        C::DeleteNote { target, note } => cite::delete_note(&mut p, *target, *note)?,
        C::SaveSource { source } => cite::save_source(&mut p, source)?,
        C::AddCitation { target, citation } => {
            cite::add_citation(&mut p, *target, citation, last_citation)?
        }
        C::UpdateCitation { id, citation } => cite::update_citation(&mut p, *id, citation)?,
        C::RemoveCitation { target, citation } => {
            cite::remove_citation(&mut p, *target, *citation)?
        }
        C::AttachMedia { target, media } => media::attach_media(&mut p, *target, media)?,
        C::DetachMedia { target, media } => media::detach_media(&mut p, *target, *media)?,
        C::SetPrimaryMedia { person, media } => media::set_primary(&mut p, *person, *media)?,
        C::ImportDraft {
            persons,
            families,
            attach_picture,
        } => {
            if *attach_picture {
                return Err(invalid("The picture must be stored by the session first."));
            }
            draft::import_draft(&mut p, persons, families, None)?
        }
        C::UpdateMedia {
            id,
            title,
            description,
            date_raw,
        } => media::update_media(
            &mut p,
            *id,
            title.as_deref(),
            description.as_deref(),
            date_raw.as_deref(),
        )?,
    };
    Ok((p.finish(cmd.label()), focus))
}

/// `importDraft` with its picture (already stored by the session) attached.
pub(crate) fn plan_draft(
    tree: &Tree,
    persons: &[crate::command::DraftPerson],
    families: &[crate::command::DraftFamily],
    picture: Option<&draft::StagedPicture>,
    now_ms: i64,
    day_order: DayOrder,
) -> Result<(ChangeSet, Option<Uuid>), ApiError> {
    let mut p = Planner::new(tree, now_ms, day_order);
    let focus = draft::import_draft(&mut p, persons, families, picture)?;
    let label = EditCommand::ImportDraft {
        persons: Vec::new(),
        families: Vec::new(),
        attach_picture: false,
    }
    .label();
    Ok((p.finish(label), focus))
}

/// Writes `cs` into `tree`: `after` present -> insert or replace, absent -> remove.
pub fn apply_to_tree(tree: &mut Tree, cs: &ChangeSet) {
    apply_to_tree_tracked(tree, cs, &mut HashMap::new());
}

/// As `apply_to_tree`; `removed_at` remembers where removed records sat so that putting them
/// back (undo of a delete) restores their place in the tree order.
pub(crate) fn apply_to_tree_tracked(
    tree: &mut Tree,
    cs: &ChangeSet,
    removed_at: &mut HashMap<Uuid, usize>,
) {
    fn put<T: Rec>(map: &mut IndexMap<Uuid, T>, rec: T, at: &mut HashMap<Uuid, usize>) {
        let id = rec.rid();
        if map.contains_key(&id) {
            map.insert(id, rec);
        } else if let Some(pos) = at.remove(&id).filter(|p| *p <= map.len()) {
            map.shift_insert(pos, id, rec);
        } else {
            map.insert(id, rec);
        }
    }
    fn take<T>(map: &mut IndexMap<Uuid, T>, id: Uuid, at: &mut HashMap<Uuid, usize>) {
        if let Some((pos, _, _)) = map.shift_remove_full(&id) {
            at.insert(id, pos);
        }
    }
    for ch in &cs.changes {
        match &ch.after {
            Some(Record::Person(r)) => put(&mut tree.persons, r.clone(), removed_at),
            Some(Record::Family(r)) => put(&mut tree.families, r.clone(), removed_at),
            Some(Record::Event(r)) => put(&mut tree.events, r.clone(), removed_at),
            Some(Record::Place(r)) => put(&mut tree.places, r.clone(), removed_at),
            Some(Record::Source(r)) => put(&mut tree.sources, r.clone(), removed_at),
            Some(Record::Repository(r)) => put(&mut tree.repositories, r.clone(), removed_at),
            Some(Record::Citation(r)) => put(&mut tree.citations, r.clone(), removed_at),
            Some(Record::Media(r)) => put(&mut tree.media, r.clone(), removed_at),
            Some(Record::Note(r)) => put(&mut tree.notes, r.clone(), removed_at),
            None => match ch.kind {
                RecordKind::Person => take(&mut tree.persons, ch.id, removed_at),
                RecordKind::Family => take(&mut tree.families, ch.id, removed_at),
                RecordKind::Event => take(&mut tree.events, ch.id, removed_at),
                RecordKind::Place => take(&mut tree.places, ch.id, removed_at),
                RecordKind::Source => take(&mut tree.sources, ch.id, removed_at),
                RecordKind::Repository => take(&mut tree.repositories, ch.id, removed_at),
                RecordKind::Citation => take(&mut tree.citations, ch.id, removed_at),
                RecordKind::Media => take(&mut tree.media, ch.id, removed_at),
                RecordKind::Note => take(&mut tree.notes, ch.id, removed_at),
            },
        }
    }
}

// ---- shared mapping helpers -------------------------------------------------------------

pub(crate) fn sex_of(kind: crate::dto::SexKind, text: Option<&str>, keep: Option<&Sex>) -> Sex {
    use crate::dto::SexKind as K;
    match kind {
        K::Male => Sex::Male,
        K::Female => Sex::Female,
        K::Intersex => Sex::Intersex,
        K::Unknown => Sex::Unknown,
        K::Other => Sex::Other(
            text.map(|t| t.trim().to_string())
                .or_else(|| match keep {
                    Some(Sex::Other(t)) => Some(t.clone()),
                    _ => None,
                })
                .unwrap_or_default(),
        ),
    }
}

pub(crate) fn rel_of(id: crate::dto::ParentRelId, keep: Option<&ParentRel>) -> ParentRel {
    use crate::dto::ParentRelId as R;
    match id {
        R::Birth => ParentRel::Birth,
        R::Adopted => ParentRel::Adopted,
        R::Step => ParentRel::Step,
        R::Foster => ParentRel::Foster,
        R::Guardian => ParentRel::Guardian,
        R::Sealed => ParentRel::Sealed,
        R::Unknown => ParentRel::Unknown,
        R::Other => match keep {
            Some(ParentRel::Other(t)) => ParentRel::Other(t.clone()),
            _ => ParentRel::Other(String::new()),
        },
    }
}

pub(crate) fn union_of(id: crate::dto::UnionKindId, keep: Option<&UnionKind>) -> UnionKind {
    use crate::dto::UnionKindId as U;
    match id {
        U::Married => UnionKind::Married,
        U::CivilUnion => UnionKind::CivilUnion,
        U::Unmarried => UnionKind::Unmarried,
        U::Unknown => UnionKind::Unknown,
        U::Other => match keep {
            Some(UnionKind::Other(t)) => UnionKind::Other(t.clone()),
            _ => UnionKind::Other(String::new()),
        },
    }
}

pub(crate) fn assertion(preferred: bool) -> Assertion {
    Assertion {
        preferred,
        proof: None,
        confidence: None,
    }
}

/// Ids of the persons and families that point at `person` in a family, in tree order.
pub(crate) fn families_of_child(tree: &Tree, person: Uuid) -> Vec<(&Family, &ChildLink)> {
    tree.families_as_child(person).collect()
}
