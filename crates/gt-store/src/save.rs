//! `Project::save_tree`: replace the whole tree in one transaction. The per-record row
//! writers (`write_*`, `delete_*`) are shared with the granular edits in `edit.rs`.

use crate::StoreError;
use crate::codec::*;
use crate::project::Project;
use gt_core::Uuid;
use gt_core::living::{LivingRule, compute_living};
use gt_core::model::*;
use gt_core::phonetic::{daitch_mokotoff, soundex};
use gt_core::tree::Tree;
use rusqlite::{Transaction, params};
use std::collections::HashMap;

/// Child tables first, so the bulk delete never cascades row by row.
const CLEAR_ORDER: [&str; 22] = [
    "search_words",
    "search_trigram",
    "name_phonetic",
    "citation_link",
    "note_link",
    "media_link",
    "ext_id",
    "participant",
    "repo_ref",
    "place_name",
    "name",
    "child_link",
    "event",
    "citation",
    "media",
    "note",
    "family",
    "person",
    "source",
    "repository",
    "place",
    "tree_header",
];

/// Full-text rows are keyed by `rowid = seq * 4 + code`, so one record's rows can be
/// replaced without scanning the (unindexed) `id` column.
pub(crate) const FTS_PERSON: i64 = 0;
pub(crate) const FTS_PLACE: i64 = 1;
pub(crate) const FTS_SOURCE: i64 = 2;
pub(crate) const FTS_NOTE: i64 = 3;

pub(crate) fn fts_rowid(code: i64, seq: i64) -> i64 {
    seq.saturating_mul(4).saturating_add(code)
}

fn s(id: &Uuid) -> String {
    id.to_string()
}

fn new_row_id() -> String {
    Uuid::now_v7().to_string()
}

/// A position as a database integer.
fn idx(i: usize) -> i64 {
    i64::try_from(i).unwrap_or(i64::MAX)
}

