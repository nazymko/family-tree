//! Moving whole trees in and out: GEDCOM import with a report, export behind a privacy preview,
//! relinking missing media, and saving a full copy of the project.
//!
//! Every media path that comes from a file (a GEDCOM, or a path stored in the project) is checked
//! lexically against the folders it may point into before any filesystem call is made on it.

use crate::dto::{DayOrder as WireDayOrder, ProjectInfo};
use crate::error::{ApiError, ErrorCode};
use crate::import_format;
use crate::names::display_name;
use crate::session::{Session, SessionClock};
use gt_core::DayOrder;
use gt_core::GedcomVersion;
use gt_core::living::{LivingRule, compute_living};
use gt_core::model::Media;
use gt_core::privacy::{LivingMode, PrivacyPolicy, PrivacySummary, filter};
use gt_core::tree::Tree;
use gt_gedcom::{ImportOptions, MissingReason, Repair, is_lexically_under};
use gt_store::{ChangeSet, Project, Record, RecordKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use ts_rs::TS;
use uuid::Uuid;

/// Meta key remembering which GEDCOM version the project was imported from (same key as `gt`).
pub const META_GEDCOM_VERSION: &str = "source_gedcom_version";
/// Meta key set on import and checked by `discard_import`.
pub const META_LAST_IMPORT: &str = "last_import_id";
/// Longest list the import report carries; `truncated` says when something was cut.
pub const REPORT_LIST_CAP: usize = 5000;
/// The sentence shown for 5.5.1 exports (same text as the UI mock).
pub const LOSSLESS_551: &str = "Roles, 'did not happen' records, shared notes and external IDs are written as extension tags; other programs may drop them.";
/// Deepest folder level the media search looks into.
pub const RELINK_MAX_DEPTH: usize = 8;
/// Most directory entries the media search visits.
const RELINK_MAX_ENTRIES: usize = 500_000;

// ---------------------------------------------------------------- wire types

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    pub file: String,
    pub project_dir: String,
    pub project_name: String,
    pub media_roots: Vec<String>,
    pub day_order: WireDayOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ImportPhase {
    Reading,
    Parsing,
    CopyingMedia,
    Saving,
    Indexing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub phase: ImportPhase,
    pub done: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MissingReasonDto {
    NotFound,
    OutsideRoots,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MissingMedia {
    pub path: String,
    pub reason: MissingReasonDto,
    /// Files that could be the one meant (reason 'ambiguous').
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RepairDto {
    pub kind: String,
    pub line: Option<u32>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UnparsedDateDto {
    pub line: u32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImportReportDto {
    pub project: ProjectInfo,
    pub encoding: String,
    pub version: Option<String>,
    pub producer: Option<String>,
    pub counts: BTreeMap<String, u32>,
    pub repairs: Vec<RepairDto>,
    pub unmapped_tags: BTreeMap<String, u32>,
    pub unparsed_dates: Vec<UnparsedDateDto>,
    pub missing_media: Vec<MissingMedia>,
    /// Photos that are web links; they are kept as links, not copied.
    pub media_urls: Vec<String>,
    pub truncated: bool,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub enum ExportVersion {
    #[serde(rename = "5.5.1")]
    #[ts(rename = "5.5.1")]
    V551,
    #[serde(rename = "7.0")]
    #[ts(rename = "7.0")]
    V70,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum LivingExport {
    Exclude,
    Placeholder,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    pub version: ExportVersion,
    pub living: LivingExport,
    pub include_private: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MentionWarning {
    pub note_excerpt: String,
    pub hidden_person: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GroupMedia {
    pub title: String,
    pub original_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyPreview {
    pub options: ExportOptions,
    pub default_version: ExportVersion,
    /// People written under their own name. Does NOT include placeholders: people in the file =
    /// exportedPersons + placeholderPersons.
    pub exported_persons: u32,
    pub placeholder_persons: u32,
    pub hidden_persons: u32,
    pub removed_records: u32,
    pub mention_warnings: Vec<MentionWarning>,
    pub group_media: Vec<GroupMedia>,
    pub lossless_notes: Vec<String>,
    /// Identifies this preview; `export_gedcom` accepts only the token of the current state.
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RelinkResult {
    pub relinked: u32,
    pub still_missing: Vec<String>,
    /// The folder held too many entries to search all of it; some files may have been missed.
    pub search_incomplete: bool,
}

// ---------------------------------------------------------------- helpers

fn io_err(what: &str, path: &Path, e: &std::io::Error) -> ApiError {
    ApiError::new(
        ErrorCode::Io,
        format!("Cannot {what} {}: {e}", path.display()),
    )
}

fn count32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn core_day_order(o: WireDayOrder) -> DayOrder {
    match o {
        WireDayOrder::Dmy => DayOrder::Dmy,
        WireDayOrder::Mdy => DayOrder::Mdy,
    }
}

fn is_empty_dir(p: &Path) -> bool {
    std::fs::read_dir(p).is_ok_and(|mut d| d.next().is_none())
}

/// Renames `dir` to a sibling `<dir>.discard-<pid>` (all or nothing; fails while something holds
/// files in it) and deletes that. Never leaves a half-deleted project under the original name.
fn remove_folder_safely(dir: &Path) -> Result<(), ApiError> {
    let mut name = dir.as_os_str().to_os_string();
    name.push(format!(".discard-{}", std::process::id()));
    let aside = PathBuf::from(name);
    let _ = std::fs::remove_dir_all(&aside);
    std::fs::rename(dir, &aside).map_err(|e| io_err("remove", dir, &e))?;
    std::fs::remove_dir_all(&aside).map_err(|e| io_err("delete", &aside, &e))
}

/// Sibling scratch path `<target>.partial-<pid>`.
fn partial_path(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_os_string();
    name.push(format!(".partial-{}", std::process::id()));
    PathBuf::from(name)
}

fn version_from_text(s: &str) -> Option<ExportVersion> {
    let s = s.trim();
    if s.starts_with('7') {
        Some(ExportVersion::V70)
    } else if s.starts_with("5.5") {
        Some(ExportVersion::V551)
    } else {
        None
    }
}

fn repair_dto(r: &Repair) -> RepairDto {
    let dto = |kind: &str, line: Option<u32>, detail: String| RepairDto {
        kind: kind.into(),
        line,
        detail,
    };
    match r {
        Repair::DanglingPointer { line, xref } => dto(
            "dangling_pointer",
            Some(*line),
            format!("{xref} points to a record that is not in the file"),
        ),
        Repair::DuplicateXref { line, xref } => dto(
            "duplicate_xref",
            Some(*line),
            format!("{xref} is used by more than one record"),
        ),
        Repair::OneWayLink { xref_from, xref_to } => dto(
            "one_way_link",
            None,
            format!("{xref_from} links to {xref_to}, which does not link back"),
        ),
        Repair::MissingHeader => dto("missing_header", None, "The file has no header".into()),
        Repair::MissingTrailer => dto("missing_trailer", None, "The file has no trailer".into()),
        Repair::Token(t) => match t {
            gt_gedcom::TokenIssue::BadLine { line, text } => {
                dto("bad_line", Some(*line), format!("Unreadable line: {text}"))
            }
            gt_gedcom::TokenIssue::LevelJump { line } => dto(
                "level_jump",
                Some(*line),
                "The line skips a nesting level".into(),
            ),
            gt_gedcom::TokenIssue::OrphanConc { line } => dto(
                "orphan_conc",
                Some(*line),
                "A continuation line has nothing to continue".into(),
            ),
        },
    }
}

fn capped<T, U>(items: &[T], truncated: &mut bool, f: impl Fn(&T) -> U) -> Vec<U> {
    if items.len() > REPORT_LIST_CAP {
        *truncated = true;
    }
    items.iter().take(REPORT_LIST_CAP).map(f).collect()
}

fn report_dto(
    rep: &gt_gedcom::ImportReport,
    project: ProjectInfo,
    extra_missing: &[MissingMedia],
) -> ImportReportDto {
    let mut truncated = false;
    let mut missing: Vec<MissingMedia> = rep
        .missing_media
        .iter()
        .map(|m| MissingMedia {
            path: m.path.clone(),
            reason: match m.reason {
                MissingReason::NotFound => MissingReasonDto::NotFound,
                MissingReason::OutsideRoots => MissingReasonDto::OutsideRoots,
                MissingReason::Ambiguous => MissingReasonDto::Ambiguous,
            },
            candidates: m
                .candidates
                .iter()
                .map(|c| c.to_string_lossy().into_owned())
                .collect(),
        })
        .collect();
    missing.extend(extra_missing.iter().cloned());
    let missing_media = capped(&missing, &mut truncated, Clone::clone);
    ImportReportDto {
        project,
        encoding: rep.encoding.clone(),
        version: rep.version.clone(),
        producer: rep.producer.clone(),
        counts: rep
            .counts
            .iter()
            .map(|(k, v)| (k.clone(), count32(*v)))
            .collect(),
        repairs: capped(&rep.repairs, &mut truncated, repair_dto),
        unmapped_tags: rep
            .unmapped_tags
            .iter()
            .map(|(k, v)| (k.clone(), count32(*v)))
            .collect(),
        unparsed_dates: capped(&rep.unparsed_dates, &mut truncated, |(l, t)| {
            UnparsedDateDto {
                line: *l,
                text: t.clone(),
            }
        }),
        missing_media,
        media_urls: capped(&rep.media_urls, &mut truncated, Clone::clone),
        truncated,
        text: rep.to_text(),
    }
}

/// FNV-1a over `bytes`; stable across runs, which `DefaultHasher` does not promise.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

// ---------------------------------------------------------------- import

/// Imports a GEDCOM file into a NEW project folder and opens it.
///
/// The project is built in a sibling scratch folder and renamed into place, so a failure leaves
/// nothing behind. An existing project, or a folder that already has files in it, is a `Conflict`.
pub fn import_gedcom(
    req: &ImportRequest,
    clock: SessionClock,
    progress: &dyn Fn(ImportProgress),
) -> Result<(Session, ImportReportDto), ApiError> {
    import_gedcom_with_opener(req, clock, progress, &|dir, clock| {
        Session::open(dir, clock)
    })
}

/// `import_gedcom` with the final "open the new project" step injectable (tests make it fail).
/// When opening fails the new folder is removed again, so a retry does not hit a `Conflict`.
#[doc(hidden)]
pub fn import_gedcom_with_opener(
    req: &ImportRequest,
    clock: SessionClock,
    progress: &dyn Fn(ImportProgress),
    open: &dyn Fn(&Path, SessionClock) -> Result<Session, ApiError>,
) -> Result<(Session, ImportReportDto), ApiError> {
    let emit = |phase, done, total| {
        progress(ImportProgress { phase, done, total });
    };
    let dir = PathBuf::from(&req.project_dir);
    if dir.join("tree.db").exists() || (dir.exists() && !is_empty_dir(&dir)) {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            format!(
                "The folder {} already has files in it. Choose a new or empty folder for the imported tree.",
                dir.display()
            ),
        ));
    }
    let file = PathBuf::from(&req.file);
    emit(ImportPhase::Reading, 0, 1);
    let format = import_format::detect(&file)?;
    emit(ImportPhase::Reading, 1, 1);

    emit(ImportPhase::Parsing, 0, 1);
    let gedcom_dir = file
        .parent()
        .map(Path::to_path_buf)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."));
    let media_roots: Vec<PathBuf> = req.media_roots.iter().map(PathBuf::from).collect();
    let mut allowed = media_roots.clone();
    allowed.push(gedcom_dir.clone());
    let converted = import_format::convert(
        format,
        &file,
        &ImportOptions {
            day_order: core_day_order(req.day_order),
            media_roots,
            gedcom_dir: Some(gedcom_dir),
        },
    )?;
    allowed.extend(converted.extra_media_roots);
    let cleanup = converted.cleanup;
    let mut outcome = converted.outcome;
    emit(ImportPhase::Parsing, 1, 1);

    let (today, _) = (clock.0)();
    let tmp = partial_path(&dir);
    if tmp.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    let mut extra_missing: Vec<MissingMedia> = Vec::new();
    let built = build_imported(
        &tmp,
        req,
        &mut outcome,
        &allowed,
        today,
        &mut extra_missing,
        &emit,
    );
    if let Some(scratch) = &cleanup {
        let _ = std::fs::remove_dir_all(scratch);
    }
    if let Err(e) = built.and_then(|()| {
        if dir.exists() {
            // An empty folder is a valid target; make room for the rename.
            std::fs::remove_dir(&dir).map_err(|e| io_err("replace", &dir, &e))?;
        }
        std::fs::rename(&tmp, &dir).map_err(|e| io_err("create", &dir, &e))
    }) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }

    emit(ImportPhase::Indexing, 0, 1);
    let session = match open(&dir, clock) {
        Ok(s) => s,
        Err(e) => {
            let _ = remove_folder_safely(&dir);
            return Err(e);
        }
    };
    emit(ImportPhase::Indexing, 1, 1);
    let dto = report_dto(&outcome.report, session.info(), &extra_missing);
    Ok((session, dto))
}

/// Creates the project in `tmp`, copies the media, saves the tree and the import meta.
fn build_imported(
    tmp: &Path,
    req: &ImportRequest,
    outcome: &mut gt_gedcom::ImportOutcome,
    allowed: &[PathBuf],
    today: i32,
    extra_missing: &mut Vec<MissingMedia>,
    emit: &dyn Fn(ImportPhase, u32, u32),
) -> Result<(), ApiError> {
    let mut project = Project::create(tmp)?;
    let total = count32(outcome.resolved_media.len());
    emit(ImportPhase::CopyingMedia, 0, total);
    let resolved = std::mem::take(&mut outcome.resolved_media);
    for (done, (id, src)) in (1u32..).zip(resolved.iter()) {
        // The importer already checked this; a second lexical check keeps the invariant local.
        let copied = if is_lexically_under(src, allowed) {
            project.add_media_file(src).ok()
        } else {
            None
        };
        match (copied, outcome.tree.media.get_mut(id)) {
            (Some((rel, sha)), Some(m)) => {
                m.path = rel;
                m.sha256 = Some(sha);
            }
            (None, Some(m)) => extra_missing.push(MissingMedia {
                path: m.path.clone(),
                reason: MissingReasonDto::NotFound,
                candidates: vec![],
            }),
            _ => {}
        }
        emit(ImportPhase::CopyingMedia, done, total);
    }
    outcome.resolved_media = resolved;

    emit(ImportPhase::Saving, 0, 1);
    project.save_tree(&outcome.tree, today)?;
    if let Some(v) = &outcome.report.version {
        project.meta_set(META_GEDCOM_VERSION, v)?;
    }
    if !req.project_name.trim().is_empty() {
        project.meta_set("project_name", req.project_name.trim())?;
    }
    project.meta_set(META_LAST_IMPORT, &gt_core::ids::new_id().to_string())?;
    drop(project);
    emit(ImportPhase::Saving, 1, 1);
    Ok(())
}

impl Session {
    /// Deletes a project folder that was just imported and never edited. A project with a change
    /// log (or one that was not created by an import) is a `Conflict`. Close the session first.
    pub fn discard_import(dir: &Path) -> Result<(), ApiError> {
        let project = Project::open(dir)?;
        let imported = project
            .meta_get(META_LAST_IMPORT)?
            .is_some_and(|v| !v.trim().is_empty());
        if !imported || !project.change_log_is_empty()? {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "This project has been edited since the import, so it cannot be discarded. Close it and delete the folder yourself if you are sure.",
            ));
        }
        drop(project);
        remove_folder_safely(dir)
    }

    /// Closes this session, then discards its project as `discard_import` does. The database
    /// handle is released first, so the folder can be removed on Windows.
    pub fn discard(self) -> Result<(), ApiError> {
        let dir = self.project.dir.clone();
        drop(self);
        Session::discard_import(&dir)
    }
}

// ---------------------------------------------------------------- relink

/// `true` for web addresses and data URLs, which are kept as links and never searched for.
fn is_remote(path: &str) -> bool {
    let p = path.trim_start();
    if p.len() >= 5 && p[..5].eq_ignore_ascii_case("data:") {
        return true;
    }
    match p.find("://") {
        Some(i) => {
            let scheme = &p[..i];
            scheme.len() > 1
                && !scheme.eq_ignore_ascii_case("file")
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
        }
        None => false,
    }
}

fn basename_lower(path: &str) -> String {
    path.rsplit('/').next().unwrap_or("").to_lowercase()
}

/// Every regular file under `root` (depth <= `RELINK_MAX_DEPTH`), by lower-case file name.
/// Symlinks are skipped, so the search cannot leave `root`.
fn index_folder(root: &Path, incomplete: &mut bool) -> HashMap<String, Vec<PathBuf>> {
    let mut out: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    let mut seen = 0usize;
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            seen += 1;
            if seen > RELINK_MAX_ENTRIES {
                *incomplete = true;
                return out;
            }
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                if depth < RELINK_MAX_DEPTH {
                    stack.push((e.path(), depth + 1));
                }
            } else if ft.is_file() {
                let name = e.file_name().to_string_lossy().to_lowercase();
                out.entry(name).or_default().push(e.path());
            }
        }
    }
    out
}

/// A file that resolves (symlinks followed) to a place under `canon_root`.
fn confirmed_under(candidate: &Path, canon_root: &Path) -> bool {
    candidate
        .canonicalize()
        .is_ok_and(|c| c.is_file() && c.starts_with(canon_root))
}

impl Session {
    /// Looks in `folder` for the files of media that were not found at import (no content hash
    /// stored): the stored path read as relative to `folder`, else a unique file of the same name
    /// anywhere below it (depth <= 8, symlinks not followed). Found files are copied into the
    /// project; the whole change is one undoable step "Link media files".
    pub fn relink_media(&mut self, folder: &Path) -> Result<RelinkResult, ApiError> {
        if !folder.is_dir() {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                format!("{} is not a folder.", folder.display()),
            ));
        }
        let canon_root = folder
            .canonicalize()
            .map_err(|e| io_err("open", folder, &e))?;
        let roots = [folder.to_path_buf()];
        let mut index: Option<HashMap<String, Vec<PathBuf>>> = None;
        let mut search_incomplete = false;
        let (_, now) = (self.clock.0)();
        let mut changes = Vec::new();
        let mut still_missing: Vec<String> = Vec::new();
        let candidates: Vec<Media> = self
            .tree
            .media
            .values()
            .filter(|m| m.sha256.is_none() && !is_remote(&m.path))
            .cloned()
            .collect();
        for m in candidates {
            let stored = m.path.replace('\\', "/");
            let mut found: Option<PathBuf> = None;
            // 1. The stored path under the chosen folder. Only a plain relative path qualifies,
            //    and it is checked lexically before the disk is touched.
            let rel = Path::new(&stored);
            if !stored.is_empty()
                && !rel.has_root()
                && rel
                    .components()
                    .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
            {
                let cand = folder.join(rel);
                if is_lexically_under(&cand, &roots) && confirmed_under(&cand, &canon_root) {
                    found = Some(cand);
                }
            }
            // 2. A unique file name match below the folder.
            if found.is_none() {
                let name = basename_lower(&stored);
                if !name.is_empty() {
                    let idx =
                        index.get_or_insert_with(|| index_folder(folder, &mut search_incomplete));
                    if let Some([one]) = idx.get(&name).map(Vec::as_slice)
                        && confirmed_under(one, &canon_root)
                    {
                        found = Some(one.clone());
                    }
                }
            }
            let stored_result = found.and_then(|f| {
                self.project
                    .add_media_file(&f)
                    .ok()
                    .map(|(rel, sha)| (f, rel, sha))
            });
            match stored_result {
                Some((src, rel, sha)) => {
                    let mut after = m.clone();
                    after.path = rel;
                    after.sha256 = Some(sha);
                    if after.original_name.is_none() {
                        after.original_name = src.file_name().map(|n| n.to_string_lossy().into());
                    }
                    after.common.changed = Some(now);
                    changes.push(gt_store::Change {
                        kind: RecordKind::Media,
                        id: m.common.id,
                        before: Some(Record::Media(m)),
                        after: Some(Record::Media(after)),
                    });
                }
                None => {
                    if !still_missing.contains(&m.path) {
                        still_missing.push(m.path.clone());
                    }
                }
            }
        }
        let relinked = count32(changes.len());
        if !changes.is_empty() {
            self.apply_planned(ChangeSet {
                group: gt_core::ids::new_id(),
                reverts: None,
                label: "Link media files".into(),
                ts: now,
                changes,
            })?;
        }
        Ok(RelinkResult {
            relinked,
            still_missing,
            search_incomplete,
        })
    }
}

// ---------------------------------------------------------------- export

fn mode_of(l: LivingExport) -> LivingMode {
    match l {
        LivingExport::Exclude => LivingMode::Exclude,
        LivingExport::Placeholder => LivingMode::Placeholder,
        LivingExport::All => LivingMode::IncludeAll,
    }
}

fn core_version(v: ExportVersion) -> GedcomVersion {
    match v {
        ExportVersion::V551 => GedcomVersion::V551,
        ExportVersion::V70 => GedcomVersion::V70,
    }
}

impl Session {
    /// The version recorded at import, else the header's, else 5.5.1 (as `gt export` does).
    pub fn default_export_version(&self) -> ExportVersion {
        self.project
            .meta_get(META_GEDCOM_VERSION)
            .ok()
            .flatten()
            .as_deref()
            .and_then(version_from_text)
            .or_else(|| {
                self.tree
                    .header
                    .source_version
                    .as_deref()
                    .and_then(version_from_text)
            })
            .unwrap_or(ExportVersion::V551)
    }

    fn privacy_policy(&self, opts: &ExportOptions) -> PrivacyPolicy {
        PrivacyPolicy {
            living: mode_of(opts.living),
            include_private: opts.include_private,
            rule: LivingRule {
                max_age_years: self.settings.living_max_age,
            },
        }
    }

    fn export_token(&self, opts: &ExportOptions) -> String {
        let key = format!("{opts:?}|{}", self.revision());
        format!("pv-{:016x}-{}", fnv1a(key.as_bytes()), self.revision())
    }

    fn filtered_for_export(&self, opts: &ExportOptions) -> (Tree, PrivacySummary) {
        let (today, _) = (self.clock.0)();
        filter(&self.tree, &self.privacy_policy(opts), today)
    }

    /// What an export with `opts` would contain, without writing anything.
    pub fn preview_export(&self, opts: &ExportOptions) -> Result<PrivacyPreview, ApiError> {
        let policy = self.privacy_policy(opts);
        let (filtered, summary) = self.filtered_for_export(opts);
        // Placeholders are the living people that survived the filter in placeholder mode.
        let placeholders: HashSet<Uuid> = if opts.living == LivingExport::Placeholder {
            let (today, _) = (self.clock.0)();
            compute_living(&self.tree, &policy.rule, today)
                .into_iter()
                .filter(|(_, l)| *l)
                .map(|(id, _)| id)
                .collect()
        } else {
            HashSet::new()
        };
        let in_file_placeholders = filtered
            .persons
            .keys()
            .filter(|id| placeholders.contains(id))
            .count();
        let exported = filtered.persons.len() - in_file_placeholders;

        let order = self.settings.name_order;
        let mention_warnings = summary
            .mention_warnings
            .iter()
            .filter_map(|(note, person)| {
                let n = self.tree.notes.get(note)?;
                let p = self.tree.persons.get(person)?;
                Some(MentionWarning {
                    note_excerpt: n.text.chars().take(80).collect(),
                    hidden_person: display_name(p, order),
                })
            })
            .collect();
        let group_media = summary
            .group_media
            .iter()
            .filter_map(|id| self.tree.media.get(id))
            .map(|m| GroupMedia {
                title: m
                    .title
                    .clone()
                    .or_else(|| m.original_name.clone())
                    .unwrap_or_else(|| "Untitled file".into()),
                original_name: m.original_name.clone(),
            })
            .collect();
        Ok(PrivacyPreview {
            options: *opts,
            default_version: self.default_export_version(),
            exported_persons: count32(exported),
            placeholder_persons: count32(in_file_placeholders),
            hidden_persons: count32(summary.hidden_persons),
            removed_records: count32(summary.removed_records),
            mention_warnings,
            group_media,
            lossless_notes: if opts.version == ExportVersion::V551 {
                vec![LOSSLESS_551.to_string()]
            } else {
                vec![]
            },
            token: self.export_token(opts),
        })
    }

    /// Writes the filtered tree to `dest`. `token` must be the one `preview_export` returned for
    /// the same options and the current tree (else `Stale`). An existing `dest` is a `Conflict`;
    /// use `export_gedcom_over` after the user confirmed replacing it. Returns the byte count.
    pub fn export_gedcom(
        &self,
        opts: &ExportOptions,
        dest: &Path,
        token: &str,
    ) -> Result<u64, ApiError> {
        self.export_gedcom_over(opts, dest, token, false)
    }

    /// As `export_gedcom`, replacing an existing `dest` when `overwrite` is set.
    pub fn export_gedcom_over(
        &self,
        opts: &ExportOptions,
        dest: &Path,
        token: &str,
        overwrite: bool,
    ) -> Result<u64, ApiError> {
        if token != self.export_token(opts) {
            return Err(ApiError::new(
                ErrorCode::Stale,
                "The preview no longer matches your choices or the tree. Preview again, then export.",
            ));
        }
        if !overwrite && dest.exists() {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                format!(
                    "{} already exists. Choose another name, or confirm that it may be replaced.",
                    dest.display()
                ),
            ));
        }
        let (filtered, _) = self.filtered_for_export(opts);
        let bytes = gt_gedcom::export(
            &filtered,
            &gt_gedcom::ExportOptions {
                version: core_version(opts.version),
            },
        );
        let mut tmp_name = dest.as_os_str().to_os_string();
        tmp_name.push(".tmp");
        let tmp = PathBuf::from(tmp_name);
        let written = (|| {
            let mut open = std::fs::OpenOptions::new();
            open.write(true).create_new(true);
            // A leftover from an interrupted export is ours to replace; anything else is not.
            let mut f = match open.open(&tmp) {
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    std::fs::remove_file(&tmp)?;
                    open.open(&tmp)?
                }
                other => other?,
            };
            f.write_all(&bytes)?;
            f.sync_all()?;
            drop(f);
            std::fs::rename(&tmp, dest)
        })();
        if let Err(e) = written {
            let _ = std::fs::remove_file(&tmp);
            return Err(io_err("write", dest, &e));
        }
        Ok(bytes.len() as u64)
    }
}

