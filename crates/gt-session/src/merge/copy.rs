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
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
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

/// A link of any kind: a symbolic link, or on Windows a junction or other reparse point.
pub(crate) fn is_link_like(m: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    m.file_type().is_symlink()
}

const SCRATCH_PREFIX: &str = "gene-tree-merge-";
/// A scratch folder left behind by a crash is removed once it is this old.
const SCRATCH_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Removes scratch folders of earlier runs (they hold a full, unfiltered snapshot of a tree) from
/// the system temp folder. Only folders named `gene-tree-merge-*` directly in `temp` and older
/// than `older_than` at `now`; a link is never followed; failures are ignored. Returns how many
/// were removed.
pub fn remove_stale_scratch(temp: &Path, older_than: Duration, now: SystemTime) -> usize {
    let Ok(rd) = std::fs::read_dir(temp) else {
        return 0;
    };
    let mut removed = 0;
    for e in rd.filter_map(Result::ok) {
        if !e.file_name().to_string_lossy().starts_with(SCRATCH_PREFIX) {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(e.path()) else {
            continue;
        };
        if is_link_like(&meta) || !meta.is_dir() {
            continue;
        }
        let old = meta
            .modified()
            .ok()
            .and_then(|m| now.duration_since(m).ok())
            .is_some_and(|age| age > older_than);
        if old && std::fs::remove_dir_all(e.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// At app start: removes the scratch folders of runs that did not finish, older than 24 hours.
pub fn remove_stale_scratch_now() -> usize {
    remove_stale_scratch(&std::env::temp_dir(), SCRATCH_MAX_AGE, SystemTime::now())
}

/// A folder in the system temp area that is removed when this goes away.
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new() -> Result<Scratch, ApiError> {
        let dir = std::env::temp_dir().join(format!("{SCRATCH_PREFIX}{}", gt_core::ids::new_id()));
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

/// Refuses a copy that holds more than `limit` records, counted in the database before anything
/// is loaded into memory.
pub(crate) fn refuse_if_huge(project: &Project, limit: usize) -> Result<(), ApiError> {
    let n = project.record_count().map_err(store_refusal)?;
    if n > limit as u64 {
        return Err(refuse(
            "This copy holds far more records than a family tree can, so it was not opened.",
        ));
    }
    Ok(())
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
/// relative to root's parent)`, so entries read `media/ab/....`. Only files some media record
/// points at: a photo of a deleted record stays on disk but is not sent.
fn media_files(root: &Path, in_use: &HashSet<String>) -> Vec<(PathBuf, String)> {
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
    out.retain(|(_, name)| in_use.contains(name));
    out
}

/// The `media/...` paths the tree's media records point at.
fn media_in_use(s: &Session) -> HashSet<String> {
    s.tree
        .media
        .values()
        .map(|m| m.path.trim().to_string())
        .collect()
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
        let files = media_files(&self.project.dir.join("media"), &media_in_use(self));
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
        for (path, name) in media_files(&self.project.dir.join("media"), &media_in_use(self)) {
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn stale_scratch_folders_are_removed_and_nothing_else() {
        let temp = tempfile::tempdir().unwrap();
        let t = temp.path();
        let mk = |name: &str| {
            let d = t.join(name);
            std::fs::create_dir_all(d.join("inner")).unwrap();
            std::fs::write(d.join("inner").join("tree.db"), b"x").unwrap();
            d
        };
        let stale = mk("gene-tree-merge-aaaa");
        let other = mk("some-other-folder");
        let prefixed_file = t.join("gene-tree-merge-file");
        std::fs::write(&prefixed_file, b"x").unwrap();
        // Not older than a day yet: nothing goes.
        let day = Duration::from_secs(24 * 60 * 60);
        assert_eq!(remove_stale_scratch(t, day, SystemTime::now()), 0);
        assert!(stale.exists());
        // Two days later: only the prefixed folder goes.
        let later = SystemTime::now() + 2 * day;
        assert_eq!(remove_stale_scratch(t, day, later), 1);
        assert!(!stale.exists());
        assert!(other.exists(), "a folder with another name stays");
        assert!(prefixed_file.exists(), "a file stays");
        // A missing temp folder is not an error.
        assert_eq!(remove_stale_scratch(&t.join("nope"), day, later), 0);
    }

    #[test]
    fn a_link_with_the_scratch_name_is_not_followed() {
        let temp = tempfile::tempdir().unwrap();
        let t = temp.path();
        let victim = t.join("victim");
        std::fs::create_dir_all(&victim).unwrap();
        std::fs::write(victim.join("keep.txt"), b"x").unwrap();
        let link = t.join("gene-tree-merge-link");
        #[cfg(windows)]
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&victim)
            .output()
            .is_ok_and(|o| o.status.success());
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(&victim, &link).is_ok();
        assert!(made, "the test needs to make a directory link");
        let later = SystemTime::now() + Duration::from_secs(3 * 24 * 60 * 60);
        assert_eq!(
            remove_stale_scratch(t, Duration::from_secs(24 * 60 * 60), later),
            0
        );
        assert!(
            victim.join("keep.txt").exists(),
            "nothing behind the link was touched"
        );
    }

    #[test]
    fn a_copy_over_the_limit_is_refused_by_count_alone() {
        let dir = tempfile::tempdir().unwrap();
        let proj = dir.path().join("p");
        let project = Project::create(&proj).unwrap();
        let conn = project.conn();
        for i in 0..3 {
            conn.execute(
                "INSERT INTO person(id, seq) VALUES (?1, ?2)",
                rusqlite::params![format!("00000000-0000-0000-0000-00000000000{i}"), i],
            )
            .unwrap();
        }
        assert_eq!(project.record_count().unwrap(), 3);
        assert!(refuse_if_huge(&project, 3).is_ok());
        assert!(refuse_if_huge(&project, 2).is_err());
    }
}
