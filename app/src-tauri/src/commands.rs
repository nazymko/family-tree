//! One Tauri command per `Api` method. Commands stay thin: lock, call the session, map errors.
//! A lock is never held across an await; the heavy ones run inside `spawn_blocking`.

use crate::settings_file;
use crate::state::{AppState, no_project};
use gt_session::{
    ApiError, AttachResult, ChartLocation, ChartMeta, ChartRequest, ChartWindow, CitationDto,
    DatePreview, DayOrder, EditCommand, EditResult, ErrorCode, ExifSuggestion, ExportOptions,
    FamilyDto, FamilyViewDto, HistoryEntry, ImportProgress, ImportReportDto, ImportRequest,
    LocationKind, LocationWarningDto, OutlineItem, OwnerRef, Page, PersonDetail, PersonQuery,
    PersonRow, PlaceOption, PrivacyPreview, ProjectInfo, RecentProject, Rect, RelinkResult,
    SearchHitDto, Session, Settings, SettingsPatch, SourceDto, TraceDraftState, TraceImageInfo,
    UndoState, WholeMeta,
};
use gt_store::{BackupReason, LocationWarning, Project};
use serde::{Deserialize, Deserializer, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

/// The event `import_gedcom` emits while it works.
pub const IMPORT_PROGRESS_EVENT: &str = "import-progress";
/// A daily backup is taken on open when the newest one is older than this.
const BACKUP_INTERVAL: Duration = Duration::from_secs(24 * 3600);

/// Registers every command and lists its name, from ONE list, so the two cannot drift apart.
macro_rules! command_list {
    ($($name:ident),+ $(,)?) => {
        /// Names of all commands in the handler, in snake_case.
        pub const COMMAND_NAMES: &[&str] = &[$(stringify!($name)),+];

        /// The invoke handler for `Builder::invoke_handler`.
        pub fn handler<R: Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
            tauri::generate_handler![$($name),+]
        }
    };
}

command_list!(
    recent_projects,
    create_project,
    open_project,
    close_project,
    save_copy,
    check_location,
    import_gedcom,
    save_import_report,
    discard_import,
    relink_media,
    list_persons,
    get_person,
    get_family_view,
    get_family,
    search,
    apply,
    undo,
    redo,
    undo_state,
    history,
    parse_date,
    suggest_places,
    suggest_sources,
    last_citation,
    chart_open,
    chart_open_whole,
    chart_lineage,
    chart_locate,
    chart_window,
    chart_outline,
    chart_close,
    exif_suggestion,
    attach_files,
    stage_trace_image,
    save_trace_draft,
    load_trace_draft,
    discard_trace_draft,
    ai_key_status,
    ai_remove_key,
    ai_read_picture,
    ai_cancel_read,
    open_media_externally,
    preview_export,
    export_gedcom,
    get_settings,
    set_settings,
);

type ApiResult<T> = Result<T, ApiError>;

fn internal(msg: impl std::fmt::Display) -> ApiError {
    ApiError::new(ErrorCode::Internal, msg.to_string())
}

/// Runs `f` on the blocking pool with the app state; the lock lives inside `f` only.
async fn blocking<R, T, F>(app: &AppHandle<R>, f: F) -> ApiResult<T>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(&AppState) -> ApiResult<T> + Send + 'static,
{
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        f(&state)
    })
    .await
    .map_err(|e| internal(format!("A background task failed: {e}")))?
}

// ------------------------------------------------------------ project life cycle

fn app_patch(state: &AppState) -> SettingsPatch {
    let s = AppState::lock(&state.settings).clone();
    SettingsPatch {
        day_order: Some(s.day_order),
        name_order: Some(s.name_order),
        theme: Some(s.theme),
        language: Some(s.language),
        backups_to_keep: Some(s.backups_to_keep),
        ..SettingsPatch::default()
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(0))
        .unwrap_or(0)
}

/// Newest `backups/tree-*.db` file time, if any.
fn newest_backup(dir: &Path) -> Option<SystemTime> {
    std::fs::read_dir(dir.join("backups"))
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.starts_with("tree-") && n.ends_with(".db")
        })
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()
}

