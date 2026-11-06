//! Working out what a relative's copy would change: nothing here writes to the tree.
//!
//! The base of each record comes from the two change logs (see the design): the copy's groups
//! that this tree has neither made nor already taken are "new", and the `before` of the first new
//! row on a record is the state both sides started from. A record whose new rows do not chain
//! into what the copy holds now, or that cannot be read, has no base, and the no-base rule
//! applies: an addition comes across, a removal is never made, and a record that differs on both
//! sides is left for the owner.

use super::copy::{MAX_RECORDS, OpenedCopy, open_copy};
use super::dto::{
    MaybeSameDto, MergeCheckDto, MergeCheckKind, MergeHistory, MergeLineDto, MergePreviewDto,
    MergeWhy,
};
use crate::error::{ApiError, ErrorCode};
use crate::field_diff::record_field_changes;
use crate::log::LogWords;
use crate::revert::{Reverted, identity, revert_change, same_ignoring_stamp, stripped};
use crate::session::Session;
use gt_core::model::{LivingOverride, Media, Person};
use gt_core::tree::Tree;
use gt_store::{Change, ChangeSet, LogRow, MergedGroup, Record, RecordKind};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;
use uuid::Uuid;

/// A row of the copy's log with more JSON than this is not from a family tree.
const MAX_ROW_JSON: usize = 8 << 20;
const MAYBE_SAME_MAX: usize = 3;

/// A photo that has to be copied into this tree's `media/` folder.
#[derive(Clone, Debug)]
pub(crate) struct MediaCopy {
    pub rel: String,
    pub sha: String,
    pub bytes: u64,
}

/// A planned merge, kept in the session until Apply or Cancel.
pub(crate) struct Plan {
    pub token: Uuid,
    /// `log_stamp` when the plan was made: any write since means a new preview.
    pub stamp: i64,
    pub name: String,
    pub source_name: String,
    pub sets: Vec<ChangeSet>,
    pub ledger: Vec<MergedGroup>,
    pub media: Vec<MediaCopy>,
    pub preview: MergePreviewDto,
    /// Kept open: the photos are read from it at apply time.
    pub copy: OpenedCopy,
}

// A short-lived plan value, one per record while a preview is built: boxing would only add noise.
#[allow(clippy::large_enum_variant)]
enum Act {
    Nothing,
    /// Not for this merge: the owner's version stands (owner changed it, or removed it).
    Left,
    Add(Record),
    Change {
        old: Record,
        base: Option<Record>,
        new: Record,
    },
    Hold {
        why: MergeWhy,
        base: Option<Record>,
        ours: Option<Record>,
        before: Option<Record>,
        after: Option<Record>,
    },
    Removal {
        ours: Record,
        base: Option<Record>,
    },
    Blocked {
        why: MergeWhy,
        before: Option<Record>,
        after: Option<Record>,
    },
}

struct Touch {
    rows: Vec<LogRow>,
    /// The new groups that touched it, in order.
    groups: Vec<Uuid>,
}

fn same(a: Option<&Record>, b: Option<&Record>) -> bool {
    same_ignoring_stamp(a, b)
}

fn records_of(t: &Tree) -> HashMap<Uuid, Record> {
    let mut m: HashMap<Uuid, Record> = HashMap::new();
    macro_rules! put {
        ($map:ident, $variant:ident) => {
            for (i, x) in &t.$map {
                m.insert(*i, Record::$variant(x.clone()));
            }
        };
    }
    put!(persons, Person);
    put!(families, Family);
    put!(events, Event);
    put!(places, Place);
    put!(sources, Source);
    put!(repositories, Repository);
    put!(citations, Citation);
    put!(media, Media);
    put!(notes, Note);
    m
}

