//! Place records: people who are not in the tree, from church books, confession lists, revisions
//! and censuses, kept per place (schema 15). A bulk table beside the tree, outside the change log.

use crate::StoreError;
use crate::project::Project;
use gt_core::Uuid;
use rusqlite::types::Value;
use rusqlite::{OptionalExtension, params, params_from_iter};
use serde::{Deserialize, Serialize};

/// The kinds a record can have, as stored.
pub const PLACE_RECORD_KINDS: [&str; 7] = [
    "birth",
    "marriage",
    "death",
    "confession",
    "revision",
    "census",
    "other",
];

/// The kind a book's `section` column means. Anything not recognised is `other`.
pub fn kind_from_section(section: &str) -> &'static str {
    let s = section.trim().to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| s.contains(w));
    if has(&["birth", "born", "bapt", "christ", "народж", "хрещ", "нарожд", "рожд"]) {
        "birth"
    } else if has(&["marr", "wedd", "шлюб", "вінч", "брак", "бракос"]) {
        "marriage"
    } else if has(&["death", "died", "burial", "смерт", "помер", "похов", "умер"]) {
        "death"
    } else if has(&["confess", "сповід", "сповѣд", "исповед", "спов"]) {
        "confession"
    } else if has(&["revis", "ревіз", "ревиз"]) {
        "revision"
    } else if has(&["census", "перепис"]) {
        "census"
    } else {
        "other"
    }
}

/// A kind id as stored, or `other` for text that names none of them.
pub fn normalize_kind(kind: &str) -> &'static str {
    let k = kind.trim().to_lowercase();
    PLACE_RECORD_KINDS
        .iter()
        .find(|x| **x == k)
        .copied()
        .unwrap_or_else(|| kind_from_section(&k))
}

/// The first plausible year (1000 to 2100) written in a date text: `1850`, `12.03.1850`,
/// `1850-03`, `бл. 1850`.
pub fn year_from_text(date: &str) -> Option<i32> {
    let b: Vec<char> = date.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i - start == 4 {
                let y: i32 = b[start..i].iter().collect::<String>().parse().ok()?;
                if (1000..=2100).contains(&y) {
                    return Some(y);
                }
            }
        } else {
            i += 1;
        }
    }
    None
}

/// Lower case with one apostrophe, so a search matches however the book wrote it.
pub fn fold_search(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            '\u{2019}' | '\u{02bc}' | '`' | '\u{2018}' => '\'',
            c => c,
        })
        .collect()
}

/// One person named in a record.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceRecordPerson {
    /// Row id once stored; 0 before.
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub given: String,
    #[serde(default)]
    pub patronymic: String,
    #[serde(default)]
    pub surname: String,
    /// The name as the book writes it.
    #[serde(default)]
    pub as_written: String,
    #[serde(default)]
    pub estate: String,
    #[serde(default)]
    pub age: String,
    #[serde(default)]
    pub note: String,
    /// The person in the tree this participant is, when a relative.
    #[serde(default)]
    pub person: Option<Uuid>,
}

/// A record to add.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceRecordInput {
    pub kind: String,
    #[serde(default)]
    pub date: String,
    /// Sortable year; read from `date` when absent.
    #[serde(default)]
    pub year: Option<i32>,
    #[serde(default)]
    pub book: String,
    #[serde(default)]
    pub page: String,
    /// The record's id in the book index. With a book it makes the record unique.
    #[serde(default)]
    pub rec_id: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub unsure: bool,
    #[serde(default)]
    pub people: Vec<PlaceRecordPerson>,
}

/// A stored record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceRecordRow {
    pub id: i64,
    pub place: Uuid,
    pub kind: String,
    pub date: String,
    pub year: Option<i32>,
    pub book: String,
    pub page: String,
    pub rec_id: String,
    pub note: String,
    pub unsure: bool,
    pub people: Vec<PlaceRecordPerson>,
}