/// Takes the daily backup when the newest is older than 24 hours. A failure is logged, not
/// raised: the project must still open.
fn daily_backup(session: &Session, keep: u32) {
    let due = match newest_backup(&session.project().dir) {
        None => true,
        Some(t) => SystemTime::now()
            .duration_since(t)
            .map(|age| age > BACKUP_INTERVAL)
            .unwrap_or(false),
    };
    if due
        && let Err(e) = session
            .project()
            .backup(BackupReason::DailyOpen, keep as usize)
    {
        eprintln!("gene-tree: daily backup failed: {e}");
    }
}

/// Makes `session` the open project: app settings applied, recents updated.
fn install(state: &AppState, mut session: Session) -> ApiResult<ProjectInfo> {
    session.set_settings(app_patch(state))?;
    let info = session.info();
    // A recent-list write failure must not block opening.
    if let Err(e) =
        settings_file::touch_recent(&state.config_dir, &info.dir, &info.name, now_secs())
    {
        eprintln!("gene-tree: could not update the recent list: {}", e.message);
    }
    let old = AppState::lock(&state.session).replace(session);
    drop(old);
    Ok(info)
}

pub fn open_project_impl(state: &AppState, dir: &str) -> ApiResult<ProjectInfo> {
    // Release the open project first: its lock would refuse reopening the same tree.
    close_project_impl(state);
    let session = Session::open(Path::new(dir), (state.clock)())?;
    let keep = AppState::lock(&state.settings).backups_to_keep;
    daily_backup(&session, keep);
    install(state, session)
}

pub fn create_project_impl(state: &AppState, dir: &str, name: &str) -> ApiResult<ProjectInfo> {
    close_project_impl(state);
    let mut project = Project::create(Path::new(dir))?;
    project.meta_set("project_name", name.trim())?;
    drop(project);
    let session = Session::open(Path::new(dir), (state.clock)())?;
    install(state, session)
}

pub fn close_project_impl(state: &AppState) {
    let old = AppState::lock(&state.session).take();
    drop(old);
}

pub fn import_gedcom_impl(
    state: &AppState,
    req: &ImportRequest,
    progress: &dyn Fn(ImportProgress),
) -> ApiResult<ImportReportDto> {
    // Release the open project first (Windows holds its files), as a new one replaces it.
    close_project_impl(state);
    let (session, report) = gt_session::import_gedcom(req, (state.clock)(), progress)?;
    install(state, session)?;
    *AppState::lock(&state.last_import) = Some(report.clone());
    Ok(report)
}

pub fn discard_import_impl(state: &AppState) -> ApiResult<()> {
    let dir = AppState::lock(&state.last_import)
        .as_ref()
        .map(|r| r.project.dir.clone())
        .ok_or_else(|| ApiError::new(ErrorCode::Invalid, "There is no import to discard."))?;
    close_project_impl(state);
    Session::discard_import(Path::new(&dir))?;
    *AppState::lock(&state.last_import) = None;
    let _ = settings_file::remove_recent(&state.config_dir, &dir);
    Ok(())
}

pub fn location_warning(dir: &Path) -> Option<LocationWarningDto> {
    gt_store::check_location(dir).map(|w| match w {
        LocationWarning::SyncedFolder(p) => LocationWarningDto {
            kind: LocationKind::SyncedFolder,
            provider: Some(p.to_string()),
        },
        LocationWarning::NetworkShare => LocationWarningDto {
            kind: LocationKind::NetworkShare,
            provider: None,
        },
    })
}

#[tauri::command(async)]
pub fn recent_projects(state: State<'_, AppState>) -> ApiResult<Vec<RecentProject>> {
    Ok(settings_file::load_recent(&state.config_dir))
}

#[tauri::command]
pub async fn create_project<R: Runtime>(
    app: AppHandle<R>,
    dir: String,
    name: String,
) -> ApiResult<ProjectInfo> {
    blocking(&app, move |s| create_project_impl(s, &dir, &name)).await
}

#[tauri::command]
pub async fn open_project<R: Runtime>(app: AppHandle<R>, dir: String) -> ApiResult<ProjectInfo> {
    blocking(&app, move |s| open_project_impl(s, &dir)).await
}

