//! THE SEAM for import formats: which kind of file the user picked, and the one place that turns
//! it into an `ImportOutcome`. The app (`gt-session`) and the command line (`gt`) both call this,
//! then build the project the same way for every format, so a new format never needs its own
//! builder. To add a format: add a variant, make `detect` return it, give it a `convert` arm.

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
    /// Gramps XML (`.gramps` gzip, or plain XML).
    Gramps,
    /// GEDZIP (`.gdz`): a zip with `gedcom.ged` and its media.
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

/// Why an import could not start. `Io` is the disk; `Invalid` is the file's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportErrorKind {
    Io,
    Invalid,
}

/// A user-facing message (full sentences, no internals) plus its kind.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ImportError {
    pub kind: ImportErrorKind,
    pub message: String,
}

impl ImportError {
    pub fn new(kind: ImportErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    fn invalid(message: impl Into<String>) -> Self {
        Self::new(ImportErrorKind::Invalid, message)
    }
}

/// A converted file. Keep it (or at least `cleanup`) alive until the media has been copied.
#[derive(Debug)]
pub struct Converted {
    pub outcome: ImportOutcome,
    /// Folders the converter created whose files the project may copy (GEDZIP extract folder).
    pub extra_media_roots: Vec<PathBuf>,
    /// Removes the scratch folder when dropped.
    pub cleanup: Option<TempDirGuard>,
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

/// A scratch folder this process created; removed (with everything in it) on drop.
#[derive(Debug)]
pub struct TempDirGuard {
    path: Option<PathBuf>,
}

impl TempDirGuard {
    /// Create a new, uniquely named folder under the system temp folder. The guard exists before
    /// the folder does, so any later failure removes it.
    pub fn create(prefix: &str) -> Result<Self, ImportError> {
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
        let guard = Self {
            path: Some(path.clone()),
        };
        std::fs::create_dir(&path).map_err(|e| io_err("create", &path, &e))?;
        Ok(guard)
    }

    pub fn path(&self) -> &Path {
        self.path.as_deref().unwrap_or(Path::new(""))
    }

