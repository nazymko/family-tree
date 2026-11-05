//! Granular persistence: apply a set of record changes in one transaction, keep the
//! full-text and phonetic rows current, and append to the change log. Also the `meta` API.

use crate::StoreError;
use crate::authors::{LogMeta, actor_columns, decode_record};
use crate::codec::{corrupt, parse_uuid};
use crate::project::Project;
use crate::save::*;
use gt_core::Uuid;
use gt_core::model::*;
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RecordKind {
    Person,
    Family,
    Event,
    Place,
    Source,
    Repository,
    Citation,
    Media,
    Note,
}

impl RecordKind {
    /// The table name, also the `target_type` written to `change_log`.
    fn as_str(self) -> &'static str {
        match self {
            RecordKind::Person => "person",
            RecordKind::Family => "family",
            RecordKind::Event => "event",
            RecordKind::Place => "place",
            RecordKind::Source => "source",
            RecordKind::Repository => "repository",
            RecordKind::Citation => "citation",
            RecordKind::Media => "media",
            RecordKind::Note => "note",
        }
    }

    pub(crate) fn parse(s: &str) -> Result<RecordKind, StoreError> {
        Ok(match s {
            "person" => RecordKind::Person,
            "family" => RecordKind::Family,
            "event" => RecordKind::Event,
            "place" => RecordKind::Place,
            "source" => RecordKind::Source,
            "repository" => RecordKind::Repository,
            "citation" => RecordKind::Citation,
            "media" => RecordKind::Media,
            "note" => RecordKind::Note,
            other => return Err(corrupt(format!("bad change_log target_type {other:?}"))),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Record {
    Person(Person),
    Family(Family),
    Event(Event),
    Place(Place),
    Source(Source),
    Repository(Repository),
    Citation(Citation),
    Media(Media),
    Note(Note),
}

impl Record {
    pub fn kind(&self) -> RecordKind {
        match self {
            Record::Person(_) => RecordKind::Person,
            Record::Family(_) => RecordKind::Family,
            Record::Event(_) => RecordKind::Event,
            Record::Place(_) => RecordKind::Place,
            Record::Source(_) => RecordKind::Source,
            Record::Repository(_) => RecordKind::Repository,
            Record::Citation(_) => RecordKind::Citation,
            Record::Media(_) => RecordKind::Media,
            Record::Note(_) => RecordKind::Note,
        }
    }

    pub fn id(&self) -> Uuid {
        match self {
            Record::Person(r) => r.common.id,
            Record::Family(r) => r.common.id,
            Record::Event(r) => r.common.id,
            Record::Place(r) => r.common.id,
            Record::Source(r) => r.common.id,
            Record::Repository(r) => r.common.id,
            Record::Citation(r) => r.common.id,
            Record::Media(r) => r.common.id,
            Record::Note(r) => r.common.id,
        }
    }
}

/// One record going from `before` to `after` (`None` = absent).
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub kind: RecordKind,
    pub id: Uuid,
    pub before: Option<Record>,
    pub after: Option<Record>,
}

/// The changes of one user action; they share a `group` and are applied atomically.
#[derive(Clone, Debug, PartialEq)]
pub struct ChangeSet {
    pub group: Uuid,
    /// The group this set undoes (set by `inverse`), recorded in the change log.
    pub reverts: Option<Uuid>,
    pub label: String,
    pub ts: i64,
    pub changes: Vec<Change>,
}

impl ChangeSet {
    /// Reversed order, `before` and `after` swapped; a fresh group that records the one it
    /// reverts, same label and time (the caller prefixes the label with "Undo: ").
    pub fn inverse(&self) -> ChangeSet {
        ChangeSet {
            group: Uuid::now_v7(),
            reverts: Some(self.group),
            label: self.label.clone(),
            ts: self.ts,
            changes: self
                .changes
                .iter()
                .rev()
                .map(|c| Change {
                    kind: c.kind,
                    id: c.id,
                    before: c.after.clone(),
                    after: c.before.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeOp {
    Create,
    Update,
    Delete,
}

impl ChangeOp {
    fn as_str(self) -> &'static str {
        match self {
            ChangeOp::Create => "create",
            ChangeOp::Update => "update",
            ChangeOp::Delete => "delete",
        }
    }

    pub(crate) fn parse(s: &str) -> Result<ChangeOp, StoreError> {
        Ok(match s {
            "create" | "insert" => ChangeOp::Create,
            "update" => ChangeOp::Update,
            "delete" => ChangeOp::Delete,
            other => return Err(corrupt(format!("bad change_log op {other:?}"))),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistoryEntry {
    pub ts: i64,
    pub group: Uuid,
    pub label: String,
    pub op: ChangeOp,
    pub kind: RecordKind,
    /// The record as this change found it and left it (`None` = absent), as stored in the log.
    pub before: Option<Record>,
    pub after: Option<Record>,
}

fn json<T: Serialize>(v: &T) -> Result<String, StoreError> {
    serde_json::to_string(v).map_err(|e| corrupt(e.to_string()))
}

fn existing_seq(tx: &Transaction, kind: RecordKind, id: &Uuid) -> Result<Option<i64>, StoreError> {
    let sql = format!("SELECT seq FROM {} WHERE id = ?1", kind.as_str());
    Ok(tx
        .prepare_cached(&sql)?
        .query_row([id.to_string()], |r| r.get(0))
        .optional()?)
}

fn next_seq(tx: &Transaction, kind: RecordKind) -> Result<i64, StoreError> {
    let sql = format!("SELECT COALESCE(MAX(seq), -1) + 1 FROM {}", kind.as_str());
    Ok(tx.prepare_cached(&sql)?.query_row([], |r| r.get(0))?)
}

/// The `seq` a record had when it was last deleted, if nobody has taken it since
/// (so that undoing a delete restores the record's place in the order).
fn deleted_seq(tx: &Transaction, kind: RecordKind, id: &Uuid) -> Result<Option<i64>, StoreError> {
    let last: Option<(String, Option<i64>)> = tx
        .prepare_cached(
            "SELECT op, before_seq FROM change_log WHERE target_type = ?1 AND target_id = ?2              ORDER BY id DESC LIMIT 1",
        )?
        .query_row(params![kind.as_str(), id.to_string()], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?;
    let Some((op, Some(seq))) = last else {
        return Ok(None);
    };
    if op != "delete" {
        return Ok(None);
    }
    let sql = format!(
        "SELECT EXISTS (SELECT 1 FROM {} WHERE seq = ?1)",
        kind.as_str()
    );
    let taken: bool = tx.prepare_cached(&sql)?.query_row([seq], |r| r.get(0))?;
    Ok((!taken).then_some(seq))
}

fn fts_code(kind: RecordKind) -> Option<i64> {
    match kind {
        RecordKind::Person => Some(FTS_PERSON),
        RecordKind::Place => Some(FTS_PLACE),
        RecordKind::Source => Some(FTS_SOURCE),
        RecordKind::Note => Some(FTS_NOTE),
        _ => None,
    }
}

/// Everything whose search text may have changed, collected from the changed records.
#[derive(Default)]
struct Affected {
    persons: HashSet<Uuid>,
    families: HashSet<Uuid>,
    places: HashSet<Uuid>,
}

impl Affected {
    fn note(&mut self, rec: &Record) {
        match rec {
            Record::Person(p) => {
                self.persons.insert(p.common.id);
            }
            Record::Family(f) => {
                self.persons
                    .extend([f.partner1, f.partner2].into_iter().flatten());
            }
            Record::Event(e) => {
                for p in &e.participants {
                    match p.target {
                        ParticipantRef::Person(u) => {
                            self.persons.insert(u);
                        }
                        ParticipantRef::Family(u) => {
                            self.families.insert(u);
                        }
                    }
                }
            }
            Record::Place(p) => {
                self.places.insert(p.common.id);
            }
            _ => {}
        }
    }
}

fn write_record(tx: &Transaction, rec: &Record) -> Result<(), StoreError> {
    let kind = rec.kind();
    let id = rec.id();
    let seq = match existing_seq(tx, kind, &id)? {
        Some(s) => s,
        None => match deleted_seq(tx, kind, &id)? {
            Some(s) => s,
            None => next_seq(tx, kind)?,
        },
    };
    match rec {
        // A new person counts as living until `living` says otherwise; an existing
        // person keeps the cached value.
        Record::Person(r) => write_person(tx, r, seq, true)?,
        Record::Family(r) => write_family(tx, r, seq)?,
        Record::Event(r) => write_event(tx, r, seq)?,
        Record::Place(r) => {
            write_place(tx, r, seq)?;
            let rowid = fts_rowid(FTS_PLACE, seq);
            remove_search(tx, rowid)?;
            let (label, body) = place_search(r);
            add_search(tx, "place", rowid, &id, &label, &body)?;
        }
        Record::Source(r) => {
            write_source(tx, r, seq)?;
            let rowid = fts_rowid(FTS_SOURCE, seq);
            remove_search(tx, rowid)?;
            let (label, body) = source_search(r);
            add_search(tx, "source", rowid, &id, &label, &body)?;
        }
        Record::Repository(r) => write_repository(tx, r, seq)?,
        Record::Citation(r) => write_citation(tx, r, seq)?,
        Record::Media(r) => write_media(tx, r, seq)?,
        Record::Note(r) => {
            write_note(tx, r, seq)?;
            let rowid = fts_rowid(FTS_NOTE, seq);
            remove_search(tx, rowid)?;
            let (label, body) = note_search(r);
            add_search(tx, "note", rowid, &id, &label, &body)?;
        }
    }
    Ok(())
}

/// Deletes a record; returns the `seq` it had.
fn delete_record(tx: &Transaction, kind: RecordKind, id: &Uuid) -> Result<Option<i64>, StoreError> {
    let seq = existing_seq(tx, kind, id)?;
    if let (Some(code), Some(seq)) = (fts_code(kind), seq) {
        remove_search(tx, fts_rowid(code, seq))?;
    }
    match kind {
        RecordKind::Person => delete_person(tx, id),
        RecordKind::Family => delete_family(tx, id),
        RecordKind::Event => delete_event(tx, id),
        RecordKind::Place => delete_place(tx, id),
        RecordKind::Source => delete_source(tx, id),
        RecordKind::Repository => delete_repository(tx, id),
        RecordKind::Citation => delete_citation(tx, id),
        RecordKind::Media => delete_media(tx, id),
        RecordKind::Note => delete_note(tx, id),
    }?;
    Ok(seq)
}

fn uuids(rows: Vec<String>) -> Result<Vec<Uuid>, StoreError> {
    rows.iter().map(|s| parse_uuid(s)).collect()
}

fn strings(tx: &Transaction, sql: &str, arg: &str) -> Result<Vec<String>, rusqlite::Error> {
    tx.prepare_cached(sql)?
        .query_map([arg], |r| r.get::<_, String>(0))?
        .collect()
}

/// Search label and body of a person, from the rows in the database.
fn person_text(tx: &Transaction, id: &str) -> Result<(String, String), StoreError> {
    let mut names = Vec::new();
    {
        let mut st = tx.prepare_cached(
            "SELECT prefix, given, surname_prefix, surnames_json, suffix, raw, call, nickname, \
             patronymic, preferred FROM name WHERE person_id = ?1 ORDER BY pos, rowid",
        )?;
        let mut rows = st.query([id])?;
        while let Some(r) = rows.next()? {
            let surnames: String = r.get(3)?;
            names.push(NameText {
                prefix: r.get(0)?,
                given: r.get(1)?,
                surname_prefix: r.get(2)?,
                surnames: serde_json::from_str(&surnames).map_err(|e| corrupt(e.to_string()))?,
                suffix: r.get(4)?,
                raw: r.get(5)?,
                call: r.get(6)?,
                nickname: r.get(7)?,
                patronymic: r.get(8)?,
                preferred: r.get(9)?,
            });
        }
    }
    // Places of the person's own events and of their family events, in event order. CROSS JOIN fixes
    // the family query's join order: left alone SQLite scans every family participant row.
    let mut found: Vec<(i64, i64, String)> = Vec::new();
    for sql in [
        "SELECT e.seq, p.pos, e.place_id FROM participant p JOIN event e ON e.id = p.event_id \
         WHERE p.target_type = 'person' AND p.target_id = ?1 AND e.place_id IS NOT NULL",
        "SELECT e.seq, p.pos, e.place_id \
         FROM (SELECT id FROM family WHERE partner1_id = ?1 \
               UNION SELECT id FROM family WHERE partner2_id = ?1) f \
         CROSS JOIN participant p ON p.target_type = 'family' AND p.target_id = f.id \
         CROSS JOIN event e ON e.id = p.event_id WHERE e.place_id IS NOT NULL",
    ] {
        let mut st = tx.prepare_cached(sql)?;
        let mut rows = st.query([id])?;
        while let Some(r) = rows.next()? {
            found.push((r.get(0)?, r.get(1)?, r.get(2)?));
        }
    }
    found.sort();
    let mut texts = Vec::with_capacity(found.len());
    for (_, _, place) in found {
        let mut parts = strings(
            tx,
            "SELECT name FROM place_name WHERE place_id = ?1 ORDER BY pos, rowid",
            &place,
        )?;
        let raw: Option<String> = tx
            .prepare_cached("SELECT raw FROM place WHERE id = ?1")?
            .query_row([&place], |r| r.get(0))
            .optional()?
            .flatten();
        parts.extend(raw);
        texts.push(parts.join(" "));
    }
    Ok(person_search(&names, texts))
}

fn refresh_person_search(tx: &Transaction, id: &Uuid) -> Result<(), StoreError> {
    let Some(seq) = existing_seq(tx, RecordKind::Person, id)? else {
        return Ok(());
    };
    let rowid = fts_rowid(FTS_PERSON, seq);
    remove_search(tx, rowid)?;
    // The unnamed-siblings marker is never searched.
    let marker: bool = tx
        .prepare_cached("SELECT sibling_marker FROM person WHERE id = ?1")?
        .query_row([id.to_string()], |r| r.get(0))?;
    if marker {
        return Ok(());
    }
    let (label, body) = person_text(tx, &id.to_string())?;
    add_search(tx, "person", rowid, id, &label, &body)
}

impl Project {
    /// Applies `cs` in ONE transaction (references are checked at commit, so the changes
    /// may come in any order). For each change: `after` present -> the record is written in
    /// place (its owned rows are replaced, its `seq` is kept; a new record goes after the
    /// current last), `after` absent -> the record and its owned rows are deleted. Rows of
    /// other records that reference it are not touched: the caller supplies those changes.
    /// Then the full-text rows of every affected person are rewritten (persons changed,
    /// event participants and, for family events, both spouses; persons with events at a
    /// changed place), `living` is stored in `person.living_cached`, and one `change_log`
    /// row per change is appended (with the author and batch from `meta`, and `meta.people`
    /// listed in `change_person`). On any error nothing is kept.
    pub fn apply_changes(
        &mut self,
        cs: &ChangeSet,
        living: &[(Uuid, bool)],
        meta: &LogMeta,
    ) -> Result<(), StoreError> {
        self.conn.set_prepared_statement_cache_capacity(64);
        let tx = self.conn.transaction()?;
        tx.pragma_update(None, "defer_foreign_keys", true)?;
        let mut affected = Affected::default();
        let group = cs.group.to_string();
        let (actor_kind, actor_id, actor_name) = actor_columns(&meta.actor);
        let batch = meta.batch.map(|b| b.to_string());
        for ch in &cs.changes {
            for rec in [&ch.before, &ch.after].into_iter().flatten() {
                affected.note(rec);
            }
            let op = match (&ch.before, &ch.after) {
                (None, Some(_)) => ChangeOp::Create,
                (Some(_), Some(_)) => ChangeOp::Update,
                (Some(_), None) => ChangeOp::Delete,
                (None, None) => return Err(corrupt("a change has neither before nor after")),
            };
            let before_seq = match &ch.after {
                Some(rec) => {
                    write_record(&tx, rec)?;
                    None
                }
                None => delete_record(&tx, ch.kind, &ch.id)?,
            };
            let before_json = ch.before.as_ref().map(json).transpose()?;
            let after_json = ch.after.as_ref().map(json).transpose()?;
            tx.prepare_cached(
                "INSERT INTO change_log(ts, target_type, target_id, op, before_json, group_id, \
                 label, after_json, before_seq, reverts_group, actor_kind, actor_id, actor_name, batch_id) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            )?
            .execute(params![
                cs.ts,
                ch.kind.as_str(),
                ch.id.to_string(),
                op.as_str(),
                before_json,
                group,
                cs.label,
                after_json,
                before_seq,
                cs.reverts.map(|u| u.to_string()),
                actor_kind,
                actor_id,
                actor_name,
                batch,
            ])?;
        }
        // The people the change is about, for every group (the log shows them); only agent
        // groups count as unreviewed.
        for person in &meta.people {
            tx.prepare_cached(
                "INSERT OR IGNORE INTO change_person(group_id, person_id) VALUES (?1, ?2)",
            )?
            .execute(params![group, person.to_string()])?;
        }

        // Place edits reach everyone with an event there; family events reach both spouses.
        for place in &affected.places {
            let events = strings(
                &tx,
                "SELECT id FROM event WHERE place_id = ?1",
                &place.to_string(),
            )?;
            for event in events {
                let mut st = tx.prepare_cached(
                    "SELECT target_type, target_id FROM participant WHERE event_id = ?1",
                )?;
                let mut rows = st.query([&event])?;
                while let Some(r) = rows.next()? {
                    let target = parse_uuid(&r.get::<_, String>(1)?)?;
                    match r.get::<_, String>(0)?.as_str() {
                        "person" => affected.persons.insert(target),
                        _ => affected.families.insert(target),
                    };
                }
            }
        }
        for fam in &affected.families {
            let partners: Option<(Option<String>, Option<String>)> = tx
                .prepare_cached("SELECT partner1_id, partner2_id FROM family WHERE id = ?1")?
                .query_row([fam.to_string()], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()?;
            if let Some((a, b)) = partners {
                affected
                    .persons
                    .extend(uuids(a.into_iter().chain(b).collect())?);
            }
        }
        for id in &affected.persons {
            refresh_person_search(&tx, id)?;
        }
        for (id, value) in living {
            tx.prepare_cached("UPDATE person SET living_cached = ?2 WHERE id = ?1")?
                .execute(params![id.to_string(), value])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Change-log entries of one record, newest first, with the record before and after each.
    pub fn history(&self, record: Uuid, limit: usize) -> Result<Vec<HistoryEntry>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT ts, group_id, label, op, target_type, before_json, after_json FROM change_log \
             WHERE target_id = ?1 ORDER BY ts DESC, id DESC LIMIT ?2",
        )?;
        let mut rows = st.query(params![
            record.to_string(),
            i64::try_from(limit).unwrap_or(i64::MAX)
        ])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let group: Option<String> = r.get(1)?;
            let label: Option<String> = r.get(2)?;
            out.push(HistoryEntry {
                ts: r.get(0)?,
                group: group
                    .as_deref()
                    .map(parse_uuid)
                    .transpose()?
                    .unwrap_or_default(),
                label: label.unwrap_or_default(),
                op: ChangeOp::parse(&r.get::<_, String>(3)?)?,
                kind: RecordKind::parse(&r.get::<_, String>(4)?)?,
                before: decode_record(r.get(5)?)?,
                after: decode_record(r.get(6)?)?,
            });
        }
        Ok(out)
    }

    pub fn meta_get(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    /// Keys used by the app: `living_max_age`, `home_person`, `last_import_id`.
    pub fn meta_set(&mut self, key: &str, value: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO meta(key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    /// Stores a living flag for many persons in one transaction.
    pub fn set_living_cached_all(
        &mut self,
        living: &HashMap<Uuid, bool>,
    ) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        {
            let mut st = tx.prepare("UPDATE person SET living_cached = ?2 WHERE id = ?1")?;
            for (id, value) in living {
                st.execute(params![id.to_string(), value])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn change_log_is_empty(&self) -> Result<bool, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT NOT EXISTS (SELECT 1 FROM change_log)", [], |r| {
                r.get(0)
            })?)
    }

    /// Rewrites every full-text row keyed by `seq` and sets meta `fts_scheme` = 2, in ONE
    /// transaction. Runs on open while the key is absent (a v1 project, or an upgrade that
    /// was interrupted); v1 saved `seq` as the position in the tree, which `index_all` assumes.
    pub(crate) fn rebuild_search(&mut self) -> Result<(), StoreError> {
        let tree = self.load_tree()?;
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM search_words", [])?;
        tx.execute("DELETE FROM search_trigram", [])?;
        index_all(&tx, &tree)?;
        tx.execute(
            "INSERT INTO meta(key, value) VALUES ('fts_scheme', '2')              ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }
}
