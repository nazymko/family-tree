//! The per-country index: built from a GeoNames zip, saved as one compact file, loaded with its search tables.
//!
//! File layout (all numbers LEB128 varints unless noted): magic `GTGZ`, version byte, country (2 bytes),
//! row count, then per row: id, lat and lon as microdegrees (zigzag), population, class byte, then the strings
//! code, admin1, admin2, name, local (a Ukrainian or Cyrillic name), keys (the folded names joined by `|`).
//! The folded keys are stored, not recomputed, so loading is a parse and a table build.

use crate::GazetteerError;
use crate::fold::{fold_name, has_cyrillic, is_ukrainian};
use crate::parse::{GeoRow, parse_line};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Seek};
use std::path::Path;

const MAGIC: &[u8; 4] = b"GTGZ";
const VERSION: u8 = 1;
/// A row with more names than this keeps only the first ones (the index stays small).
const MAX_ALTERNATES: usize = 40;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: u32,
    pub lat: f64,
    pub lon: f64,
    pub population: u32,
    pub class: u8,
    pub code: String,
    pub admin1: String,
    pub admin2: String,
    pub name: String,
    /// A Ukrainian (else any Cyrillic) name for display; empty when there is none.
    pub local: String,
    /// The folded names (name, ascii name, alternates), distinct, joined by `|`.
    pub keys: String,
}

impl Entry {
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.keys.split('|').filter(|k| !k.is_empty())
    }

    /// The name to show: the local one when there is one.
    pub fn display(&self) -> &str {
        if self.local.is_empty() {
            &self.name
        } else {
            &self.local
        }
    }
}

#[derive(Debug, Default)]
pub struct Index {
    pub country: String,
    pub(crate) entries: Vec<Entry>,
    /// token -> id into `vocab`
    pub(crate) lookup: HashMap<Box<str>, u32>,
    /// the distinct tokens with the rows that have them
    pub(crate) vocab: Vec<(Box<str>, Vec<u32>)>,
    /// (first letter, length in letters) -> token ids, for one-letter-off matching
    pub(crate) shape: HashMap<(char, usize), Vec<u32>>,
    pub(crate) admin1_rows: HashMap<String, u32>,
    pub(crate) admin2_rows: HashMap<String, u32>,
}

fn pick_local(row: &GeoRow) -> String {
    let cyr = || row.alternates.iter().filter(|a| has_cyrillic(a));
    cyr()
        .find(|a| is_ukrainian(a))
        .or_else(|| cyr().find(|a| !a.contains(['ы', 'э', 'ъ', 'ё', 'Ы', 'Э', 'Ъ', 'Ё'])))
        .cloned()
        .unwrap_or_default()
}