/// Citation, note, media and external-id links of one record.
fn save_common(tx: &Transaction, ty: &str, c: &Common) -> Result<(), StoreError> {
    let id = s(&c.id);
    save_cites_notes(tx, ty, &id, &c.citations, &c.notes)?;
    for (pos, m) in c.media.iter().enumerate() {
        let r = m.region.as_ref();
        tx.prepare_cached(
            "INSERT INTO media_link(id, target_type, target_id, pos, media_id, \"primary\", \
             region_x, region_y, region_w, region_h) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        )?
        .execute(params![
            new_row_id(),
            ty,
            id,
            idx(pos),
            s(&m.media),
            m.primary,
            r.map(|r| f64::from(r.x)),
            r.map(|r| f64::from(r.y)),
            r.map(|r| f64::from(r.w)),
            r.map(|r| f64::from(r.h)),
        ])?;
    }
    for (pos, e) in c.ext_ids.iter().enumerate() {
        let (kind, scheme) = ext_kind_cols(&e.kind);
        tx.prepare_cached(
            "INSERT INTO ext_id(id, target_type, target_id, pos, kind, scheme, value) \
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
        )?
        .execute(params![
            new_row_id(),
            ty,
            id,
            idx(pos),
            kind,
            scheme,
            e.value
        ])?;
    }
    Ok(())
}

fn save_cites_notes(
    tx: &Transaction,
    ty: &str,
    id: &str,
    cites: &[Uuid],
    notes: &[Uuid],
) -> Result<(), StoreError> {
    for (pos, c) in cites.iter().enumerate() {
        tx.prepare_cached(
            "INSERT INTO citation_link(id, target_type, target_id, pos, citation_id) \
             VALUES (?1,?2,?3,?4,?5)",
        )?
        .execute(params![new_row_id(), ty, id, idx(pos), s(c)])?;
    }
    for (pos, n) in notes.iter().enumerate() {
        tx.prepare_cached(
            "INSERT INTO note_link(id, target_type, target_id, pos, note_id) \
             VALUES (?1,?2,?3,?4,?5)",
        )?
        .execute(params![new_row_id(), ty, id, idx(pos), s(n)])?;
    }
    Ok(())
}

/// Deletes the links of one record (`ty` is the target type, `id` its id).
fn clear_links(tx: &Transaction, ty: &str, id: &str) -> Result<(), StoreError> {
    for sql in [
        "DELETE FROM citation_link WHERE target_type = ?1 AND target_id = ?2",
        "DELETE FROM note_link WHERE target_type = ?1 AND target_id = ?2",
        "DELETE FROM media_link WHERE target_type = ?1 AND target_id = ?2",
        "DELETE FROM ext_id WHERE target_type = ?1 AND target_id = ?2",
    ] {
        tx.prepare_cached(sql)?.execute(params![ty, id])?;
    }
    Ok(())
}

/// The name fields that feed the search rows (from a `Name` or straight from the database).
pub(crate) struct NameText {
    pub prefix: Option<String>,
    pub given: Option<String>,
    pub surname_prefix: Option<String>,
    pub surnames: Vec<String>,
    pub suffix: Option<String>,
    pub raw: Option<String>,
    pub call: Option<String>,
    pub nickname: Option<String>,
    pub patronymic: Option<String>,
    pub preferred: bool,
}

impl From<&Name> for NameText {
    fn from(n: &Name) -> Self {
        NameText {
            prefix: n.prefix.clone(),
            given: n.given.clone(),
            surname_prefix: n.surname_prefix.clone(),
            surnames: n.surnames.clone(),
            suffix: n.suffix.clone(),
            raw: n.raw.clone(),
            call: n.call.clone(),
            nickname: n.nickname.clone(),
            patronymic: n.patronymic.clone(),
            preferred: n.assertion.preferred,
        }
    }
}

fn display_name(n: &NameText) -> String {
    let mut parts: Vec<&str> = Vec::new();
    parts.extend(n.prefix.as_deref());
    parts.extend(n.given.as_deref());
    parts.extend(n.patronymic.as_deref());
    parts.extend(n.surname_prefix.as_deref());
    parts.extend(n.surnames.iter().map(String::as_str));
    parts.extend(n.suffix.as_deref());
    let joined = parts
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if joined.is_empty() {
        n.raw.clone().unwrap_or_default()
    } else {
        joined
    }
}

/// Names and raw text of a place.
pub(crate) fn place_text_of(p: &Place) -> String {
    let mut parts: Vec<&str> = p.names.iter().map(|n| n.name.as_str()).collect();
    parts.extend(p.raw.as_deref());
    parts.join(" ")
}

fn place_text(tree: &Tree, id: Uuid) -> String {
    tree.places.get(&id).map(place_text_of).unwrap_or_default()
}

/// Label and body text of a person: every name variant and the places of their events.
pub(crate) fn person_search(names: &[NameText], place_texts: Vec<String>) -> (String, String) {
    let preferred = names.iter().find(|n| n.preferred).or(names.first());
    let label = preferred.map(display_name).unwrap_or_default();
    let mut body: Vec<String> = Vec::new();
    for n in names {
        body.push(display_name(n));
        body.extend(n.raw.clone());
        body.extend(n.call.clone());
        body.extend(n.nickname.clone());
        body.extend(n.patronymic.clone());
    }
    body.extend(place_texts);
    (label, body.join(" "))
}

pub(crate) fn place_search(p: &Place) -> (String, String) {
    let label = p
        .names
        .first()
        .map(|n| n.name.clone())
        .or_else(|| p.raw.clone())
        .unwrap_or_default();
    (label, place_text_of(p))
}

pub(crate) fn source_search(src: &Source) -> (String, String) {
    let label = src
        .title
        .clone()
        .or_else(|| src.abbreviation.clone())
        .unwrap_or_default();
    let body = [&src.author, &src.publication, &src.abbreviation, &src.text]
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    (label, body)
}

pub(crate) fn note_search(n: &Note) -> (String, String) {
    (n.text.chars().take(60).collect(), n.text.clone())
}

pub(crate) fn add_search(
    tx: &Transaction,
    kind: &str,
    rowid: i64,
    id: &Uuid,
    label: &str,
    body: &str,
) -> Result<(), StoreError> {
    let label = label.replace(['\n', '\r'], " ");
    tx.prepare_cached(
        "INSERT INTO search_words(rowid, kind, id, label, body) VALUES (?1,?2,?3,?4,?5)",
    )?
    .execute(params![rowid, kind, s(id), label, body])?;
    tx.prepare_cached("INSERT INTO search_trigram(rowid, kind, id, text) VALUES (?1,?2,?3,?4)")?
        .execute(params![rowid, kind, s(id), format!("{label}\n{body}")])?;
    Ok(())
}

pub(crate) fn remove_search(tx: &Transaction, rowid: i64) -> Result<(), StoreError> {
    tx.prepare_cached("DELETE FROM search_words WHERE rowid = ?1")?
        .execute([rowid])?;
    tx.prepare_cached("DELETE FROM search_trigram WHERE rowid = ?1")?
        .execute([rowid])?;
    Ok(())
}

fn save_phonetic(tx: &Transaction, n: &Name) -> Result<(), StoreError> {
    let put = |algo: &str, code: String| -> Result<(), StoreError> {
        tx.prepare_cached(
            "INSERT INTO name_phonetic(id, name_id, algo, code) VALUES (?1,?2,?3,?4)",
        )?
        .execute(params![new_row_id(), s(&n.id), algo, code])?;
        Ok(())
    };
    for surname in &n.surnames {
        if let Some(code) = soundex(surname) {
            put("soundex", code)?;
        }
        for code in daitch_mokotoff(surname) {
            put("daitch_mokotoff", code)?;
        }
    }
    Ok(())
}

impl Project {
    /// The living rule from meta `living_max_age` (default 100).
    pub fn living_rule(&self) -> LivingRule {
        let v: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'living_max_age'",
                [],
                |r| r.get(0),
            )
            .ok();
        LivingRule {
            max_age_years: v.and_then(|v| v.trim().parse().ok()).unwrap_or(100),
        }
    }

    /// Replaces the whole tree in ONE transaction; recomputes `name_phonetic`,
    /// `person.living_cached` (living rule from meta key `living_max_age`, default 100)
    /// and the full-text rows. The position in each `IndexMap` is written as `seq`.
    pub fn save_tree(&mut self, tree: &Tree, today_jdn: i32) -> Result<(), StoreError> {
        let living = compute_living(tree, &self.living_rule(), today_jdn);
        self.conn.set_prepared_statement_cache_capacity(64);
        let tx = self.conn.transaction()?;
        // Records reference each other in any order; check references at commit.
        tx.pragma_update(None, "defer_foreign_keys", true)?;
        for t in CLEAR_ORDER {
            tx.execute(&format!("DELETE FROM {t}"), [])?;
        }
        save_all(&tx, tree, &living)?;
        tx.commit()?;
        Ok(())
    }
}