#[tauri::command(async)]
pub fn close_project(state: State<'_, AppState>) -> ApiResult<()> {
    close_project_impl(&state);
    Ok(())
}

#[tauri::command]
pub async fn save_copy<R: Runtime>(app: AppHandle<R>, dest: String) -> ApiResult<()> {
    blocking(&app, move |s| {
        s.with_session(|se| se.save_copy(Path::new(&dest)))
    })
    .await
}

#[tauri::command(async)]
pub fn check_location(dir: String) -> ApiResult<Option<LocationWarningDto>> {
    Ok(location_warning(Path::new(&dir)))
}

#[tauri::command]
pub async fn import_gedcom<R: Runtime>(
    app: AppHandle<R>,
    req: ImportRequest,
) -> ApiResult<ImportReportDto> {
    let emitter = app.clone();
    blocking(&app, move |s| {
        import_gedcom_impl(s, &req, &|p| {
            let _ = emitter.emit(IMPORT_PROGRESS_EVENT, p);
        })
    })
    .await
}

#[tauri::command(async)]
pub fn save_import_report(state: State<'_, AppState>, dest: String) -> ApiResult<()> {
    let text = AppState::lock(&state.last_import)
        .as_ref()
        .map(|r| r.text.clone())
        .ok_or_else(|| ApiError::new(ErrorCode::Invalid, "There is no import report to save."))?;
    settings_file::write_atomic(Path::new(&dest), text.as_bytes())
        .map_err(|e| ApiError::new(ErrorCode::Io, format!("Could not save the report: {e}")))
}

#[tauri::command(async)]
pub fn discard_import(state: State<'_, AppState>) -> ApiResult<()> {
    discard_import_impl(&state)
}

#[tauri::command]
pub async fn relink_media<R: Runtime>(
    app: AppHandle<R>,
    folder: String,
) -> ApiResult<RelinkResult> {
    blocking(&app, move |s| {
        s.with_session(|se| se.relink_media(Path::new(&folder)))
    })
    .await
}

// ------------------------------------------------------------ read side

#[tauri::command(async)]
pub fn list_persons(
    state: State<'_, AppState>,
    q: PersonQuery,
    offset: usize,
    limit: usize,
) -> ApiResult<Page<PersonRow>> {
    state.with_session(|s| Ok(s.list_persons(&q, offset, limit)))
}

#[tauri::command(async)]
pub fn get_person(state: State<'_, AppState>, id: Uuid) -> ApiResult<PersonDetail> {
    state.with_session(|s| s.get_person(id))
}

#[tauri::command(async)]
pub fn get_family_view(state: State<'_, AppState>, id: Uuid) -> ApiResult<FamilyViewDto> {
    state.with_session(|s| s.family_view(id))
}

#[tauri::command(async)]
pub fn get_family(state: State<'_, AppState>, id: Uuid) -> ApiResult<FamilyDto> {
    state.with_session(|s| s.get_family(id))
}

#[tauri::command(async)]
pub fn search(
    state: State<'_, AppState>,
    query: String,
    limit: usize,
) -> ApiResult<Vec<SearchHitDto>> {
    state.with_session(|s| s.search(&query, limit))
}

// ------------------------------------------------------------ edits

#[tauri::command(async)]
pub fn apply(state: State<'_, AppState>, cmd: EditCommand) -> ApiResult<EditResult> {
    state.with_session(|s| s.apply(cmd))
}

#[tauri::command(async)]
pub fn undo(state: State<'_, AppState>) -> ApiResult<Option<EditResult>> {
    state.with_session(|s| s.undo())
}

#[tauri::command(async)]
pub fn redo(state: State<'_, AppState>) -> ApiResult<Option<EditResult>> {
    state.with_session(|s| s.redo())
}

#[tauri::command(async)]
pub fn undo_state(state: State<'_, AppState>) -> ApiResult<UndoState> {
    state.with_session(|s| Ok(s.undo_state()))
}

#[tauri::command(async)]
pub fn history(
    state: State<'_, AppState>,
    record: Uuid,
    limit: usize,
) -> ApiResult<Vec<HistoryEntry>> {
    state.with_session(|s| s.history(record, limit))
}