fn entry_of(row: GeoRow) -> Entry {
    let mut keys: Vec<String> = Vec::new();
    let names = std::iter::once(&row.name)
        .chain(std::iter::once(&row.ascii))
        .chain(row.alternates.iter().take(MAX_ALTERNATES));
    for n in names {
        let k = fold_name(n);
        if !k.is_empty() && !keys.contains(&k) {
            keys.push(k);
        }
    }
    Entry {
        id: row.id,
        lat: row.lat,
        lon: row.lon,
        population: row.population,
        class: row.class,
        local: pick_local(&row),
        keys: keys.join("|"),
        code: row.code,
        admin1: row.admin1,
        admin2: row.admin2,
        name: row.name,
    }
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

fn zigzag(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

fn unzigzag(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    put_varint(out, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn byte(&mut self) -> Result<u8, GazetteerError> {
        let v = *self.b.get(self.at).ok_or(GazetteerError::BadIndex)?;
        self.at += 1;
        Ok(v)
    }
    fn varint(&mut self) -> Result<u64, GazetteerError> {
        let mut v = 0u64;
        for shift in (0..70).step_by(7) {
            let b = self.byte()?;
            v |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(GazetteerError::BadIndex)
    }
    fn str(&mut self) -> Result<&'a str, GazetteerError> {
        let n = usize::try_from(self.varint()?).map_err(|_| GazetteerError::BadIndex)?;
        let end = self.at.checked_add(n).ok_or(GazetteerError::BadIndex)?;
        let s = self.b.get(self.at..end).ok_or(GazetteerError::BadIndex)?;
        self.at = end;
        std::str::from_utf8(s).map_err(|_| GazetteerError::BadIndex)
    }
}

impl Index {
    /// Builds the index of a country from parsed rows.
    pub fn build(country: &str, rows: Vec<GeoRow>) -> Result<Index, GazetteerError> {
        let country = country.to_ascii_uppercase();
        if country.len() != 2 || !country.bytes().all(|b| b.is_ascii_alphabetic()) {
            return Err(GazetteerError::BadCountry);
        }
        if rows.is_empty() {
            return Err(GazetteerError::Empty);
        }
        let entries = rows.into_iter().map(entry_of).collect();
        Ok(Index::from_entries(country, entries))
    }

    fn from_entries(country: String, entries: Vec<Entry>) -> Index {
        let mut ix = Index {
            country,
            entries,
            ..Default::default()
        };
        for (i, e) in ix.entries.iter().enumerate() {
            let i = i as u32;
            match e.code.as_str() {
                "ADM1" => {
                    ix.admin1_rows.insert(e.admin1.clone(), i);
                }
                "ADM2" => {
                    ix.admin2_rows
                        .insert(format!("{}.{}", e.admin1, e.admin2), i);
                }
                _ => {}
            }
            for key in e.keys() {
                for tok in key.split(' ') {
                    let id = match ix.lookup.get(tok) {
                        Some(id) => *id,
                        None => {
                            let id = ix.vocab.len() as u32;
                            ix.vocab.push((tok.into(), Vec::new()));
                            ix.lookup.insert(tok.into(), id);
                            if let Some(c) = tok.chars().next() {
                                ix.shape
                                    .entry((c, tok.chars().count()))
                                    .or_default()
                                    .push(id);
                            }
                            id
                        }
                    };
                    if let Some((_, rows)) = ix.vocab.get_mut(id as usize)
                        && rows.last() != Some(&i)
                    {
                        rows.push(i);
                    }
                }
            }
        }
        ix
    }

    /// Reads a GeoNames `<CC>.zip` (the entry `<CC>.txt` inside) and builds the index.
    pub fn from_zip<R: Read + Seek>(country: &str, zip: R) -> Result<Index, GazetteerError> {
        let cc = country.to_ascii_uppercase();
        let mut archive = zip::ZipArchive::new(zip)?;
        let file = archive.by_name(&format!("{cc}.txt"))?;
        Index::from_reader(&cc, BufReader::new(file))
    }

    /// Builds the index from the text of the dump.
    pub fn from_reader<R: BufRead>(country: &str, reader: R) -> Result<Index, GazetteerError> {
        let mut rows = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if let Some(r) = parse_line(&line) {
                rows.push(r);
            }
        }
        Index::build(country, rows)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.entries.len() * 80);
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.extend_from_slice(self.country.as_bytes());
        put_varint(&mut out, self.entries.len() as u64);
        for e in &self.entries {
            put_varint(&mut out, u64::from(e.id));
            put_varint(&mut out, zigzag((e.lat * 1e6).round() as i64));
            put_varint(&mut out, zigzag((e.lon * 1e6).round() as i64));
            put_varint(&mut out, u64::from(e.population));
            out.push(e.class);
            for s in [&e.code, &e.admin1, &e.admin2, &e.name, &e.local, &e.keys] {
                put_str(&mut out, s);
            }
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Index, GazetteerError> {
        if bytes.get(..4) != Some(&MAGIC[..]) || bytes.get(4) != Some(&VERSION) {
            return Err(GazetteerError::BadIndex);
        }
        let country = std::str::from_utf8(bytes.get(5..7).ok_or(GazetteerError::BadIndex)?)
            .map_err(|_| GazetteerError::BadIndex)?
            .to_string();
        let mut r = Reader { b: bytes, at: 7 };
        let n = usize::try_from(r.varint()?).map_err(|_| GazetteerError::BadIndex)?;
        let mut entries = Vec::with_capacity(n.min(2_000_000));
        for _ in 0..n {
            let id = u32::try_from(r.varint()?).map_err(|_| GazetteerError::BadIndex)?;
            let lat = unzigzag(r.varint()?) as f64 / 1e6;
            let lon = unzigzag(r.varint()?) as f64 / 1e6;
            let population = u32::try_from(r.varint()?).map_err(|_| GazetteerError::BadIndex)?;
            let class = r.byte()?;
            let mut s = [""; 6];
            for slot in &mut s {
                *slot = r.str()?;
            }
            let [code, admin1, admin2, name, local, keys] = s;
            entries.push(Entry {
                id,
                lat,
                lon,
                population,
                class,
                code: code.into(),
                admin1: admin1.into(),
                admin2: admin2.into(),
                name: name.into(),
                local: local.into(),
                keys: keys.into(),
            });
        }
        Ok(Index::from_entries(country, entries))
    }

    pub fn save(&self, path: &Path) -> Result<(), GazetteerError> {
        let tmp = path.with_extension("gtgz.part");
        std::fs::write(&tmp, self.to_bytes())?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Index, GazetteerError> {
        Index::from_bytes(&std::fs::read(path)?)
    }
}

/// What is known about one downloaded country.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountryInfo {
    pub country: String,
    pub rows: usize,
    pub bytes: u64,
}

/// Every country index found in the app's gazetteer folder.
#[derive(Debug, Default)]
pub struct Gazetteer {
    pub indexes: Vec<Index>,
}

impl Gazetteer {
    /// Loads every `<CC>.gtgz` of the folder; a damaged file is skipped (and listed in the second value).
    pub fn load_dir(dir: &Path) -> (Gazetteer, Vec<String>) {
        let mut g = Gazetteer::default();
        let mut skipped = Vec::new();
        let Ok(rd) = std::fs::read_dir(dir) else {
            return (g, skipped);
        };
        let mut paths: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            if p.extension().and_then(|e| e.to_str()) != Some("gtgz") {
                continue;
            }
            match Index::load(&p) {
                Ok(ix) => g.indexes.push(ix),
                Err(_) => skipped.push(p.display().to_string()),
            }
        }
        (g, skipped)
    }

    pub fn countries(&self) -> Vec<CountryInfo> {
        self.indexes
            .iter()
            .map(|i| CountryInfo {
                country: i.country.clone(),
                rows: i.len(),
                bytes: i.to_bytes().len() as u64,
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.indexes.is_empty()
    }
}

/// The number of rows an index file holds, from its first bytes alone.
pub fn peek_rows(path: &Path) -> Option<usize> {
    let mut head = [0u8; 24];
    let n = std::fs::File::open(path).ok()?.read(&mut head).ok()?;
    if head.get(..4) != Some(&MAGIC[..]) || head.get(4) != Some(&VERSION) {
        return None;
    }
    let mut r = Reader {
        b: head.get(..n)?,
        at: 7,
    };
    usize::try_from(r.varint().ok()?).ok()
}

/// The path of a country's index inside the gazetteer folder.
pub fn index_path(dir: &Path, country: &str) -> std::path::PathBuf {
    dir.join(format!("{}.gtgz", country.to_ascii_uppercase()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_line;

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn row(
        id: u32,
        name: &str,
        alts: &str,
        class: &str,
        code: &str,
        a1: &str,
        a2: &str,
        pop: u32,
    ) -> String {
        format!(
            "{id}\t{name}\t{name}\t{alts}\t49.5\t30.5\t{class}\t{code}\tXX\t\t{a1}\t{a2}\t\t\t{pop}\t\t0\tEurope/Kyiv\t2020-01-01"
        )
    }

    #[test]
    fn round_trip_keeps_everything() {
        let text = [
            row(
                1,
                "Ivanivka",
                "Іванівка,Ivanovka",
                "P",
                "PPL",
                "10",
                "1001",
                300,
            ),
            row(
                2,
                "Zelena Oblast",
                "Зелена область",
                "A",
                "ADM1",
                "10",
                "",
                0,
            ),
        ]
        .join("\n");
        let ix = Index::from_reader("xx", text.as_bytes()).expect("index");
        assert_eq!(ix.country, "XX");
        assert_eq!(ix.len(), 2);
        let back = Index::from_bytes(&ix.to_bytes()).expect("bytes");
        assert_eq!(back.entries(), ix.entries());
        assert_eq!(back.entries()[0].local, "Іванівка");
        assert!((back.entries()[0].lat - 49.5).abs() < 1e-6);
    }

    #[test]
    fn damaged_bytes_are_refused() {
        assert!(Index::from_bytes(b"nope").is_err());
        let ix = Index::build(
            "XX",
            vec![parse_line(&row(1, "A", "", "P", "PPL", "1", "1", 1)).expect("row")],
        )
        .expect("ix");
        let mut b = ix.to_bytes();
        b.truncate(b.len() - 3);
        assert!(Index::from_bytes(&b).is_err());
        assert!(Index::build("XXX", vec![]).is_err());
        assert!(Index::build("XX", vec![]).is_err());
    }

    #[test]
    fn zip_and_folder_round_trip() {
        use std::io::Write;
        let text = row(1, "Ivanivka", "", "P", "PPL", "10", "1001", 5);
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            z.start_file("XX.txt", zip::write::SimpleFileOptions::default())
                .expect("start");
            z.write_all(text.as_bytes()).expect("write");
            z.finish().expect("finish");
        }
        buf.set_position(0);
        let ix = Index::from_zip("xx", buf).expect("from zip");
        let dir = tempfile::tempdir().expect("dir");
        ix.save(&index_path(dir.path(), "xx")).expect("save");
        std::fs::write(dir.path().join("YY.gtgz"), b"junk").expect("junk");
        let (g, skipped) = Gazetteer::load_dir(dir.path());
        assert_eq!(g.indexes.len(), 1);
        assert_eq!(skipped.len(), 1);
        assert_eq!(g.countries()[0].rows, 1);
    }
}
