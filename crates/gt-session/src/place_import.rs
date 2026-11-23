//! Reading the owner's index files (TSV) into place records.
//!
//! One file row is one person in one record; all rows with the same `rec_id` are one record. The
//! 16 columns are
//! `book year section rec_no rec_id date page role given patronymic surname as_written estate age note unsure`.
//! A `rec_id` often reads `<code>:<village>:<year>:<section>:<no>`; a book of one village has none
//! in it. Reading is a pure function of the files (no session), so the app can do it without
//! holding the tree; adding the records is `Session::add_place_records`, chunk by chunk.

use crate::error::{ApiError, ErrorCode};
use crate::place_events::KindCount;
use gt_store::{
    PlaceRecordInput, PlaceRecordPerson, kind_from_section, normalize_kind, year_from_text,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use ts_rs::TS;
use uuid::Uuid;

/// Records are added to the file this many at a time, each chunk in its own transaction.
pub const IMPORT_CHUNK: usize = 2000;
/// The most rows an import reads in all (a guard against a wrong file).
const ROWS_MAX: u64 = 3_000_000;
/// The most people one record keeps (a census household can be large).
const PEOPLE_MAX: usize = crate::place_events::PLACE_RECORD_PEOPLE_MAX;

const COLUMNS: [&str; 16] = [
    "book",
    "year",
    "section",
    "rec_no",
    "rec_id",
    "date",
    "page",
    "role",
    "given",
    "patronymic",
    "surname",
    "as_written",
    "estate",
    "age",
    "note",
    "unsure",
];

/// Which book and which village of it a record belongs to: what the owner maps to a place.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaceImportKey {
    pub book: String,
    /// Empty for a book of one village.
    pub village: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaceImportGroupDto {
    pub book: String,
    pub village: String,
    pub records: u32,
    pub rows: u32,
    pub year_from: Option<i32>,
    pub year_to: Option<i32>,
    pub kinds: Vec<KindCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaceImportScanDto {
    pub files: u32,
    pub rows: u32,
    /// Rows left out for having too few columns.
    pub bad_rows: u32,
    pub groups: Vec<PlaceImportGroupDto>,
}

/// Where a book and village go. `place` absent: leave them out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaceImportMapDto {
    pub book: String,
    pub village: String,
    pub place: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaceImportLineDto {
    pub book: String,
    pub village: String,
    pub place: Option<Uuid>,
    pub records: u32,
    /// Records not stored yet (what the import would add).
    pub new: u32,
    /// Records already stored with the same book and record id.
    pub duplicates: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaceImportPreviewDto {
    pub lines: Vec<PlaceImportLineDto>,
    pub total_new: u32,
    pub total_duplicates: u32,
    /// Records of books and villages with no place chosen.
    pub unmapped: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaceImportResultDto {
    pub added: u32,
    pub duplicates: u32,
    pub unmapped: u32,
}

/// Progress of an import, emitted by the app while it adds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaceImportProgressDto {
    pub done: u32,
    pub total: u32,
}

/// Everything read from the files.
#[derive(Debug, Default)]
pub struct ParsedIndex {
    pub files: u32,
    pub rows: u32,
    pub bad_rows: u32,
    pub records: Vec<(PlaceImportKey, PlaceRecordInput)>,
}

/// The village in `<code>:<village>:<year>:<section>:<no>`, or none.
pub fn village_of(rec_id: &str) -> String {
    let parts: Vec<&str> = rec_id.split(':').collect();
    if parts.len() >= 5 {
        parts
            .get(1)
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    } else {
        String::new()
    }
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "так" | "так." | "+" | "?"
    )
}

/// Reads index files. Rows with the same `rec_id` of one book become one record; a row with no
/// `rec_id` takes one from its book, year, section and number.
pub fn parse_index_files(paths: &[PathBuf]) -> Result<ParsedIndex, ApiError> {
    let mut out = ParsedIndex::default();
    let mut at: HashMap<(String, String), usize> = HashMap::new();
    for path in paths {
        let file =
            std::fs::File::open(path).map_err(|e| crate::transfer::io_err("read", path, &e))?;
        out.files += 1;
        let mut cols: [usize; 16] = std::array::from_fn(|i| i);
        let mut width = COLUMNS.len();
        let mut first = true;
        for line in BufReader::new(file).split(b'\n') {
            let bytes = line.map_err(|e| crate::transfer::io_err("read", path, &e))?;
            let text = String::from_utf8_lossy(&bytes);
            let line = text.trim_end_matches(['\r', '\n']);
            let line = if first {
                line.trim_start_matches('\u{feff}')
            } else {
                line
            };
            if line.trim().is_empty() {
                first = false;
                continue;
            }
            let cells: Vec<&str> = line.split('\t').collect();
            if first {
                first = false;
                let names: Vec<String> = cells.iter().map(|c| c.trim().to_lowercase()).collect();
                if names.iter().any(|n| n == "rec_id") && names.iter().any(|n| n == "book") {
                    for (i, c) in COLUMNS.iter().enumerate() {
                        if let Some(p) = names.iter().position(|n| n == c) {
                            cols[i] = p;
                        } else {
                            cols[i] = usize::MAX;
                        }
                    }
                    width = names.len();
                    continue;
                }
            }
            if u64::from(out.rows) >= ROWS_MAX {
                return Err(ApiError::new(
                    ErrorCode::Invalid,
                    "The files have too many rows to import at once.",
                ));
            }
            out.rows += 1;
            if cells.len() < 5.min(width) {
                out.bad_rows += 1;
                continue;
            }
            let cell = |i: usize| -> &str {
                match cols.get(i).copied() {
                    Some(c) if c != usize::MAX => cells.get(c).copied().unwrap_or("").trim(),
                    _ => "",
                }
            };
            let book = cell(0).to_string();
            let year_col = cell(1);
            let section = cell(2);
            let rec_no = cell(3);
            let mut rec_id = cell(4).to_string();
            if rec_id.is_empty() && !rec_no.is_empty() {
                rec_id = format!("{book}:{year_col}:{section}:{rec_no}");
            }
            let village = village_of(&rec_id);
            let person = PlaceRecordPerson {
                role: cell(7).into(),
                given: cell(8).into(),
                patronymic: cell(9).into(),
                surname: cell(10).into(),
                as_written: cell(11).into(),
                estate: cell(12).into(),
                age: cell(13).into(),
                note: cell(14).into(),
                ..Default::default()
            };
            let key = (book.clone(), rec_id.clone());
            let slot = if rec_id.is_empty() {
                None
            } else {
                at.get(&key).copied()
            };
            match slot {
                Some(i) => {
                    if let Some((_, rec)) = out.records.get_mut(i) {
                        if rec.date.is_empty() {
                            rec.date = cell(5).into();
                        }
                        if rec.page.is_empty() {
                            rec.page = cell(6).into();
                        }
                        rec.unsure |= truthy(cell(15));
                        if rec.people.len() < PEOPLE_MAX {
                            rec.people.push(person);
                        }
                    }
                }
                None => {
                    let date = cell(5).to_string();
                    let year = year_col
                        .trim()
                        .parse::<i32>()
                        .ok()
                        .filter(|y| (1000..=2100).contains(y))
                        .or_else(|| year_from_text(&date));
                    let rec = PlaceRecordInput {
                        kind: kind_from_section(section).into(),
                        date,
                        year,
                        book: book.clone(),
                        page: cell(6).into(),
                        rec_id: rec_id.clone(),
                        note: String::new(),
                        unsure: truthy(cell(15)),
                        people: vec![person],
                    };
                    if !rec_id.is_empty() {
                        at.insert(key, out.records.len());
                    }
                    out.records.push((PlaceImportKey { book, village }, rec));
                }
            }
        }
    }
    Ok(out)
}

impl ParsedIndex {
    /// The books and villages found, with how many records each holds.
    pub fn scan(&self) -> PlaceImportScanDto {
        struct Acc {
            records: u32,
            rows: u32,
            from: Option<i32>,
            to: Option<i32>,
            kinds: BTreeMap<String, u32>,
        }
        let mut groups: BTreeMap<&PlaceImportKey, Acc> = BTreeMap::new();
        for (k, r) in &self.records {
            let g = groups.entry(k).or_insert(Acc {
                records: 0,
                rows: 0,
                from: None,
                to: None,
                kinds: BTreeMap::new(),
            });
            g.records += 1;
            g.rows += r.people.len() as u32;
            if let Some(y) = r.year {
                g.from = Some(g.from.map_or(y, |f| f.min(y)));
                g.to = Some(g.to.map_or(y, |t| t.max(y)));
            }
            *g.kinds.entry(normalize_kind(&r.kind).into()).or_default() += 1;
        }
        PlaceImportScanDto {
            files: self.files,
            rows: self.rows,
            bad_rows: self.bad_rows,
            groups: groups
                .into_iter()
                .map(|(k, g)| PlaceImportGroupDto {
                    book: k.book.clone(),
                    village: k.village.clone(),
                    records: g.records,
                    rows: g.rows,
                    year_from: g.from,
                    year_to: g.to,
                    kinds: g
                        .kinds
                        .into_iter()
                        .map(|(kind, count)| KindCount { kind, count })
                        .collect(),
                })
                .collect(),
        }
    }

    /// The records of each mapped book and village, with their place; records of an unmapped one
    /// are counted in the second value.
    pub fn split(
        &self,
        mapping: &[PlaceImportMapDto],
    ) -> (Vec<(PlaceImportKey, Uuid, Vec<PlaceRecordInput>)>, u32) {
        let target: HashMap<PlaceImportKey, Uuid> = mapping
            .iter()
            .filter_map(|m| {
                Some((
                    PlaceImportKey {
                        book: m.book.clone(),
                        village: m.village.clone(),
                    },
                    m.place?,
                ))
            })
            .collect();
        let mut by_key: BTreeMap<PlaceImportKey, Vec<PlaceRecordInput>> = BTreeMap::new();
        let mut unmapped = 0u32;
        for (k, r) in &self.records {
            if target.contains_key(k) {
                by_key.entry(k.clone()).or_default().push(r.clone());
            } else {
                unmapped += 1;
            }
        }
        let parts = by_key
            .into_iter()
            .filter_map(|(k, recs)| {
                let place = *target.get(&k)?;
                Some((k, place, recs))
            })
            .collect();
        (parts, unmapped)
    }
}

impl crate::session::Session {
    /// What importing `parsed` under `mapping` would do: per book and village how many records are
    /// new and how many are stored already.
    pub fn place_import_preview(
        &self,
        parsed: &ParsedIndex,
        mapping: &[PlaceImportMapDto],
    ) -> Result<PlaceImportPreviewDto, ApiError> {
        let (parts, unmapped) = parsed.split(mapping);
        let mut out = PlaceImportPreviewDto {
            lines: Vec::new(),
            total_new: 0,
            total_duplicates: 0,
            unmapped,
        };
        for (key, place, recs) in parts {
            let ids: Vec<&str> = recs.iter().map(|r| r.rec_id.as_str()).collect();
            let dup = self.count_place_records_present(&key.book, &ids)?;
            let records = recs.len() as u32;
            let dup = dup.min(records);
            out.total_new += records - dup;
            out.total_duplicates += dup;
            out.lines.push(PlaceImportLineDto {
                book: key.book,
                village: key.village,
                place: Some(place),
                records,
                new: records - dup,
                duplicates: dup,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn village_from_the_record_id() {
        assert_eq!(village_of("DAK:Zarichne:1850:birth:12"), "Zarichne");
        assert_eq!(village_of("TB-1:1850:birth:12"), "");
        assert_eq!(village_of(""), "");
    }
}