// ---------------------------------------------------------------- save copy

/// Whether `dest` is (or would be) inside `project`, compared after resolving the parts that
/// exist (symlinks, `..`) and, on Windows, ignoring case.
fn inside_project(project: &Path, dest: &Path) -> bool {
    let canon = |p: &Path| -> PathBuf {
        // Resolve the longest existing ancestor, then re-append the rest.
        let mut rest = Vec::new();
        let mut cur = p.to_path_buf();
        loop {
            if let Ok(c) = cur.canonicalize() {
                return rest.iter().rev().fold(c, |acc, n| acc.join(n));
            }
            match (cur.file_name().map(|n| n.to_owned()), cur.parent()) {
                (Some(n), Some(parent)) => {
                    rest.push(n);
                    cur = parent.to_path_buf();
                }
                _ => return p.to_path_buf(),
            }
        }
    };
    let fold = |p: PathBuf| -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(p.to_string_lossy().to_lowercase())
        } else {
            p
        }
    };
    fold(canon(dest)).starts_with(fold(canon(project)))
}

fn copy_media_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let ft = e.file_type()?;
        let name = e.file_name();
        if ft.is_symlink() || name.to_string_lossy().starts_with(".part-") {
            continue;
        }
        if ft.is_dir() {
            copy_media_tree(&e.path(), &dst.join(&name))?;
        } else if ft.is_file() {
            std::fs::copy(e.path(), dst.join(&name))?;
        }
    }
    Ok(())
}

