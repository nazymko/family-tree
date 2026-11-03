//! Full-text search: word prefixes, trigram substrings (and CJK), phonetic surname keys.

use crate::StoreError;
use crate::codec::parse_uuid;
use crate::project::Project;
use gt_core::Uuid;
use gt_core::phonetic::soundex;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HitKind {
    Person,
    Place,
    Source,
    Note,
}

impl HitKind {
    fn parse(s: &str) -> Result<HitKind, StoreError> {
        Ok(match s {
            "person" => HitKind::Person,
            "place" => HitKind::Place,
            "source" => HitKind::Source,
            "note" => HitKind::Note,
            other => return Err(StoreError::Corrupt(format!("bad search kind {other:?}"))),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub kind: HitKind,
    pub id: Uuid,
    pub label: String,
    pub snippet: Option<String>,
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF | 0x20000..=0x2FFFF)
}

/// An FTS5 phrase literal: double quotes doubled.
fn quote(term: &str) -> String {
    format!("\"{}\"", term.replace('"', "\"\""))
}

impl Project {
    /// Union of word-prefix matches, trigram matches and phonetic matches, deduplicated
    /// by id and ranked in that order, truncated to `limit`.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, StoreError> {
        let query = query.trim();
        if query.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let lim = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut hits: Vec<SearchHit> = Vec::new();
        let mut seen: HashSet<Uuid> = HashSet::new();
        let mut push = |hits: &mut Vec<SearchHit>, h: SearchHit| {
            if seen.insert(h.id) {
                hits.push(h);
            }
        };

        // 1. Words: every term as a prefix.
        let terms: Vec<String> = query
            .split_whitespace()
            .map(|t| format!("{}*", quote(t)))
            .collect();
        {
            let mut st = self.conn.prepare_cached(
                "SELECT kind, id, label, snippet(search_words, 3, '[', ']', '...', 10) \
                 FROM search_words WHERE search_words MATCH ?1 ORDER BY rank LIMIT ?2",
            )?;
            let mut rows = match st.query(rusqlite::params![terms.join(" "), lim]) {
                Ok(r) => r,
                // A query FTS5 cannot parse simply has no word matches.
                Err(rusqlite::Error::SqliteFailure(..)) => return Ok(hits),
                Err(e) => return Err(e.into()),
            };
            while let Some(r) = rows.next()? {
                let snippet: Option<String> = r.get(3)?;
                push(
                    &mut hits,
                    SearchHit {
                        kind: HitKind::parse(&r.get::<_, String>(0)?)?,
                        id: parse_uuid(&r.get::<_, String>(1)?)?,
                        label: r.get(2)?,
                        snippet: snippet.filter(|s| !s.is_empty()),
                    },
                );
            }
        }

        // 2. Trigram: at least 3 characters; shorter CJK text falls back to a substring scan.
        let chars = query.chars().count();
        let cjk = query.chars().any(is_cjk);
        if chars >= 3 || cjk {
            let (sql, arg) = if chars >= 3 {
                (
                    "SELECT kind, id, text FROM search_trigram WHERE search_trigram MATCH ?1 \
                     ORDER BY rank LIMIT ?2",
                    quote(query),
                )
            } else {
                let escaped = query
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_");
                (
                    "SELECT kind, id, text FROM search_trigram WHERE text LIKE ?1 ESCAPE '\\' \
                     LIMIT ?2",
                    format!("%{escaped}%"),
                )
            };
            let mut st = self.conn.prepare_cached(sql)?;
            let mut rows = st.query(rusqlite::params![arg, lim])?;
            while let Some(r) = rows.next()? {
                let text: String = r.get(2)?;
                let label = text.lines().next().unwrap_or_default().to_string();
                push(
                    &mut hits,
                    SearchHit {
                        kind: HitKind::parse(&r.get::<_, String>(0)?)?,
                        id: parse_uuid(&r.get::<_, String>(1)?)?,
                        label,
                        snippet: None,
                    },
                );
            }
        }

        // 3. Phonetic: soundex of the last term equals a stored surname code.
        if let Some(code) = query.split_whitespace().next_back().and_then(soundex) {
            let mut st = self.conn.prepare_cached(
                "SELECT w.id, w.label FROM search_words w WHERE w.kind = 'person' AND w.id IN \
                 (SELECT n.person_id FROM name_phonetic p JOIN name n ON n.id = p.name_id \
                  WHERE p.algo = 'soundex' AND p.code = ?1) \
                 ORDER BY w.label COLLATE icu_root LIMIT ?2",
            )?;
            let mut rows = st.query(rusqlite::params![code, lim])?;
            while let Some(r) = rows.next()? {
                push(
                    &mut hits,
                    SearchHit {
                        kind: HitKind::Person,
                        id: parse_uuid(&r.get::<_, String>(0)?)?,
                        label: r.get(1)?,
                        snippet: None,
                    },
                );
            }
        }

        hits.truncate(limit);
        Ok(hits)
    }
}
