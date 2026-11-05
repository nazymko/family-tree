import type {
  BatchDto,
  LogFilterDto,
  LogRowDto,
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
} from './types';
import type { AgentAccessState, NewConnectionDto } from './types';

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
  /** Every person of the project in one chart. The layout is kept per tree revision, so asking again is cheap. */
  chartOpenWhole(): Promise<WholeMeta>;
  /** The node, its ancestors and its descendants in a whole-tree chart. */
  chartLineage(chartId: number, nodeId: number): Promise<number[]>;
  /** Where a person sits in a whole-tree chart; null when the person is not in the project. */
  chartLocate(chartId: number, person: Id): Promise<ChartLocation | null>;
  /** Names the person every box's `kinship` is worded against (default: the chart's starting person; none for the whole tree). */
  chartSetReference(chartId: number, person: Id | null): Promise<void>;
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
  /** Writes a chart picture (PNG bytes) to `dest`, replacing the file when it exists (the save dialog already asked). */
  savePicture(dest: string, bytes: Uint8Array): Promise<{ bytes: number }>;
  getSettings(): Promise<Settings>;
  setSettings(patch: Partial<Settings>): Promise<Settings>;
  /** Who changed what, newest first. The owner's changes never count as "not yet reviewed". */
  changeLog(filter: LogFilterDto, offset: number, limit: number): Promise<Page<LogRowDto>>;
  changeBatches(): Promise<BatchDto[]>;
  markReviewed(scope: ReviewScopeDto): Promise<void>;
  /** People an agent changed that nobody has reviewed yet; these carry the badge. */
  unreviewedPeople(): Promise<Id[]>;
  personAgentChanges(id: Id): Promise<LogRowDto[]>;
  /** Undoes one agent change; a fact someone changed since stays and is named in the report. */
  undoChangeGroup(group: string): Promise<UndoReport>;
  undoChangeBatch(batch: string): Promise<UndoReport>;
  // --- Agent access ---
  agentAccessState(): Promise<AgentAccessState>;
  /** Turns the local MCP server on or off and sets its port. A taken port comes back as `error` in the state, not as a rejection. */
  agentAccessSet(enabled: boolean, port: number): Promise<AgentAccessState>;
  /** The key and snippet in the answer are the only time they exist in plain text. */
  agentConnectionCreate(name: string): Promise<NewConnectionDto>;
  agentConnectionRevoke(id: string): Promise<void>;
  /** The rules every connecting agent receives, as plain text. */
  agentRules(): Promise<string>;
  // --- end of Agent access ---
}

/** What an agent's write tells the open window. `touched` holds the ids of the people and records it changed. */
export interface AgentChangeEvent {
  touched: string[];
  agent: string;
}

/** OS dialogs and drag-drop; separate from Api so the mock can fake it. */
export interface Platform {
  pickFile(opts: { title: string; extensions?: string[] }): Promise<string | null>;
  pickFiles(opts: { title: string; extensions?: string[] }): Promise<string[]>;
  pickFolder(title: string): Promise<string | null>;
  pickSavePath(opts: { title: string; defaultName: string; extensions: string[]; filters?: { name: string; extensions: string[] }[] }): Promise<string | null>;
  onFilesDropped(cb: (paths: string[]) => void, onHover?: (over: boolean) => void): () => void;
  onTreeChangedByAgent(cb: (e: AgentChangeEvent) => void): () => void;
  reveal(path: string): Promise<void>;
}