impl Session {
    /// A full, unfiltered copy of the project in a new folder `dest`: a consistent snapshot of
    /// `tree.db` (as a backup makes it) plus every media file. `dest` must not exist, or be an
    /// empty folder, and must not be inside this project. The copy is not an import: it cannot
    /// be discarded as one.
    pub fn save_copy(&self, dest: &Path) -> Result<(), ApiError> {
        self.save_copy_hooked(dest, &|_| Ok(()))
    }

    /// `save_copy` with a hook run in the scratch folder after the database is copied (tests make
    /// it fail to prove a failed copy leaves nothing behind).
    #[doc(hidden)]
    pub fn save_copy_hooked(
        &self,
        dest: &Path,
        hook: &dyn Fn(&Path) -> std::io::Result<()>,
    ) -> Result<(), ApiError> {
        if inside_project(&self.project.dir, dest) {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "Choose a folder outside the project for the copy.",
            ));
        }
        if dest.exists() && !(dest.is_dir() && is_empty_dir(dest)) {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                format!(
                    "The folder {} already has files in it. Choose a new or empty folder for the copy.",
                    dest.display()
                ),
            ));
        }
        let tmp = partial_path(dest);
        let _ = std::fs::remove_dir_all(&tmp);
        let result = self.build_copy(&tmp, hook).and_then(|()| {
            if dest.exists() {
                std::fs::remove_dir(dest).map_err(|e| io_err("replace", dest, &e))?;
            }
            std::fs::rename(&tmp, dest).map_err(|e| io_err("create", dest, &e))
        });
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&tmp);
        }
        result
    }

    fn build_copy(
        &self,
        tmp: &Path,
        hook: &dyn Fn(&Path) -> std::io::Result<()>,
    ) -> Result<(), ApiError> {
        for sub in ["media", "thumbs", "backups"] {
            let p = tmp.join(sub);
            std::fs::create_dir_all(&p).map_err(|e| io_err("create", &p, &e))?;
        }
        let db = tmp.join("tree.db");
        let target = db.to_str().ok_or_else(|| {
            ApiError::new(ErrorCode::Invalid, "The folder name is not valid text.")
        })?;
        self.project
            .conn()
            .execute("VACUUM INTO ?1", [target])
            .map_err(|e| ApiError::new(ErrorCode::Io, format!("Cannot copy the tree: {e}")))?;
        hook(tmp).map_err(|e| io_err("copy to", tmp, &e))?;
        let media = self.project.dir.join("media");
        if media.is_dir() {
            copy_media_tree(&media, &tmp.join("media")).map_err(|e| io_err("copy", &media, &e))?;
        }
        let mut copy = Project::open(tmp)?;
        if copy
            .meta_get(META_LAST_IMPORT)?
            .is_some_and(|v| !v.is_empty())
        {
            copy.meta_set(META_LAST_IMPORT, "")?;
        }
        Ok(())
    }
}