fn record_count(t: &Tree) -> usize {
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

fn rank(k: RecordKind) -> u8 {
    match k {
        RecordKind::Person => 0,
        RecordKind::Family => 1,
        RecordKind::Event => 2,
        RecordKind::Place => 3,
        RecordKind::Source => 4,
        RecordKind::Repository => 5,
        RecordKind::Citation => 6,
        RecordKind::Media => 7,
        RecordKind::Note => 8,
    }
}

fn decode(json: &Option<String>, kind: RecordKind, id: Uuid) -> Option<Option<Record>> {
    match json {
        None => Some(None),
        Some(s) => {
            let r: Record = serde_json::from_str(s).ok()?;
            (r.kind() == kind && r.id() == id).then_some(Some(r))
        }
    }
}

/// The state both sides started from, when the copy's new rows on this record can be trusted:
/// each row starts where the one before ended, every row reads with today's model, and the last
/// one ends at what the copy holds now. `None`: no base.
fn base_of(
    rows: &[LogRow],
    kind: RecordKind,
    id: Uuid,
    theirs: Option<&Record>,
) -> Option<Option<Record>> {
    let mut first: Option<Option<Record>> = None;
    let mut prev: Option<Option<Record>> = None;
    for r in rows {
        let before = decode(&r.before_json, kind, id)?;
        let after = decode(&r.after_json, kind, id)?;
        match &prev {
            None => first = Some(before),
            Some(p) => {
                if !same(p.as_ref(), before.as_ref()) {
                    return None;
                }
            }
        }
        prev = Some(after);
    }
    let last = prev?;
    same(last.as_ref(), theirs).then_some(())?;
    first
}

// ---- what a change would make visible, or take away ------------------------------------------

fn rank_of(o: &Option<LivingOverride>) -> u8 {
    match o {
        Some(LivingOverride::Living) => 2,
        None => 1,
        Some(LivingOverride::Deceased) => 0,
    }
}

fn person_more_visible(old: &Person, new: &Person) -> bool {
    if rank_of(&new.living_override) < rank_of(&old.living_override) {
        return true;
    }
    old.names
        .iter()
        .any(|n| n.private && new.names.iter().any(|m| m.id == n.id && !m.private))
}

/// A change that would make a living or private person easier to see is never taken by itself.
fn more_visible(old: &Record, new: &Record) -> bool {
    let c = crate::revert::common_of;
    if c(old).private && !c(new).private {
        return true;
    }
    match (old, new) {
        (Record::Person(a), Record::Person(b)) => person_more_visible(a, b),
        _ => false,
    }
}

fn lost(o: &Value, n: &Value) -> bool {
    match (o, n) {
        (Value::Object(a), Value::Object(b)) => a
            .iter()
            .any(|(k, av)| b.get(k).is_some_and(|bv| lost(av, bv))),
        (Value::Array(a), Value::Array(b)) => {
            let ia: Vec<Value> = a.iter().map(identity).collect();
            let ib: Vec<Value> = b.iter().map(identity).collect();
            let missing = ia.iter().filter(|i| !ib.contains(i)).count();
            let fresh = ib.iter().filter(|i| !ia.contains(i)).count();
            if missing > fresh {
                return true;
            }
            a.iter().any(|x| {
                let id = identity(x);
                b.iter()
                    .find(|y| identity(y) == id)
                    .is_some_and(|y| lost(x, y))
            })
        }
        _ => false,
    }
}

/// A change that would take away a name, a link or a witness is listed, not applied.
fn takes_away(old: &Record, new: &Record) -> bool {
    match (stripped(old), stripped(new)) {
        (Some((o, _)), Some((n, _))) => lost(&o, &n),
        _ => true,
    }
}

// ---- photos ---------------------------------------------------------------------------------

enum MediaCheck {
    Link,
    File(MediaCopy, bool),
    Bad(MergeWhy),
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(crate) fn sha_of(path: &Path) -> std::io::Result<(String, u64)> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut total = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        total += n as u64;
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    Ok((hex, total))
}

/// `media/<aa>/<sha>[.ext]` split into `(aa, file name, sha)`; anything else is not a path this
/// app writes.
pub(crate) fn media_path_parts(path: &str) -> Option<(&str, &str, &str)> {
    let mut it = path.split('/');
    let (root, aa, file) = (it.next()?, it.next()?, it.next()?);
    if root != "media" || it.next().is_some() {
        return None;
    }
    let (stem, ext) = match file.split_once('.') {
        Some((s, e)) => (s, Some(e)),
        None => (file, None),
    };
    let ext_ok = ext.is_none_or(|e| {
        !e.is_empty() && e.len() <= 10 && e.bytes().all(|b| b.is_ascii_alphanumeric())
    });
    (is_hex64(stem) && ext_ok && aa == &stem[..2]).then_some((aa, file, stem))
}

fn check_media(sess: &Session, copy: &OpenedCopy, m: &Media) -> MediaCheck {
    let path = m.path.trim();
    if path.starts_with("http://") || path.starts_with("https://") {
        return MediaCheck::Link;
    }
    let Some((aa, file, stem)) = media_path_parts(path) else {
        return MediaCheck::Bad(MergeWhy::PhotoUnsafe);
    };
    if let Some(sha) = m.sha256.as_deref()
        && !sha.eq_ignore_ascii_case(stem)
    {
        return MediaCheck::Bad(MergeWhy::PhotoUnsafe);
    }
    let dir = copy.media_root.join(aa);
    let src = dir.join(file);
    let kind = |p: &Path| std::fs::symlink_metadata(p);
    match (kind(&dir), kind(&src)) {
        (Ok(d), Ok(f)) => {
            if d.file_type().is_symlink() || f.file_type().is_symlink() || !f.is_file() {
                return MediaCheck::Bad(MergeWhy::PhotoUnsafe);
            }
        }
        _ => return MediaCheck::Bad(MergeWhy::PhotoMissing),
    }
    let Ok((sha, bytes)) = sha_of(&src) else {
        return MediaCheck::Bad(MergeWhy::PhotoMissing);
    };
    if sha != stem {
        return MediaCheck::Bad(MergeWhy::PhotoDamaged);
    }
    let here = sess.project.dir.join("media").join(aa).join(file);
    let present = std::fs::symlink_metadata(&here).is_ok_and(|f| f.is_file())
        && sha_of(&here).is_ok_and(|(h, _)| h == stem);
    MediaCheck::File(
        MediaCopy {
            rel: format!("media/{aa}/{file}"),
            sha,
            bytes,
        },
        present,
    )
}

// ---- classification -----------------------------------------------------------------------

struct Ctx<'a> {
    sess: &'a Session,
    theirs: &'a HashMap<Uuid, Record>,
    ours: &'a HashMap<Uuid, Record>,
    based: bool,
    their_groups: &'a HashSet<Uuid>,
    touches: &'a HashMap<Uuid, Touch>,
}