// ------------------------------------------------------------ lookups

#[tauri::command(async)]
pub fn parse_date(input: String, order: DayOrder) -> ApiResult<DatePreview> {
    Ok(gt_session::parse_date_preview(&input, order))
}

#[tauri::command(async)]
pub fn suggest_places(
    state: State<'_, AppState>,
    prefix: String,
    limit: usize,
) -> ApiResult<Vec<PlaceOption>> {
    state.with_session(|s| Ok(s.suggest_places(&prefix, limit)))
}

#[tauri::command(async)]
pub fn suggest_sources(
    state: State<'_, AppState>,
    prefix: String,
    limit: usize,
) -> ApiResult<Vec<SourceDto>> {
    state.with_session(|s| Ok(s.suggest_sources(&prefix, limit)))
}

#[tauri::command(async)]
pub fn last_citation(state: State<'_, AppState>) -> ApiResult<Option<CitationDto>> {
    state.with_session(|s| Ok(s.last_citation()))
}

// ------------------------------------------------------------ charts

// NOTE: `chart_open`, `attach_files` and `export_gedcom` hold the session mutex for the whole call
// (gtmedia thumbnail decoding in `protocol` runs under the same mutex, so it waits for them).
// This is accepted for now; do not restructure without a measured need.
#[tauri::command]
pub async fn chart_open<R: Runtime>(app: AppHandle<R>, req: ChartRequest) -> ApiResult<ChartMeta> {
    blocking(&app, move |s| s.with_session(|se| se.chart_open(&req))).await
}

/// The whole-tree chart. The layout is kept per tree revision, so reopening the view is instant.
#[tauri::command]
pub async fn chart_open_whole<R: Runtime>(app: AppHandle<R>) -> ApiResult<WholeMeta> {
    blocking(&app, move |s| s.with_session(|se| se.chart_open_whole())).await
}

#[tauri::command(async)]
pub fn chart_lineage(
    state: State<'_, AppState>,
    chart_id: u32,
    node_id: u32,
) -> ApiResult<Vec<u32>> {
    state.with_session(|s| s.chart_lineage(chart_id, node_id))
}

#[tauri::command(async)]
pub fn chart_locate(
    state: State<'_, AppState>,
    chart_id: u32,
    person: Uuid,
) -> ApiResult<Option<ChartLocation>> {
    state.with_session(|s| s.chart_locate(chart_id, person))
}

#[tauri::command(async)]
pub fn chart_window(
    state: State<'_, AppState>,
    chart_id: u32,
    rect: Rect,
    zoom: f32,
) -> ApiResult<ChartWindow> {
    state.with_session(|s| s.chart_window(chart_id, &rect, zoom))
}

#[tauri::command(async)]
pub fn chart_outline(
    state: State<'_, AppState>,
    chart_id: u32,
    parent_node: Option<u32>,
    limit: usize,
) -> ApiResult<Vec<OutlineItem>> {
    state.with_session(|s| s.chart_outline(chart_id, parent_node, limit))
}

#[tauri::command(async)]
pub fn chart_close(state: State<'_, AppState>, chart_id: u32) -> ApiResult<()> {
    state.with_session(|s| {
        s.chart_close(chart_id);
        Ok(())
    })
}

// ------------------------------------------------------------ media

#[tauri::command(async)]
pub fn exif_suggestion(
    state: State<'_, AppState>,
    media: Uuid,
) -> ApiResult<Option<ExifSuggestion>> {
    state.with_session(|s| s.exif_suggestion(media))
}

#[tauri::command]
pub async fn attach_files<R: Runtime>(
    app: AppHandle<R>,
    target: OwnerRef,
    paths: Vec<String>,
) -> ApiResult<AttachResult> {
    blocking(&app, move |s| {
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        s.with_session(|se| se.attach_files(target, &paths))
    })
    .await
}

// ------------------------------------------------------------ trace a tree picture

/// Copies the picked picture into the open project as the one being traced.
#[tauri::command(async)]
pub fn stage_trace_image(state: State<'_, AppState>, path: String) -> ApiResult<TraceImageInfo> {
    state.with_session(|s| s.stage_trace_image(Path::new(&path)))
}

