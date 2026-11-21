//! One Tauri command per `Api` method. Commands stay thin: lock, call the session, map errors.
//! A lock is never held across an await; the heavy ones run inside `spawn_blocking`.

use crate::agent_access;
use crate::settings_file;
use crate::state::{AppState, no_project};
use gt_session::{
    ApiError, AttachResult, BatchDto, ChartLocation, ChartMeta, ChartRequest, ChartWindow,
    CitationDto, CopySizeDto, CopyZipDto, DatePreview, DayOrder, EditCommand, EditResult,
    ErrorCode, ExifSuggestion, ExportOptions, FamilyDto, FamilyViewDto, HistoryEntry,
    ImportProgress, ImportReportDto, ImportRequest, LocationDto, LocationKind, LocationWarningDto,
    LogAgentDto, LogFilterDto, LogRowDto, MediaDto, MergePreviewDto, MergeResultDto, OutlineItem,
    OwnerRef, Page, PersonDetail, PersonQuery, PersonRow, PlaceOption, PrivacyPreview, ProjectInfo,
    RecentProject, RecordHomeDto, RecordRefDto, Rect, RelinkResult, ReviewScopeDto, SearchHitDto,
    Session, Settings, SettingsPatch, SourceDto, TraceDraftState, TraceImageInfo, UndoReport,
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
    send_copy_zip,
    copy_size,
    open_received_copy,
    merge_preview,
    merge_apply,
    merge_cancel,
    undo_merge,
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
    change_log,
    change_batches,
    change_log_agents,
    mark_reviewed,
    unreviewed_people,
    person_agent_changes,
    person_history,
    has_untracked_changes,
    record_home,
    locate_records,
    undo_change_group,
    undo_change_batch,
    history,
    parse_date,
    suggest_places,
    suggest_sources,
    map_data,
    add_map_layer_file,
    find_ozi_map,
    ozi_corners,
    ozi_check,
    gazetteer_status,
    gazetteer_remote_size,
    gazetteer_download,
    gazetteer_remove,
    gazetteer_suggest,
    last_citation,
    chart_open,
    chart_open_whole,
    chart_open_selection,
    chart_lineage,
    chart_locate,
    chart_set_reference,
    chart_window,
    chart_outline,
    chart_close,
    exif_suggestion,
    attach_files,
    unlinked_media,
    all_pictures,
    get_recording,
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
    save_picture,
    make_picture_folder,
    get_settings,
    set_settings,
    agent_access_state,
    agent_access_set,
    agent_access_require_key,
    agent_connection_create,
    agent_connection_revoke,
    agent_rules,
    app_info,
    check_for_update,
    open_project_link,
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
        chart_directions: Some(s.chart_directions),
        ..SettingsPatch::default()
    }
}

/// Unix milliseconds, the unit `RecentProject::opened_at` documents.
fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(0))
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
        settings_file::touch_recent(&state.config_dir, &info.dir, &info.name, now_millis())
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

/// The tree as one zip file, to send to a relative. Nothing leaves the computer from here.
#[tauri::command]
pub async fn send_copy_zip<R: Runtime>(app: AppHandle<R>, dest: String) -> ApiResult<CopyZipDto> {
    blocking(&app, move |s| {
        s.with_session(|se| se.save_copy_zip(Path::new(&dest)))
    })
    .await
}

/// How big the zip would be, for the warning shown before it is made.
#[tauri::command(async)]
pub fn copy_size(state: State<'_, AppState>) -> ApiResult<CopySizeDto> {
    state.with_session(|s| s.copy_size())
}

/// Unpacks a zip received from a relative into a new folder and opens it as a tree.
#[tauri::command]
pub async fn open_received_copy<R: Runtime>(
    app: AppHandle<R>,
    zip: String,
    dest: String,
) -> ApiResult<ProjectInfo> {
    blocking(&app, move |s| {
        gt_session::merge::open_received_copy(Path::new(&zip), Path::new(&dest))?;
        open_project_impl(s, &dest)
    })
    .await
}

