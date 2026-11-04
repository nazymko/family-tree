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
  AiReadResult,
  TraceDraftState,
  TraceImageInfo,
  UndoState,
} from './types';

export type * from './types';

export interface Api {
  recentProjects(): Promise<RecentProject[]>;
  createProject(dir: string, name: string): Promise<ProjectInfo>;
  openProject(dir: string): Promise<ProjectInfo>;
  closeProject(): Promise<void>;
  saveCopy(dest: string): Promise<void>;
  checkLocation(dir: string): Promise<LocationWarningDto | null>;
  importGedcom(req: ImportRequest, onProgress?: (p: ImportProgress) => void): Promise<ImportReportDto>;
  saveImportReport(dest: string): Promise<void>;
  /** Deletes the project created by the last import (only before the first edit). */
  discardImport(): Promise<void>;
  relinkMedia(folder: string): Promise<RelinkResult>;
  listPersons(q: PersonQuery, offset: number, limit: number): Promise<Page<PersonRow>>;
  getPerson(id: Id): Promise<PersonDetail>;
  getFamilyView(id: Id): Promise<FamilyViewDto>;
  getFamily(id: Id): Promise<FamilyDto>;
  search(query: string, limit: number): Promise<SearchHitDto[]>;
  /** `silent`: the caller shows its own confirmation, so the shell must not raise a toast. */
  apply(cmd: EditCommand, opts?: { silent?: boolean }): Promise<EditResult>;
  undo(): Promise<EditResult | null>;
  redo(): Promise<EditResult | null>;
  undoState(): Promise<UndoState>;
  history(record: Id, limit: number): Promise<HistoryEntry[]>;
  parseDate(input: string, order: DayOrder): Promise<DatePreview>;
  suggestPlaces(prefix: string, limit: number): Promise<PlaceOption[]>;
  suggestSources(prefix: string, limit: number): Promise<SourceDto[]>;
  lastCitation(): Promise<CitationDto | null>;
  chartOpen(req: ChartRequest): Promise<ChartMeta>;
  chartWindow(chartId: number, rect: Rect, zoom: number): Promise<ChartWindow>;
  chartOutline(chartId: number, parentNode: number | null, limit: number): Promise<OutlineItem[]>;
  chartClose(chartId: number): Promise<void>;
  /** Synchronous: a gtmedia: URL. */
  /** Opens the media item's original file with the system viewer; Rust resolves the path (no path crosses the IPC). */
  openMediaExternally(id: Id): Promise<void>;
  mediaUrl(id: Id, size: 'thumb' | 'preview' | 'original'): string;
  exifSuggestion(media: Id): Promise<ExifSuggestion | null>;
  /** add_media_file + Media record + link, one undo step. */
  /** `silent` asks the shell's default toaster to stay quiet because the caller shows its own notice. */
  attachFiles(target: OwnerRef, paths: string[], opts?: { silent?: boolean }): Promise<AttachResult>;
  /** Copies a picture into the open tree as the one being traced. */
  stageTraceImage(path: string): Promise<TraceImageInfo>;
  /** Synchronous: a gtmedia: URL of the picture being traced; `version` changes when a new picture is staged. */
  traceImageUrl(version: number): string;
  /** Saves the trace draft (opaque text) inside the project. */
  saveTraceDraft(json: string): Promise<void>;
  loadTraceDraft(): Promise<TraceDraftState>;
  /** Forgets the draft and deletes the picture copy. */
  discardTraceDraft(): Promise<void>;
  /** Is an Anthropic API key stored in the system keychain? */
  aiKeyStatus(): Promise<boolean>;
  aiRemoveKey(): Promise<void>;
  /**
   * Sends the traced picture to Anthropic's Claude service. `consent` must be true on every call.
   * `key` is a key typed into the dialog; it is kept in the keychain once Anthropic accepts it.
   */
  aiReadPicture(consent: boolean, key: string | null): Promise<AiReadResult>;
  /** Stops the read in flight; the request is aborted. */
  aiCancelRead(): Promise<void>;
  previewExport(opts: ExportOptions): Promise<PrivacyPreview>;
  exportGedcom(opts: ExportOptions, dest: string, previewToken: string): Promise<{ bytes: number }>;
  getSettings(): Promise<Settings>;
  setSettings(patch: Partial<Settings>): Promise<Settings>;
}

/** OS dialogs and drag-drop; separate from Api so the mock can fake it. */
export interface Platform {
  pickFile(opts: { title: string; extensions?: string[] }): Promise<string | null>;
  pickFiles(opts: { title: string; extensions?: string[] }): Promise<string[]>;
  pickFolder(title: string): Promise<string | null>;
  pickSavePath(opts: { title: string; defaultName: string; extensions: string[] }): Promise<string | null>;
  onFilesDropped(cb: (paths: string[]) => void, onHover?: (over: boolean) => void): () => void;
  reveal(path: string): Promise<void>;
}