#[tauri::command(async)]
pub fn save_trace_draft(state: State<'_, AppState>, json: String) -> ApiResult<()> {
    state.with_session(|s| s.save_trace_draft(&json))
}

#[tauri::command(async)]
pub fn load_trace_draft(state: State<'_, AppState>) -> ApiResult<TraceDraftState> {
    state.with_session(|s| s.load_trace_draft())
}

#[tauri::command(async)]
pub fn discard_trace_draft(state: State<'_, AppState>) -> ApiResult<()> {
    state.with_session(|s| s.discard_trace_draft())
}

#[tauri::command(async)]
pub fn ai_key_status() -> bool {
    crate::ai::key_status(&crate::ai::Keychain)
}

#[tauri::command(async)]
pub fn ai_remove_key() -> ApiResult<()> {
    crate::ai::remove_key(&crate::ai::Keychain)
}

/// Sends the traced picture to Anthropic. `consent` must be true on every call. The picture is read
/// while the session is locked; the lock is released before the (slow) network call.
#[tauri::command]
pub async fn ai_read_picture(
    state: State<'_, AppState>,
    consent: bool,
    key: Option<String>,
) -> ApiResult<gt_session::AiReadResult> {
    let (bytes, _) = state.with_session(|s| s.trace_image_bytes())?;
    crate::ai::CANCEL.store(false, std::sync::atomic::Ordering::SeqCst);
    crate::ai::read_picture_async(
        std::sync::Arc::new(crate::ai::ReqwestHttp),
        std::sync::Arc::new(crate::ai::Keychain),
        consent,
        key,
        bytes,
        crate::ai::DEFAULT_MODEL.to_string(),
    )
    .await
}

/// Stops the read that is in flight; the request is aborted.
#[tauri::command(async)]
pub fn ai_cancel_read() {
    crate::ai::CANCEL.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// The original file of a media item, but only when its CONTENT is an image or a PDF. The type is
/// sniffed from magic bytes by the session; the file name and the stored MIME are never trusted,
/// because an imported GEDCOM can name any file (`.lnk`, `.hta`, `.exe`) as a "photo".
pub fn checked_original_path(state: &AppState, id: Uuid) -> ApiResult<PathBuf> {
    state.with_session(|s| {
        let path = s.media_original_path(id)?;
        let refused = || {
            ApiError::new(
                ErrorCode::Invalid,
                "Gene Tree will only open photos and PDF documents. This file is some other kind of file, so it was not opened.",
            )
        };
        match s.media_bytes(id, gt_session::MediaSize::Original) {
            // Both must agree: the OS picks the program by extension, so a script that merely
            // starts with image or PDF magic bytes (e.g. `x.hta` beginning `GIF89a`) is refused.
            Ok((_, mime)) if extension_matches(&path, &mime) => Ok(path),
            Ok(_) => Err(refused()),
            Err(e) if e.code == ErrorCode::Invalid => Err(refused()),
            Err(e) => Err(e),
        }
    })
}

/// Content types that may be handed to the operating system's default program, with the file
/// extensions each may carry.
const SAFE_OPEN_TYPES: &[(&str, &[&str])] = &[
    ("image/jpeg", &["jpg", "jpeg", "jpe", "jfif"]),
    ("image/png", &["png"]),
    ("image/webp", &["webp"]),
    ("image/gif", &["gif"]),
    ("image/tiff", &["tif", "tiff"]),
    ("application/pdf", &["pdf"]),
];

/// True when the sniffed `mime` is safe to open and the file's extension belongs to it.
fn extension_matches(path: &Path, mime: &str) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let ext = ext.to_ascii_lowercase();
    SAFE_OPEN_TYPES
        .iter()
        .any(|(m, exts)| *m == mime && exts.contains(&ext.as_str()))
}

/// Opens the original file of a media item with the OS default program. The path is resolved by
/// the session (under `<project>/media`, lexical check first) and opened from Rust, so the webview
/// needs no `opener:allow-open-path` permission and never names a path.
#[tauri::command(async)]
pub fn open_media_externally<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: Uuid,
) -> ApiResult<()> {
    let path = checked_original_path(&state, id)?;
    app.opener()
        .open_path(path.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|e| ApiError::new(ErrorCode::Io, format!("Cannot open the file: {e}")))
}

