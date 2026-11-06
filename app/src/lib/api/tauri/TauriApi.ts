import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Api } from '../api';
import type { AgentAccessState, NewConnectionDto } from '../types';
import type {
  BatchDto,
  CopySizeDto,
  CopyZipDto,
  MergePreviewDto,
  MergeResultDto,
  LogAgentDto,
  LogFilterDto,
  LogRowDto,
  MediaDto,
  RecordHomeDto,
  ReviewScopeDto,
  UndoReport,
  AttachResult,
  ChartLocation,
  ChartMeta,
  ChartRequest,
  ChartWindow,
  CitationDto,
  DatePreview,
  DayOrder,
  EditCommand,
  EditResult,
  ExifSuggestion,
  ExportOptions,
  FamilyDto,
  FamilyViewDto,
  HistoryEntry,
  Id,
  ImportProgress,
  ImportReportDto,
  ImportRequest,
  LocationWarningDto,
  OutlineItem,
  OwnerRef,
  Page,
  PersonDetail,
  PersonQuery,
  PersonRow,
  PlaceOption,
  PrivacyPreview,
  ProjectInfo,
  RecentProject,
  Rect,
  RelinkResult,
  SearchHitDto,
  Settings,
  SourceDto,
  AiReadResult,
  TraceDraftState,
  TraceImageInfo,
  UndoState,
  WholeMeta,
} from '../types';
import { toApiError } from './errors';

/** One `invoke` with errors normalised. Argument keys are camelCase; Tauri maps them to the Rust parameter names. */
async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    throw toApiError(e);
  }
}