fn save_all(tx: &Transaction, tree: &Tree, living: &HashMap<Uuid, bool>) -> Result<(), StoreError> {
    let h = &tree.header;
    tx.execute(
        "INSERT INTO tree_header(id, producer, source_version, unknown_json) VALUES (1,?1,?2,?3)",
        params![h.producer, h.source_version, unknown_json(&h.unknown)?],
    )?;
    for (i, (id, p)) in tree.persons.iter().enumerate() {
        insert_person(tx, p, idx(i), living.get(id).copied().unwrap_or(true))?;
    }
    for (i, f) in tree.families.values().enumerate() {
        insert_family(tx, f, idx(i))?;
    }
    for (i, e) in tree.events.values().enumerate() {
        insert_event(tx, e, idx(i))?;
    }
    for (i, p) in tree.places.values().enumerate() {
        insert_place(tx, p, idx(i))?;
    }
    for (i, src) in tree.sources.values().enumerate() {
        insert_source(tx, src, idx(i))?;
    }
    for (i, r) in tree.repositories.values().enumerate() {
        insert_repository(tx, r, idx(i))?;
    }
    for (i, c) in tree.citations.values().enumerate() {
        insert_citation(tx, c, idx(i))?;
    }
    for (i, m) in tree.media.values().enumerate() {
        insert_media(tx, m, idx(i))?;
    }
    for (i, n) in tree.notes.values().enumerate() {
        insert_note(tx, n, idx(i))?;
    }
    index_all(tx, tree)
}