/// What an add did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AddOutcome {
    pub added: u32,
    /// Skipped: a record with the same book and record id was there already.
    pub duplicates: u32,
}

/// Which records a query wants.
#[derive(Clone, Debug, Default)]
pub struct PlaceRecordFilter {
    /// The place and the places under it.
    pub places: Vec<Uuid>,
    pub kinds: Vec<String>,
    pub year_from: Option<i32>,
    pub year_to: Option<i32>,
    /// `Some(true)`: only records naming a relative; `Some(false)`: only those naming none.
    pub relatives: Option<bool>,
    /// Text in a name, already folded with [`fold_search`]; empty for any.
    pub query: String,
}

/// Counts of the records a filter selects, by kind and by decade (undated ones leave decades out).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaceRecordCounts {
    pub total: u32,
    pub by_kind: Vec<(String, u32)>,
    pub by_decade: Vec<(i32, u32)>,
}

/// The sort key of a record in a list: dated records by year, undated last, then the id.
pub type PlaceRecordKey = (i64, Option<i32>);

fn where_clause(f: &PlaceRecordFilter, with_kind_and_years: bool) -> (String, Vec<Value>) {
    let mut sql = String::from("r.place_id IN (SELECT value FROM json_each(?1))");
    let ids: Vec<String> = f.places.iter().map(Uuid::to_string).collect();
    let mut args: Vec<Value> = vec![Value::Text(
        serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into()),
    )];
    if with_kind_and_years {
        if !f.kinds.is_empty() {
            let kinds: Vec<&String> = f.kinds.iter().collect();
            sql.push_str(" AND r.kind IN (SELECT value FROM json_each(?");
            args.push(Value::Text(
                serde_json::to_string(&kinds).unwrap_or_else(|_| "[]".into()),
            ));
            sql.push_str(&args.len().to_string());
            sql.push_str("))");
        }
        if let Some(y) = f.year_from {
            args.push(Value::Integer(i64::from(y)));
            sql.push_str(&format!(" AND r.year >= ?{}", args.len()));
        }
        if let Some(y) = f.year_to {
            args.push(Value::Integer(i64::from(y)));
            sql.push_str(&format!(" AND r.year <= ?{}", args.len()));
        }
    }
    match f.relatives {
        Some(true) => sql.push_str(
            " AND EXISTS (SELECT 1 FROM place_record_person p WHERE p.record_id = r.id AND p.person_id IS NOT NULL)",
        ),
        Some(false) => sql.push_str(
            " AND NOT EXISTS (SELECT 1 FROM place_record_person p WHERE p.record_id = r.id AND p.person_id IS NOT NULL)",
        ),
        None => {}
    }
    if !f.query.trim().is_empty() {
        args.push(Value::Text(f.query.trim().to_string()));
        sql.push_str(&format!(" AND instr(r.search, ?{}) > 0", args.len()));
    }
    (sql, args)
}

fn text_of(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

fn search_blob(people: &[PlaceRecordPerson]) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for p in people {
        for s in [&p.given, &p.patronymic, &p.surname, &p.as_written] {
            if !s.trim().is_empty() {
                parts.push(s.trim());
            }
        }
    }
    fold_search(&parts.join(" "))
}