// ------------------------------------------------------------ export

#[derive(Debug, Clone, Copy, Serialize)]
pub struct BytesWritten {
    pub bytes: u64,
}

#[tauri::command]
pub async fn preview_export<R: Runtime>(
    app: AppHandle<R>,
    opts: ExportOptions,
) -> ApiResult<PrivacyPreview> {
    blocking(&app, move |s| s.with_session(|se| se.preview_export(&opts))).await
}

/// The caller has already had the replace-file question answered by the native save dialog,
/// so an existing destination is replaced.
#[tauri::command]
pub async fn export_gedcom<R: Runtime>(
    app: AppHandle<R>,
    opts: ExportOptions,
    dest: String,
    preview_token: String,
) -> ApiResult<BytesWritten> {
    blocking(&app, move |s| {
        s.with_session(|se| {
            se.export_gedcom_over(&opts, Path::new(&dest), &preview_token, true)
                .map(|bytes| BytesWritten { bytes })
        })
    })
    .await
}

// ------------------------------------------------------------ settings

/// `Partial<Settings>` from the UI. `homePerson: null` clears it; absent leaves it alone.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsPatchWire {
    pub day_order: Option<DayOrder>,
    pub name_order: Option<gt_session::NameOrder>,
    pub theme: Option<gt_session::Theme>,
    pub language: Option<gt_session::Language>,
    pub backups_to_keep: Option<u32>,
    pub living_max_age: Option<u32>,
    #[serde(deserialize_with = "some_or_null")]
    pub home_person: Option<Option<Uuid>>,
}

fn some_or_null<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Option<Uuid>>, D::Error> {
    Option::<Uuid>::deserialize(d).map(Some)
}

pub fn get_settings_impl(state: &AppState) -> Settings {
    if let Some(s) = AppState::lock(&state.session).as_ref() {
        return s.settings().clone();
    }
    let a = AppState::lock(&state.settings).clone();
    Settings {
        day_order: a.day_order,
        name_order: a.name_order,
        theme: a.theme,
        language: a.language,
        backups_to_keep: a.backups_to_keep,
        ..Settings::default()
    }
}

pub fn set_settings_impl(state: &AppState, patch: SettingsPatchWire) -> ApiResult<Settings> {
    let has_session = AppState::lock(&state.session).is_some();
    if !has_session && (patch.living_max_age.is_some() || patch.home_person.is_some()) {
        return Err(no_project());
    }
    if patch.backups_to_keep == Some(0) {
        return Err(ApiError::new(
            ErrorCode::Invalid,
            "Keep at least one backup.",
        ));
    }
    // The project part first: it validates, and nothing is saved if it refuses.
    if has_session {
        let p = SettingsPatch {
            day_order: patch.day_order,
            name_order: patch.name_order,
            theme: patch.theme,
            language: patch.language,
            backups_to_keep: patch.backups_to_keep,
            living_max_age: patch.living_max_age,
            home_person: patch.home_person,
        };
        state.with_session(|s| s.set_settings(p).map(|_| ()))?;
    }
    let snapshot = {
        let mut a = AppState::lock(&state.settings);
        if let Some(v) = patch.day_order {
            a.day_order = v;
        }
        if let Some(v) = patch.name_order {
            a.name_order = v;
        }
        if let Some(v) = patch.theme {
            a.theme = v;
        }
        if let Some(v) = patch.language {
            a.language = v;
        }
        if let Some(v) = patch.backups_to_keep {
            a.backups_to_keep = v;
        }
        a.clone()
    };
    settings_file::save_settings(&state.config_dir, &snapshot)?;
    Ok(get_settings_impl(state))
}

#[tauri::command(async)]
pub fn get_settings(state: State<'_, AppState>) -> ApiResult<Settings> {
    Ok(get_settings_impl(&state))
}

#[tauri::command(async)]
pub fn set_settings(state: State<'_, AppState>, patch: SettingsPatchWire) -> ApiResult<Settings> {
    set_settings_impl(&state, patch)
}