/// Writes the full-text rows of every person, place, source and note; the position in each
/// map is the record's `seq`.
pub(crate) fn index_all(tx: &Transaction, tree: &Tree) -> Result<(), StoreError> {
    // Places of each person's own events, built once (events_of per person is quadratic).
    let mut event_places: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for e in tree.events.values() {
        let Some(pl) = e.place else { continue };
        for part in &e.participants {
            match part.target {
                ParticipantRef::Person(pid) => event_places.entry(pid).or_default().push(pl),
                // A family event (marriage) belongs to both partners.
                ParticipantRef::Family(fid) => {
                    if let Some(f) = tree.families.get(&fid) {
                        for pid in [f.partner1, f.partner2].into_iter().flatten() {
                            event_places.entry(pid).or_default().push(pl);
                        }
                    }
                }
            }
        }
    }
    for (i, (id, p)) in tree.persons.iter().enumerate() {
        let names: Vec<NameText> = p.names.iter().map(NameText::from).collect();
        let places = event_places
            .get(id)
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .map(|pl| place_text(tree, *pl))
            .collect();
        let (label, body) = person_search(&names, places);
        let rowid = fts_rowid(FTS_PERSON, idx(i));
        add_search(tx, "person", rowid, id, &label, &body)?;
    }
    for (i, (id, p)) in tree.places.iter().enumerate() {
        let (label, body) = place_search(p);
        add_search(tx, "place", fts_rowid(FTS_PLACE, idx(i)), id, &label, &body)?;
    }
    for (i, (id, src)) in tree.sources.iter().enumerate() {
        let (label, body) = source_search(src);
        let rowid = fts_rowid(FTS_SOURCE, idx(i));
        add_search(tx, "source", rowid, id, &label, &body)?;
    }
    for (i, (id, n)) in tree.notes.iter().enumerate() {
        let (label, body) = note_search(n);
        add_search(tx, "note", fts_rowid(FTS_NOTE, idx(i)), id, &label, &body)?;
    }
    Ok(())
}

// ---- per-record writers ----------------------------------------------------------------
//
// `insert_*` writes the record row (an upsert: an existing row keeps its `seq`, and for a
// person its `living_cached`, and is never deleted, so rows that reference it survive) and
// its owned rows. `write_*` first removes the owned rows, so it replaces a record in place.
// `delete_*` removes the owned rows and the record. Referencing records are never touched.

pub(crate) fn insert_person(
    tx: &Transaction,
    p: &Person,
    seq: i64,
    living: bool,
) -> Result<(), StoreError> {
    let c = &p.common;
    let id = &c.id;
    tx.prepare_cached(
        "INSERT INTO person(id, seq, private, changed, unknown_json, sex, living_override, \
         living_cached) VALUES (?1,?2,?3,?4,?5,?6,?7,?8) \
         ON CONFLICT(id) DO UPDATE SET private=excluded.private, changed=excluded.changed, \
         unknown_json=excluded.unknown_json, sex=excluded.sex, \
         living_override=excluded.living_override",
    )?
    .execute(params![
        s(id),
        seq,
        c.private,
        c.changed,
        unknown_json(&c.unknown)?,
        enum_text(&p.sex),
        p.living_override.as_ref().map(enum_text),
        living,
    ])?;
    save_common(tx, "person", c)?;
    for (pos, n) in p.names.iter().enumerate() {
        save_name(tx, id, pos, n)?;
    }
    Ok(())
}

fn clear_person_owned(tx: &Transaction, id: &str) -> Result<(), StoreError> {
    for sql in [
        "DELETE FROM name_phonetic WHERE name_id IN (SELECT id FROM name WHERE person_id = ?1)",
        "DELETE FROM citation_link WHERE target_type = 'name' \
         AND target_id IN (SELECT id FROM name WHERE person_id = ?1)",
        "DELETE FROM note_link WHERE target_type = 'name' \
         AND target_id IN (SELECT id FROM name WHERE person_id = ?1)",
        "DELETE FROM name WHERE person_id = ?1",
    ] {
        tx.prepare_cached(sql)?.execute([id])?;
    }
    clear_links(tx, "person", id)
}

pub(crate) fn write_person(
    tx: &Transaction,
    p: &Person,
    seq: i64,
    living: bool,
) -> Result<(), StoreError> {
    clear_person_owned(tx, &s(&p.common.id))?;
    insert_person(tx, p, seq, living)
}

pub(crate) fn delete_person(tx: &Transaction, id: &Uuid) -> Result<(), StoreError> {
    let id = s(id);
    clear_person_owned(tx, &id)?;
    tx.prepare_cached("DELETE FROM person WHERE id = ?1")?
        .execute([&id])?;
    Ok(())
}

