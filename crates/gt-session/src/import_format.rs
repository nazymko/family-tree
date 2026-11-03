//! THE SEAM for import formats: which kind of file the user picked, and the one place that turns
//! it into an `ImportOutcome`. `transfer::import_gedcom` then builds the project the same way for
//! every format (temp folder, media copy, report), so a new format never needs its own builder.
//!
//! To add a format: give its variant a real `convert` arm below (one line calling the converter),
//! and make `detect` return it. The variants `Gramps` and `Gedzip` are placeholders: they are
//! already detected, and `convert` answers "not supported yet" until the converters are wired.

use crate::error::{ApiError, ErrorCode};
use gt_gedcom::{ImportOptions, ImportOutcome};
use std::io::Read;
use std::path::{Path, PathBuf};

/// The kind of file being imported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFormat {
    /// A GEDCOM 5.5.1 / 7.0 text file (also the fallback for anything not recognised).
    Gedcom,
    /// A Древо Жизни (Genery) `.atdb` SQLite database.
    Atdb,
    /// PLACEHOLDER: Gramps XML (`.gramps` gzip, or plain `.xml`). Wire it in `convert` with
    /// `gt_import_gramps::import_gramps(&std::fs::read(path)?, opts)` and map its error.
    Gramps,
    /// PLACEHOLDER: GEDZIP (`.gdz`, a zip with `gedcom.ged` and media). Wire it in `convert` with
    /// `gt_gedcom::gedzip::import_gedzip(&bytes, &extract_dir, opts)`; the extract folder goes in
    /// `Converted::extra_media_roots` (so its files may be copied) and `Converted::cleanup`.
    /// Create the `TempDirGuard` BEFORE extracting, so every later `?` removes the folder.
    Gedzip,
}

impl ImportFormat {
    /// What the format is called in messages ("a GEDCOM file").
    pub fn label(self) -> &'static str {
        match self {
            Self::Gedcom => "GEDCOM",
            Self::Atdb => "Древо Жизни",
            Self::Gramps => "Gramps XML",
            Self::Gedzip => "GEDZIP",
        }
    }
}

/// The result of converting a file, plus what the project builder must know about it.
#[derive(Debug)]
pub struct Converted {
    pub outcome: ImportOutcome,
    /// Folders (besides the media roots and the file's own folder) that resolved media may come
    /// from, e.g. a GEDZIP's extract folder.
    pub extra_media_roots: Vec<PathBuf>,
    /// A scratch folder that is deleted when this guard drops (after the project is built, or on
    /// any error or panic on the way).
    pub cleanup: Option<TempDirGuard>,
}

/// Owns a scratch folder and deletes it on `Drop`. A converter that extracts files (GEDZIP) makes
/// the guard BEFORE it creates the folder, so a later `?` drops the guard and removes the folder;
/// on success the guard travels in `Converted::cleanup`, and the project builder drops it after
/// the media is copied. Call `disarm` only to keep the folder on purpose.
#[derive(Debug)]
pub struct TempDirGuard {
    path: Option<PathBuf>,
}

impl TempDirGuard {
    /// Adopts `path` (it may not exist yet; a missing folder is fine on drop).
    pub fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    /// Creates a fresh, uniquely named folder under the system temp folder.
    pub fn create(prefix: &str) -> Result<Self, ApiError> {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "{prefix}-{}-{}-{nanos}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let guard = Self::new(path.clone());
        std::fs::create_dir_all(&path).map_err(|e| io_err("create", &path, &e))?;
        Ok(guard)
    }

    pub fn path(&self) -> &Path {
        self.path.as_deref().unwrap_or(Path::new(""))
    }

    /// Gives up ownership: the folder stays.
    pub fn disarm(mut self) -> PathBuf {
        self.path.take().unwrap_or_default()
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        if let Some(p) = self.path.take() {
            let _ = std::fs::remove_dir_all(p);
        }
    }
}

impl Converted {
    fn plain(outcome: ImportOutcome) -> Self {
        Self {
            outcome,
            extra_media_roots: vec![],
            cleanup: None,
        }
    }
}

