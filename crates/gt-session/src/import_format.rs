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
    /// A scratch folder to delete once the project has been built.
    pub cleanup: Option<PathBuf>,
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
    if head.starts_with(gt_import_atdb::SQLITE_MAGIC) {
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
                .map_err(|e| ApiError::new(ErrorCode::Invalid, e.to_string()))
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
