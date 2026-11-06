//! Copies of the tree as files: sending one as a single zip, unpacking a received one, and
//! opening a received copy (a folder or a zip, both untrusted) in a scratch folder for the merge.
//!
//! Whatever the merge reads, it reads from a scratch copy; the folder or zip the relative sent
//! is never opened in place and never modified.

use crate::error::{ApiError, ErrorCode};
use crate::merge::dto::{CopySizeDto, CopyZipDto};
use crate::session::Session;
use gt_gedcom::GedzipError;
use gt_store::{Project, StoreError};
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

const DB_FILE: &str = "tree.db";
/// A database file larger than this is not a family tree.
const MAX_DB_BYTES: u64 = 2 << 30;
/// Most records a copy may hold before it is refused as unreasonable.
pub(crate) const MAX_RECORDS: usize = 5_000_000;

fn io_err(what: &str, path: &Path, e: &std::io::Error) -> ApiError {
    ApiError::new(
        ErrorCode::Io,
        format!("Cannot {what} {}: {e}", path.display()),
    )
}

fn refuse(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::Invalid, message)
}

/// A folder in the system temp area that is removed when this goes away.
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new() -> Result<Scratch, ApiError> {
        let dir = std::env::temp_dir().join(format!("gene-tree-merge-{}", gt_core::ids::new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| io_err("create", &dir, &e))?;
        Ok(Scratch(dir))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A received copy, opened from a scratch folder.
pub(crate) struct OpenedCopy {
    /// Dropped first: the scratch folder cannot be removed while its database is open.
    pub project: Project,
    /// Where the copy's photos are: its own `media/` folder, read and never written.
    pub media_root: PathBuf,
    /// The folder or file name the copy came from.
    pub source_name: String,
    _scratch: Scratch,
}

/// Plain words for a zip that cannot be used.
pub(crate) fn zip_refusal(e: GedzipError) -> ApiError {
    match e {
        GedzipError::Io(e) => {
            ApiError::new(ErrorCode::Io, format!("Cannot read the zip file: {e}"))
        }
        GedzipError::Zip(_) => refuse("This is not a zip file Gene Tree can read."),
        GedzipError::MissingEntry(_) | GedzipError::MissingGedcom => {
            refuse("This zip file is not a copy of a Gene Tree tree: it has no tree.db.")
        }
        GedzipError::TooManyEntries(_)
        | GedzipError::EntryTooLarge(_)
        | GedzipError::TotalTooLarge
        | GedzipError::RatioTooHigh(_) => {
            refuse("This zip file is far larger than a family tree can be, so it was not opened.")
        }
        GedzipError::UnsafePath(_)
        | GedzipError::Symlink(_)
        | GedzipError::Encrypted(_)
        | GedzipError::Duplicate(_) => refuse(
            "This zip file contains entries that are not safe to unpack, so it was not opened.",
        ),
    }
}

/// Plain words for a database that cannot be used.
pub(crate) fn store_refusal(e: StoreError) -> ApiError {
    match e {
        StoreError::NewerSchema { .. } => ApiError::new(
            ErrorCode::Invalid,
            "This copy was saved by a newer Gene Tree. Update the app first.",
        ),
        StoreError::NotAProject(_) => refuse("This is not a copy of a Gene Tree tree."),
        StoreError::Io(e) => ApiError::new(ErrorCode::Io, format!("Cannot read this copy: {e}")),
        StoreError::Locked(_) => ApiError::new(ErrorCode::Conflict, "This copy is in use."),
        StoreError::Sql(_) | StoreError::Corrupt(_) | StoreError::AlreadyExists(_) => {
            refuse("Cannot read this copy: the tree in it is damaged.")
        }
    }
}

/// Opens a received copy for reading. A folder: its `tree.db` is copied to a scratch folder. A zip:
/// it is unpacked there under the GEDZIP limits. Either way the copy is then opened with
/// `Project::open_untrusted`; an older schema is migrated in the scratch copy only.
pub(crate) fn open_copy(source: &Path) -> Result<OpenedCopy, ApiError> {
    let meta = std::fs::symlink_metadata(source)
        .map_err(|_| refuse("Cannot find that folder or file. Choose the copy again."))?;
    if meta.file_type().is_symlink() {
        return Err(refuse(
            "A link is not a copy. Choose the folder or file itself.",
        ));
    }
    let source_name = source
        .file_stem()
        .or_else(|| source.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let scratch = Scratch::new()?;
    let media_root;
    if meta.is_dir() {
        let db = source.join(DB_FILE);
        let db_meta = std::fs::symlink_metadata(&db).map_err(|_| {
            refuse("This folder is not a copy of a Gene Tree tree: it has no tree.db.")
        })?;
        if db_meta.file_type().is_symlink() || !db_meta.is_file() {
            return Err(refuse(
                "This folder is not a copy of a Gene Tree tree: it has no tree.db.",
            ));
        }
        if db_meta.len() > MAX_DB_BYTES {
            return Err(refuse(
                "The tree in this copy is far larger than a family tree can be, so it was not opened.",
            ));
        }
        std::fs::copy(&db, scratch.path().join(DB_FILE)).map_err(|e| io_err("read", &db, &e))?;
        // A copy made while its app was open still has part of its data in the write-ahead file.
        let wal = source.join("tree.db-wal");
        if wal.is_file()
            && std::fs::symlink_metadata(&wal).is_ok_and(|m| !m.file_type().is_symlink())
        {
            std::fs::copy(&wal, scratch.path().join("tree.db-wal"))
                .map_err(|e| io_err("read", &wal, &e))?;
        }
        media_root = source.join("media");
    } else if meta.is_file() {
        gt_gedcom::extract_zip_file(source, scratch.path(), DB_FILE).map_err(zip_refusal)?;
        media_root = scratch.path().join("media");
    } else {
        return Err(refuse("Choose a folder or a zip file."));
    }
    let project = Project::open_untrusted(scratch.path()).map_err(store_refusal)?;
    Ok(OpenedCopy {
        project,
        media_root,
        source_name,
        _scratch: scratch,
    })
}

// ---------------------------------------------------------------- sending

/// Files under `root` (not links, not half-copied `.part-` files) as `(path, forward-slash path
/// relative to root's parent)`, so entries read `media/ab/....`.
fn media_files(root: &Path) -> Vec<(PathBuf, String)> {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<(PathBuf, String)>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for e in entries {
            let Ok(ft) = e.file_type() else { continue };
            let name = e.file_name().to_string_lossy().into_owned();
            if ft.is_symlink() || name.starts_with(".part-") {
                continue;
            }
            let child = format!("{rel}/{name}");
            if ft.is_dir() {
                walk(&e.path(), &child, out);
            } else if ft.is_file() {
                out.push((e.path(), child));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, "media", &mut out);
    out
}

/// Photos, audio and video are already compressed; deflating them again only costs time.
fn already_compressed(name: &str) -> bool {
    let lower = name.to_lowercase();
    [
        ".jpg", ".jpeg", ".png", ".webp", ".gif", ".mp3", ".m4a", ".ogg", ".opus", ".mp4", ".mov",
        ".zip", ".heic",
    ]
    .iter()
    .any(|e| lower.ends_with(e))
}

fn partial_file(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_os_string();
    name.push(format!(".partial-{}", std::process::id()));
    PathBuf::from(name)
}

impl Session {
    /// How big the zip of this tree would be, before compression: the database plus the photos.
    pub fn copy_size(&self) -> Result<CopySizeDto, ApiError> {
        let db = self.project.dir.join(DB_FILE);
        let db_bytes = std::fs::metadata(&db)
            .map_err(|e| io_err("read", &db, &e))?
            .len();
        let files = media_files(&self.project.dir.join("media"));
        let photo_bytes: u64 = files
            .iter()
            .filter_map(|(p, _)| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();
        Ok(CopySizeDto {
            bytes: db_bytes + photo_bytes,
            photos: u32::try_from(files.len()).unwrap_or(u32::MAX),
            people: u32::try_from(
                self.tree
                    .persons
                    .values()
                    .filter(|p| !p.sibling_marker)
                    .count(),
            )
            .unwrap_or(u32::MAX),
        })
    }

    /// A clean copy of the tree as ONE zip file at `dest` (an existing file is replaced; the
    /// caller has already asked): a consistent snapshot of `tree.db` (as a backup makes it, so no
    /// open write-ahead file) at the root, and the photos under `media/`. No `backups/`, no
    /// thumbnails. The copy is unfiltered: it holds living people, private records and the whole
    /// edit history. Nothing is delivered; the app has no server.
    pub fn save_copy_zip(&self, dest: &Path) -> Result<CopyZipDto, ApiError> {
        if dest.as_os_str().is_empty() || dest.is_dir() {
            return Err(refuse("Choose a file name for the zip file."));
        }
        if crate::transfer::inside_project(&self.project.dir, dest) {
            return Err(refuse(
                "Choose a place outside the tree's own folder for the zip file.",
            ));
        }
        let stage = partial_file(dest);
        let _ = std::fs::remove_file(&stage);
        let made = self
            .build_zip(&stage)
            .and_then(|()| std::fs::rename(&stage, dest).map_err(|e| io_err("create", dest, &e)));
        if made.is_err() {
            let _ = std::fs::remove_file(&stage);
        }
        made?;
        let bytes = std::fs::metadata(dest)
            .map_err(|e| io_err("read", dest, &e))?
            .len();
        Ok(CopyZipDto {
            path: dest.to_string_lossy().into_owned(),
            bytes,
        })
    }

    fn build_zip(&self, stage: &Path) -> Result<(), ApiError> {
        // The snapshot: the same as the folder copy makes, in a scratch folder.
        let scratch = Scratch::new()?;
        let db = scratch.path().join(DB_FILE);
        let target = db
            .to_str()
            .ok_or_else(|| refuse("The folder name is not valid text."))?;
        self.project
            .conn()
            .execute("VACUUM INTO ?1", [target])
            .map_err(|e| ApiError::new(ErrorCode::Io, format!("Cannot copy the tree: {e}")))?;
        {
            let mut snap = Project::open(scratch.path())?;
            if snap
                .meta_get(crate::transfer::META_LAST_IMPORT)?
                .is_some_and(|v| !v.is_empty())
            {
                snap.meta_set(crate::transfer::META_LAST_IMPORT, "")?;
            }
        }
        let file = std::fs::File::create(stage).map_err(|e| io_err("create", stage, &e))?;
        let mut zw = zip::ZipWriter::new(std::io::BufWriter::new(file));
        let deflated = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        let stored = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .large_file(true);
        let zip_err = |e: zip::result::ZipError| {
            ApiError::new(ErrorCode::Io, format!("Cannot write the zip file: {e}"))
        };
        let add = |zw: &mut zip::ZipWriter<_>,
                   src: &Path,
                   name: &str,
                   opts: SimpleFileOptions|
         -> Result<(), ApiError> {
            zw.start_file(name, opts).map_err(zip_err)?;
            let mut f = std::fs::File::open(src).map_err(|e| io_err("read", src, &e))?;
            std::io::copy(&mut f, zw).map_err(|e| io_err("write", stage, &e))?;
            Ok(())
        };
        add(&mut zw, &db, DB_FILE, deflated)?;
        for (path, name) in media_files(&self.project.dir.join("media")) {
            let opts = if already_compressed(&name) {
                stored
            } else {
                deflated
            };
            add(&mut zw, &path, &name, opts)?;
        }
        let mut inner = zw.finish().map_err(zip_err)?;
        inner.flush().map_err(|e| io_err("write", stage, &e))?;
        Ok(())
    }
}

/// Unpacks a zip received from a relative into `dest` (a new folder, or an empty one) under the
/// GEDZIP limits, checks that it holds an intact tree, and leaves a folder that opens like any
/// project. Nothing is left behind on a failure.
pub fn open_received_copy(zip_file: &Path, dest: &Path) -> Result<(), ApiError> {
    if !zip_file.is_file() {
        return Err(refuse("Choose the zip file you received."));
    }
    if dest.exists()
        && !(dest.is_dir() && std::fs::read_dir(dest).is_ok_and(|mut d| d.next().is_none()))
    {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            format!(
                "The folder {} already has files in it. Choose a new or empty folder.",
                dest.display()
            ),
        ));
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_err("create", parent, &e))?;
    }
    let mut name = dest.as_os_str().to_os_string();
    name.push(format!(".partial-{}", std::process::id()));
    let stage = PathBuf::from(name);
    let _ = std::fs::remove_dir_all(&stage);
    let result = (|| {
        std::fs::create_dir(&stage).map_err(|e| io_err("create", &stage, &e))?;
        gt_gedcom::extract_zip_file(zip_file, &stage, DB_FILE).map_err(zip_refusal)?;
        // Intact, ours, and not newer than this app; an older schema is brought up to date.
        let project = Project::open_untrusted(&stage).map_err(store_refusal)?;
        drop(project);
        if dest.exists() {
            std::fs::remove_dir(dest).map_err(|e| io_err("replace", dest, &e))?;
        }
        std::fs::rename(&stage, dest).map_err(|e| io_err("create", dest, &e))
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&stage);
    }
    result
}