impl Ctx<'_> {
    /// This tree's owner removed the record: a delete in this tree's log that is not the undo of
    /// an earlier merge. (Undoing a merge puts the owner back where they were, and the copy is
    /// offered again.)
    fn owner_removed(&self, id: Uuid) -> bool {
        let project = &self.sess.project;
        let Ok(rows) = project.record_rows(id) else {
            return true;
        };
        let from_copy = |g: Uuid| {
            project
                .log_group(g)
                .ok()
                .flatten()
                .is_some_and(|lg| matches!(lg.actor, Some(gt_store::Actor::Copy { .. })))
        };
        rows.iter().any(|r| {
            r.op == gt_store::ChangeOp::Delete
                && match r.group {
                    None => true,
                    Some(g) => match project.log_group(g) {
                        Ok(Some(lg)) => {
                            !(matches!(lg.actor, Some(gt_store::Actor::Copy { .. }))
                                || lg.reverts.is_some_and(from_copy))
                        }
                        _ => true,
                    },
                }
        })
    }

    /// This tree's own log has a row on the record that is not one of the copy's groups.
    fn owner_changed(&self, id: Uuid) -> bool {
        match self.sess.project.record_rows(id) {
            Ok(rows) => rows
                .iter()
                .any(|r| r.group.is_none_or(|g| !self.their_groups.contains(&g))),
            Err(_) => true,
        }
    }

    fn classify(&self, id: Uuid, kind: RecordKind) -> Act {
        let t = self.theirs.get(&id);
        let o = self.ours.get(&id);
        let touch = self.touches.get(&id);
        if self.based
            && let Some(tc) = touch
            && let Some(base) = base_of(&tc.rows, kind, id, t)
        {
            return self.three_way(id, kind, base, t, o);
        }
        match (t, o) {
            (None, None) => Act::Nothing,
            // A record only the copy has: an addition, unless this tree removed it (a removal
            // is never undone by a merge).
            (Some(t), None) => {
                if self.owner_removed(id) {
                    Act::Left
                } else {
                    Act::Add(t.clone())
                }
            }
            // Only here: never a removal.
            (None, Some(_)) => Act::Left,
            (Some(t), Some(o)) => {
                if same(Some(t), Some(o)) {
                    Act::Nothing
                } else if self.based && touch.is_none() && self.owner_changed(id) {
                    Act::Left
                } else {
                    Act::Hold {
                        why: MergeWhy::NoHistory,
                        base: None,
                        ours: Some(o.clone()),
                        before: Some(o.clone()),
                        after: Some(t.clone()),
                    }
                }
            }
        }
    }

    fn three_way(
        &self,
        id: Uuid,
        kind: RecordKind,
        base: Option<Record>,
        t: Option<&Record>,
        o: Option<&Record>,
    ) -> Act {
        match (base, t, o) {
            (None, None, _) => Act::Nothing,
            (None, Some(t), None) => {
                if self.owner_removed(id) {
                    Act::Left
                } else {
                    Act::Add(t.clone())
                }
            }
            (None, Some(t), Some(o)) => {
                if same(Some(t), Some(o)) {
                    Act::Nothing
                } else {
                    Act::Hold {
                        why: MergeWhy::CreatedOnBoth,
                        base: None,
                        ours: Some(o.clone()),
                        before: Some(o.clone()),
                        after: Some(t.clone()),
                    }
                }
            }
            (Some(_), None, None) => Act::Nothing,
            (Some(b), None, Some(o)) => Act::Removal {
                ours: o.clone(),
                base: Some(b),
            },
            (Some(b), Some(t), None) => Act::Hold {
                why: MergeWhy::RemovedHere,
                base: Some(b.clone()),
                ours: None,
                before: Some(b),
                after: Some(t.clone()),
            },
            (Some(b), Some(t), Some(o)) => {
                if same(Some(t), Some(o)) {
                    return Act::Nothing;
                }
                // The relative's change is `b -> t`; taking it onto `o` is undoing `t -> b`.
                let ch = Change {
                    kind,
                    id,
                    before: Some(t.clone()),
                    after: Some(b.clone()),
                };
                let hold = |why: MergeWhy, after: &Record| Act::Hold {
                    why,
                    base: Some(b.clone()),
                    ours: Some(o.clone()),
                    before: Some(b.clone()),
                    after: Some(after.clone()),
                };
                match revert_change(&ch, Some(o), &HashSet::new()) {
                    Reverted::Unchanged => Act::Nothing,
                    Reverted::Write(Some(m)) => {
                        if same(Some(&m), Some(o)) {
                            Act::Nothing
                        } else if more_visible(o, &m) {
                            Act::Hold {
                                why: MergeWhy::MoreVisible,
                                base: Some(b.clone()),
                                ours: Some(o.clone()),
                                before: Some(o.clone()),
                                after: Some(m),
                            }
                        } else if takes_away(o, &m) {
                            Act::Hold {
                                why: MergeWhy::TakesAway,
                                base: Some(b.clone()),
                                ours: Some(o.clone()),
                                before: Some(o.clone()),
                                after: Some(m),
                            }
                        } else {
                            Act::Change {
                                old: o.clone(),
                                base: Some(b),
                                new: m,
                            }
                        }
                    }
                    Reverted::Write(None) | Reverted::Conflict(_) => hold(MergeWhy::BothChanged, t),
                }
            }
        }
    }
}

