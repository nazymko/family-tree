//! GEDZIP (GEDCOM 7 zip container) reading. The archive is untrusted: every entry name is
//! validated, sizes are enforced on the bytes actually streamed (never on header claims), and
//! files are only written beneath the caller's `extract_dir`.

use crate::import::{ImportOptions, ImportOutcome, import, is_lexically_under};
use std::collections::HashSet;
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};
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
    #[error("the archive has no {0} at its root")]
    MissingEntry(String),
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

/// Windows device names that must never become file names (checked on every platform so an
/// archive is rejected the same way everywhere).
fn is_reserved_device(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or("").trim_end();
    let up = stem.to_uppercase();
    if matches!(
        up.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    let mut chars = up.chars();
    let head: String = chars.by_ref().take(3).collect();
    matches!(head.as_str(), "COM" | "LPT")
        && matches!(
            chars.next(),
            Some('1'..='9' | '\u{b9}' | '\u{b2}' | '\u{b3}')
        )
        && chars.next().is_none()
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
            || is_reserved_device(c)
        {
            return Err(bad());
        }
    }
    Ok(())
}

/// Where one entry's bytes go.
enum Out {
    Mem(Vec<u8>),
    File(std::fs::File),
}

impl Write for Out {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        match self {
            Out::Mem(v) => v.write(b),
            Out::File(f) => f.write(b),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Out::Mem(v) => v.flush(),
            Out::File(f) => f.flush(),
        }
    }
}

/// Running totals, enforced on the bytes really produced.
struct Budget<'a> {
    lim: &'a Limits,
    total: u64,
    archive_len: u64,
}

impl Budget<'_> {
    fn copy(
        &mut self,
        src: &mut impl Read,
        claimed_compressed: u64,
        name: &str,
        cap: u64,
        sink: &mut dyn Write,
    ) -> Result<(), GedzipError> {
        let claimed = claimed_compressed.max(1);
        let mut buf = [0u8; 64 * 1024];
        let mut n_entry = 0u64;
        loop {
            let n = src.read(&mut buf)?;
            if n == 0 {
                return Ok(());
            }
            n_entry += n as u64;
            self.total += n as u64;
            if n_entry > cap {
                return Err(GedzipError::EntryTooLarge(name.to_string()));
            }
            if self.total > self.lim.max_total_bytes {
                return Err(GedzipError::TotalTooLarge);
            }
            // Per-entry ratio against the (possibly lying) header, and overall against the
            // real archive size, which cannot lie.
            if (n_entry > self.lim.ratio_floor && n_entry / claimed > self.lim.max_ratio)
                || (self.total > self.lim.ratio_floor
                    && self.total / self.archive_len.max(1) > self.lim.max_ratio)
            {
                return Err(GedzipError::RatioTooHigh(name.to_string()));
            }
            sink.write_all(buf.get(..n).unwrap_or_default())?;
        }
    }
}

fn entry_cap(name: &str, special: Option<&str>, lim: &Limits) -> u64 {
    if special == Some(name) {
        lim.max_gedcom_bytes
    } else {
        lim.max_entry_bytes
    }
}

/// Pass 1, no writes: every name, flag, the count, duplicates, and the header sizes.
fn scan<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    lim: &Limits,
    special: Option<&str>,
    required: &str,
) -> Result<(), GedzipError> {
    if archive.len() > lim.max_entries {
        return Err(GedzipError::TooManyEntries(lim.max_entries));
    }
    let mut seen: HashSet<String> = HashSet::new();
    let mut claimed_total = 0u64;
    let mut has_required = false;
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let raw = file.name()?.into_owned();
        check_path(&raw)?;
        if file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return Err(GedzipError::Symlink(raw));
        }
        if file.encrypted() {
            return Err(GedzipError::Encrypted(raw));
        }
        if file.is_dir() {
            continue;
        }
        if !seen.insert(raw.to_lowercase()) {
            return Err(GedzipError::Duplicate(raw));
        }
        if file.size() > entry_cap(&raw, special, lim) {
            return Err(GedzipError::EntryTooLarge(raw));
        }
        claimed_total = claimed_total.saturating_add(file.size());
        if claimed_total > lim.max_total_bytes {
            return Err(GedzipError::TotalTooLarge);
        }
        has_required |= raw == required;
    }
    if has_required {
        Ok(())
    } else if required == GEDCOM_NAME {
        Err(GedzipError::MissingGedcom)
    } else {
        Err(GedzipError::MissingEntry(required.to_string()))
    }
}