fn io_err(what: &str, path: &Path, e: &std::io::Error) -> ApiError {
    ApiError::new(
        ErrorCode::Io,
        format!("Cannot {what} {}: {e}", path.display()),
    )
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Decides the format from the first bytes of the file (magic numbers), then its extension.
/// Reads at most 2 KiB. Any SQLite file is `Atdb`; `convert` then refuses one that lacks the
/// Древо Жизни tables, so another database is never read as GEDCOM.
pub fn detect(path: &Path) -> Result<ImportFormat, ApiError> {
    let mut head = Vec::with_capacity(2048);
    std::fs::File::open(path)
        .and_then(|f| f.take(2048).read_to_end(&mut head))
        .map_err(|e| io_err("read", path, &e))?;
    Ok(detect_bytes(&head, path))
}

fn detect_bytes(head: &[u8], path: &Path) -> ImportFormat {
    let ext = extension(path);
    // A `.atdb` with a damaged header is still an atdb: it errors instead of importing as GEDCOM.
    if head.starts_with(gt_import_atdb::SQLITE_MAGIC) || ext == "atdb" {
        // A SQLite file is never GEDCOM; whether it is an atdb is decided by its tables.
        return ImportFormat::Atdb;
    }
    if head.starts_with(b"PK\x03\x04") || ext == "gdz" {
        return ImportFormat::Gedzip;
    }
    if head.starts_with(&[0x1f, 0x8b]) || ext == "gramps" {
        return ImportFormat::Gramps;
    }
    let text = String::from_utf8_lossy(head);
    let t = text.trim_start_matches('\u{feff}').trim_start();
    if t.starts_with("<?xml") || t.starts_with("<database") {
        if text.contains("gramps") {
            return ImportFormat::Gramps;
        }
        // Some other XML: GEDCOM X and friends are not supported, and are not GEDCOM either.
        return ImportFormat::Gedcom;
    }
    ImportFormat::Gedcom
}

/// Converts the file at `path` with the converter of `format`.
pub fn convert(
    format: ImportFormat,
    path: &Path,
    opts: &ImportOptions,
) -> Result<Converted, ApiError> {
    match format {
        ImportFormat::Gedcom => {
            let bytes = std::fs::read(path).map_err(|e| io_err("read", path, &e))?;
            Ok(Converted::plain(gt_gedcom::import(&bytes, opts)))
        }
        ImportFormat::Atdb => {
            if !gt_import_atdb::is_atdb(path) {
                return Err(ApiError::new(
                    ErrorCode::Invalid,
                    format!(
                        "{} is a database file, but not a Древо Жизни (.atdb) one. Choose a GEDCOM or Древо Жизни file.",
                        path.display()
                    ),
                ));
            }
            gt_import_atdb::import_atdb(path, opts)
                .map(Converted::plain)
                .map_err(|e| {
                    let code = match e {
                        gt_import_atdb::AtdbError::Open { .. } => ErrorCode::Io,
                        _ => ErrorCode::Invalid,
                    };
                    ApiError::new(code, e.to_string())
                })
        }
        // Placeholders: replace the arm with the converter call (see the variant's docs).
        ImportFormat::Gramps | ImportFormat::Gedzip => Err(not_supported_yet(format, path)),
    }
}

fn not_supported_yet(format: ImportFormat, path: &Path) -> ApiError {
    ApiError::new(
        ErrorCode::Invalid,
        format!(
            "{} files ({}) cannot be imported yet. Export a GEDCOM file from the program that made it, or choose a GEDCOM or Древо Жизни file.",
            format.label(),
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        ),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn d(head: &[u8], name: &str) -> ImportFormat {
        detect_bytes(head, Path::new(name))
    }

    #[test]
    fn detects_by_magic_then_extension() {
        assert_eq!(d(b"SQLite format 3\0rest", "x.bin"), ImportFormat::Atdb);
        assert_eq!(d(b"PK\x03\x04", "x.zip"), ImportFormat::Gedzip);
        assert_eq!(d(b"", "pack.GDZ"), ImportFormat::Gedzip);
        assert_eq!(d(&[0x1f, 0x8b, 8], "t.gramps"), ImportFormat::Gramps);
        assert_eq!(
            d(
                b"<?xml version=\"1.0\"?><database xmlns=\"http://gramps-project.org/xml/1.7.1/\">",
                "t.xml"
            ),
            ImportFormat::Gramps
        );
        assert_eq!(d(b"0 HEAD\n1 CHAR UTF-8\n", "t.ged"), ImportFormat::Gedcom);
        assert_eq!(d(b"\xef\xbb\xbf0 HEAD\n", "noext"), ImportFormat::Gedcom);
    }

    #[test]
    fn damaged_atdb_extension_is_still_atdb() {
        assert_eq!(d(b"garbage", "t.atdb"), ImportFormat::Atdb);
    }

    #[test]
    fn guard_removes_the_folder_on_error_and_on_drop() {
        fn fake_converter(succeed: bool) -> Result<(Converted, PathBuf), ApiError> {
            let guard = TempDirGuard::create("gt-guard-test")?;
            let dir = guard.path().to_path_buf();
            std::fs::write(dir.join("x.jpg"), b"x").unwrap();
            if !succeed {
                return Err(ApiError::new(ErrorCode::Invalid, "later step failed"));
            }
            Ok((
                Converted {
                    outcome: gt_gedcom::import(b"0 HEAD\n0 TRLR\n", &ImportOptions::default()),
                    extra_media_roots: vec![dir.clone()],
                    cleanup: Some(guard),
                },
                dir,
            ))
        }
        assert!(fake_converter(false).is_err());
        let (conv, dir) = fake_converter(true).unwrap();
        assert!(dir.is_dir());
        drop(conv);
        assert!(!dir.exists());
        let kept = TempDirGuard::create("gt-guard-test").unwrap().disarm();
        assert!(kept.is_dir());
        std::fs::remove_dir_all(kept).unwrap();
    }

    #[test]
    fn placeholders_say_not_supported_yet() {
        let e = convert(
            ImportFormat::Gramps,
            Path::new("a.gramps"),
            &ImportOptions::default(),
        )
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::Invalid);
        assert!(
            e.message.contains("cannot be imported yet"),
            "{}",
            e.message
        );
    }
}