/// Reads a relative's copy (a folder or a zip) and says what merging it would do. Changes nothing.
#[tauri::command]
pub async fn merge_preview<R: Runtime>(
    app: AppHandle<R>,
    source: String,
    name: String,
) -> ApiResult<MergePreviewDto> {
    blocking(&app, move |s| {
        s.with_session(|se| se.merge_preview(Path::new(&source), &name))
    })
    .await
}

/// Applies the preview made last; `stale` when the tree changed since.
#[tauri::command]
pub async fn merge_apply<R: Runtime>(app: AppHandle<R>, token: Uuid) -> ApiResult<MergeResultDto> {
    blocking(&app, move |s| s.with_session(|se| se.merge_apply(token))).await
}

#[tauri::command(async)]
pub fn merge_cancel(state: State<'_, AppState>) -> ApiResult<()> {
    state.with_session(|s| {
        s.merge_cancel();
        Ok(())
    })
}

#[tauri::command(async)]
pub fn undo_merge(state: State<'_, AppState>, batch: Uuid) -> ApiResult<UndoReport> {
    state.with_session(|s| s.undo_merge(batch))
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

// ------------------------------------------------------------ change log

#[tauri::command(async)]
pub fn change_log(
    state: State<'_, AppState>,
    filter: LogFilterDto,
    offset: usize,
    limit: usize,
) -> ApiResult<Page<LogRowDto>> {
    state.with_session(|s| s.log(filter, offset, limit))
}

#[tauri::command(async)]
pub fn change_batches(state: State<'_, AppState>) -> ApiResult<Vec<BatchDto>> {
    state.with_session(|s| s.batches())
}

#[tauri::command(async)]
pub fn change_log_agents(state: State<'_, AppState>) -> ApiResult<Vec<LogAgentDto>> {
    state.with_session(|s| s.log_agents())
}

#[tauri::command(async)]
pub fn mark_reviewed(state: State<'_, AppState>, scope: ReviewScopeDto) -> ApiResult<()> {
    state.with_session(|s| s.mark_reviewed(scope))
}

#[tauri::command(async)]
pub fn unreviewed_people(state: State<'_, AppState>) -> ApiResult<Vec<Uuid>> {
    state.with_session(|s| s.unreviewed_people())
}

#[tauri::command(async)]
pub fn person_agent_changes(
    state: State<'_, AppState>,
    id: Uuid,
    offset: usize,
    limit: usize,
) -> ApiResult<Page<LogRowDto>> {
    state.with_session(|s| s.person_agent_changes(id, offset, limit))
}

#[tauri::command(async)]
pub fn has_untracked_changes(state: State<'_, AppState>) -> ApiResult<bool> {
    state.with_session(|s| s.has_untracked_changes())
}

#[tauri::command(async)]
pub fn record_home(
    state: State<'_, AppState>,
    kind: String,
    id: Uuid,
) -> ApiResult<Option<RecordHomeDto>> {
    state.with_session(|s| s.record_home(&kind, id))
}

#[tauri::command(async)]
pub fn locate_records(
    state: State<'_, AppState>,
    refs: Vec<RecordRefDto>,
) -> ApiResult<Vec<Option<LocationDto>>> {
    state.with_session(|s| s.locate_records(&refs))
}

#[tauri::command(async)]
pub fn person_history(
    state: State<'_, AppState>,
    id: Uuid,
    offset: usize,
    limit: usize,
) -> ApiResult<Page<LogRowDto>> {
    state.with_session(|s| s.person_history(id, offset, limit))
}

#[tauri::command(async)]
pub fn undo_change_group(state: State<'_, AppState>, group: Uuid) -> ApiResult<UndoReport> {
    state.with_session(|s| s.undo_group(group))
}

#[tauri::command(async)]
pub fn undo_change_batch(state: State<'_, AppState>, batch: Uuid) -> ApiResult<UndoReport> {
    state.with_session(|s| s.undo_batch(batch))
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

/// Everything the Map view draws, for one scope.
#[tauri::command(async)]
pub fn map_data(
    state: State<'_, AppState>,
    scope: gt_session::MapScope,
) -> ApiResult<gt_session::MapDataDto> {
    state.with_session(|s| Ok(s.map_data(&scope)))
}

/// Copies a picture into the tree and lays it on the Map as an old map. Corners come from
/// `layer.corners`, else from the OziExplorer `.map` file at `ozi_map_path`, else none (the
/// owner places it). Returns the picture's media id.
#[tauri::command]
pub async fn add_map_layer_file<R: Runtime>(
    app: AppHandle<R>,
    path: String,
    title: Option<String>,
    mut layer: gt_session::MapLayerInput,
    ozi_map_path: Option<String>,
) -> ApiResult<Uuid> {
    blocking(&app, move |s| {
        let path = PathBuf::from(path);
        if layer.corners.is_none()
            && let Some(map) = ozi_map_path.filter(|p| !p.trim().is_empty())
        {
            // A `.map` that does not fit the picture is never used silently: the window asks
            // `ozi_check` first and lets the owner correct the corners.
            let c = gt_session::map_layers::ozi_corners(Path::new(&map), &path)?;
            layer.corners = Some(c.fitting()?.to_vec());
        }
        let dir = s.with_session(|s| Ok(s.project_dir()))?;
        let prepared = gt_session::prepare_map_file(&dir, &path)?;
        s.with_session(|s| s.commit_map_layer(prepared, title, &layer))
    })
    .await
}

/// The OziExplorer `.map` file that lies next to a picture and names it, if any.
#[tauri::command(async)]
pub fn find_ozi_map(path: String) -> Option<String> {
    gt_session::map_layers::find_ozi_map(Path::new(&path)).map(|p| p.to_string_lossy().into_owned())
}

/// The corners an OziExplorer `.map` file gives a picture already in the tree, and whether the
/// file fits it.
#[tauri::command(async)]
pub fn ozi_corners(
    state: State<'_, AppState>,
    map_path: String,
    media: Uuid,
) -> ApiResult<gt_session::OziPlacementDto> {
    let picture = state.with_session(|s| s.media_original_path(media))?;
    gt_session::map_layers::ozi_placement(Path::new(&map_path), &picture)
}

/// The corners an OziExplorer `.map` file gives a picture the owner is about to add, and whether
/// the file fits it.
#[tauri::command(async)]
pub fn ozi_check(map_path: String, picture_path: String) -> ApiResult<gt_session::OziPlacementDto> {
    gt_session::map_layers::ozi_placement(Path::new(&map_path), Path::new(&picture_path))
}

/// The event `gazetteer_download` emits while it works.
pub const GAZETTEER_PROGRESS_EVENT: &str = "gazetteer-progress";

/// Which countries of the offline place data are on this computer.
#[tauri::command(async)]
pub fn gazetteer_status(state: State<'_, AppState>) -> gt_session::GazetteerStatusDto {
    crate::gazetteer::status(&state.config_dir)
}

/// The download size of a country (bytes), asked of GeoNames when the person chooses the country.
#[tauri::command]
pub async fn gazetteer_remote_size<R: Runtime>(
    app: AppHandle<R>,
    country: String,
) -> ApiResult<Option<u64>> {
    blocking(&app, move |_| crate::gazetteer::remote_size(&country)).await
}

/// Downloads and indexes one country. Only ever runs from the person's click; the one network use of the place data.
#[tauri::command]
pub async fn gazetteer_download<R: Runtime>(
    app: AppHandle<R>,
    country: String,
) -> ApiResult<gt_session::GazetteerCountryDto> {
    let emitter = app.clone();
    blocking(&app, move |s| {
        let done = crate::gazetteer::download(&s.config_dir, &country, &|p| {
            let _ = emitter.emit(GAZETTEER_PROGRESS_EVENT, p);
        })?;
        *AppState::lock(&s.gazetteer) = None;
        Ok(done)
    })
    .await
}

#[tauri::command(async)]
pub fn gazetteer_remove(state: State<'_, AppState>, country: String) -> ApiResult<()> {
    crate::gazetteer::remove(&state.config_dir, &country)?;
    *AppState::lock(&state.gazetteer) = None;
    Ok(())
}

/// Candidates for a place of the tree from the downloaded place data; empty when nothing is downloaded.
#[tauri::command(async)]
pub fn gazetteer_suggest(
    state: State<'_, AppState>,
    place: Uuid,
    limit: usize,
) -> ApiResult<Vec<gt_session::GazetteerCandidateDto>> {
    let parts = state.with_session(|s| Ok(s.place_parts(place)))?;
    let gz = {
        let mut slot = AppState::lock(&state.gazetteer);
        slot.get_or_insert_with(|| crate::gazetteer::load(&state.config_dir))
            .clone()
    };
    let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
    Ok(gz
        .search(&refs, limit.clamp(1, 20))
        .into_iter()
        .map(Into::into)
        .collect())
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

// NOTE: `chart_open` and `export_gedcom` hold the session mutex for the whole call
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

/// The chart of chosen people: them, their ancestors and their partners, and with `all_descendants` everyone below them.
#[tauri::command]
pub async fn chart_open_selection<R: Runtime>(
    app: AppHandle<R>,
    people: Vec<Uuid>,
    all_descendants: bool,
) -> ApiResult<WholeMeta> {
    blocking(&app, move |s| {
        s.with_session(|se| se.chart_open_selection(&people, all_descendants))
    })
    .await
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
pub fn chart_set_reference(
    state: State<'_, AppState>,
    chart_id: u32,
    person: Option<Uuid>,
) -> ApiResult<()> {
    state.with_session(|s| s.chart_set_reference(chart_id, person))
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

/// Attaching in two steps so that a long copy and hash never holds the session: the project
/// folder is read under the lock, the files are sniffed, copied and hashed with the lock free,
/// and the lock is taken again only to record them. `between` runs after the copy (a test uses
/// it to take the lock itself, or to close the project).
pub fn attach_in_two_steps(
    state: &AppState,
    target: OwnerRef,
    paths: &[PathBuf],
    between: impl FnOnce(),
) -> ApiResult<AttachResult> {
    let dir = state.with_session(|s| Ok(s.project_dir()))?;
    let prepared = gt_session::prepare_attach(&dir, target.kind, paths);
    between();
    state.with_session(|s| s.commit_attach(target, prepared))
}

#[tauri::command]
pub async fn attach_files<R: Runtime>(
    app: AppHandle<R>,
    target: OwnerRef,
    paths: Vec<String>,
) -> ApiResult<AttachResult> {
    blocking(&app, move |s| {
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        attach_in_two_steps(s, target, &paths, || ())
    })
    .await
}

/// Media that no record links to (left behind by a deleted note or person), so the owner can remove them.
#[tauri::command(async)]
pub fn unlinked_media(state: State<'_, AppState>) -> ApiResult<Vec<MediaDto>> {
    state.with_session(|s| Ok(s.unlinked_media()))
}

/// Every picture and document, one per version group, to pick another version of a page from.
#[tauri::command(async)]
pub fn all_pictures(state: State<'_, AppState>) -> ApiResult<Vec<MediaDto>> {
    state.with_session(|s| Ok(s.all_pictures()))
}

/// A recording with its sources and every clip of it, for the full player.
#[tauri::command(async)]
pub fn get_recording(
    state: State<'_, AppState>,
    media: Uuid,
) -> ApiResult<gt_session::recording::RecordingDto> {
    state.with_session(|s| s.get_recording(media))
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
pub fn ai_key_status(state: State<'_, AppState>) -> bool {
    crate::ai::key_status(state.keys.as_ref())
}

#[tauri::command(async)]
pub fn ai_remove_key(state: State<'_, AppState>) -> ApiResult<()> {
    crate::ai::remove_key(state.keys.as_ref())
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
        std::sync::Arc::new(state.keys.clone()),
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
                "Family Tree will only open photos and PDF documents. This file is some other kind of file, so it was not opened.",
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

// ------------------------------------------------------------ about

/// The version of this build (see `about.rs`).
#[tauri::command]
pub fn app_info() -> crate::about::AppInfo {
    crate::about::current()
}

/// Asks GitHub for the newest release; runs only when the person presses the button. Never fails: the answer says why.
#[tauri::command]
pub async fn check_for_update() -> crate::about::UpdateStatus {
    crate::about::check_for_update().await
}

/// Opens a page of the project's repository in the system browser; false for any other address.
#[tauri::command(async)]
pub fn open_project_link<R: Runtime>(app: AppHandle<R>, url: String) -> bool {
    crate::about::open_project_link(app, url)
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

// ------------------------------------------------------------ chart pictures

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Writes a finished chart picture. The caller has already had the replace-file question answered by the
/// native save dialog, so an existing file is replaced.
pub fn save_picture_impl(path: &str, bytes: &[u8]) -> ApiResult<BytesWritten> {
    let invalid = |m: &str| ApiError::new(ErrorCode::Invalid, m);
    if path.trim().is_empty() {
        return Err(invalid(
            "There is no file name to save the picture to. Choose where to save it and try again.",
        ));
    }
    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let content_ok = match ext.as_str() {
        "png" => bytes.len() > PNG_SIGNATURE.len() && bytes[..PNG_SIGNATURE.len()] == PNG_SIGNATURE,
        "svg" => std::str::from_utf8(bytes).is_ok_and(|s| {
            let s = s.trim_start();
            s.starts_with("<svg") || s.starts_with("<?xml")
        }),
        _ => {
            return Err(invalid(
                "Pictures are saved as .png or .svg files. Add one of those endings to the file name and try again.",
            ));
        }
    };
    if !content_ok {
        return Err(invalid(
            "The picture could not be made. Try saving it again.",
        ));
    }
    let dest = Path::new(path);
    if dest.is_dir() {
        return Err(invalid(
            "That is a folder, not a file. Choose a file name and try again.",
        ));
    }
    let parent_ok = dest
        .parent()
        .is_none_or(|p| p.as_os_str().is_empty() || p.is_dir());
    if !parent_ok {
        return Err(invalid(
            "That folder does not exist. Choose another folder and try again.",
        ));
    }
    settings_file::write_atomic(dest, bytes).map_err(|e| {
        ApiError::new(
            ErrorCode::Io,
            format!("Could not save the picture: {e}. Choose another place and try again."),
        )
    })?;
    Ok(BytesWritten {
        bytes: bytes.len() as u64,
    })
}

/// Saves a chart picture. The body is the raw PNG bytes (no JSON array of numbers); the destination travels in the
/// `x-gt-path` header as base64 of its UTF-8 text, so any file name survives the header.
#[tauri::command(async)]
pub fn save_picture(request: tauri::ipc::Request<'_>) -> ApiResult<BytesWritten> {
    use base64::Engine as _;
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err(ApiError::new(
            ErrorCode::Invalid,
            "The picture could not be made. Try saving it again.",
        ));
    };
    let path = request
        .headers()
        .get("x-gt-path")
        .and_then(|v| {
            base64::engine::general_purpose::STANDARD
                .decode(v.as_bytes())
                .ok()
        })
        .and_then(|b| String::from_utf8(b).ok())
        .unwrap_or_default();
    save_picture_impl(&path, bytes)
}

/// Makes a NEW folder `name` in `dir` for a set of chart pictures (the A3 sheets of one chart) and returns its path.
/// When `name` is taken (by a folder or a file) it tries `name (2)`, `name (3)` and so on: the folder is always new, so
/// nothing already on the disk is written into or replaced.
pub fn make_picture_folder_impl(dir: &str, name: &str) -> ApiResult<String> {
    let invalid = |m: &str| ApiError::new(ErrorCode::Invalid, m);
    let name = name.trim();
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\'])
    {
        return Err(invalid(
            "There is no folder name for the sheets. Choose a file name and try again.",
        ));
    }
    let parent = Path::new(dir);
    if dir.trim().is_empty() || !parent.is_dir() {
        return Err(invalid(
            "That folder does not exist. Choose another folder and try again.",
        ));
    }
    for n in 1..=999u32 {
        let candidate = if n == 1 {
            parent.join(name)
        } else {
            parent.join(format!("{name} ({n})"))
        };
        match std::fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate.to_string_lossy().into_owned()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(ApiError::new(
                    ErrorCode::Io,
                    format!("Could not make a folder for the sheets: {e}. Choose another place and try again."),
                ));
            }
        }
    }
    Err(ApiError::new(
        ErrorCode::Io,
        "Could not make a folder for the sheets: too many folders of that name. Choose another name and try again.",
    ))
}

/// Makes a new folder for a set of chart pictures (see `make_picture_folder_impl`).
#[tauri::command]
pub fn make_picture_folder(dir: String, name: String) -> ApiResult<String> {
    make_picture_folder_impl(&dir, &name)
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
    pub chart_directions: Option<gt_session::ChartDirections>,
    pub living_max_age: Option<u32>,
    #[serde(deserialize_with = "some_or_null")]
    pub home_person: Option<Option<Uuid>>,
    pub tree_name: Option<String>,
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
        chart_directions: a.chart_directions,
        ..Settings::default()
    }
}

pub fn set_settings_impl(state: &AppState, patch: SettingsPatchWire) -> ApiResult<Settings> {
    let has_session = AppState::lock(&state.session).is_some();
    if !has_session
        && (patch.living_max_age.is_some()
            || patch.home_person.is_some()
            || patch.tree_name.is_some())
    {
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
            chart_directions: patch.chart_directions,
            living_max_age: patch.living_max_age,
            home_person: patch.home_person,
            tree_name: patch.tree_name,
        };
        state.with_session(|s| s.set_settings(p).map(|_| ()))?;
    }
    {
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
        if let Some(v) = patch.chart_directions {
            a.chart_directions = v;
        }
        // Under the lock: agent access saves this file too, and neither may undo the other.
        settings_file::save_settings(&state.config_dir, &a)?;
    }
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

// ------------------------------------------------------------ agent access

#[tauri::command(async)]
pub fn agent_access_state(state: State<'_, AppState>) -> agent_access::AgentAccessState {
    agent_access::access_state(&state)
}

/// `port` is wider than a port, so that one out of range gets the plain refusal.
#[tauri::command(async)]
pub fn agent_access_set<R: Runtime>(
    app: AppHandle<R>,
    enabled: bool,
    port: u32,
) -> ApiResult<agent_access::AgentAccessState> {
    agent_access::set_access(&app, enabled, port)
}

#[tauri::command(async)]
pub fn agent_access_require_key(
    state: State<'_, AppState>,
    required: bool,
) -> ApiResult<agent_access::AgentAccessState> {
    agent_access::set_require_key(&state, required)
}

#[tauri::command(async)]
pub fn agent_connection_create(
    state: State<'_, AppState>,
    name: String,
) -> ApiResult<agent_access::NewConnectionDto> {
    agent_access::create_connection(&state, &name)
}

#[tauri::command(async)]
pub fn agent_connection_revoke(state: State<'_, AppState>, id: String) -> ApiResult<()> {
    agent_access::revoke_connection(&state, &id)
}

#[tauri::command(async)]
pub fn agent_rules() -> &'static str {
    gt_mcp::RULES
}