pub(crate) fn insert_family(tx: &Transaction, f: &Family, seq: i64) -> Result<(), StoreError> {
    let c = &f.common;
    let id = &c.id;
    tx.prepare_cached(
        "INSERT INTO family(id, seq, private, changed, unknown_json, partner1_id, partner2_id, \
         kind) VALUES (?1,?2,?3,?4,?5,?6,?7,?8) \
         ON CONFLICT(id) DO UPDATE SET private=excluded.private, changed=excluded.changed, \
         unknown_json=excluded.unknown_json, partner1_id=excluded.partner1_id, \
         partner2_id=excluded.partner2_id, kind=excluded.kind",
    )?
    .execute(params![
        s(id),
        seq,
        c.private,
        c.changed,
        unknown_json(&c.unknown)?,
        f.partner1.as_ref().map(s),
        f.partner2.as_ref().map(s),
        enum_text(&f.kind),
    ])?;
    save_common(tx, "family", c)?;
    for (pos, ch) in f.children.iter().enumerate() {
        let link_id = new_row_id();
        tx.prepare_cached(
            "INSERT INTO child_link(id, unknown_json, family_id, person_id, pos, rel1, rel2, \
             preferred, assertion_preferred, proof, confidence) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        )?
        .execute(params![
            link_id,
            unknown_json(&ch.unknown)?,
            s(id),
            s(&ch.person),
            idx(pos),
            enum_text(&ch.rel1),
            enum_text(&ch.rel2),
            ch.preferred,
            ch.assertion.preferred,
            ch.assertion.proof.as_ref().map(enum_text),
            ch.assertion.confidence,
        ])?;
        save_cites_notes(tx, "child_link", &link_id, &ch.citations, &[])?;
    }
    Ok(())
}

fn clear_family_owned(tx: &Transaction, id: &str) -> Result<(), StoreError> {
    for sql in [
        "DELETE FROM citation_link WHERE target_type = 'child_link' \
         AND target_id IN (SELECT id FROM child_link WHERE family_id = ?1)",
        "DELETE FROM note_link WHERE target_type = 'child_link' \
         AND target_id IN (SELECT id FROM child_link WHERE family_id = ?1)",
        "DELETE FROM child_link WHERE family_id = ?1",
    ] {
        tx.prepare_cached(sql)?.execute([id])?;
    }
    clear_links(tx, "family", id)
}

pub(crate) fn write_family(tx: &Transaction, f: &Family, seq: i64) -> Result<(), StoreError> {
    clear_family_owned(tx, &s(&f.common.id))?;
    insert_family(tx, f, seq)
}

pub(crate) fn delete_family(tx: &Transaction, id: &Uuid) -> Result<(), StoreError> {
    let id = s(id);
    clear_family_owned(tx, &id)?;
    tx.prepare_cached("DELETE FROM family WHERE id = ?1")?
        .execute([&id])?;
    Ok(())
}

pub(crate) fn insert_event(tx: &Transaction, e: &Event, seq: i64) -> Result<(), StoreError> {
    let c = &e.common;
    let id = &c.id;
    let d = date_cols(&e.date)?;
    tx.prepare_cached(
        "INSERT INTO event(id, seq, private, changed, unknown_json, kind, date_raw, date_json, \
         date_sort_min, date_sort_max, place_id, description, age, cause, negative, preferred, \
         proof, confidence) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18) \
         ON CONFLICT(id) DO UPDATE SET private=excluded.private, changed=excluded.changed, \
         unknown_json=excluded.unknown_json, kind=excluded.kind, date_raw=excluded.date_raw, \
         date_json=excluded.date_json, date_sort_min=excluded.date_sort_min, \
         date_sort_max=excluded.date_sort_max, place_id=excluded.place_id, \
         description=excluded.description, age=excluded.age, cause=excluded.cause, \
         negative=excluded.negative, preferred=excluded.preferred, proof=excluded.proof, \
         confidence=excluded.confidence",
    )?
    .execute(params![
        s(id),
        seq,
        c.private,
        c.changed,
        unknown_json(&c.unknown)?,
        enum_text(&e.kind),
        d.raw,
        d.json,
        d.min,
        d.max,
        e.place.as_ref().map(s),
        e.description,
        e.age,
        e.cause,
        e.negative,
        e.assertion.preferred,
        e.assertion.proof.as_ref().map(enum_text),
        e.assertion.confidence,
    ])?;
    save_common(tx, "event", c)?;
    for (pos, p) in e.participants.iter().enumerate() {
        let (ty, target) = match &p.target {
            ParticipantRef::Person(u) => ("person", u),
            ParticipantRef::Family(u) => ("family", u),
        };
        tx.prepare_cached(
            "INSERT INTO participant(id, event_id, pos, target_type, target_id, role) \
             VALUES (?1,?2,?3,?4,?5,?6)",
        )?
        .execute(params![
            new_row_id(),
            s(id),
            idx(pos),
            ty,
            s(target),
            enum_text(&p.role)
        ])?;
    }
    Ok(())
}

fn clear_event_owned(tx: &Transaction, id: &str) -> Result<(), StoreError> {
    tx.prepare_cached("DELETE FROM participant WHERE event_id = ?1")?
        .execute([id])?;
    clear_links(tx, "event", id)
}

pub(crate) fn write_event(tx: &Transaction, e: &Event, seq: i64) -> Result<(), StoreError> {
    clear_event_owned(tx, &s(&e.common.id))?;
    insert_event(tx, e, seq)
}

pub(crate) fn delete_event(tx: &Transaction, id: &Uuid) -> Result<(), StoreError> {
    let id = s(id);
    clear_event_owned(tx, &id)?;
    tx.prepare_cached("DELETE FROM event WHERE id = ?1")?
        .execute([&id])?;
    Ok(())
}

pub(crate) fn insert_place(tx: &Transaction, p: &Place, seq: i64) -> Result<(), StoreError> {
    let c = &p.common;
    let id = &c.id;
    tx.prepare_cached(
        "INSERT INTO place(id, seq, private, changed, unknown_json, parent_id, kind, lat, lon, \
         raw) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) \
         ON CONFLICT(id) DO UPDATE SET private=excluded.private, changed=excluded.changed, \
         unknown_json=excluded.unknown_json, parent_id=excluded.parent_id, kind=excluded.kind, \
         lat=excluded.lat, lon=excluded.lon, raw=excluded.raw",
    )?
    .execute(params![
        s(id),
        seq,
        c.private,
        c.changed,
        unknown_json(&c.unknown)?,
        p.parent.as_ref().map(s),
        p.kind,
        p.lat,
        p.lon,
        p.raw,
    ])?;
    save_common(tx, "place", c)?;
    for (pos, n) in p.names.iter().enumerate() {
        let d = date_cols(&n.date)?;
        tx.prepare_cached(
            "INSERT INTO place_name(id, place_id, pos, name, lang, date_raw, date_json, \
             date_sort_min, date_sort_max) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        )?
        .execute(params![
            new_row_id(),
            s(id),
            idx(pos),
            n.name,
            n.lang,
            d.raw,
            d.json,
            d.min,
            d.max
        ])?;
    }
    Ok(())
}

fn clear_place_owned(tx: &Transaction, id: &str) -> Result<(), StoreError> {
    tx.prepare_cached("DELETE FROM place_name WHERE place_id = ?1")?
        .execute([id])?;
    clear_links(tx, "place", id)
}

pub(crate) fn write_place(tx: &Transaction, p: &Place, seq: i64) -> Result<(), StoreError> {
    clear_place_owned(tx, &s(&p.common.id))?;
    insert_place(tx, p, seq)
}

pub(crate) fn delete_place(tx: &Transaction, id: &Uuid) -> Result<(), StoreError> {
    let id = s(id);
    clear_place_owned(tx, &id)?;
    tx.prepare_cached("DELETE FROM place WHERE id = ?1")?
        .execute([&id])?;
    Ok(())
}

pub(crate) fn insert_source(tx: &Transaction, src: &Source, seq: i64) -> Result<(), StoreError> {
    let c = &src.common;
    let id = &c.id;
    tx.prepare_cached(
        "INSERT INTO source(id, seq, private, changed, unknown_json, title, author, publication, \
         abbreviation, text) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) \
         ON CONFLICT(id) DO UPDATE SET private=excluded.private, changed=excluded.changed, \
         unknown_json=excluded.unknown_json, title=excluded.title, author=excluded.author, \
         publication=excluded.publication, abbreviation=excluded.abbreviation, text=excluded.text",
    )?
    .execute(params![
        s(id),
        seq,
        c.private,
        c.changed,
        unknown_json(&c.unknown)?,
        src.title,
        src.author,
        src.publication,
        src.abbreviation,
        src.text,
    ])?;
    save_common(tx, "source", c)?;
    for (pos, r) in src.repositories.iter().enumerate() {
        tx.prepare_cached(
            "INSERT INTO repo_ref(id, source_id, pos, repository_id, call_number) \
             VALUES (?1,?2,?3,?4,?5)",
        )?
        .execute(params![
            new_row_id(),
            s(id),
            idx(pos),
            s(&r.repository),
            r.call_number
        ])?;
    }
    Ok(())
}

fn clear_source_owned(tx: &Transaction, id: &str) -> Result<(), StoreError> {
    tx.prepare_cached("DELETE FROM repo_ref WHERE source_id = ?1")?
        .execute([id])?;
    clear_links(tx, "source", id)
}

pub(crate) fn write_source(tx: &Transaction, src: &Source, seq: i64) -> Result<(), StoreError> {
    clear_source_owned(tx, &s(&src.common.id))?;
    insert_source(tx, src, seq)
}

pub(crate) fn delete_source(tx: &Transaction, id: &Uuid) -> Result<(), StoreError> {
    let id = s(id);
    clear_source_owned(tx, &id)?;
    tx.prepare_cached("DELETE FROM source WHERE id = ?1")?
        .execute([&id])?;
    Ok(())
}

pub(crate) fn insert_repository(
    tx: &Transaction,
    r: &Repository,
    seq: i64,
) -> Result<(), StoreError> {
    let c = &r.common;
    tx.prepare_cached(
        "INSERT INTO repository(id, seq, private, changed, unknown_json, name, address, \
         urls_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8) \
         ON CONFLICT(id) DO UPDATE SET private=excluded.private, changed=excluded.changed, \
         unknown_json=excluded.unknown_json, name=excluded.name, address=excluded.address, \
         urls_json=excluded.urls_json",
    )?
    .execute(params![
        s(&c.id),
        seq,
        c.private,
        c.changed,
        unknown_json(&c.unknown)?,
        r.name,
        r.address,
        serde_json::to_string(&r.urls).map_err(|e| corrupt(e.to_string()))?,
    ])?;
    save_common(tx, "repository", c)
}

pub(crate) fn write_repository(
    tx: &Transaction,
    r: &Repository,
    seq: i64,
) -> Result<(), StoreError> {
    clear_links(tx, "repository", &s(&r.common.id))?;
    insert_repository(tx, r, seq)
}

pub(crate) fn delete_repository(tx: &Transaction, id: &Uuid) -> Result<(), StoreError> {
    let id = s(id);
    clear_links(tx, "repository", &id)?;
    tx.prepare_cached("DELETE FROM repository WHERE id = ?1")?
        .execute([&id])?;
    Ok(())
}

pub(crate) fn insert_citation(tx: &Transaction, ct: &Citation, seq: i64) -> Result<(), StoreError> {
    let c = &ct.common;
    let d = date_cols(&ct.date)?;
    tx.prepare_cached(
        "INSERT INTO citation(id, seq, private, changed, unknown_json, source_id, page, \
         date_raw, date_json, date_sort_min, date_sort_max, text, quality, ev_original, \
         ev_primary, ev_direct) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16) \
         ON CONFLICT(id) DO UPDATE SET private=excluded.private, changed=excluded.changed, \
         unknown_json=excluded.unknown_json, source_id=excluded.source_id, page=excluded.page, \
         date_raw=excluded.date_raw, date_json=excluded.date_json, \
         date_sort_min=excluded.date_sort_min, date_sort_max=excluded.date_sort_max, \
         text=excluded.text, quality=excluded.quality, ev_original=excluded.ev_original, \
         ev_primary=excluded.ev_primary, ev_direct=excluded.ev_direct",
    )?
    .execute(params![
        s(&c.id),
        seq,
        c.private,
        c.changed,
        unknown_json(&c.unknown)?,
        ct.source.as_ref().map(s),
        ct.page,
        d.raw,
        d.json,
        d.min,
        d.max,
        ct.text,
        ct.quality,
        ct.evidence.original,
        ct.evidence.primary_info,
        ct.evidence.direct,
    ])?;
    save_common(tx, "citation", c)
}

pub(crate) fn write_citation(tx: &Transaction, ct: &Citation, seq: i64) -> Result<(), StoreError> {
    clear_links(tx, "citation", &s(&ct.common.id))?;
    insert_citation(tx, ct, seq)
}

pub(crate) fn delete_citation(tx: &Transaction, id: &Uuid) -> Result<(), StoreError> {
    let id = s(id);
    clear_links(tx, "citation", &id)?;
    tx.prepare_cached("DELETE FROM citation WHERE id = ?1")?
        .execute([&id])?;
    Ok(())
}

pub(crate) fn insert_media(tx: &Transaction, m: &Media, seq: i64) -> Result<(), StoreError> {
    let c = &m.common;
    let d = date_cols(&m.date)?;
    tx.prepare_cached(
        "INSERT INTO media(id, seq, private, changed, unknown_json, path, original_name, mime, \
         title, date_raw, date_json, date_sort_min, date_sort_max, description, sha256) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15) \
         ON CONFLICT(id) DO UPDATE SET private=excluded.private, changed=excluded.changed, \
         unknown_json=excluded.unknown_json, path=excluded.path, \
         original_name=excluded.original_name, mime=excluded.mime, title=excluded.title, \
         date_raw=excluded.date_raw, date_json=excluded.date_json, \
         date_sort_min=excluded.date_sort_min, date_sort_max=excluded.date_sort_max, \
         description=excluded.description, sha256=excluded.sha256",
    )?
    .execute(params![
        s(&c.id),
        seq,
        c.private,
        c.changed,
        unknown_json(&c.unknown)?,
        m.path,
        m.original_name,
        m.mime,
        m.title,
        d.raw,
        d.json,
        d.min,
        d.max,
        m.description,
        m.sha256,
    ])?;
    save_common(tx, "media", c)
}

pub(crate) fn write_media(tx: &Transaction, m: &Media, seq: i64) -> Result<(), StoreError> {
    clear_links(tx, "media", &s(&m.common.id))?;
    insert_media(tx, m, seq)
}

pub(crate) fn delete_media(tx: &Transaction, id: &Uuid) -> Result<(), StoreError> {
    let id = s(id);
    clear_links(tx, "media", &id)?;
    tx.prepare_cached("DELETE FROM media WHERE id = ?1")?
        .execute([&id])?;
    Ok(())
}

pub(crate) fn insert_note(tx: &Transaction, n: &Note, seq: i64) -> Result<(), StoreError> {
    let c = &n.common;
    tx.prepare_cached(
        "INSERT INTO note(id, seq, private, changed, unknown_json, kind, text, lang, shared) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9) \
         ON CONFLICT(id) DO UPDATE SET private=excluded.private, changed=excluded.changed, \
         unknown_json=excluded.unknown_json, kind=excluded.kind, text=excluded.text, \
         lang=excluded.lang, shared=excluded.shared",
    )?
    .execute(params![
        s(&c.id),
        seq,
        c.private,
        c.changed,
        unknown_json(&c.unknown)?,
        enum_text(&n.kind),
        n.text,
        n.lang,
        n.shared,
    ])?;
    save_common(tx, "note", c)
}

pub(crate) fn write_note(tx: &Transaction, n: &Note, seq: i64) -> Result<(), StoreError> {
    clear_links(tx, "note", &s(&n.common.id))?;
    insert_note(tx, n, seq)
}

pub(crate) fn delete_note(tx: &Transaction, id: &Uuid) -> Result<(), StoreError> {
    let id = s(id);
    clear_links(tx, "note", &id)?;
    tx.prepare_cached("DELETE FROM note WHERE id = ?1")?
        .execute([&id])?;
    Ok(())
}

fn save_name(tx: &Transaction, person: &Uuid, pos: usize, n: &Name) -> Result<(), StoreError> {
    let d = date_cols(&n.date)?;
    let surnames = serde_json::to_string(&n.surnames).map_err(|e| corrupt(e.to_string()))?;
    tx.prepare_cached(
        "INSERT INTO name(id, private, unknown_json, person_id, pos, kind, preferred, proof, \
         confidence, prefix, given, call, surname_prefix, surnames_json, patronymic, suffix, \
         nickname, lang, date_raw, date_json, date_sort_min, date_sort_max, raw) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23)",
    )?
    .execute(params![
        s(&n.id),
        n.private,
        unknown_json(&n.unknown)?,
        s(person),
        idx(pos),
        enum_text(&n.kind),
        n.assertion.preferred,
        n.assertion.proof.as_ref().map(enum_text),
        n.assertion.confidence,
        n.prefix,
        n.given,
        n.call,
        n.surname_prefix,
        surnames,
        n.patronymic,
        n.suffix,
        n.nickname,
        n.lang,
        d.raw,
        d.json,
        d.min,
        d.max,
        n.raw,
    ])?;
    save_cites_notes(tx, "name", &s(&n.id), &n.citations, &n.notes)?;
    save_phonetic(tx, n)
}
