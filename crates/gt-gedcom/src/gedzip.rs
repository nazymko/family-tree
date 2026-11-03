//! GEDZIP (GEDCOM 7 zip container) reading. The archive is untrusted: every entry name is
//! validated, sizes are enforced on the bytes actually streamed (never on header claims), and
//! files are only written beneath the caller's `extract_dir`.

use crate::import::{ImportOptions, ImportOutcome, import, is_lexically_under};
use std::cell::RefCell;
use std::collections::HashSet;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use zip::ZipArchive;

/// Name of the GEDCOM file at the zip root.
const GEDCOM_NAME: &str = "gedcom.ged";

/// Resource limits for one archive.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    pub max_entries: usize,
    pub max_entry_bytes: u64,
    pub max_total_bytes: u64,
    pub max_gedcom_bytes: u64,
    /// Maximum uncompressed/compressed ratio (applies once output exceeds `ratio_floor`).
    pub max_ratio: u64,
    pub ratio_floor: u64,
}

pub(crate) const LIMITS: Limits = Limits {
    max_entries: 50_000,
    max_entry_bytes: 2 << 30,
    max_total_bytes: 4 << 30,
    max_gedcom_bytes: 512 << 20,
    max_ratio: 200,
    ratio_floor: 1 << 20,
};

#[derive(Debug, thiserror::Error)]
pub enum GedzipError {
    #[error("not a valid zip archive: {0}")]
    Zip(String),
    #[error("the archive has no gedcom.ged at its root")]
    MissingGedcom,
    #[error("unsafe path in the archive: {0:?}")]
    UnsafePath(String),
    #[error("the archive contains a symbolic link: {0:?}")]
    Symlink(String),
    #[error("the archive contains an encrypted entry: {0:?}")]
    Encrypted(String),
    #[error("duplicate entry after normalisation: {0:?}")]
    Duplicate(String),
    #[error("the archive has more than {0} entries")]
    TooManyEntries(usize),
    #[error("entry {0:?} is larger than the allowed size")]
    EntryTooLarge(String),
    #[error("the archive expands to more than the allowed total size")]
    TotalTooLarge,
    #[error("compression ratio too high (possible zip bomb) at {0:?}")]
    RatioTooHigh(String),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<zip::result::ZipError> for GedzipError {
    fn from(e: zip::result::ZipError) -> Self {
        GedzipError::Zip(e.to_string())
    }
}

#[derive(Debug)]
pub struct GedzipContents {
    /// Bytes of the root `gedcom.ged`.
    pub gedcom: Vec<u8>,
    /// `(zip path, bytes)` for every other file.
    pub media: Vec<(String, Vec<u8>)>,
}

/// Validate a zip-relative path.
fn check_path(name: &str) -> Result<(), GedzipError> {
    let bad = || GedzipError::UnsafePath(name.to_string());
    if name.is_empty() || name.starts_with('/') || name.contains('\\') {
        return Err(bad());
    }
    let trimmed = name.strip_suffix('/').unwrap_or(name);
    for c in trimmed.split('/') {
        if c.is_empty()
            || c == "."
            || c == ".."
            || c.contains(':')
            || c.chars().any(char::is_control)
            || c.ends_with('.')
            || c.ends_with(' ')
        {
            return Err(bad());
        }
    }
    Ok(())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while let Some(&c) = b.get(i) {
        let hex = |k: usize| b.get(k).and_then(|&h| (h as char).to_digit(16));
        if c == b'%'
            && let (Some(h), Some(l)) = (hex(i + 1), hex(i + 2))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(c);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Every spelling under which a GEDCOM `FILE` may name this entry (raw name, percent-decoded,
/// percent-encoded), each validated. The first is the raw name.
fn spellings(raw: &str) -> Result<Vec<String>, GedzipError> {
    check_path(raw)?;
    let mut v = vec![raw.to_string()];
    for alt in [percent_decode(raw), percent_encode(raw)] {
        if !v.contains(&alt) {
            check_path(&alt)?;
            v.push(alt);
        }
    }
    Ok(v)
}

/// A destination for one media file.
trait Sink {
    fn writer(&mut self) -> &mut dyn Write;
    fn finish(self: Box<Self>) -> Result<(), GedzipError>;
}

type MediaVec = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

struct MemSink {
    buf: Vec<u8>,
    name: String,
    out: MediaVec,
}

impl Sink for MemSink {
    fn writer(&mut self) -> &mut dyn Write {
        &mut self.buf
    }
    fn finish(self: Box<Self>) -> Result<(), GedzipError> {
        let s = *self;
        s.out.borrow_mut().push((s.name, s.buf));
        Ok(())
    }
}

struct FileSink {
    file: std::fs::File,
    first: PathBuf,
    aliases: Vec<PathBuf>,
}

impl Sink for FileSink {
    fn writer(&mut self) -> &mut dyn Write {
        &mut self.file
    }
    fn finish(self: Box<Self>) -> Result<(), GedzipError> {
        let s = *self;
        drop(s.file);
        for a in &s.aliases {
            if std::fs::hard_link(&s.first, a).is_err() {
                std::fs::copy(&s.first, a)?;
            }
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
/// Copy one entry into `sink`, enforcing the caps on the bytes really produced.
fn stream_entry(
    file: &mut impl Read,
    claimed_compressed: u64,
    name: &str,
    total: &mut u64,
    archive_len: u64,
    lim: &Limits,
    sink: &mut dyn Write,
    entry_cap: u64,
) -> Result<(), GedzipError> {
    let claimed = claimed_compressed.max(1);
    let mut buf = [0u8; 64 * 1024];
    let mut n_entry = 0u64;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        n_entry += n as u64;
        *total += n as u64;
        if n_entry > entry_cap {
            return Err(GedzipError::EntryTooLarge(name.to_string()));
        }
        if *total > lim.max_total_bytes {
            return Err(GedzipError::TotalTooLarge);
        }
        // Per-entry ratio against the (possibly lying) header, and overall against the real
        // archive size, which cannot lie.
        if (n_entry > lim.ratio_floor && n_entry / claimed > lim.max_ratio)
            || (*total > lim.ratio_floor && *total / archive_len.max(1) > lim.max_ratio)
        {
            return Err(GedzipError::RatioTooHigh(name.to_string()));
        }
        sink.write_all(buf.get(..n).unwrap_or_default())?;
    }
}

/// Walk the archive: `open(spellings)` returns the sink for each non-gedcom file; the root
/// `gedcom.ged` is returned.
fn walk(
    bytes: &[u8],
    lim: &Limits,
    mut open: impl FnMut(&[String]) -> Result<Box<dyn Sink>, GedzipError>,
) -> Result<Vec<u8>, GedzipError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    if archive.len() > lim.max_entries {
        return Err(GedzipError::TooManyEntries(lim.max_entries));
    }
    let archive_len = bytes.len() as u64;
    let mut seen: HashSet<String> = HashSet::new();
    let mut gedcom: Option<Vec<u8>> = None;
    let mut total = 0u64;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let raw = file.name()?.into_owned();
        let sp = spellings(&raw)?;
        if file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return Err(GedzipError::Symlink(raw));
        }
        if file.encrypted() {
            return Err(GedzipError::Encrypted(raw));
        }
        if file.is_dir() {
            continue;
        }
        for s in &sp {
            if !seen.insert(s.to_lowercase()) {
                return Err(GedzipError::Duplicate(raw));
            }
        }
        let claimed = file.compressed_size();
        if raw == GEDCOM_NAME {
            let mut v = Vec::new();
            let cap = lim.max_gedcom_bytes;
            stream_entry(
                &mut file,
                claimed,
                &raw,
                &mut total,
                archive_len,
                lim,
                &mut v,
                cap,
            )?;
            gedcom = Some(v);
        } else {
            let mut sink = open(&sp)?;
            let cap = lim.max_entry_bytes;
            stream_entry(
                &mut file,
                claimed,
                &raw,
                &mut total,
                archive_len,
                lim,
                sink.writer(),
                cap,
            )?;
            sink.finish()?;
        }
    }
    gedcom.ok_or(GedzipError::MissingGedcom)
}

fn read_with(bytes: &[u8], lim: &Limits) -> Result<GedzipContents, GedzipError> {
    let out: MediaVec = Rc::new(RefCell::new(Vec::new()));
    let gedcom = walk(bytes, lim, |sp| {
        Ok(Box::new(MemSink {
            buf: Vec::new(),
            name: sp.first().cloned().unwrap_or_default(),
            out: out.clone(),
        }))
    })?;
    let media = out.take();
    Ok(GedzipContents { gedcom, media })
}

/// Read a GEDZIP fully into memory (every limit still applies).
pub fn read_gedzip(bytes: &[u8]) -> Result<GedzipContents, GedzipError> {
    read_with(bytes, &LIMITS)
}

fn extract_with(
    bytes: &[u8],
    extract_dir: &Path,
    opts: &ImportOptions,
    lim: &Limits,
) -> Result<ImportOutcome, GedzipError> {
    std::fs::create_dir_all(extract_dir)?;
    let roots = [extract_dir.to_path_buf()];
    let gedcom = walk(bytes, lim, |sp| {
        let mut paths = Vec::new();
        for s in sp {
            let p = extract_dir.join(s);
            if !is_lexically_under(&p, &roots) {
                return Err(GedzipError::UnsafePath(s.clone()));
            }
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            paths.push(p);
        }
        let first = paths.remove(0);
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&first)?;
        Ok(Box::new(FileSink {
            file,
            first,
            aliases: paths,
        }))
    })?;
    let mut o = opts.clone();
    o.gedcom_dir = Some(extract_dir.to_path_buf());
    Ok(import(&gedcom, &o))
}

/// Extract the media under `extract_dir` and run the normal GEDCOM import over `gedcom.ged`,
/// with `gedcom_dir = extract_dir` so `FILE` references resolve. `extract_dir` should be a
/// fresh folder: an existing file with an entry's name is an error, never overwritten. Each
/// file is also reachable under its percent-decoded/encoded spelling (hard link or copy),
/// because the GEDCOM importer matches `FILE` text literally.
pub fn import_gedzip(
    bytes: &[u8],
    extract_dir: &Path,
    opts: &ImportOptions,
) -> Result<ImportOutcome, GedzipError> {
    extract_with(bytes, extract_dir, opts, &LIMITS)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;

    const GED: &str = "0 HEAD\n1 GEDC\n2 VERS 7.0\n0 @I1@ INDI\n1 NAME A /B/\n1 OBJE @M1@\n1 OBJE @M2@\n0 @M1@ OBJE\n1 FILE media/a.jpg\n2 FORM image/jpeg\n0 @M2@ OBJE\n1 FILE pics/my%20photo.png\n2 FORM image/png\n0 TRLR\n";

    fn make(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (n, d) in entries {
            w.start_file(*n, o).unwrap();
            w.write_all(d).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    fn opts() -> ImportOptions {
        ImportOptions::default()
    }

    #[test]
    fn happy_path_resolves_media() {
        let z = make(&[
            ("gedcom.ged", GED.as_bytes()),
            ("media/a.jpg", b"jpegdata"),
            ("pics/my photo.png", b"pngdata"),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let out = import_gedzip(&z, dir.path(), &opts()).unwrap();
        assert_eq!(out.resolved_media.len(), 2, "{:?}", out.report);
        for (_, p) in &out.resolved_media {
            assert!(p.starts_with(dir.path()) && p.is_file());
        }
        assert_eq!(
            std::fs::read(dir.path().join("media/a.jpg")).unwrap(),
            b"jpegdata"
        );
        let c = read_gedzip(&z).unwrap();
        assert_eq!(c.gedcom, GED.as_bytes());
        assert_eq!(c.media.len(), 2);
    }

    #[test]
    fn missing_gedcom() {
        let z = make(&[("media/a.jpg", b"x")]);
        assert!(matches!(read_gedzip(&z), Err(GedzipError::MissingGedcom)));
    }

    #[test]
    fn rejects_zip_slip_and_absolute() {
        for name in [
            "../x",
            "a/../../x",
            "/abs/x",
            "C:/x",
            "c:x",
            "a\\b",
            "%2e%2e/x",
            "a%5Cb",
        ] {
            let z = make(&[("gedcom.ged", GED.as_bytes()), (name, b"x")]);
            let dir = tempfile::tempdir().unwrap();
            let sub = dir.path().join("out");
            let r = import_gedzip(&z, &sub, &opts());
            assert!(
                matches!(r, Err(GedzipError::UnsafePath(_))),
                "{name}: {r:?}"
            );
            assert!(!dir.path().join("x").exists());
        }
    }

    #[test]
    fn rejects_duplicates_and_too_many() {
        let z = make(&[
            ("gedcom.ged", GED.as_bytes()),
            ("a.jpg", b"1"),
            ("A.JPG", b"2"),
        ]);
        assert!(matches!(read_gedzip(&z), Err(GedzipError::Duplicate(_))));
        let z = make(&[
            ("gedcom.ged", GED.as_bytes()),
            ("a b", b"1"),
            ("a%20b", b"2"),
        ]);
        assert!(matches!(read_gedzip(&z), Err(GedzipError::Duplicate(_))));
        let lim = Limits {
            max_entries: 2,
            ..LIMITS
        };
        let z = make(&[("gedcom.ged", b"0 TRLR\n"), ("a", b"1"), ("b", b"2")]);
        assert!(matches!(
            read_with(&z, &lim),
            Err(GedzipError::TooManyEntries(2))
        ));
    }

    #[test]
    fn rejects_symlink() {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = SimpleFileOptions::default();
        w.start_file("gedcom.ged", o).unwrap();
        w.write_all(b"0 TRLR\n").unwrap();
        w.add_symlink("link", "/etc/passwd", o).unwrap();
        let z = w.finish().unwrap().into_inner();
        assert!(matches!(read_gedzip(&z), Err(GedzipError::Symlink(_))));
    }

    #[test]
    fn size_caps_apply_to_streamed_bytes_when_header_lies() {
        let big: Vec<u8> = (0..(2u32 << 20))
            .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
            .collect();
        let mut z = make(&[("gedcom.ged", b"0 TRLR\n"), ("big.bin", &big)]);
        // Patch the declared uncompressed size to 10 in the local and central headers.
        let mut patched = 0;
        for i in 0..z.len().saturating_sub(60) {
            let (off, name_off) = match z.get(i..i + 4) {
                Some(b"PK\x03\x04") => (22, 30),
                Some(b"PK\x01\x02") => (24, 46),
                _ => continue,
            };
            if z.get(i + name_off..i + name_off + 7) == Some(b"big.bin".as_slice()) {
                z[i + off..i + off + 4].copy_from_slice(&10u32.to_le_bytes());
                patched += 1;
            }
        }
        assert_eq!(patched, 2);
        let lim = Limits {
            max_entry_bytes: 1 << 20,
            ratio_floor: u64::MAX,
            ..LIMITS
        };
        assert!(read_with(&z, &lim).is_err());
        // A truthful header over the cap is rejected too.
        let z = make(&[("gedcom.ged", b"0 TRLR\n"), ("big.bin", &big)]);
        assert!(matches!(
            read_with(&z, &lim),
            Err(GedzipError::EntryTooLarge(_))
        ));
        let lim = Limits {
            max_total_bytes: 1 << 20,
            ratio_floor: u64::MAX,
            ..LIMITS
        };
        assert!(matches!(
            read_with(&z, &lim),
            Err(GedzipError::TotalTooLarge)
        ));
    }

    #[test]
    fn rejects_zip_bomb_by_ratio() {
        let zeros = vec![0u8; 16 << 20];
        let z = make(&[("gedcom.ged", b"0 TRLR\n"), ("bomb.bin", &zeros)]);
        assert!(matches!(read_gedzip(&z), Err(GedzipError::RatioTooHigh(_))));
        let dir = tempfile::tempdir().unwrap();
        assert!(import_gedzip(&z, dir.path(), &opts()).is_err());
    }
}