impl Project {
    /// Adds records to a place in ONE transaction. A record whose book and record id are already
    /// stored (and the record id is not empty) is skipped, so adding the same file twice adds
    /// nothing the second time.
    pub fn add_place_records(
        &mut self,
        place: Uuid,
        records: &[PlaceRecordInput],
        batch: Option<Uuid>,
        now_ms: i64,
    ) -> Result<AddOutcome, StoreError> {
        let tx = self.conn.transaction()?;
        let mut out = AddOutcome::default();
        {
            let mut rec = tx.prepare_cached(
                "INSERT OR IGNORE INTO place_record(place_id, kind, date, year, book, page, rec_id, \
                 note, unsure, search, batch_id, added_ms) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )?;
            let mut per = tx.prepare_cached(
                "INSERT INTO place_record_person(record_id, ord, role, given, patronymic, surname, \
                 as_written, estate, age, note, person_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;
            let place_text = place.to_string();
            let batch_text = batch.map(|b| b.to_string());
            for r in records {
                let date = text_of(&r.date, 80);
                let year = r.year.or_else(|| year_from_text(&date));
                let changed = rec.execute(params![
                    place_text,
                    normalize_kind(&r.kind),
                    date,
                    year,
                    text_of(&r.book, 200),
                    text_of(&r.page, 80),
                    text_of(&r.rec_id, 200),
                    text_of(&r.note, 4000),
                    i32::from(r.unsure),
                    search_blob(&r.people),
                    batch_text,
                    now_ms,
                ])?;
                if changed == 0 {
                    out.duplicates += 1;
                    continue;
                }
                let id = tx.last_insert_rowid();
                for (i, p) in r.people.iter().enumerate() {
                    per.execute(params![
                        id,
                        i as i64,
                        text_of(&p.role, 80),
                        text_of(&p.given, 200),
                        text_of(&p.patronymic, 200),
                        text_of(&p.surname, 200),
                        text_of(&p.as_written, 400),
                        text_of(&p.estate, 200),
                        text_of(&p.age, 80),
                        text_of(&p.note, 2000),
                        p.person.map(|u| u.to_string()),
                    ])?;
                }
                out.added += 1;
            }
        }
        tx.commit()?;
        Ok(out)
    }

    /// How many of `rec_ids` of `book` are stored already (for an import preview).
    pub fn count_place_records_present(
        &self,
        book: &str,
        rec_ids: &[&str],
    ) -> Result<u32, StoreError> {
        let mut st = self
            .conn
            .prepare_cached("SELECT 1 FROM place_record WHERE book = ?1 AND rec_id = ?2")?;
        let mut n = 0;
        for id in rec_ids {
            if id.is_empty() {
                continue;
            }
            if st.query_row(params![book, id], |_| Ok(())).optional()?.is_some() {
                n += 1;
            }
        }
        Ok(n)
    }

    /// The `(id, year)` of every record the filter selects, dated ones in year order, then the
    /// undated, ties by id. Light enough to hold for a whole village.
    pub fn place_record_keys(
        &self,
        f: &PlaceRecordFilter,
    ) -> Result<Vec<PlaceRecordKey>, StoreError> {
        let (w, args) = where_clause(f, true);
        let sql = format!(
            "SELECT r.id, r.year FROM place_record r WHERE {w} \
             ORDER BY r.year IS NULL, r.year, r.id"
        );
        let mut st = self.conn.prepare(&sql)?;
        let rows = st.query_map(params_from_iter(args), |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Counts by kind and decade of the records the filter selects, not looking at its kinds and
    /// years (so the overview stays whole while one is chosen).
    pub fn place_record_counts(
        &self,
        f: &PlaceRecordFilter,
    ) -> Result<PlaceRecordCounts, StoreError> {
        let (w, args) = where_clause(f, false);
        let mut out = PlaceRecordCounts::default();
        let mut st = self.conn.prepare(&format!(
            "SELECT r.kind, COUNT(*) FROM place_record r WHERE {w} GROUP BY r.kind"
        ))?;
        for row in st.query_map(params_from_iter(args.iter().cloned()), |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?))
        })? {
            let (k, n) = row?;
            out.total += n;
            out.by_kind.push((k, n));
        }
        let mut st = self.conn.prepare(&format!(
            "SELECT (r.year / 10) * 10, COUNT(*) FROM place_record r WHERE {w} AND r.year IS NOT NULL \
             GROUP BY 1 ORDER BY 1"
        ))?;
        for row in st.query_map(params_from_iter(args), |r| {
            Ok((r.get::<_, i32>(0)?, r.get::<_, u32>(1)?))
        })? {
            out.by_decade.push(row?);
        }
        Ok(out)
    }

    /// The records with these ids, with their people, in the order given.
    pub fn place_record_rows(&self, ids: &[i64]) -> Result<Vec<PlaceRecordRow>, StoreError> {
        let mut rec = self.conn.prepare_cached(
            "SELECT place_id, kind, date, year, book, page, rec_id, note, unsure \
             FROM place_record WHERE id = ?1",
        )?;
        let mut per = self.conn.prepare_cached(
            "SELECT id, role, given, patronymic, surname, as_written, estate, age, note, person_id \
             FROM place_record_person WHERE record_id = ?1 ORDER BY ord",
        )?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let head = rec
                .query_row([id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Option<i32>>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, String>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, String>(7)?,
                        r.get::<_, i32>(8)?,
                    ))
                })
                .optional()?;
            let Some((place, kind, date, year, book, page, rec_id, note, unsure)) = head else {
                continue;
            };
            let people = per
                .query_map([id], |r| {
                    let person: Option<String> = r.get(9)?;
                    Ok(PlaceRecordPerson {
                        id: r.get(0)?,
                        role: r.get(1)?,
                        given: r.get(2)?,
                        patronymic: r.get(3)?,
                        surname: r.get(4)?,
                        as_written: r.get(5)?,
                        estate: r.get(6)?,
                        age: r.get(7)?,
                        note: r.get(8)?,
                        person: person.and_then(|s| Uuid::parse_str(&s).ok()),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            out.push(PlaceRecordRow {
                id: *id,
                place: Uuid::parse_str(&place)
                    .map_err(|e| StoreError::Corrupt(format!("place record place: {e}")))?,
                kind,
                date,
                year,
                book,
                page,
                rec_id,
                note,
                unsure: unsure != 0,
                people,
            });
        }
        Ok(out)
    }

    /// Links a participant of a record to a person of the tree, or takes the link off. `false`
    /// when there is no such participant.
    pub fn link_place_record_person(
        &mut self,
        participant: i64,
        person: Option<Uuid>,
    ) -> Result<bool, StoreError> {
        let n = self.conn.execute(
            "UPDATE place_record_person SET person_id = ?2 WHERE id = ?1",
            params![participant, person.map(|p| p.to_string())],
        )?;
        Ok(n > 0)
    }

    /// Records of a place and the places under it, for anything that must know whether a place
    /// holds any.
    pub fn count_place_records(&self, places: &[Uuid]) -> Result<u32, StoreError> {
        let ids: Vec<String> = places.iter().map(Uuid::to_string).collect();
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM place_record WHERE place_id IN (SELECT value FROM json_each(?1))",
            [serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into())],
            |r| r.get(0),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_from_sections() {
        assert_eq!(kind_from_section("birth"), "birth");
        assert_eq!(kind_from_section("Народження"), "birth");
        assert_eq!(kind_from_section("marriage"), "marriage");
        assert_eq!(kind_from_section("шлюб"), "marriage");
        assert_eq!(kind_from_section("Death"), "death");
        assert_eq!(kind_from_section("сповідь"), "confession");
        assert_eq!(kind_from_section("ревізія"), "revision");
        assert_eq!(kind_from_section("перепис"), "census");
        assert_eq!(kind_from_section("whatever"), "other");
        assert_eq!(normalize_kind("revision"), "revision");
    }

    #[test]
    fn years_from_dates() {
        assert_eq!(year_from_text("1850"), Some(1850));
        assert_eq!(year_from_text("12.03.1850"), Some(1850));
        assert_eq!(year_from_text("бл. 1850-03"), Some(1850));
        assert_eq!(year_from_text("12.03"), None);
        assert_eq!(year_from_text("15000"), None);
        assert_eq!(year_from_text(""), None);
    }
}