    /// Keep the folder: the caller takes over removing it.
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

fn io_err(what: &str, path: &Path, e: &std::io::Error) -> ImportError {
    ImportError::new(
        ImportErrorKind::Io,
        format!("Cannot {what} {}: {e}", path.display()),
    )
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Which format `path` holds: magic bytes first, then the extension; GEDCOM otherwise.
pub fn detect(path: &Path) -> Result<ImportFormat, ImportError> {
    let mut head = Vec::with_capacity(2048);
    std::fs::File::open(path)
        .and_then(|f| f.take(2048).read_to_end(&mut head))
        .map_err(|e| io_err("read", path, &e))?;
    Ok(detect_bytes(&head, path))
}

fn detect_bytes(head: &[u8], path: &Path) -> ImportFormat {
    let ext = extension(path);
    if head.starts_with(gt_import_atdb::SQLITE_MAGIC) || ext == "atdb" {
        return ImportFormat::Atdb;
    }
    if head.starts_with(b"PK\x03\x04") || ext == "gdz" {
        return ImportFormat::Gedzip;
    }
    if gt_import_gramps::is_gzip(head) || ext == "gramps" {
        return ImportFormat::Gramps;
    }
    let text = String::from_utf8_lossy(head);
    let t = text.trim_start_matches('\u{feff}').trim_start();
    if (t.starts_with("<?xml") || t.starts_with("<database")) && text.contains("gramps") {
        return ImportFormat::Gramps;
    }
    ImportFormat::Gedcom
}

/// Convert `path` (already detected as `format`) into an import outcome.
pub fn convert(
    format: ImportFormat,
    path: &Path,
    opts: &ImportOptions,
) -> Result<Converted, ImportError> {
    match format {
        ImportFormat::Gedcom => {
            let bytes = std::fs::read(path).map_err(|e| io_err("read", path, &e))?;
            Ok(Converted::plain(gt_gedcom::import(&bytes, opts)))
        }
        ImportFormat::Atdb => {
            if !gt_import_atdb::is_atdb(path) {
                return Err(ImportError::invalid(format!(
                    "{} is a database file, but not a Древо Жизни (.atdb) one. Choose a GEDCOM, GEDZIP, Gramps or Древо Жизни file.",
                    file_name(path)
                )));
            }
            gt_import_atdb::import_atdb(path, opts)
                .map(Converted::plain)
                .map_err(|e| {
                    let kind = match e {
                        gt_import_atdb::AtdbError::Open { .. } => ImportErrorKind::Io,
                        _ => ImportErrorKind::Invalid,
                    };
                    ImportError::new(kind, e.to_string())
                })
        }
        ImportFormat::Gramps => {
            let bytes = std::fs::read(path).map_err(|e| io_err("read", path, &e))?;
            gt_import_gramps::import_gramps(&bytes, opts)
                .map(Converted::plain)
                .map_err(|e| {
                    ImportError::invalid(format!(
                        "{} could not be imported as a Gramps file: {e}.",
                        file_name(path)
                    ))
                })
        }
        ImportFormat::Gedzip => {
            let bytes = std::fs::read(path).map_err(|e| io_err("read", path, &e))?;
            // The guard exists before anything is extracted; the extractor's staging folder sits
            // inside it too, so a panic or an error anywhere below leaves nothing behind.
            let guard = TempDirGuard::create("gene-tree-gedzip")?;
            let media = guard.path().join("media");
            let outcome = gt_gedcom::import_gedzip(&bytes, &media, opts).map_err(|e| {
                ImportError::invalid(format!(
                    "{} could not be imported as a GEDZIP file: {e}.",
                    file_name(path)
                ))
            })?;
            Ok(Converted {
                outcome,
                extra_media_roots: vec![media],
                cleanup: Some(guard),
            })
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Write;

    fn d(head: &[u8], name: &str) -> ImportFormat {
        detect_bytes(head, Path::new(name))
    }

    #[test]
    fn detects_by_magic_then_extension() {
        assert_eq!(d(b"SQLite format 3\0rest", "x.bin"), ImportFormat::Atdb);
        assert_eq!(d(b"PK\x03\x04", "x.zip"), ImportFormat::Gedzip);
        assert_eq!(d(b"", "pack.GDZ"), ImportFormat::Gedzip);
        assert_eq!(d(&[0x1f, 0x8b, 8], "t.bin"), ImportFormat::Gramps);
        assert_eq!(d(b"", "t.Gramps"), ImportFormat::Gramps);
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

    fn scratch() -> TempDirGuard {
        TempDirGuard::create("gt-import-test").unwrap()
    }

    const GED7: &str = "0 HEAD\n1 GEDC\n2 VERS 7.0\n0 @I1@ INDI\n1 NAME Ivan /Koval/\n1 OBJE @M1@\n0 @M1@ OBJE\n1 FILE photos/a.jpg\n2 FORM image/jpeg\n0 TRLR\n";

    fn gdz(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        for (n, data) in entries {
            w.start_file(*n, o).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn gedzip_media_lives_until_the_converted_value_is_dropped() {
        let dir = scratch();
        let file = dir.path().join("family.gdz");
        std::fs::write(
            &file,
            gdz(&[("gedcom.ged", GED7.as_bytes()), ("photos/a.jpg", b"jpeg")]),
        )
        .unwrap();
        assert_eq!(detect(&file).unwrap(), ImportFormat::Gedzip);
        let conv = convert(ImportFormat::Gedzip, &file, &ImportOptions::default()).unwrap();
        assert_eq!(conv.outcome.tree.persons.len(), 1);
        assert_eq!(conv.outcome.resolved_media.len(), 1);
        let (_, media) = conv.outcome.resolved_media.first().unwrap();
        assert!(media.starts_with(&conv.extra_media_roots[0]), "{media:?}");
        assert_eq!(std::fs::read(media).unwrap(), b"jpeg");
        let root = conv.cleanup.as_ref().unwrap().path().to_path_buf();
        drop(conv);
        assert!(!root.exists());
        broken_gedzip_is_invalid_and_leaves_no_scratch_folder();
    }

    // Not a #[test] of its own: it counts this process's scratch folders, which the other GEDZIP
    // test creates too; run sequentially from there.
    fn broken_gedzip_is_invalid_and_leaves_no_scratch_folder() {
        let dir = scratch();
        let file = dir.path().join("broken.gdz");
        std::fs::write(&file, gdz(&[("readme.txt", b"no gedcom here")])).unwrap();
        let before = scratch_count();
        let e = convert(ImportFormat::Gedzip, &file, &ImportOptions::default()).unwrap_err();
        assert_eq!(e.kind, ImportErrorKind::Invalid);
        assert!(
            e.message
                .starts_with("broken.gdz could not be imported as a GEDZIP file"),
            "{}",
            e.message
        );
        assert_eq!(scratch_count(), before);
    }

    fn scratch_count() -> usize {
        std::fs::read_dir(std::env::temp_dir())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(&format!("gene-tree-gedzip-{}-", std::process::id()))
            })
            .count()
    }

    const GRAMPS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
  <header><created date="2026-01-01" version="5.2.0"/></header>
  <people>
    <person handle="_p1" id="I0001"><gender>M</gender>
      <name type="Birth Name"><first>Ivan</first><surname>Koval</surname></name>
    </person>
  </people>
</database>"#;

    #[test]
    fn gramps_plain_and_gzipped_both_import() {
        let dir = scratch();
        let plain = dir.path().join("t.xml");
        std::fs::write(&plain, GRAMPS).unwrap();
        let gz = dir.path().join("t.gramps");
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(GRAMPS.as_bytes()).unwrap();
        std::fs::write(&gz, enc.finish().unwrap()).unwrap();
        for f in [&plain, &gz] {
            assert_eq!(detect(f).unwrap(), ImportFormat::Gramps);
            let conv = convert(ImportFormat::Gramps, f, &ImportOptions::default()).unwrap();
            assert_eq!(conv.outcome.tree.persons.len(), 1);
            assert!(conv.cleanup.is_none());
        }
    }

    #[test]
    fn non_gramps_xml_is_invalid_with_a_plain_message() {
        let dir = scratch();
        let f = dir.path().join("t.gramps");
        std::fs::write(&f, "<html/>").unwrap();
        let e = convert(ImportFormat::Gramps, &f, &ImportOptions::default()).unwrap_err();
        assert_eq!(e.kind, ImportErrorKind::Invalid);
        assert!(
            e.message
                .starts_with("t.gramps could not be imported as a Gramps file"),
            "{}",
            e.message
        );
    }

    #[test]
    fn missing_file_is_io() {
        let e = detect(Path::new("definitely/not/here.ged")).unwrap_err();
        assert_eq!(e.kind, ImportErrorKind::Io);
    }
}