/// Scan, then stream every file. `make(name)` opens the destination; `done(name, out)` receives
/// it back. The root `gedcom.ged` is always returned, never passed to `make`.
fn run(
    bytes: &[u8],
    lim: &Limits,
    make: impl FnMut(&str) -> Result<Out, GedzipError>,
    done: impl FnMut(&str, Out),
) -> Result<Vec<u8>, GedzipError> {
    run_on(
        Cursor::new(bytes),
        bytes.len() as u64,
        lim,
        Some(GEDCOM_NAME),
        GEDCOM_NAME,
        make,
        done,
    )
}

/// The same over any seekable reader. `special` is the one entry returned in memory instead of
/// going through `make`; `required` must be present at the archive root.
fn run_on<R: Read + Seek>(
    reader: R,
    archive_len: u64,
    lim: &Limits,
    special: Option<&str>,
    required: &str,
    mut make: impl FnMut(&str) -> Result<Out, GedzipError>,
    mut done: impl FnMut(&str, Out),
) -> Result<Vec<u8>, GedzipError> {
    let mut archive = ZipArchive::new(reader)?;
    scan(&mut archive, lim, special, required)?;
    let mut budget = Budget {
        lim,
        total: 0,
        archive_len,
    };
    let mut gedcom = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        if file.is_dir() {
            continue;
        }
        let raw = file.name()?.into_owned();
        let claimed = file.compressed_size();
        let cap = entry_cap(&raw, special, lim);
        if special == Some(raw.as_str()) {
            budget.copy(&mut file, claimed, &raw, cap, &mut gedcom)?;
        } else {
            let mut out = make(&raw)?;
            budget.copy(&mut file, claimed, &raw, cap, &mut out)?;
            done(&raw, out);
        }
    }
    Ok(gedcom)
}

fn read_with(bytes: &[u8], lim: &Limits) -> Result<GedzipContents, GedzipError> {
    let mut media = Vec::new();
    let gedcom = run(
        bytes,
        lim,
        |_| Ok(Out::Mem(Vec::new())),
        |name, out| {
            if let Out::Mem(v) = out {
                media.push((name.to_string(), v));
            }
        },
    )?;
    Ok(GedzipContents { gedcom, media })
}

/// Read a GEDZIP fully into memory (every limit still applies, so this may buffer up to the
/// total cap; prefer [`import_gedzip`] for large archives).
pub fn read_gedzip(bytes: &[u8]) -> Result<GedzipContents, GedzipError> {
    read_with(bytes, &LIMITS)
}

/// Sibling staging folder for `extract_dir`.
fn staging_path(extract_dir: &Path) -> PathBuf {
    let mut name = extract_dir.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".partial-{}", std::process::id()));
    extract_dir.with_file_name(name)
}

fn extract_to(bytes: &[u8], stage: &Path, lim: &Limits) -> Result<Vec<u8>, GedzipError> {
    let roots = [stage.to_path_buf()];
    run(
        bytes,
        lim,
        |name| {
            let p = stage.join(name);
            if !is_lexically_under(&p, &roots) {
                return Err(GedzipError::UnsafePath(name.to_string()));
            }
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&p)?;
            Ok(Out::File(f))
        },
        |_, _| {},
    )
}

fn extract_with(
    bytes: &[u8],
    extract_dir: &Path,
    opts: &ImportOptions,
    lim: &Limits,
) -> Result<ImportOutcome, GedzipError> {
    if let Some(parent) = extract_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let stage = staging_path(extract_dir);
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir(&stage)?;
    let gedcom = match extract_to(bytes, &stage, lim) {
        Ok(g) => g,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&stage);
            return Err(e);
        }
    };
    // An existing, empty extract_dir is fine; anything else is refused.
    if extract_dir.exists()
        && let Err(e) = std::fs::remove_dir(extract_dir)
    {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(e.into());
    }
    if let Err(e) = std::fs::rename(&stage, extract_dir) {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(e.into());
    }
    let mut o = opts.clone();
    o.gedcom_dir = Some(extract_dir.to_path_buf());
    Ok(import(&gedcom, &o))
}