/** The real backend: every method is one Tauri command named in snake_case. */
export class TauriApi implements Api {
  recentProjects(): Promise<RecentProject[]> {
    return call('recent_projects');
  }
  createProject(dir: string, name: string): Promise<ProjectInfo> {
    return call('create_project', { dir, name });
  }
  openProject(dir: string): Promise<ProjectInfo> {
    return call('open_project', { dir });
  }
  closeProject(): Promise<void> {
    return call('close_project');
  }
  saveCopy(dest: string): Promise<void> {
    return call('save_copy', { dest });
  }
  sendCopyZip(dest: string): Promise<CopyZipDto> {
    return call('send_copy_zip', { dest });
  }
  copySize(): Promise<CopySizeDto> {
    return call('copy_size');
  }
  openReceivedCopy(zip: string, dest: string): Promise<ProjectInfo> {
    return call('open_received_copy', { zip, dest });
  }
  mergePreview(source: string, name: string): Promise<MergePreviewDto> {
    return call('merge_preview', { source, name });
  }
  mergeApply(token: string): Promise<MergeResultDto> {
    return call('merge_apply', { token });
  }
  mergeCancel(): Promise<void> {
    return call('merge_cancel');
  }
  undoMerge(batch: string): Promise<UndoReport> {
    return call('undo_merge', { batch });
  }
  checkLocation(dir: string): Promise<LocationWarningDto | null> {
    return call('check_location', { dir });
  }
  async importGedcom(req: ImportRequest, onProgress?: (p: ImportProgress) => void): Promise<ImportReportDto> {
    let unlisten: (() => void) | null = null;
    try {
      if (onProgress) unlisten = await listen<ImportProgress>('import-progress', (e) => onProgress(e.payload));
      return await call<ImportReportDto>('import_gedcom', { req });
    } finally {
      unlisten?.();
    }
  }
  saveImportReport(dest: string): Promise<void> {
    return call('save_import_report', { dest });
  }
  discardImport(): Promise<void> {
    return call('discard_import');
  }
  relinkMedia(folder: string): Promise<RelinkResult> {
    return call('relink_media', { folder });
  }
  listPersons(q: PersonQuery, offset: number, limit: number): Promise<Page<PersonRow>> {
    return call('list_persons', { q, offset, limit });
  }
  getPerson(id: Id): Promise<PersonDetail> {
    return call('get_person', { id });
  }
  getFamilyView(id: Id): Promise<FamilyViewDto> {
    return call('get_family_view', { id });
  }
  getFamily(id: Id): Promise<FamilyDto> {
    return call('get_family', { id });
  }
  search(query: string, limit: number): Promise<SearchHitDto[]> {
    return call('search', { query, limit });
  }
  // `opts.silent` only steers the UI's toaster (observeApi); the backend never sees it.
  apply(cmd: EditCommand): Promise<EditResult> {
    return call('apply', { cmd });
  }
  undo(): Promise<EditResult | null> {
    return call('undo');
  }
  redo(): Promise<EditResult | null> {
    return call('redo');
  }
  undoState(): Promise<UndoState> {
    return call('undo_state');
  }
  history(record: Id, limit: number): Promise<HistoryEntry[]> {
    return call('history', { record, limit });
  }
  parseDate(input: string, order: DayOrder): Promise<DatePreview> {
    return call('parse_date', { input, order });
  }
  suggestPlaces(prefix: string, limit: number): Promise<PlaceOption[]> {
    return call('suggest_places', { prefix, limit });
  }
  suggestSources(prefix: string, limit: number): Promise<SourceDto[]> {
    return call('suggest_sources', { prefix, limit });
  }
  lastCitation(): Promise<CitationDto | null> {
    return call('last_citation');
  }
  chartOpen(req: ChartRequest): Promise<ChartMeta> {
    return call('chart_open', { req });
  }
  chartWindow(chartId: number, rect: Rect, zoom: number): Promise<ChartWindow> {
    return call('chart_window', { chartId, rect, zoom });
  }
  chartOutline(chartId: number, parentNode: number | null, limit: number): Promise<OutlineItem[]> {
    return call('chart_outline', { chartId, parentNode, limit });
  }
  chartClose(chartId: number): Promise<void> {
    return call('chart_close', { chartId });
  }
  chartOpenWhole(): Promise<WholeMeta> {
    return call('chart_open_whole');
  }
  chartOpenSelection(people: Id[], allDescendants: boolean): Promise<WholeMeta> {
    return call('chart_open_selection', { people, allDescendants });
  }
  chartOpenArranged(around: Id, people?: Id[], allDescendants = false): Promise<WholeMeta> {
    return call('chart_open_arranged', { around, people: people ?? null, allDescendants });
  }
  chartLineage(chartId: number, nodeId: number): Promise<number[]> {
    return call('chart_lineage', { chartId, nodeId });
  }
  chartLocate(chartId: number, person: Id): Promise<ChartLocation | null> {
    return call('chart_locate', { chartId, person });
  }
  chartSetReference(chartId: number, person: Id | null): Promise<void> {
    return call('chart_set_reference', { chartId, person });
  }
  mediaUrl(id: Id, size: 'thumb' | 'preview' | 'original'): string {
    // Not convertFileSrc(`${id}/${size}`): that percent-encodes the slash and the protocol handler rejects it.
    return `${convertFileSrc(id, 'gtmedia')}/${size}`;
  }
  openMediaExternally(id: Id): Promise<void> {
    return call('open_media_externally', { id });
  }
  exifSuggestion(media: Id): Promise<ExifSuggestion | null> {
    return call('exif_suggestion', { media });
  }
  unlinkedMedia(): Promise<MediaDto[]> {
    return call('unlinked_media');
  }
  attachFiles(target: OwnerRef, paths: string[]): Promise<AttachResult> {
    return call('attach_files', { target, paths });
  }
  stageTraceImage(path: string): Promise<TraceImageInfo> {
    return call('stage_trace_image', { path });
  }
  traceImageUrl(version: number): string {
    return `${convertFileSrc('trace', 'gtmedia')}/picture?v=${version}`;
  }
  saveTraceDraft(json: string): Promise<void> {
    return call('save_trace_draft', { json });
  }
  loadTraceDraft(): Promise<TraceDraftState> {
    return call('load_trace_draft');
  }
  discardTraceDraft(): Promise<void> {
    return call('discard_trace_draft');
  }
  aiKeyStatus(): Promise<boolean> {
    return call('ai_key_status');
  }
  aiRemoveKey(): Promise<void> {
    return call('ai_remove_key');
  }
  aiCancelRead(): Promise<void> {
    return call('ai_cancel_read');
  }
  aiReadPicture(consent: boolean, key: string | null): Promise<AiReadResult> {
    return call('ai_read_picture', { consent, key });
  }
  previewExport(opts: ExportOptions): Promise<PrivacyPreview> {
    return call('preview_export', { opts });
  }
  exportGedcom(opts: ExportOptions, dest: string, previewToken: string): Promise<{ bytes: number }> {
    return call('export_gedcom', { opts, dest, previewToken });
  }
  async savePicture(dest: string, bytes: Uint8Array): Promise<{ bytes: number }> {
    // The picture travels as the raw request body; the path goes in a header as base64 of its UTF-8 text.
    const path = btoa(String.fromCharCode(...new TextEncoder().encode(dest)));
    try {
      return await invoke<{ bytes: number }>('save_picture', bytes, { headers: { 'x-gt-path': path } });
    } catch (e) {
      throw toApiError(e);
    }
  }
  getSettings(): Promise<Settings> {
    return call('get_settings');
  }
  setSettings(patch: Partial<Settings>): Promise<Settings> {
    return call('set_settings', { patch });
  }
  changeLog(filter: LogFilterDto, offset: number, limit: number): Promise<Page<LogRowDto>> {
    return call('change_log', { filter, offset, limit });
  }
  changeBatches(): Promise<BatchDto[]> {
    return call('change_batches');
  }
  changeLogAgents(): Promise<LogAgentDto[]> {
    return call('change_log_agents');
  }
  markReviewed(scope: ReviewScopeDto): Promise<void> {
    return call('mark_reviewed', { scope });
  }
  unreviewedPeople(): Promise<Id[]> {
    return call('unreviewed_people');
  }
  personAgentChanges(id: Id, offset: number, limit: number): Promise<Page<LogRowDto>> {
    return call('person_agent_changes', { id, offset, limit });
  }
  hasUntrackedChanges(): Promise<boolean> {
    return call('has_untracked_changes');
  }
  recordHome(kind: 'note' | 'citation', id: Id): Promise<RecordHomeDto | null> {
    return call('record_home', { kind, id });
  }
  personHistory(id: Id, offset: number, limit: number): Promise<Page<LogRowDto>> {
    return call('person_history', { id, offset, limit });
  }
  undoChangeGroup(group: string): Promise<UndoReport> {
    return call('undo_change_group', { group });
  }
  undoChangeBatch(batch: string): Promise<UndoReport> {
    return call('undo_change_batch', { batch });
  }
  // --- Agent access ---
  agentAccessState(): Promise<AgentAccessState> {
    return call('agent_access_state');
  }
  agentAccessSet(enabled: boolean, port: number): Promise<AgentAccessState> {
    return call('agent_access_set', { enabled, port });
  }
  agentAccessRequireKey(required: boolean): Promise<AgentAccessState> {
    return call('agent_access_require_key', { required });
  }
  agentConnectionCreate(name: string): Promise<NewConnectionDto> {
    return call('agent_connection_create', { name });
  }
  agentConnectionRevoke(id: string): Promise<void> {
    return call('agent_connection_revoke', { id });
  }
  agentRules(): Promise<string> {
    return call('agent_rules');
  }
  // --- end of Agent access ---
}
