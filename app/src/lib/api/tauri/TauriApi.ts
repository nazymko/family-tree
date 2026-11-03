import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Api } from '../api';
import type {
  AttachResult,
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
  UndoState,
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
  mediaUrl(id: Id, size: 'thumb' | 'preview' | 'original'): string {
    return convertFileSrc(`${id}/${size}`, 'gtmedia');
  }
  exifSuggestion(media: Id): Promise<ExifSuggestion | null> {
    return call('exif_suggestion', { media });
  }
  attachFiles(target: OwnerRef, paths: string[]): Promise<AttachResult> {
    return call('attach_files', { target, paths });
  }
  previewExport(opts: ExportOptions): Promise<PrivacyPreview> {
    return call('preview_export', { opts });
  }
  exportGedcom(opts: ExportOptions, dest: string, previewToken: string): Promise<{ bytes: number }> {
    return call('export_gedcom', { opts, dest, previewToken });
  }
  getSettings(): Promise<Settings> {
    return call('get_settings');
  }
  setSettings(patch: Partial<Settings>): Promise<Settings> {
    return call('set_settings', { patch });
  }
}
