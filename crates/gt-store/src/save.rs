//! `Project::save_tree`: replace the whole tree in one transaction.

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

fn s(id: &Uuid) -> String {
    id.to_string()
}

fn new_row_id() -> String {
    Uuid::now_v7().to_string()
}

fn seq(i: usize) -> i64 {
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
            seq(pos),
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
            seq(pos),
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
        .execute(params![new_row_id(), ty, id, seq(pos), s(c)])?;
    }
    for (pos, n) in notes.iter().enumerate() {
        tx.prepare_cached(
            "INSERT INTO note_link(id, target_type, target_id, pos, note_id) \
             VALUES (?1,?2,?3,?4,?5)",
        )?
        .execute(params![new_row_id(), ty, id, seq(pos), s(n)])?;
    }
    Ok(())
}

fn display_name(n: &Name) -> String {
    let mut parts: Vec<&str> = Vec::new();
    parts.extend(n.prefix.as_deref());
    parts.extend(n.given.as_deref());
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

fn place_text(tree: &Tree, id: Uuid) -> String {
    let Some(p) = tree.places.get(&id) else {
        return String::new();
    };
    let mut parts: Vec<&str> = p.names.iter().map(|n| n.name.as_str()).collect();
    parts.extend(p.raw.as_deref());
    parts.join(" ")
}

/// Label and body text of a person: every name variant and the places of their events.
fn person_search(tree: &Tree, p: &Person, event_places: &[Uuid]) -> (String, String) {
    let preferred = p
        .names
        .iter()
        .find(|n| n.assertion.preferred)
        .or(p.names.first());
    let label = preferred.map(display_name).unwrap_or_default();
    let mut body: Vec<String> = Vec::new();
    for n in &p.names {
        body.push(display_name(n));
        body.extend(n.raw.clone());
        body.extend(n.call.clone());
        body.extend(n.nickname.clone());
        body.extend(n.patronymic.clone());
    }
    for pl in event_places {
        body.push(place_text(tree, *pl));
    }
    (label, body.join(" "))
}

fn add_search(
    tx: &Transaction,
    kind: &str,
    id: &Uuid,
    label: &str,
    body: &str,
) -> Result<(), StoreError> {
    let label = label.replace(['\n', '\r'], " ");
    tx.prepare_cached("INSERT INTO search_words(kind, id, label, body) VALUES (?1,?2,?3,?4)")?
        .execute(params![kind, s(id), label, body])?;
    tx.prepare_cached("INSERT INTO search_trigram(kind, id, text) VALUES (?1,?2,?3)")?
        .execute(params![kind, s(id), format!("{label}\n{body}")])?;
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
    fn living_rule(&self) -> LivingRule {
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
    let h = &tree.header;
    tx.execute(
        "INSERT INTO tree_header(id, producer, source_version, unknown_json) VALUES (1,?1,?2,?3)",
        params![h.producer, h.source_version, unknown_json(&h.unknown)?],
    )?;

    for (i, (id, p)) in tree.persons.iter().enumerate() {
        let c = &p.common;
        tx.prepare_cached(
            "INSERT INTO person(id, seq, private, changed, unknown_json, sex, living_override, \
             living_cached) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        )?
        .execute(params![
            s(id),
            seq(i),
            c.private,
            c.changed,
            unknown_json(&c.unknown)?,
            enum_text(&p.sex),
            p.living_override.as_ref().map(enum_text),
            living.get(id).copied().unwrap_or(true),
        ])?;
        save_common(tx, "person", c)?;
        for (pos, n) in p.names.iter().enumerate() {
            save_name(tx, id, pos, n)?;
        }
        let (label, body) = person_search(tree, p, event_places.get(id).map_or(&[], Vec::as_slice));
        add_search(tx, "person", id, &label, &body)?;
    }

    for (i, (id, f)) in tree.families.iter().enumerate() {
        let c = &f.common;
        tx.prepare_cached(
            "INSERT INTO family(id, seq, private, changed, unknown_json, partner1_id, partner2_id, \
             kind) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        )?
        .execute(params![
            s(id),
            seq(i),
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
                seq(pos),
                enum_text(&ch.rel1),
                enum_text(&ch.rel2),
                ch.preferred,
                ch.assertion.preferred,
                ch.assertion.proof.as_ref().map(enum_text),
                ch.assertion.confidence,
            ])?;
            save_cites_notes(tx, "child_link", &link_id, &ch.citations, &[])?;
        }
    }

    for (i, (id, e)) in tree.events.iter().enumerate() {
        let c = &e.common;
        let d = date_cols(&e.date)?;
        tx.prepare_cached(
            "INSERT INTO event(id, seq, private, changed, unknown_json, kind, date_raw, date_json, \
             date_sort_min, date_sort_max, place_id, description, age, cause, negative, preferred, \
             proof, confidence) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
        )?
        .execute(params![
            s(id),
            seq(i),
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
                seq(pos),
                ty,
                s(target),
                enum_text(&p.role)
            ])?;
        }
    }

    for (i, (id, p)) in tree.places.iter().enumerate() {
        let c = &p.common;
        tx.prepare_cached(
            "INSERT INTO place(id, seq, private, changed, unknown_json, parent_id, kind, lat, lon, \
             raw) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        )?
        .execute(params![
            s(id),
            seq(i),
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
                seq(pos),
                n.name,
                n.lang,
                d.raw,
                d.json,
                d.min,
                d.max
            ])?;
        }
        let label = p
            .names
            .first()
            .map(|n| n.name.clone())
            .or_else(|| p.raw.clone())
            .unwrap_or_default();
        add_search(tx, "place", id, &label, &place_text(tree, *id))?;
    }

    for (i, (id, src)) in tree.sources.iter().enumerate() {
        let c = &src.common;
        tx.prepare_cached(
            "INSERT INTO source(id, seq, private, changed, unknown_json, title, author, publication, \
             abbreviation, text) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        )?
        .execute(params![
            s(id),
            seq(i),
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
                seq(pos),
                s(&r.repository),
                r.call_number
            ])?;
        }
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
        add_search(tx, "source", id, &label, &body)?;
    }

    for (i, (id, r)) in tree.repositories.iter().enumerate() {
        let c = &r.common;
        tx.prepare_cached(
            "INSERT INTO repository(id, seq, private, changed, unknown_json, name, address, \
             urls_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        )?
        .execute(params![
            s(id),
            seq(i),
            c.private,
            c.changed,
            unknown_json(&c.unknown)?,
            r.name,
            r.address,
            serde_json::to_string(&r.urls).map_err(|e| corrupt(e.to_string()))?,
        ])?;
        save_common(tx, "repository", c)?;
    }

    for (i, (id, ct)) in tree.citations.iter().enumerate() {
        let c = &ct.common;
        let d = date_cols(&ct.date)?;
        tx.prepare_cached(
            "INSERT INTO citation(id, seq, private, changed, unknown_json, source_id, page, \
             date_raw, date_json, date_sort_min, date_sort_max, text, quality, ev_original, \
             ev_primary, ev_direct) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
        )?
        .execute(params![
            s(id),
            seq(i),
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
        save_common(tx, "citation", c)?;
    }

    for (i, (id, m)) in tree.media.iter().enumerate() {
        let c = &m.common;
        let d = date_cols(&m.date)?;
        tx.prepare_cached(
            "INSERT INTO media(id, seq, private, changed, unknown_json, path, original_name, mime, \
             title, date_raw, date_json, date_sort_min, date_sort_max, description, sha256) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        )?
        .execute(params![
            s(id),
            seq(i),
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
        save_common(tx, "media", c)?;
    }

    for (i, (id, n)) in tree.notes.iter().enumerate() {
        let c = &n.common;
        tx.prepare_cached(
            "INSERT INTO note(id, seq, private, changed, unknown_json, kind, text, lang, shared) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        )?
        .execute(params![
            s(id),
            seq(i),
            c.private,
            c.changed,
            unknown_json(&c.unknown)?,
            enum_text(&n.kind),
            n.text,
            n.lang,
            n.shared,
        ])?;
        save_common(tx, "note", c)?;
        let label: String = n.text.chars().take(60).collect();
        add_search(tx, "note", id, &label, &n.text)?;
    }
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
        seq(pos),
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