/// Extract the media under `extract_dir` and run the normal GEDCOM import over `gedcom.ged`,
/// with `gedcom_dir = extract_dir` so `FILE` references resolve (GEDCOM 7 `FILE` values are
/// percent-decoded by the importer). Every entry is validated before anything is written; files
/// go to a sibling `<extract_dir>.partial-<pid>` folder that is renamed into place only on
/// success and removed on any error. `extract_dir` must not exist or must be an empty folder.
pub fn import_gedzip(
    bytes: &[u8],
    extract_dir: &Path,
    opts: &ImportOptions,
) -> Result<ImportOutcome, GedzipError> {
    extract_with(bytes, extract_dir, opts, &LIMITS)
}

/// Unpacks a zip file into `dest` (an existing, empty folder) with the same limits and checks as
/// a GEDZIP: names validated, no symlinks, no encrypted entries, entry count, per-entry and total
/// sizes and the compression ratio enforced on the bytes really written, every file created
/// beneath `dest`. `required` (such as `tree.db`) must be at the root. On any error `dest` is
/// left for the caller to delete.
pub fn extract_zip_file(path: &Path, dest: &Path, required: &str) -> Result<(), GedzipError> {
    extract_zip_file_with(path, dest, required, &LIMITS)
}

fn extract_zip_file_with(
    path: &Path,
    dest: &Path,
    required: &str,
    lim: &Limits,
) -> Result<(), GedzipError> {
    let file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    let roots = [dest.to_path_buf()];
    run_on(
        std::io::BufReader::new(file),
        len,
        lim,
        None,
        required,
        |name| {
            let p = dest.join(name);
            if !is_lexically_under(&p, &roots) {
                return Err(GedzipError::UnsafePath(name.to_string()));
            }
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&p)?;
            Ok(Out::File(f))
        },
        |_, _| {},
    )?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;

    const GED: &str = "0 HEAD\n1 GEDC\n2 VERS 7.0\n0 @I1@ INDI\n1 NAME A /B/\n1 OBJE @M1@\n1 OBJE @M2@\n1 OBJE @M3@\n0 @M1@ OBJE\n1 FILE media/a.jpg\n2 FORM image/jpeg\n0 @M2@ OBJE\n1 FILE pics/my%20photo.png\n2 FORM image/png\n0 @M3@ OBJE\n1 FILE %D1%84%D0%BE%D1%82%D0%BE/%D0%91%D0%B0%D0%B1%D1%83%D1%81%D1%8F.jpg\n2 FORM image/jpeg\n0 TRLR\n";

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
            ("фото/Бабуся.jpg", b"cyr"),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let out = import_gedzip(&z, dir.path(), &opts()).unwrap();
        assert_eq!(out.resolved_media.len(), 3, "{:?}", out.report);
        for (_, p) in &out.resolved_media {
            assert!(p.starts_with(dir.path()) && p.is_file());
        }
        assert_eq!(
            std::fs::read(dir.path().join("media/a.jpg")).unwrap(),
            b"jpegdata"
        );
        let c = read_gedzip(&z).unwrap();
        assert_eq!(c.gedcom, GED.as_bytes());
        assert_eq!(c.media.len(), 3);
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
            "NUL",
            "dir/con.txt",
            "Com1",
            "lpt9.png",
            "a.txt:stream",
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
        assert!(matches!(
            read_with(&z, &lim),
            Err(GedzipError::EntryTooLarge(_) | GedzipError::Io(_))
        ));
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
    fn failing_archive_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        // Bad name late in the archive: rejected by the first pass, nothing written.
        let z = make(&[
            ("gedcom.ged", GED.as_bytes()),
            ("media/a.jpg", b"x"),
            ("../evil", b"y"),
        ]);
        assert!(import_gedzip(&z, &out, &opts()).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        // Failure while streaming (bomb after a good file): staging folder is removed.
        let zeros = vec![0u8; 16 << 20];
        let z = make(&[
            ("gedcom.ged", GED.as_bytes()),
            ("media/a.jpg", b"x"),
            ("zzz.bin", &zeros),
        ]);
        assert!(import_gedzip(&z, &out, &opts()).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn nonempty_extract_dir_is_refused_and_untouched() {
        let base = tempfile::tempdir().unwrap();
        let out = base.path().join("out");
        std::fs::create_dir(&out).unwrap();
        std::fs::write(out.join("keep.txt"), b"k").unwrap();
        let z = make(&[("gedcom.ged", GED.as_bytes()), ("media/a.jpg", b"x")]);
        assert!(import_gedzip(&z, &out, &opts()).is_err());
        assert!(out.join("keep.txt").is_file());
        assert_eq!(std::fs::read_dir(base.path()).unwrap().count(), 1);
    }

    #[test]
    fn reserved_device_names() {
        for n in [
            "CON",
            "prn",
            "Aux.txt",
            "nul",
            "COM1",
            "com9.png",
            "LPT3",
            "CONIN$",
            "COM\u{b9}",
        ] {
            assert!(is_reserved_device(n), "{n}");
        }
        for n in ["COM0", "COM10", "console", "a.con", "lpt", "nulx"] {
            assert!(!is_reserved_device(n), "{n}");
        }
    }

    #[test]
    fn rejects_zip_bomb_by_ratio() {
        let zeros = vec![0u8; 16 << 20];
        let z = make(&[("gedcom.ged", b"0 TRLR\n"), ("bomb.bin", &zeros)]);
        assert!(matches!(read_gedzip(&z), Err(GedzipError::RatioTooHigh(_))));
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            import_gedzip(&z, dir.path(), &opts()),
            Err(GedzipError::RatioTooHigh(_))
        ));
    }

    fn zip_file(entries: &[(&str, &[u8])]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("copy.zip");
        std::fs::write(&path, make(entries)).unwrap();
        (dir, path)
    }

    #[test]
    fn project_zip_extracts_every_file_and_needs_its_required_entry() {
        let (_keep, z) = zip_file(&[("tree.db", b"db"), ("media/ab/abc.jpg", b"img")]);
        let out = tempfile::tempdir().unwrap();
        extract_zip_file(&z, out.path(), "tree.db").unwrap();
        assert_eq!(std::fs::read(out.path().join("tree.db")).unwrap(), b"db");
        assert_eq!(std::fs::read(out.path().join("media/ab/abc.jpg")).unwrap(), b"img");
        let (_keep2, other) = zip_file(&[("other.db", b"x")]);
        let out2 = tempfile::tempdir().unwrap();
        assert!(matches!(
            extract_zip_file(&other, out2.path(), "tree.db"),
            Err(GedzipError::MissingEntry(_))
        ));
    }

    #[test]
    fn project_zip_refuses_unsafe_names_bombs_and_oversized_entries() {
        for name in ["../x.db", "/abs/tree.db", "a/../../b", "c:/x", r"media\x"] {
            let (_k, z) = zip_file(&[("tree.db", b"db"), (name, b"x")]);
            let out = tempfile::tempdir().unwrap();
            assert!(
                matches!(
                    extract_zip_file(&z, out.path(), "tree.db"),
                    Err(GedzipError::UnsafePath(_))
                ),
                "{name}"
            );
        }
        let zeros = vec![0u8; 16 << 20];
        let (_k, bomb) = zip_file(&[("tree.db", b"db"), ("bomb.bin", &zeros)]);
        let out = tempfile::tempdir().unwrap();
        assert!(matches!(
            extract_zip_file(&bomb, out.path(), "tree.db"),
            Err(GedzipError::RatioTooHigh(_))
        ));
        let tiny = Limits {
            max_entry_bytes: 8,
            ..LIMITS
        };
        let (_k, big) = zip_file(&[("tree.db", b"0123456789")]);
        let out = tempfile::tempdir().unwrap();
        assert!(matches!(
            extract_zip_file_with(&big, out.path(), "tree.db", &tiny),
            Err(GedzipError::EntryTooLarge(_))
        ));
        let few = Limits {
            max_entries: 1,
            ..LIMITS
        };
        let (_k, many) = zip_file(&[("tree.db", b"db"), ("a", b"1")]);
        let out = tempfile::tempdir().unwrap();
        assert!(matches!(
            extract_zip_file_with(&many, out.path(), "tree.db", &few),
            Err(GedzipError::TooManyEntries(_))
        ));
    }
}