fn name_key(p: &Person) -> Option<String> {
    let n = p.names.first()?;
    let given = n.given.as_deref()?.trim().to_lowercase();
    let sur = n.surnames.join(" ").trim().to_lowercase();
    (!given.is_empty() && !sur.is_empty()).then(|| format!("{given}|{sur}"))
}

fn kind_of(r: &Record) -> RecordKind {
    r.kind()
}

fn cap32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl Session {
    /// Reads the copy at `source` (a folder or a zip), works out what merging it would do, and
    /// keeps the plan for `merge_apply`. Writes nothing to the tree, its database or its folder.
    pub fn merge_preview(
        &mut self,
        source: &Path,
        name: &str,
    ) -> Result<MergePreviewDto, ApiError> {
        self.pending_merge = None;
        let plan = self.make_plan(source, name)?;
        let preview = plan.preview.clone();
        self.pending_merge = Some(Box::new(plan));
        Ok(preview)
    }

    /// Drops the preview waiting for Apply, if any.
    pub fn merge_cancel(&mut self) {
        self.pending_merge = None;
    }

    #[allow(clippy::too_many_lines)]
    fn make_plan(&self, source: &Path, name: &str) -> Result<Plan, ApiError> {
        let copy = open_copy(source)?;
        let their_tree = copy.project.load_tree()?;
        if record_count(&their_tree) > MAX_RECORDS {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "This copy holds far more records than a family tree can, so it was not opened.",
            ));
        }
        let name = {
            let n: String = name.trim().chars().take(80).collect();
            if n.is_empty() {
                copy.source_name.clone()
            } else {
                n
            }
        };
        let theirs = records_of(&their_tree);
        let ours = records_of(&self.tree);

        // Logs.
        let their_order = copy.project.log_groups_in_order()?;
        let our_order = self.project.log_groups_in_order()?;
        let ledgered = self.project.merged_source_groups()?;
        let our_set: HashSet<Uuid> = our_order.iter().copied().collect();
        let their_set: HashSet<Uuid> = their_order.iter().copied().collect();
        let shared_groups = their_order.iter().filter(|g| our_set.contains(g)).count();
        let shared_people = their_tree
            .persons
            .keys()
            .filter(|id| self.tree.persons.contains_key(*id))
            .count();
        if shared_people == 0 && shared_groups == 0 {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "This does not look like a copy of this tree: it shares no people with it.",
            ));
        }
        let based = !their_order.is_empty() && (shared_groups > 0 || our_order.is_empty());
        let history = if based {
            MergeHistory::Full
        } else if their_order.is_empty() {
            MergeHistory::None
        } else {
            MergeHistory::Unrelated
        };

        // The copy's new groups, and every record they touched.
        let new_groups: Vec<Uuid> = if based {
            their_order
                .iter()
                .filter(|g| !our_set.contains(g) && !ledgered.contains(g))
                .copied()
                .collect()
        } else {
            Vec::new()
        };
        let mut touches: HashMap<Uuid, Touch> = HashMap::new();
        let mut group_label: HashMap<Uuid, (String, i64)> = HashMap::new();
        let mut group_records: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for g in &new_groups {
            for r in copy.project.group_rows(*g)? {
                let big = |j: &Option<String>| j.as_ref().is_some_and(|s| s.len() > MAX_ROW_JSON);
                if big(&r.before_json) || big(&r.after_json) {
                    return Err(ApiError::new(
                        ErrorCode::Invalid,
                        "The edit history in this copy holds an entry far larger than a family tree can, so it was not opened.",
                    ));
                }
                group_label
                    .entry(*g)
                    .or_insert_with(|| (r.label.clone(), r.ts));
                let recs = group_records.entry(*g).or_default();
                if !recs.contains(&r.target) {
                    recs.push(r.target);
                }
                let t = touches.entry(r.target).or_insert_with(|| Touch {
                    rows: Vec::new(),
                    groups: Vec::new(),
                });
                if t.groups.last() != Some(g) {
                    t.groups.push(*g);
                }
                t.rows.push(r);
            }
        }
        for t in touches.values_mut() {
            t.rows.sort_by_key(|r| r.id);
        }

        let ctx = Ctx {
            sess: self,
            theirs: &theirs,
            ours: &ours,
            based,
            their_groups: &their_set,
            touches: &touches,
        };

        // Classify every record either side has.
        let mut ids: Vec<(u8, Uuid, RecordKind)> = theirs
            .iter()
            .chain(ours.iter())
            .map(|(i, r)| (rank(kind_of(r)), *i, kind_of(r)))
            .collect();
        for (i, t) in &touches {
            if let Some(r) = t.rows.first() {
                ids.push((rank(r.kind), *i, r.kind));
            }
        }
        ids.sort_by_key(|(k, i, _)| (*k, *i));
        ids.dedup_by_key(|(_, i, _)| *i);
        let mut acts: Vec<(Uuid, RecordKind, Act)> = ids
            .into_iter()
            .map(|(_, id, kind)| (id, kind, ctx.classify(id, kind)))
            .collect();

        // Photos: a record is not written when its photo cannot come across.
        let mut media_need: HashMap<Uuid, (MediaCopy, bool)> = HashMap::new();
        for (id, _, act) in &mut acts {
            let (rec, old) = match act {
                Act::Add(r) => (r.clone(), None),
                Act::Change { new, old, .. } => (new.clone(), Some(old.clone())),
                _ => continue,
            };
            let Record::Media(m) = &rec else { continue };
            if let Some(Record::Media(o)) = &old
                && o.path == m.path
                && o.sha256 == m.sha256
            {
                continue;
            }
            match check_media(self, &copy, m) {
                MediaCheck::Link => {}
                MediaCheck::File(c, present) => {
                    media_need.insert(*id, (c, present));
                }
                MediaCheck::Bad(why) => {
                    *act = Act::Blocked {
                        why,
                        before: old,
                        after: Some(rec),
                    };
                }
            }
        }

        // A record is not written when it would point at one that is not coming across.
        let writes: Vec<Change> = acts
            .iter()
            .filter_map(|(id, kind, act)| match act {
                Act::Add(r) => Some(Change {
                    kind: *kind,
                    id: *id,
                    before: None,
                    after: Some(r.clone()),
                }),
                Act::Change { old, new, .. } => Some(Change {
                    kind: *kind,
                    id: *id,
                    before: Some(old.clone()),
                    after: Some(new.clone()),
                }),
                _ => None,
            })
            .collect();
        let dropped = crate::revert::dangling_writes(&self.tree, &writes);
        for (id, _, act) in &mut acts {
            if !dropped.contains(id) {
                continue;
            }
            *act = match std::mem::replace(act, Act::Nothing) {
                Act::Add(r) => Act::Blocked {
                    why: MergeWhy::NeedsMissing,
                    before: None,
                    after: Some(r),
                },
                Act::Change { old, new, .. } => Act::Blocked {
                    why: MergeWhy::NeedsMissing,
                    before: Some(old),
                    after: Some(new),
                },
                other => other,
            };
        }

        // Who owns each applied write: the last new group that touched the record.
        let now = (self.clock.0)().1;
        let mut sets: Vec<ChangeSet> = Vec::new();
        let mut set_of_group: HashMap<Option<Uuid>, usize> = HashMap::new();
        let mut applied: HashMap<Uuid, usize> = HashMap::new();
        let mut order: Vec<Option<Uuid>> = new_groups.iter().map(|g| Some(*g)).collect();
        order.push(None);
        let mut by_owner: HashMap<Option<Uuid>, Vec<usize>> = HashMap::new();
        for (i, (id, _, act)) in acts.iter().enumerate() {
            if matches!(act, Act::Add(_) | Act::Change { .. }) {
                let last = touches.get(id).and_then(|t| t.groups.last().copied());
                by_owner.entry(last).or_default().push(i);
            }
        }
        for owner in &order {
            for &i in by_owner.get(owner).map_or(&[][..], Vec::as_slice) {
                let Some((id, kind, act)) = acts.get(i) else {
                    continue;
                };
                let (before, after) = match act {
                    Act::Add(r) => (None, r.clone()),
                    Act::Change { old, new, .. } => (Some(old.clone()), new.clone()),
                    _ => continue,
                };
                let idx = *set_of_group.entry(*owner).or_insert_with(|| {
                    let label = owner
                        .and_then(|g| group_label.get(&g))
                        .map_or_else(|| format!("Merged from {name}"), |(l, _)| l.clone());
                    sets.push(ChangeSet {
                        group: gt_core::ids::new_id(),
                        reverts: None,
                        label,
                        ts: now,
                        changes: Vec::new(),
                    });
                    sets.len() - 1
                });
                let mut after = after;
                crate::revert::stamp_changed(&mut after, now);
                sets[idx].changes.push(Change {
                    kind: *kind,
                    id: *id,
                    before,
                    after: Some(after),
                });
                applied.insert(*id, idx);
            }
        }

        // The ledger: a group is done when everything it touched was applied or needs nothing.
        let mut ledger: Vec<MergedGroup> = Vec::new();
        let act_at: HashMap<Uuid, usize> = acts
            .iter()
            .enumerate()
            .map(|(i, (id, _, _))| (*id, i))
            .collect();
        for g in &new_groups {
            let mut done = true;
            let mut owner_set: Option<usize> = set_of_group.get(&Some(*g)).copied();
            for id in group_records.get(g).map_or(&[][..], Vec::as_slice) {
                if let Some(i) = applied.get(id) {
                    owner_set = owner_set.or(Some(*i));
                    continue;
                }
                let resolved = act_at
                    .get(id)
                    .and_then(|i| acts.get(*i))
                    .is_none_or(|(_, _, a)| matches!(a, Act::Nothing | Act::Left));
                done &= resolved;
            }
            if done {
                ledger.push(MergedGroup {
                    source_group: *g,
                    merge_group: owner_set.map(|i| sets[i].group),
                    source_name: name.clone(),
                    source_ts: group_label.get(g).map(|(_, ts)| *ts),
                });
            }
        }

        // The preview, in words.
        let media: Vec<MediaCopy> = {
            let mut seen: HashSet<String> = HashSet::new();
            let mut out = Vec::new();
            for (id, (c, present)) in &media_need {
                if applied.contains_key(id) && !present && seen.insert(c.rel.clone()) {
                    out.push(c.clone());
                }
            }
            out.sort_by(|a, b| a.rel.cmp(&b.rel));
            out
        };
        let preview = self.build_preview(
            &name,
            &copy.source_name,
            history,
            &acts,
            &media,
            applied.len(),
            &ours,
        );
        let stamp = self.project.log_stamp()?;
        let mut plan = Plan {
            token: gt_core::ids::new_id(),
            stamp,
            name,
            source_name: copy.source_name.clone(),
            sets,
            ledger,
            media,
            preview,
            copy,
        };
        plan.preview.token = plan.token;
        Ok(plan)
    }

    #[allow(clippy::too_many_arguments)]
    fn build_preview(
        &self,
        name: &str,
        source_name: &str,
        history: MergeHistory,
        acts: &[(Uuid, RecordKind, Act)],
        media: &[MediaCopy],
        to_apply: usize,
        ours: &HashMap<Uuid, Record>,
    ) -> MergePreviewDto {
        // What to show, in one list so the log's wording can name records added together.
        struct Shown {
            list: u8,
            change: Change,
            why: Option<MergeWhy>,
            also: bool,
            yours: Option<(Record, Record)>,
            maybe_same: bool,
        }
        const ADDED: u8 = 0;
        const CHANGED: u8 = 1;
        const UNDECIDED: u8 = 2;
        const REMOVED: u8 = 3;
        const BLOCKED: u8 = 4;
        let mut shown: Vec<Shown> = Vec::new();
        let mut left_alone = 0usize;
        for (id, kind, act) in acts {
            let mk = |before: Option<Record>, after: Option<Record>| Change {
                kind: *kind,
                id: *id,
                before,
                after,
            };
            match act {
                Act::Nothing => {}
                Act::Left => left_alone += 1,
                Act::Add(r) => shown.push(Shown {
                    list: ADDED,
                    change: mk(None, Some(r.clone())),
                    why: None,
                    also: false,
                    yours: None,
                    maybe_same: matches!(r, Record::Person(_)),
                }),
                Act::Change { old, base, new } => shown.push(Shown {
                    list: CHANGED,
                    change: mk(Some(old.clone()), Some(new.clone())),
                    why: None,
                    also: base.as_ref().is_some_and(|b| !same(Some(b), Some(old))),
                    yours: None,
                    maybe_same: false,
                }),
                Act::Hold {
                    why,
                    base,
                    ours,
                    before,
                    after,
                } => shown.push(Shown {
                    list: UNDECIDED,
                    change: mk(before.clone(), after.clone()),
                    why: Some(*why),
                    also: false,
                    yours: base.clone().zip(ours.clone()),
                    maybe_same: false,
                }),
                Act::Removal { ours, base } => shown.push(Shown {
                    list: REMOVED,
                    change: mk(Some(ours.clone()), None),
                    why: None,
                    also: base.as_ref().is_some_and(|b| !same(Some(b), Some(ours))),
                    yours: None,
                    maybe_same: false,
                }),
                Act::Blocked { why, before, after } => shown.push(Shown {
                    list: BLOCKED,
                    change: mk(before.clone(), after.clone()),
                    why: Some(*why),
                    also: false,
                    yours: None,
                    maybe_same: false,
                }),
            }
        }
        let changes: Vec<Change> = shown.iter().map(|s| s.change.clone()).collect();
        let dtos = self.change_dtos(&changes, false);
        let words = LogWords {
            session: self,
            group: &[],
        };
        // Names already in this tree, for "might be the same person".
        let mut by_name: HashMap<String, Vec<Uuid>> = HashMap::new();
        if shown.iter().any(|s| s.maybe_same) {
            for (id, p) in self.tree.persons.iter().filter(|(_, p)| !p.sibling_marker) {
                if let Some(k) = name_key(p) {
                    by_name.entry(k).or_default().push(*id);
                }
            }
        }
        let mut out: [Vec<MergeLineDto>; 5] = Default::default();
        for (s, dto) in shown.iter().zip(dtos) {
            let person = self
                .people_of(&ChangeSet {
                    group: Uuid::nil(),
                    reverts: None,
                    label: String::new(),
                    ts: 0,
                    changes: vec![s.change.clone()],
                })
                .into_iter()
                .find(|p| self.tree.persons.contains_key(p));
            let maybe_same = match (&s.change.after, s.maybe_same) {
                (Some(Record::Person(p)), true) => name_key(p)
                    .and_then(|k| by_name.get(&k))
                    .map(|v| {
                        v.iter()
                            .take(MAYBE_SAME_MAX)
                            .filter_map(|id| self.person_ref(*id))
                            .map(|r| MaybeSameDto {
                                id: r.id,
                                name: r.name,
                                lifespan: r.lifespan,
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
            let yours = s
                .yours
                .as_ref()
                .map(|(b, o)| record_field_changes(Some(b), Some(o), &words))
                .unwrap_or_default();
            if let Some(slot) = out.get_mut(usize::from(s.list)) {
                slot.push(MergeLineDto {
                    change: dto,
                    person,
                    why: s.why,
                    also_edited_here: s.also,
                    yours,
                    maybe_same,
                });
            }
        }
        let [added, changed, undecided, removed, blocked] = out;
        let _ = ours;
        let nothing_new = to_apply == 0
            && undecided.is_empty()
            && removed.is_empty()
            && blocked.is_empty()
            && added.is_empty()
            && changed.is_empty();
        MergePreviewDto {
            token: Uuid::nil(),
            name: name.to_string(),
            source_name: source_name.to_string(),
            history,
            checks: vec![
                MergeCheckDto {
                    kind: MergeCheckKind::ReadableCopy,
                    ok: true,
                },
                MergeCheckDto {
                    kind: MergeCheckKind::SameTree,
                    ok: true,
                },
                MergeCheckDto {
                    kind: MergeCheckKind::History,
                    ok: history == MergeHistory::Full,
                },
            ],
            nothing_new,
            to_apply: cap32(to_apply),
            added,
            changed,
            undecided,
            removed,
            blocked,
            left_alone: cap32(left_alone),
            photos: cap32(media.len()),
            photo_bytes: media.iter().map(|m| m.bytes).sum(),
        }
    }
}
