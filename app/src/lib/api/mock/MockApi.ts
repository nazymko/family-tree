import type { Api, StartupOutcome } from '../api';
import type { AgentAccessState, AppInfo, NewConnectionDto, UpdateStatus, WholeBlockDto } from '../types';
import type {
  ActorDto,
  BatchDto,
  CopySizeDto,
  CopyZipDto,
  MergeLineDto,
  MergePreviewDto,
  MergeResultDto,
  LogAgentDto,
  BatchRef,
  LogChangeDto,
  LogFactDto,
  LogFilterDto,
  LogRowDto,
  RecordHomeDto,
  RecordRefDto,
  LocationDto,
  RecordingDto,
  ReviewScopeDto,
  UndoReport,
  RelinkResult,
  AttachResult,
  ChartLocation,
  ChartMeta,
  ChartNode,
  Kinship,
  ChartRequest,
  ChartWindow,
  CitationDto,
  CitationInput,
  DatePreview,
  DayOrder,
  EditCommand,
  EditResult,
  EventDto,
  EventInput,
  GodchildDto,
  GodparentDto,
  GodparentInput,
  ExifSuggestion,
  ExportOptions,
  FamilyDto,
  FamilyViewDto,
  FieldChangeDto,
  HistoryEntry,
  Id,
  ImportProgress,
  ImportReportDto,
  ImportRequest,
  LocationWarningDto,
  MediaDto,
  MediaLinkDto,
  PlaceEventsQuery,
  PlaceEventsPage,
  PlaceEventDto,
  PlaceImportScanDto,
  PlaceImportMapDto,
  PlaceImportPreviewDto,
  PlaceImportResultDto,
  PlaceImportProgressDto,

  NameDto,
  NoteDto,
  OutlineItem,
  OwnerRef,
  Page,
  ParentSetDto,
  PartnershipDto,
  PersonDetail,
  PersonDraft,
  PersonQuery,
  PersonRef,
  PersonRow,
  PlaceOption,
  PrivacyPreview,
  ProjectInfo,
  RecentProject,
  Rect,
  SearchHitDto,
  Settings,
  SexKind,
  SourceDto,
  AiReadResult,
  TraceDraftState,
  TraceImageInfo,
  UndoState,
  WholeMeta,
  GazetteerCandidateDto,
  GazetteerCountryDto,
  GazetteerProgressDto,
  GazetteerStatusDto,
  MapDataDto,
  MapEventDto,
  MapLinkDto,
  MapPersonDto,
  MapPlaceDto,
  MapScope,
  MapCorner,
  MapLayerDto,
  MapLayerInput,
  OziPlacementDto,
} from '../types';
import { personFieldChanges } from './fieldDiff';
import { IdGen, fold, hashString, makeEvent, makeName, makeNote, mulberry32 } from './build';
import { modernVariants } from './orthography';
import type { Db, FamilyRec, Fixture, PersonRec } from './model';
import { MockApiError } from './model';
import { orientLayout, type Layout } from './mockLayout';
import { DEFAULT_DIRECTIONS } from '../../chart/direction';
import { clockMs } from '../../media/clips';
import { layoutDescendants, layoutPedigree, outlineOf, type RustLayoutSource } from './layout';
import { GROUP_BASE, layoutAround, layoutAroundAll, layoutAroundOpen, layoutRelatives, openFlags, relativesOutline, type MockOpen, type RelLayout } from './relatives';
import { layoutWhole, lineageOf, type WholeResult } from './wholeLayout';
import { kinshipFinder } from './kinship';
import { relationLabel, relationRole, type PartnerOf, type Rel } from './relation';
import { toDateDto, yearOf } from './mockParse';
import { lifespanText, previewDate } from './mockDate';

export { MockApiError } from './model';

/** Fixed so living/deceased results do not drift with the calendar. */
const REFERENCE_YEAR = 2026;

/** One second of 8 kHz silence as a WAV data URL: what the mock serves for a recording. */
const SILENT_WAV = (() => {
  const data = 8000 * 2;
  const bytes = new Uint8Array(44 + data);
  const view = new DataView(bytes.buffer);
  const text = (at: number, s: string) => [...s].forEach((c, i) => view.setUint8(at + i, c.charCodeAt(0)));
  text(0, 'RIFF');
  view.setUint32(4, 36 + data, true);
  text(8, 'WAVEfmt ');
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, 8000, true);
  view.setUint32(28, 16000, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  text(36, 'data');
  view.setUint32(40, data, true);
  let bin = '';
  for (const b of bytes) bin += String.fromCharCode(b);
  return `data:audio/wav;base64,${btoa(bin)}`;
})();

function mockMime(name: string): string {
  const ext = /\.([A-Za-z0-9]+)$/.exec(name)?.[1]?.toLowerCase();
  switch (ext) {
    case 'jpg':
    case 'jpeg':
      return 'image/jpeg';
    case 'png':
      return 'image/png';
    case 'pdf':
      return 'application/pdf';
    case 'mp3':
      return 'audio/mpeg';
    case 'wav':
      return 'audio/wav';
    case 'ogg':
    case 'oga':
    case 'opus':
      return 'audio/ogg';
    case 'm4a':
      return 'audio/mp4';
    case 'aac':
      return 'audio/aac';
    case 'flac':
      return 'audio/flac';
    default:
      return 'application/octet-stream';
  }
}
const MAX_PAGE = 200;
const MAX_SEARCH = 50;

interface Snapshot {
  label: string;
  db: Db;
}

interface HistoryRecord {
  ts: number;
  label: string;
  op: HistoryEntry['op'];
  touched: Set<Id>;
  fields: Map<Id, FieldChangeDto[]>;
}

/** One row of the change log; `fact` is what an agent's undo has to take away. */
interface AgentLogEntry {
  group: string;
  ts: number;
  actor: ActorDto;
  batch: BatchRef | null;
  label: string;
  changes: LogChangeDto[];
  people: { id: Id; name: string; reviewed: number | null }[];
  reverted: boolean;
  fact?: { person: Id; note: Id; conflict: boolean };
}

/** See `MockApi.recordAgentBatch`. A batch exists only when `title` is given. */
/** The built-in agent's name in either language, ignoring case and spacing: no connection may take it. */
function isBuiltinAgentName(name: string): boolean {
  const plain = name.split(/\s+/).filter(Boolean).join(' ').toLowerCase();
  return plain === 'agent without a key' || plain === 'агент без ключа';
}

export interface AgentBatchSpec {
  agent?: string;
  /** The agent's id, for a test that needs a fixed one; otherwise one is made per name. */
  agentId?: string;
  title?: string;
  sourceFile?: string;
  report?: string;
  open?: boolean;
  changes: {
    person: Id;
    note?: string;
    label?: string;
    kind?: string;
    /** The record the change is about; the note it adds when omitted. */
    record?: Id;
    /** The table the panel shows for the change. */
    fields?: FieldChangeDto[];
    op?: HistoryEntry['op'];
    fact?: LogFactDto;
    sources?: string[];
    conflict?: boolean;
  }[];
}

export interface AgentBatchResult {
  batch: string | null;
  groups: string[];
  touched: string[];
  agent: string;
}

interface ChartState {
  kind: ChartRequest['kind'];
  layout: Layout | RelLayout;
  version: number;
  /** Who every box's kinship is worded against: the starting person, or the one named by chartSetReference. */
  reference?: Id | null;
  kin?: { ref: Id; find: (to: Id) => Kinship | null };
  /** Whole-tree charts only. */
  whole?: { result: WholeResult; surnames: string[]; meta: WholeMeta; reversed: boolean; selection?: { people: Id[]; descendants: boolean } };
}

interface OwnerBag {
  notes?: NoteDto[];
  citations?: CitationDto[];
  media?: MediaLinkDto[];
}

const LABELS: Record<EditCommand['type'], string> = {
  createPerson: 'Add person',
  updatePerson: 'Edit person',
  deletePerson: 'Delete person',
  saveName: 'Edit name',
  deleteName: 'Delete name',
  saveEvent: 'Save event',
  deleteEvent: 'Delete event',
  addGodparent: 'Add godparent',
  removeGodparent: 'Remove godparent',
  quickAdd: 'Add person',
  linkExisting: 'Add person',
  saveFamily: 'Save family',
  deleteFamily: 'Delete family',
  linkChild: 'Add child to family',
  updateChildLink: 'Edit child link',
  unlinkChild: 'Remove child from family',
  reorderChildren: 'Reorder children',
  addSiblingMarker: 'Add unnamed brothers and sisters',
  saveNote: 'Save note',
  deleteNote: 'Delete note',
  saveSource: 'Save source',
  addCitation: 'Add citation',
  updateCitation: 'Edit citation',
  removeCitation: 'Remove citation',
  attachMedia: 'Attach media',
  detachMedia: 'Detach media',
  deleteMedia: 'Delete media',
  setPrimaryMedia: 'Set primary media',
  updateMedia: 'Edit media',
  setMediaTranscription: 'Edit transcription',
  setMediaRotation: 'Turn picture',
  groupMediaVersions: 'Group versions',
  ungroupMediaVersion: 'Ungroup version',
  addAudioClip: 'Add clip',
  updateAudioClip: 'Edit clip',
  deleteAudioClip: 'Delete clip',
  importDraft: 'Add traced people',
  setPlaceCoordinates: 'Set place coordinates',
  createPlace: 'Add place',
  setMapLayer: 'Change map layer',
  reorderMapLayers: 'Reorder map layers',
  setMapLayersVisible: 'Show maps on the Map',
  addNumber: 'Record number',
};

/** The label the backend gives SetMapLayersVisible: show, hide, or both at once. */
function visibilityLabel(cmd: Extract<EditCommand, { type: 'setMapLayersVisible' }>): string {
  if (cmd.hidden.length === 0) return 'Show maps on the Map';
  if (cmd.shown.length === 0) return 'Hide maps on the Map';
  return 'Choose maps shown on the Map';
}

/** What a synthetic OziExplorer .map file gives in the mock: a sheet around 50.3 N, 30.6 E. */
const MOCK_OZI_CORNERS: MapCorner[] = [
  { latE6: 50_600_000, lonE6: 30_000_000 },
  { latE6: 50_600_000, lonE6: 31_200_000 },
  { latE6: 49_900_000, lonE6: 31_200_000 },
  { latE6: 49_900_000, lonE6: 30_000_000 },
];
const missing = (what: string, id: Id) => new MockApiError('not_found', `No ${what} with id "${id}" in this project.`);
const invalid = (message: string) => new MockApiError('invalid', message);
const markerEntry = (id: Id) => new MockApiError('not_found', `"${id}" is the entry for unnamed brothers and sisters. It has no details.`);

/** An unnamed person is valid in genealogy; show it as Unknown. */
function unnamedOk(n: NameDto): NameDto {
  return n.display ? n : { ...n, display: 'Unknown' };
}

/** A tiny invented country for the browser mock and the screenshots: villages that exist nowhere. */
export const MOCK_GAZETTEER = [
  { id: 9000001, name: 'Berezivka', local: 'Берегівка', region: 'Zelena oblast', lat: 50.52, lon: 30.28, pop: 1200 },
  { id: 9000002, name: 'Berezivka', local: 'Берегівка', region: 'Syni oblast', lat: 49.1, lon: 33.4, pop: 300 },
  { id: 9000003, name: 'Dubky', local: 'Дубки', region: 'Zelena oblast', lat: 50.31, lon: 31.05, pop: 800 },
  { id: 9000004, name: 'Hlukhiv', local: 'Глухів', region: 'Zelena oblast', lat: 51.2, lon: 33.1, pop: 30000 },
  { id: 9000005, name: 'Kyiv', local: 'Київ', region: 'Kyiv', lat: 50.45, lon: 30.52, pop: 2800000 },
];

export class MockApi implements Api {
  private db: Db;
  private settings: Settings;
  private ids: IdGen;
  private clock: number;
  private undoStack: Snapshot[] = [];
  private redoStack: Snapshot[] = [];
  private lastCit: CitationDto | null = null;
  private log: HistoryRecord[] = [];
  private logRows: AgentLogEntry[] = [];
  private logBatches = new Map<string, Omit<BatchDto, 'groups' | 'people' | 'unreviewed' | 'standing'>>();
  private agentIds = new Map<string, string>();
  private charts = new Map<number, ChartState>();
  private nextChart = 1;
  private version = 0;
  private previews = new Map<string, ExportOptions>();
  private projectName: string;
  private trace: { json: string | null; image: TraceImageInfo | null; version: number } = { json: null, image: null, version: 0 };
  private rowCache: { version: number; rows: PersonRow[]; folded: Map<Id, string> } | null = null;
  private sortCache = new Map<string, PersonRow[]>();

  constructor(fixture: Fixture) {
    this.db = {
      persons: new Map(fixture.persons.map((p) => [p.id, structuredClone(p)])),
      families: new Map(fixture.families.map((f) => [f.id, structuredClone(f)])),
      places: new Map(fixture.places.map((p) => [p.id, p])),
      sources: new Map(fixture.sources.map((s) => [s.id, s])),
      media: new Map(fixture.media.map((m) => [m.id, m])),
      coords: new Map(Object.entries(fixture.placeCoords ?? {})),
    };
    // Fixture citations point at the fixture's SourceDto objects; re-point them at this database's copies.
    for (const p of this.db.persons.values()) this.relinkSources(p);
    this.projectName = fixture.name;
    this.ids = new IdGen(mulberry32(0x5eed), 1_800_000_000_000);
    this.clock = 1_750_000_000_000;
    this.settings = {
      dayOrder: 'dmy',
      nameOrder: 'given-first',
      theme: 'system',
      language: 'system',
      backupsToKeep: 10,
      reopenLast: true,
      chartDirections: { ...DEFAULT_DIRECTIONS },
      livingMaxAge: 100,
      homePerson: fixture.homePerson,
      treeName: '',
    };
  }

  // ---------------------------------------------------------------- projects

  async startupOutcome(): Promise<StartupOutcome> {
    return { opened: null, notice: null };
  }

  async recentProjects(): Promise<RecentProject[]> {
    return [
      { dir: 'C:/Genealogy/Schmidt family', name: 'Schmidt family', openedAt: this.clock - 3_600_000, missing: false },
      { dir: 'D:/Old drive/Weber tree', name: 'Weber tree', openedAt: this.clock - 90 * 86_400_000, missing: true },
    ];
  }

  async createProject(dir: string, name: string): Promise<ProjectInfo> {
    this.projectName = name;
    return this.info(dir);
  }

  async openProject(dir: string): Promise<ProjectInfo> {
    return this.info(dir);
  }

  async closeProject(): Promise<void> {
    this.charts.clear();
  }

  /** Destinations passed to `saveCopy`, for tests. */
  readonly savedCopies: string[] = [];

  /** Like the backend, the copy is a project FOLDER: a path that names a file (`.zip`) is refused. */
  async saveCopy(dest: string): Promise<void> {
    if (/\.(zip|ged)$/i.test(dest)) throw new Error(`"${dest}" is a file. Choose an empty folder for the copy.`);
    this.savedCopies.push(dest);
  }

  /** Destinations passed to `sendCopyZip`, for tests. */
  readonly sentZips: string[] = [];
  /** Size the warning shows; tests and screenshots may change it. */
  copyBytes = 4_812_000;

  async sendCopyZip(dest: string): Promise<CopyZipDto> {
    if (!/\.zip$/i.test(dest)) throw invalid(`"${dest}" is not a zip file name. Choose a name ending in .zip.`);
    this.sentZips.push(dest);
    return { path: dest, bytes: this.copyBytes };
  }

  async copySize(): Promise<CopySizeDto> {
    const people = [...this.db.persons.values()].filter((p) => !p.siblingMarker).length;
    return { bytes: this.copyBytes, photos: 38, recordings: 4, people };
  }

  /** Zip and folder passed to `openReceivedCopy`, for tests. */
  readonly receivedCopies: { zip: string; dest: string }[] = [];

  async openReceivedCopy(zip: string, dest: string): Promise<ProjectInfo> {
    if (!/\.zip$/i.test(zip)) throw invalid('This is not a zip file Family Tree can read.');
    this.receivedCopies.push({ zip, dest });
    return this.info(dest);
  }

  /** What the next `mergePreview` answers (its token is replaced); null builds one from the people in the mock. */
  mergeFixture: MergePreviewDto | null = null;
  /** Sources passed to `mergePreview`, and tokens passed to `mergeApply`, for tests. */
  readonly mergeSources: string[] = [];
  readonly mergeApplied: string[] = [];
  /** Makes the next `mergeApply` fail as if the tree changed after the preview. */
  mergeStaleOnce = false;
  private mergeToken: string | null = null;
  private mergeLast: MergePreviewDto | null = null;
  private merges = new Map<string, number>();

  async mergePreview(source: string, name: string): Promise<MergePreviewDto> {
    if (/bad|broken/i.test(source)) throw invalid('This is not a Family Tree copy.');
    this.mergeSources.push(source);
    const base = this.mergeFixture ?? this.sampleMerge(name);
    const dto: MergePreviewDto = {
      ...structuredClone(base),
      token: this.ids.next(),
      name: name.trim() || base.name,
      sourceName: source.replace(/[\\/]+$/, '').split(/[\\/]/).pop()?.replace(/\.zip$/i, '') ?? source,
    };
    this.mergeToken = dto.token;
    this.mergeLast = dto;
    return dto;
  }

  async mergeApply(token: string): Promise<MergeResultDto> {
    const p = this.mergeLast;
    if (!p || token !== this.mergeToken || this.mergeStaleOnce) {
      this.mergeStaleOnce = false;
      throw new MockApiError('stale', 'The tree changed after the copy was looked at. Look at the copy again.');
    }
    this.mergeApplied.push(token);
    const batch = this.ids.next();
    this.merges.set(batch, p.added.length + p.changed.length);
    this.mergeToken = null;
    return {
      batch,
      name: p.name,
      added: p.added.length,
      changed: p.changed.length,
      photosCopied: p.photos,
      leftForYou: p.undecided.length + p.removed.length + p.blocked.length,
    };
  }

  async mergeCancel(): Promise<void> {
    this.mergeToken = null;
  }

  async undoMerge(batch: string): Promise<UndoReport> {
    const n = this.merges.get(batch);
    if (n === undefined) throw missing('merge', batch);
    this.merges.delete(batch);
    return { undone: n, skipped: [] };
  }

  private sampleMerge(name: string): MergePreviewDto {
    const people = [...this.db.persons.values()].filter((p) => !p.siblingMarker);
    const nameOf = (i: number) => {
      const p = people[i % Math.max(people.length, 1)];
      return p ? this.ref(p).name : 'Unknown';
    };
    const idOf = (i: number) => people[i % Math.max(people.length, 1)]?.id ?? this.ids.next();
    const line = (
      kind: string,
      id: Id,
      op: 'create' | 'update' | 'delete',
      detail: string,
      fields: FieldChangeDto[],
      extra: Partial<MergeLineDto> = {},
    ): MergeLineDto => ({
      change: { kind, id, op, detail, fact: { kind: 'text', text: detail }, sources: [], fields },
      person: kind === 'person' && op !== 'create' ? id : null,
      why: null,
      alsoEditedHere: false,
      yours: [],
      maybeSame: [],
      ...extra,
    });
    const field = (f: string, before: string | null, after: string | null): FieldChangeDto => ({ field: f, before, after });
    return {
      token: '',
      name,
      sourceName: 'copy',
      history: 'full',
      checks: [
        { kind: 'readable_copy', ok: true },
        { kind: 'same_tree', ok: true },
        { kind: 'history', ok: true },
      ],
      nothingNew: false,
      toApply: 5,
      added: [
        line('person', this.ids.next(), 'create', 'Taras Koval', [field('given', null, 'Taras'), field('surnames', null, 'Koval')], {
          maybeSame: [{ id: idOf(2), name: nameOf(2), lifespan: 'b. 1968' }],
        }),
        line('note', this.ids.next(), 'create', 'Born in a village near Lviv, 1911.', [field('text', null, 'Born in a village near Lviv, 1911.')]),
      ],
      changed: [
        line('person', idOf(0), 'update', nameOf(0), [field('sex', 'unknown', 'female')]),
        line('person', idOf(1), 'update', nameOf(1), [field('nickname', null, 'Vanya')], { alsoEditedHere: true }),
      ],
      undecided: [
        line('person', idOf(3), 'update', nameOf(3), [field('sex', 'male', 'female')], {
          why: 'both_changed',
          yours: [field('sex', 'male', 'unknown')],
        }),
        line('person', idOf(4), 'update', nameOf(4), [field('private', 'yes', 'no')], { why: 'more_visible' }),
      ],
      removed: [line('person', idOf(5), 'delete', nameOf(5), [])],
      blocked: [line('media', this.ids.next(), 'create', 'letter.pdf', [], { why: 'photo_missing' })],
      leftAlone: 3,
      photos: 1,
      photoBytes: 182_000,
    };
  }

  async checkLocation(dir: string): Promise<LocationWarningDto | null> {
    if (/onedrive/i.test(dir)) return { kind: 'synced_folder', provider: 'OneDrive' };
    if (/dropbox/i.test(dir)) return { kind: 'synced_folder', provider: 'Dropbox' };
    if (/^(\\\\|\/\/)/.test(dir)) return { kind: 'network_share', provider: null };
    return null;
  }

  async importGedcom(req: ImportRequest, onProgress?: (p: ImportProgress) => void): Promise<ImportReportDto> {
    const phases: ImportProgress['phase'][] = ['reading', 'parsing', 'copying_media', 'saving', 'indexing'];
    for (const [i, phase] of phases.entries()) {
      onProgress?.({ phase, done: 0, total: 100 });
      await Promise.resolve();
      onProgress?.({ phase, done: 100, total: 100 });
      void i;
    }
    const project = await this.info(req.projectDir, req.projectName);
    return {
      project: { ...project, locationWarning: await this.checkLocation(req.projectDir) },
      encoding: 'UTF-8',
      version: '5.5.1',
      producer: 'Mock Genealogy 4.2',
      counts: { persons: this.db.persons.size, families: this.db.families.size, sources: this.db.sources.size, media: this.db.media.size },
      repairs: [
        { kind: 'missing_trailer', line: null, detail: 'The file ended without a TRLR record. Added one.' },
        { kind: 'bad_xref', line: 1204, detail: 'A family pointed to a person that is not in the file. Dropped the link.' },
        { kind: 'level_jump', line: 3310, detail: 'A line jumped two levels down. Read it as a child of the previous line.' },
      ],
      unmappedTags: { _FSFTID: 412, _UID: 1380 },
      unparsedDates: [{ line: 880, text: 'sometime in the 1850s?' }],
      missingMedia: [
        { path: 'photos/wedding-1878.jpg', reason: 'notFound', candidates: [] },
        { path: 'photos/farm.jpg', reason: 'notFound', candidates: [] },
        { path: 'D:/Scans/census-1900.jpg', reason: 'outsideRoots', candidates: [] },
        { path: 'portrait.jpg', reason: 'ambiguous', candidates: ['photos/anna/portrait.jpg', 'photos/karl/portrait.jpg'] },
      ],
      mediaUrls: ['https://example.org/images/grandfather.jpg'],
      truncated: false,
      text: 'Mock import report\nPersons: ' + this.db.persons.size + '\n',
    };
  }

  async saveImportReport(): Promise<void> {}
  async discardImport(): Promise<void> {}

  async relinkMedia(): Promise<RelinkResult> {
    return { relinked: 1, stillMissing: ['photos/farm.jpg'], searchIncomplete: false };
  }

  // ------------------------------------------------------------------ reads

  async listPersons(q: PersonQuery, offset: number, limit: number): Promise<Page<PersonRow>> {
    const lim = Math.max(0, Math.min(limit, MAX_PAGE));
    const off = Math.max(0, offset);
    const sorted = this.sortedRows(q);
    const page: Page<PersonRow> = { total: sorted.length, offset: off, items: structuredClone(sorted.slice(off, off + lim)) };
    if (q.sort === 'surname' || q.sort === 'given') {
      const seen = new Set<string>();
      page.letters = [];
      sorted.forEach((r, i) => {
        const key = q.sort === 'surname' ? r.surname : r.given;
        const letter = (fold(key).charAt(0) || '#').toUpperCase();
        if (!seen.has(letter)) {
          seen.add(letter);
          page.letters!.push({ letter, offset: i });
        }
      });
    }
    return structuredClone(page);
  }

  async getPerson(id: Id): Promise<PersonDetail> {
    const p = this.person(id);
    if (p.siblingMarker) throw markerEntry(id);
    return structuredClone({
      ref: this.ref(p),
      sex: p.sex,
      sexText: p.sexText,
      livingOverride: p.livingOverride,
      livingComputed: this.computeLiving({ ...p, livingOverride: null }),
      private: p.private,
      names: p.names,
      events: p.events,
      parentSets: this.parentSets(id),
      partnerships: this.partnerships(id),
      notes: p.notes,
      // The person holds copies; edits (title, transcription) land on the media store.
      media: p.media.map((l) => ({ ...l, media: this.shownMedia(l.media.id) ?? l.media })),
      citations: p.citations,
      ...this.godchildrenField(id),
      changed: p.changed,
    });
  }

  /** `{ godchildren }` when `id` stood godparent for anyone, else nothing (the field is optional, as from Rust). */
  private godchildrenField(id: Id): { godchildren?: GodchildDto[] } {
    const list = this.godchildrenOf(id);
    return list.length > 0 ? { godchildren: list } : {};
  }

  /** Everyone `id` stood godparent for (linked godparents only), oldest first. */
  private godchildrenOf(id: Id): GodchildDto[] {
    const out: { sort: number; row: GodchildDto }[] = [];
    for (const child of this.db.persons.values()) {
      for (const e of child.events) {
        for (const g of e.godparents ?? []) {
          if (g.person?.id !== id) continue;
          out.push({ sort: e.date?.sortMin ?? Infinity, row: { event: e.id, kind: e.kind, date: e.date, person: this.ref(child), role: g.role } });
        }
      }
    }
    return out.sort((a, b) => a.sort - b.sort).map((x) => x.row);
  }

  /** One godparent from the input, checked as the Rust planner checks it. */
  private godparentFrom(input: GodparentInput, old: GodparentDto[], principal: Id | null): GodparentDto {
    const name = input.name?.trim() || null;
    if ((input.person ? 1 : 0) + (name ? 1 : 0) !== 1)
      throw invalid('A godparent is either a person in the tree or a name as the record writes it: give one of the two.');
    if (!['godfather', 'godmother', 'godparent'].includes(input.role)) throw invalid(`"${input.role}" is not a godparent's role. Use godfather or godmother.`);
    if (input.person && input.person === principal) throw invalid('Nobody is their own godparent.');
    const keep = input.id && old.some((g) => g.id === input.id) ? input.id : this.ids.next();
    return {
      id: keep,
      role: input.role,
      person: input.person ? this.ref(this.person(input.person)) : null,
      name,
      note: input.note?.trim() || null,
    };
  }

  private static sameGodparent(a: GodparentDto, b: GodparentDto): boolean {
    if (a.person || b.person) return !!a.person && !!b.person && a.person.id === b.person.id;
    const fold = (s: string | null) => (s ?? '').split(/\s+/).filter(Boolean).join(' ').toLowerCase();
    return fold(a.name) === fold(b.name);
  }

  async getFamilyView(id: Id): Promise<FamilyViewDto> {
    const p = this.person(id);
    if (p.siblingMarker) throw markerEntry(id);
    const parentSets = this.parentSets(id);
    const seen = new Set<Id>([id]);
    const sibs: PersonRec[] = [];
    for (const f of this.db.families.values()) {
      if (!f.children.some((c) => c.person === id)) continue;
      for (const c of f.children) {
        if (seen.has(c.person)) continue;
        seen.add(c.person);
        const s = this.db.persons.get(c.person);
        if (s) sibs.push(s);
      }
    }
    sibs.sort((a, b) => (this.birthSort(a) ?? Infinity) - (this.birthSort(b) ?? Infinity));
    return structuredClone({
      focus: this.ref(p),
      focusSummary: this.summary(p),
      parentSets,
      partnerships: this.partnerships(id),
      siblings: sibs.map((s) => this.ref(s)),
    });
  }

  async getFamily(id: Id): Promise<FamilyDto> {
    const f = this.db.families.get(id);
    if (!f) throw missing('family', id);
    const pr = (pid: Id | null): PersonRef | null => {
      const p = pid ? this.db.persons.get(pid) : undefined;
      return p ? this.ref(p) : null;
    };
    return structuredClone({
      id: f.id,
      partners: [pr(f.partner1), pr(f.partner2)] as [PersonRef | null, PersonRef | null],
      kind: f.kind,
      summary: this.unionSummary(f),
      events: f.events,
      children: f.children.flatMap((c) => {
        const cp = this.db.persons.get(c.person);
        return cp ? [{ person: this.ref(cp), rel: c.rel, birth: this.birth(cp)?.date?.display ?? '', preferred: c.preferred }] : [];
      }),
      notes: f.notes,
      media: f.media ?? [],
    });
  }

  /** Every list of media links in the tree, with the record that holds it. */
  private linkLists(): { owner: OwnerRef; links: MediaLinkDto[]; person: PersonRec | null }[] {
    const out: { owner: OwnerRef; links: MediaLinkDto[]; person: PersonRec | null }[] = [];
    const cites = (cs: CitationDto[], person: PersonRec | null) => {
      for (const c of cs) if (c.media) out.push({ owner: { kind: 'citation', id: c.id }, links: c.media, person });
    };
    const notes = (ns: NoteDto[], person: PersonRec | null) => {
      for (const n of ns) out.push({ owner: { kind: 'note', id: n.id }, links: n.media, person });
    };
    for (const p of this.db.persons.values()) {
      out.push({ owner: { kind: 'person', id: p.id }, links: p.media, person: p });
      notes(p.notes, p);
      cites(p.citations, p);
      for (const n of p.names) cites(n.citations, p);
      for (const e of p.events) {
        out.push({ owner: { kind: 'event', id: e.id }, links: e.media, person: p });
        notes(e.notes, p);
        cites(e.citations, p);
      }
    }
    for (const f of this.db.families.values()) {
      const first = (f.partner1 && this.db.persons.get(f.partner1)) || (f.partner2 && this.db.persons.get(f.partner2)) || null;
      if (f.media) out.push({ owner: { kind: 'family', id: f.id }, links: f.media, person: first });
      notes(f.notes, first);
      cites(f.citations, first);
      for (const e of f.events) {
        out.push({ owner: { kind: 'event', id: e.id }, links: e.media, person: first });
        notes(e.notes, first);
        cites(e.citations, first);
      }
    }
    for (const s of this.db.sources.values()) out.push({ owner: { kind: 'source', id: s.id }, links: s.media, person: null });
    return out;
  }

  async getRecording(media: Id): Promise<RecordingDto> {
    const m = this.db.media.get(media);
    if (!m) throw missing('media item', media);
    const sources: RecordingDto['sources'] = [];
    const clips: RecordingDto['clips'] = [];
    for (const { owner, links, person } of this.linkLists()) {
      for (const l of links) {
        if (l.media.id !== media) continue;
        if (!l.clip) {
          if (owner.kind === 'source') sources.push({ id: owner.id, title: this.db.sources.get(owner.id)?.title ?? null });
          continue;
        }
        const title =
          owner.kind === 'note'
            ? (this.findNote(owner.id)?.text.split('\n')[0] ?? undefined)
            : owner.kind === 'citation'
              ? (this.findCitation(owner.id)?.source?.title ?? undefined)
              : undefined;
        const ev = owner.kind === 'event' ? this.findEvent(owner.id)?.ev : undefined;
        clips.push({
          clip: structuredClone(l.clip),
          target: owner,
          ...(title ? { title } : {}),
          ...(ev ? { eventKind: ev.kind } : {}),
          ...(person ? { person: this.ref(person) } : {}),
        });
      }
    }
    clips.sort((a, b) => a.clip.startMs - b.clip.startMs || a.clip.endMs - b.clip.endMs);
    return structuredClone({ media: m, sources, clips });
  }

  async search(query: string, limit: number): Promise<SearchHitDto[]> {
    const q = fold(query.trim());
    const lim = Math.max(0, Math.min(limit, MAX_SEARCH));
    if (!q || lim === 0) return [];
    const hits: SearchHitDto[] = [];
    const { rows, folded } = this.rows();
    const starts: SearchHitDto[] = [];
    const contains: SearchHitDto[] = [];
    for (const r of rows) {
      if (r.siblingMarker) continue;
      const f = folded.get(r.id)!;
      const at = f.indexOf(q);
      if (at < 0) continue;
      const person: PersonRef = { id: r.id, name: r.name, lifespan: r.lifespan, sex: r.sex, living: r.living, private: r.private, ...(r.birthSurname ? { birthSurname: r.birthSurname } : {}), ...(r.asRecorded ? { asRecorded: r.asRecorded } : {}), siblingMarker: false };
      const hit: SearchHitDto = { kind: 'person', id: r.id, label: r.name, snippet: r.lifespan || null, person };
      (at === 0 || f[at - 1] === ' ' ? starts : contains).push(hit);
      if (starts.length >= lim) break;
    }
    hits.push(...starts, ...contains);
    for (const pl of this.db.places.values()) {
      if (hits.length >= lim) break;
      if (fold(pl.full).includes(q)) hits.push({ kind: 'place', id: pl.id, label: pl.full, snippet: null, person: null });
    }
    for (const s of this.db.sources.values()) {
      if (hits.length >= lim) break;
      if (fold(s.title ?? '').includes(q)) hits.push({ kind: 'source', id: s.id, label: s.title ?? '', snippet: null, person: null });
    }
    if (hits.length < lim) {
      for (const p of this.db.persons.values()) {
        const n = p.notes.find((x) => fold(x.text).includes(q));
        if (n) hits.push({ kind: 'note', id: n.id, label: n.text.slice(0, 60), snippet: n.text.slice(0, 120), person: this.ref(p) });
        if (hits.length >= lim) break;
      }
    }
    return hits.slice(0, lim);
  }

  async parseDate(input: string, order: DayOrder): Promise<DatePreview> {
    return previewDate(input, order);
  }

  async suggestPlaces(prefix: string, limit: number): Promise<PlaceOption[]> {
    const q = fold(prefix.trim());
    const out: PlaceOption[] = [];
    for (const p of this.db.places.values()) {
      if (!q || fold(p.name).startsWith(q) || fold(p.full).startsWith(q)) out.push(p);
      if (out.length >= Math.min(limit, MAX_SEARCH)) break;
    }
    return structuredClone(out);
  }

  async suggestSources(prefix: string, limit: number): Promise<SourceDto[]> {
    const q = fold(prefix.trim());
    const out: SourceDto[] = [];
    for (const s of this.db.sources.values()) {
      if (!q || fold(s.title ?? '').includes(q)) out.push({ ...s, media: s.media?.map((l) => ({ ...l, media: this.shownMedia(l.media.id) ?? l.media })) });
      if (out.length >= Math.min(limit, MAX_SEARCH)) break;
    }
    return structuredClone(out);
  }

  async lastCitation(): Promise<CitationDto | null> {
    return this.lastCit ? structuredClone(this.lastCit) : null;
  }

  // ----------------------------------------------------------------- history

  async undo(): Promise<EditResult | null> {
    const e = this.undoStack.pop();
    if (!e) return null;
    this.redoStack.push({ label: e.label, db: this.db });
    this.db = e.db;
    this.touch();
    this.record(`Undo: ${e.label}`, 'update', new Set());
    return { label: `Undo: ${e.label}`, focus: null, touched: [], undo: this.currentUndo() };
  }

  async redo(): Promise<EditResult | null> {
    const e = this.redoStack.pop();
    if (!e) return null;
    this.undoStack.push({ label: e.label, db: this.db });
    this.db = e.db;
    this.touch();
    this.record(`Redo: ${e.label}`, 'update', new Set());
    return { label: `Redo: ${e.label}`, focus: null, touched: [], undo: this.currentUndo() };
  }

  async undoState(): Promise<UndoState> {
    return this.currentUndo();
  }

  async history(record: Id, limit: number): Promise<HistoryEntry[]> {
    if (limit <= 0) return [];
    return this.log
      .filter((h) => h.touched.has(record))
      .slice(-Math.max(0, limit))
      .reverse()
      .map((h) => ({ ts: h.ts, label: h.label, op: h.op, fields: structuredClone(h.fields.get(record) ?? []) }));
  }

  // -------------------------------------------------------------- change log

  async changeLog(filter: LogFilterDto, offset: number, limit: number): Promise<Page<LogRowDto>> {
    const unreviewed = (e: AgentLogEntry) => this.standing(e) && e.people.some((p) => p.reviewed === null && (!filter.person || p.id === filter.person));
    const rows = this.logRows
      .filter((e) => {
        if (filter.actorKind && e.actor.kind !== filter.actorKind) return false;
        if (filter.actorId && (e.actor.kind !== 'agent' || e.actor.id !== filter.actorId)) return false;
        if (filter.batch && e.batch?.id !== filter.batch) return false;
        if (filter.person && !e.people.some((p) => p.id === filter.person)) return false;
        return !filter.unreviewedOnly || unreviewed(e);
      })
      .reverse();
    const take = Math.min(limit, MAX_PAGE);
    return { total: rows.length, offset, items: rows.slice(offset, offset + take).map((e) => this.logRow(e)) };
  }

  async changeBatches(): Promise<BatchDto[]> {
    return [...this.logBatches.values()].reverse().map((b) => {
      const rows = this.logRows.filter((e) => e.batch?.id === b.id);
      const left = rows.filter((e) => this.standing(e)).flatMap((e) => e.people);
      return {
        ...b,
        groups: rows.length,
        people: new Set(rows.flatMap((e) => e.people.map((p) => p.id))).size,
        unreviewed: new Set(left.filter((p) => p.reviewed === null).map((p) => p.id)).size,
        standing: rows.filter((e) => this.standing(e)).length,
      };
    });
  }

  async changeLogAgents(): Promise<LogAgentDto[]> {
    const seen = new Map<string, string>();
    for (const e of this.logRows) if (e.actor.kind === 'agent') seen.set(e.actor.id, e.actor.name);
    return [...seen].map(([id, name]) => ({ id, name })).sort((a, b) => a.name.localeCompare(b.name));
  }

  async markReviewed(scope: ReviewScopeDto): Promise<void> {
    const at = this.now();
    if (scope.kind === 'group') this.logGroup(scope.id);
    else if (scope.kind === 'batch' && !this.logBatches.has(scope.id)) throw missing('batch', scope.id);
    for (const e of this.logRows) {
      if (e.actor.kind !== 'agent') continue;
      if (scope.kind === 'group' && e.group !== scope.id) continue;
      if (scope.kind === 'batch' && e.batch?.id !== scope.id) continue;
      for (const p of e.people) if ((scope.kind !== 'person' || p.id === scope.id) && p.reviewed === null) p.reviewed = at;
    }
  }

  async unreviewedPeople(): Promise<Id[]> {
    const left = this.logRows.filter((e) => this.standing(e)).flatMap((e) => e.people);
    return [...new Set(left.filter((p) => p.reviewed === null).map((p) => p.id))];
  }

  async personAgentChanges(id: Id, offset: number, limit: number): Promise<Page<LogRowDto>> {
    const rows = this.logRows.filter((e) => e.actor.kind === 'agent' && this.standing(e) && e.people.some((p) => p.id === id)).reverse();
    return { total: rows.length, offset, items: rows.slice(offset, offset + Math.min(limit, MAX_PAGE)).map((e) => this.logRow(e)) };
  }

  async hasUntrackedChanges(): Promise<boolean> {
    return this.logRows.some((e) => e.actor.kind === 'beforeTracking');
  }

  async recordHome(kind: 'note' | 'citation', id: Id): Promise<RecordHomeDto | null> {
    const holds = (bag: OwnerBag) => (kind === 'note' ? bag.notes : bag.citations)?.some((x) => x.id === id);
    const home = (owner: OwnerRef, bag: OwnerBag): RecordHomeDto => ({
      owner,
      ...(kind === 'note' ? { note: structuredClone(bag.notes!.find((x) => x.id === id)!) } : { citation: structuredClone(bag.citations!.find((x) => x.id === id)!) }),
    });
    const bags: [OwnerRef, OwnerBag][] = [];
    for (const p of this.db.persons.values()) {
      bags.push([{ kind: 'person', id: p.id }, { notes: p.notes, citations: p.citations }]);
      for (const n of p.names) bags.push([{ kind: 'name', id: n.id }, { citations: n.citations }]);
      for (const e of p.events) bags.push([{ kind: 'event', id: e.id }, { notes: e.notes, citations: e.citations }]);
    }
    for (const f of this.db.families.values()) {
      bags.push([{ kind: 'family', id: f.id }, { notes: f.notes, citations: f.citations }]);
      for (const e of f.events) bags.push([{ kind: 'event', id: e.id }, { notes: e.notes, citations: e.citations }]);
    }
    const found = bags.find(([, bag]) => holds(bag));
    return found ? home(found[0], found[1]) : null;
  }

  async locateRecords(refs: RecordRefDto[]): Promise<(LocationDto | null)[]> {
    const { persons, families, sources } = this.db;
    /** Holder of each note, citation and picture, by id; the first holder wins, as in the backend. */
    const holders = new Map<Id, OwnerRef>();
    /** Recordings a holder links as a stretch (a clip). */
    const clipped = new Set<Id>();
    const hold = (owner: OwnerRef, bag: OwnerBag) => {
      for (const x of [...(bag.notes ?? []), ...(bag.citations ?? [])]) if (!holders.has(x.id)) holders.set(x.id, owner);
      for (const m of bag.media ?? []) {
        if (!holders.has(m.media.id)) {
          holders.set(m.media.id, owner);
          if (m.clip) clipped.add(m.media.id);
        }
      }
    };
    const events = new Map<Id, { page: LocationDto['page']; id: Id }>();
    const citations = new Map<Id, Id | null>();
    for (const p of persons.values()) {
      hold({ kind: 'person', id: p.id }, p);
      for (const n of p.names) hold({ kind: 'name', id: n.id }, n);
      for (const e of p.events) {
        hold({ kind: 'event', id: e.id }, e);
        if (!p.siblingMarker) events.set(e.id, { page: 'person', id: p.id });
      }
    }
    for (const f of families.values()) {
      hold({ kind: 'family', id: f.id }, f);
      for (const e of f.events) {
        hold({ kind: 'event', id: e.id }, e);
        events.set(e.id, { page: 'family', id: f.id });
      }
    }
    for (const bag of [...persons.values(), ...families.values()]) {
      for (const c of [...bag.citations, ...('events' in bag ? bag.events.flatMap((e) => e.citations) : [])]) citations.set(c.id, c.source?.id ?? null);
    }
    const nameOwner = new Map<Id, Id>();
    for (const p of persons.values()) for (const n of p.names) nameOwner.set(n.id, p.id);
    type Page = { page: LocationDto['page']; id: Id };
    const pageOf = (o: OwnerRef): Page | null => {
      switch (o.kind) {
        case 'person':
          return persons.has(o.id) && !persons.get(o.id)!.siblingMarker ? { page: 'person', id: o.id } : null;
        case 'name':
          return nameOwner.has(o.id) ? { page: 'person', id: nameOwner.get(o.id)! } : null;
        case 'family':
          return families.has(o.id) ? { page: 'family', id: o.id } : null;
        case 'source':
          return sources.has(o.id) ? { page: 'source', id: o.id } : null;
        case 'event':
          return events.get(o.id) ?? null;
        case 'citation': {
          const s = citations.get(o.id);
          return s && sources.has(s) ? { page: 'source', id: s } : null;
        }
        default:
          return null;
      }
    };
    /** Up through the holders to a record with a page: the page and the records walked through. */
    const climb = (start: OwnerRef): { page: Page; chain: OwnerRef[] } | null => {
      let here: OwnerRef | undefined = start;
      const chain: OwnerRef[] = [];
      for (let i = 0; here && i < 16; i++) {
        chain.push(here);
        const page = pageOf(here);
        if (page) return { page, chain };
        here = holders.get(here.id);
      }
      return null;
    };
    /** Whether the page draws chain[i] as a row or item of its own. */
    const drawn = (page: Page, chain: OwnerRef[], i: number): boolean => {
      const node = chain[i];
      const next = chain[i + 1];
      const heldByPage = !!next && next.id === page.id;
      const clip = clipped.has(node.id) && !!next && next.kind !== 'name';
      if (node.kind === 'event') return true;
      if (page.page === 'person') {
        if (node.kind === 'name') return true;
        if (node.kind === 'note' || node.kind === 'citation') return heldByPage;
        if (node.kind === 'media') return heldByPage || clip;
      } else if (page.page === 'family') {
        if (node.kind === 'note') return heldByPage;
        if (node.kind === 'media') return heldByPage && clip;
      } else if (node.kind === 'media') return heldByPage;
      return false;
    };
    const located = (start: OwnerRef): LocationDto | null => {
      const found = climb(start);
      if (!found) return null;
      const { page, chain } = found;
      const at = chain.findIndex((_, i) => drawn(page, chain, i));
      const target = at >= 0 ? { kind: chain[at].kind, id: chain[at].id } : { kind: page.page as string, id: page.id };
      return { page: page.page, id: page.id, target };
    };
    const noteIds = new Set<Id>();
    for (const bag of [...persons.values(), ...families.values()]) for (const n of bag.notes) noteIds.add(n.id);
    for (const p of persons.values()) for (const e of p.events) for (const n of e.notes) noteIds.add(n.id);
    for (const f of families.values()) for (const e of f.events) for (const n of e.notes) noteIds.add(n.id);
    return refs.map((r) => {
      switch (r.kind) {
        case 'person':
        case 'family':
        case 'source':
        case 'event':
          return located({ kind: r.kind, id: r.id });
        case 'citation':
          return citations.has(r.id) ? located({ kind: 'citation', id: r.id }) : null;
        case 'media':
          return this.db.media.has(r.id) ? located({ kind: 'media', id: r.id }) : null;
        case 'note':
          return noteIds.has(r.id) ? located({ kind: 'note', id: r.id }) : null;
        default:
          return null;
      }
    });
  }

  async personHistory(id: Id, offset: number, limit: number): Promise<Page<LogRowDto>> {
    const rows = this.logRows.filter((e) => e.people.some((p) => p.id === id)).reverse();
    return { total: rows.length, offset, items: rows.slice(offset, offset + Math.min(limit, MAX_PAGE)).map((e) => this.logRow(e)) };
  }

  async undoChangeGroup(group: string): Promise<UndoReport> {
    const e = this.logGroup(group);
    if (e.actor.kind !== 'agent') throw invalid("Only an agent's change can be undone from the change log.");
    return this.revertGroup(e);
  }

  async undoChangeBatch(batch: string): Promise<UndoReport> {
    if (!this.logBatches.has(batch)) throw missing('batch', batch);
    const report: UndoReport = { undone: 0, skipped: [] };
    for (const e of this.logRows.filter((r) => r.batch?.id === batch).reverse()) {
      const r = this.revertGroup(e);
      report.undone += r.undone;
      report.skipped.push(...r.skipped);
    }
    return report;
  }

  /**
   * Test helper: what an agent's write leaves behind. Each change is one group holding a note on the person;
   * the Ctrl+Z stack stays as it was. `conflict` makes a later undo find the fact changed, so it is skipped.
   * Pair with `MockPlatform.simulateAgentBatch`, which also fires the event. Limit: the mock's Ctrl+Z restores a
   * whole snapshot, so undoing an owner edit made before an agent's write also drops that write's note.
   */
  recordAgentBatch(spec: AgentBatchSpec): AgentBatchResult {
    const agent = spec.agent ?? 'Claude';
    let agentId = spec.agentId ?? this.agentIds.get(agent);
    if (!agentId) {
      agentId = this.ids.next();
      this.agentIds.set(agent, agentId);
    }
    const actor: ActorDto = { kind: 'agent', id: agentId, name: agent };
    let batch: BatchRef | null = null;
    if (spec.title !== undefined) {
      batch = { id: this.ids.next(), title: spec.title };
      const started = this.now();
      this.logBatches.set(batch.id, {
        ...batch,
        sourceFile: spec.sourceFile ?? null,
        actorId: agentId,
        actorName: agent,
        started,
        ended: spec.open ? null : started,
        report: spec.report ?? null,
      });
    }
    const groups: string[] = [];
    const touched = new Set<Id>();
    for (const c of spec.changes) {
      const text = c.note ?? 'Agent note';
      this.db = structuredClone(this.db);
      this.execute({ type: 'saveNote', target: { kind: 'person', id: c.person }, note: { kind: 'general', text } });
      const p = this.person(c.person);
      const note = p.notes[p.notes.length - 1].id;
      p.changed = this.now();
      const fact: LogFactDto = c.fact ?? { kind: 'text', text };
      const entry = this.addLogEntry({
        actor,
        batch,
        label: c.label ?? 'Save note',
        changes: [{ kind: c.kind ?? 'note', id: c.record ?? note, op: c.op ?? 'create', detail: text, fact, sources: c.sources ?? [], fields: c.fields ?? [] }],
        people: [{ id: c.person, name: this.ref(p).name, reviewed: null }],
        fact: { person: c.person, note, conflict: c.conflict ?? false },
      });
      groups.push(entry.group);
      touched.add(c.person).add(note);
    }
    this.touch();
    return { batch: batch?.id ?? null, groups, touched: [...touched], agent };
  }

  /** Test helper: a row written before changes had authors. */
  recordBeforeTracking(label: string, person: Id): void {
    const name = this.ref(this.person(person)).name;
    this.addLogEntry({
      actor: { kind: 'beforeTracking' },
      batch: null,
      label,
      changes: [{ kind: 'person', id: person, op: 'update', detail: name, fact: { kind: 'text', text: name }, sources: [], fields: [] }],
      people: [{ id: person, name, reviewed: null }],
    });
  }

  // ------------------------------------------------------------------- edits

  async apply(cmd: EditCommand): Promise<EditResult> {
    const addedStep = (cmd.type === 'quickAdd' || cmd.type === 'linkExisting') && cmd.parentRel === 'step' && (cmd.relation === 'father' || cmd.relation === 'mother');
    const label = addedStep ? `Add step${cmd.relation}` : cmd.type === 'quickAdd' || cmd.type === 'linkExisting' ? `Add ${cmd.relation}` : cmd.type === 'setMapLayersVisible' ? visibilityLabel(cmd) : LABELS[cmd.type];
    const before = this.db;
    this.db = structuredClone(before);
    let outcome: { focus: Id | null; touched: Id[] };
    try {
      outcome = this.execute(cmd);
    } catch (e) {
      this.db = before;
      throw e;
    }
    this.undoStack.push({ label, db: before });
    if (this.undoStack.length > 200) this.undoStack.shift();
    this.redoStack = [];
    this.recountSources();
    this.touch();
    const touched = [...new Set(outcome.touched)];
    for (const id of touched) {
      const p = this.db.persons.get(id);
      if (p) p.changed = this.now();
    }
    const op: HistoryEntry['op'] = cmd.type === 'createPerson' || cmd.type === 'quickAdd' ? 'create' : /^delete/.test(cmd.type) ? 'delete' : 'update';
    this.record(label, op, new Set(touched), before);
    return { label, focus: outcome.focus, touched, undo: this.currentUndo() };
  }

  async attachFiles(target: OwnerRef, paths: string[]): Promise<AttachResult> {
    const failed: AttachResult['failed'] = [];
    const accepted: { path: string; name: string }[] = [];
    const known = new Set<string>();
    for (const p of this.db.persons.values()) {
      for (const l of p.media) known.add((l.media.originalName ?? '').toLowerCase());
      for (const n of p.notes) for (const l of n.media) known.add((l.media.originalName ?? '').toLowerCase());
    }
    const audioName = /\.(mp3|wav|ogg|oga|opus|m4a|aac|flac)$/i;
    for (const path of paths) {
      const name = path.replace(/[\\/]+$/, '').split(/[\\/]/).pop() ?? '';
      if (!name) failed.push({ path, reason: 'The file has no name.' });
      else if (/[\\/]$/.test(path) || !/\.[A-Za-z0-9]{1,8}$/.test(name)) failed.push({ path, reason: 'Folders cannot be attached. Choose files.' });
      else if (/\.(exe|bat|cmd|msi|dll|com|scr)$/i.test(name)) failed.push({ path, reason: 'This file type cannot be attached.' });
      else if (target.kind === 'note' && !audioName.test(name)) failed.push({ path, reason: 'only sound recordings can be attached to a note' });
      else if (target.kind !== 'note' && target.kind !== 'source' && audioName.test(name)) failed.push({ path, reason: 'sound recordings can only be attached to a note or a source' });
      else if (known.has(name.toLowerCase()) || accepted.some((a) => a.name.toLowerCase() === name.toLowerCase()))
        failed.push({ path, reason: 'Already attached' });
      else accepted.push({ path, name });
    }
    if (accepted.length === 0) return { attached: [], failed };

    const before = this.db;
    this.db = structuredClone(before);
    try {
      const bag = this.owner(target);
      if (!bag.media) throw invalid('Media can be attached to people only in the mock.');
      const attached: MediaDto[] = [];
      for (const a of accepted) {
        const m: MediaDto = {
          id: this.ids.next(),
          title: a.name.replace(/\.[^.]+$/, ''),
          mime: mockMime(a.name),
          originalName: a.name,
          date: null,
          description: null,
          resolved: true,
          width: audioName.test(a.name) ? null : 1024,
          height: audioName.test(a.name) ? null : 768,
          sha256: null,
          transcription: [],
          rotation: 0,
          // The backend reads a recording's length from its headers; the mock invents one from the name.
          ...(audioName.test(a.name) ? { durationMs: 60_000 * (20 + (hashString(a.name) % 25)) + 17_000 } : {}),
        };
        this.db.media.set(m.id, m);
        bag.media.push({ media: m, primary: bag.media.length === 0 });
        attached.push(m);
        // Like the real session: an oversized image is attached but listed in `failed` (no preview).
        if (/huge/i.test(a.name)) failed.push({ path: a.path, reason: 'image too large to preview' });
      }
      this.undoStack.push({ label: 'Attach media', db: before });
      this.redoStack = [];
      this.touch();
      this.record('Attach media', 'update', new Set([target.id]));
      return { attached, failed };
    } catch (e) {
      this.db = before;
      throw e;
    }
  }

  // ------------------------------------------------------------------ charts

  async chartOpen(req: ChartRequest): Promise<ChartMeta> {
    if (req.kind === 'whole') throw invalid('The whole tree has no starting person. Open it with the whole-tree command.');
    if (this.person(req.root).siblingMarker) throw markerEntry(req.root);
    const generations = Math.max(1, Math.min(req.generations, req.kind === 'pedigree' || req.kind === 'around' || req.kind === 'relatives' ? 255 : 20));
    if (req.kind === 'around') {
      const chosen = [req.root, ...(req.roots ?? [])].filter((id, i, all) => all.indexOf(id) === i && this.db.persons.has(id) && !this.person(id).siblingMarker);
      if (chosen.length > 1) return this.openAroundMany(chosen, req.kin ?? 'line');
    }
    const src: RustLayoutSource = {
      ref: (id) => this.ref(this.person(id)),
      birth: (id) => this.birthSort(this.person(id)),
      parents: (id) => this.preferredParents(id),
      unions: (id) => this.unionsOf(id),
    };
    // The line of one person has no unnamed-siblings markers in it.
    const line: RustLayoutSource = {
      ...src,
      unions: (id) => src.unions(id).map((u) => ({ ...u, children: u.children.filter((c) => !this.person(c).siblingMarker) })),
    };
    // Side branches opened per person (one chosen person only; people not in the project and markers are left out).
    const open: MockOpen[] =
      req.kind === 'around'
        ? (req.open ?? [])
            .filter((o) => this.db.persons.has(o.person) && !this.person(o.person).siblingMarker && (o.siblings || o.children))
            .map((o) => ({ key: o.person, siblings: o.siblings, children: o.children }))
        : [];
    const openBase = req.kin === 'all' ? 'all' : req.kin === 'siblings' ? 'siblings' : 'line';
    const laid =
      req.kind === 'around' && open.length > 0
        ? layoutAroundOpen(req.root, generations, openBase, open, openBase === 'line' ? { ...src, unions: line.unions } : src, line)
        : req.kind === 'around'
        ? req.kin === 'all'
          ? layoutAroundAll(req.root, generations, src)
          : req.kin === 'siblings'
            ? layoutRelatives(req.root, generations, false, src, true, true)
            : layoutAround(req.root, generations, line)
        : req.kind === 'pedigree'
        ? layoutPedigree(req.root, generations, src)
        : req.kind === 'descendants'
          ? layoutDescendants(req.root, generations, src)
          : layoutRelatives(req.root, generations, req.auntsUncles ?? true, src);
    if (req.kind === 'around') {
      const flags = openFlags(laid as RelLayout, openBase, openBase === 'line' ? line : src);
      laid.nodes.forEach((n, i) => {
        if (flags[i].siblings) n.canOpenSiblings = true;
        if (flags[i].children) n.canOpenChildren = true;
        if (n.dupOf === null && open.some((o) => o.key === n.person?.id)) n.opened = true;
      });
    }
    // The side the ancestors are drawn on, for this kind of chart.
    const layout = orientLayout(laid, req.kind, req.direction ?? this.settings.chartDirections[req.kind]);
    if (req.print) {
      // Compact cards for a print, like the engine's: narrower and a little lower, the gaps in the same proportion.
      const fx = 0.7;
      const fy = 0.85;
      for (const n of layout.nodes) {
        n.x *= fx;
        n.w *= fx;
        n.y *= fy;
        n.h *= fy;
      }
      for (const e of layout.edges) e.pts = e.pts.map((v, i) => v * (i % 2 === 0 ? fx : fy));
      layout.bounds = { w: layout.bounds.w * fx, h: layout.bounds.h * fy };
    }
    const chartId = this.nextChart++;
    this.charts.set(chartId, { kind: req.kind, layout, version: this.version, reference: req.root });
    return {
      chartId,
      kind: req.kind,
      bounds: layout.bounds,
      nodeCount: layout.nodes.length,
      personCount: new Set(layout.nodes.map((n) => n.person?.id)).size,
      rootNode: 0,
      truncated: layout.truncated,
      version: this.version,
    };
  }

  /**
   * The brothers and sisters of the chosen people (and, for 'all', of every ancestor on their lines), with partners and all
   * descendants, named like the backend names them. People already on the chart keep their place.
   */
  private extraKin(chosen: Id[], kin: 'siblings' | 'all', on: Set<Id>): Map<Id, Rel> {
    const fams = [...this.db.families.values()];
    const real = (id: Id) => this.db.persons.has(id) && !this.person(id).siblingMarker;
    const partnersOf = (f: (typeof fams)[number]) => [f.partner1, f.partner2].filter((x): x is Id => x !== null && real(x));
    const parentsOf = (p: Id): Id[] => [...new Set(fams.filter((f) => f.children.some((c) => c.person === p)).flatMap(partnersOf))];
    const out = new Map<Id, Rel>();
    const taken = (id: Id) => on.has(id) || out.has(id);
    const addTree = (root: Id, up: number, rel: Rel) => {
      out.set(root, rel);
      let level = [root];
      for (let down = 1; level.length > 0; down++) {
        const next: Id[] = [];
        for (const p of level) {
          for (const f of fams) {
            if (!partnersOf(f).includes(p)) continue;
            const rank = p === root ? (up === 1 ? 'sibling' : up === 2 ? 'aunt' : { up, down }) : { up, down };
            for (const q of partnersOf(f)) if (!taken(q)) out.set(q, { t: 'partner', of: rank as PartnerOf });
            for (const c of f.children) {
              if (!real(c.person) || taken(c.person)) continue;
              out.set(c.person, up === 1 && down === 1 ? { t: 'nibling' } : { t: 'kin', up, down: down + 1 });
              next.push(c.person);
            }
          }
        }
        level = next;
      }
    };
    const levels = new Map<Id, number>(chosen.map((c) => [c, 0]));
    const todo = [...chosen];
    for (let i = 0; i < todo.length; i++) {
      const p = todo[i];
      if (kin === 'siblings' && levels.get(p)! > 0) break;
      for (const q of parentsOf(p)) if (!levels.has(q)) { levels.set(q, levels.get(p)! + 1); todo.push(q); }
    }
    const order = [...levels.entries()].filter(([, g]) => kin === 'all' || g === 0).sort((a, b) => a[1] - b[1]);
    for (const [x, g] of order) {
      const own = fams.filter((f) => f.children.some((c) => c.person === x));
      const ps = parentsOf(x);
      for (const f of fams) {
        if (!partnersOf(f).some((q) => ps.includes(q))) continue;
        for (const c of f.children) {
          if (c.person === x || ps.includes(c.person) || !real(c.person) || taken(c.person) || levels.has(c.person)) continue;
          const half = !own.includes(f) && !(ps.length === 2 && partnersOf(f).length === 2 && partnersOf(f).every((q) => ps.includes(q)));
          addTree(c.person, g + 1, g === 0 ? { t: 'sibling', half } : g === 1 ? { t: 'aunt', half } : { t: 'kin', up: g + 1, down: 1 });
        }
      }
    }
    return out;
  }

  /**
   * The chart built around several people: the chosen people, their ancestors (shared ones once) and descendants, laid out
   * like the chart of chosen people on the whole tree. Boxes are worded against the NEAREST chosen person. Only the line;
   * the generations limit is not applied here.
   */
  private openAroundMany(chosen: Id[], kin: 'line' | 'siblings' | 'all'): ChartMeta {
    const { on } = this.selectionSubset(chosen, true);
    const extra = kin === 'line' ? new Map<Id, Rel>() : this.extraKin(chosen, kin, on);
    for (const id of extra.keys()) on.add(id);
    const persons = [...this.db.persons.values()].filter((p) => on.has(p.id) && !p.siblingMarker);
    const rows = new Map(this.rows().rows.map((r) => [r.id, r]));
    // The mock draws a family of the whole-tree charts with the ancestors on top or at the bottom only.
    const reversed = ['bottom', 'right'].includes(this.settings.chartDirections.around);
    const result = layoutWhole({
      persons: persons.map((p) => ({ id: p.id, surname: rows.get(p.id)?.surname ?? '', birth: this.birthSort(p) })),
      families: [...this.db.families.values()].map((f) => ({ partners: [f.partner1, f.partner2], children: f.children.map((c) => c.person) })),
      ref: (id) => this.ref(this.person(id)),
    }, reversed);
    const nodes = result.nodes;
    const chosenNodes = chosen.map((id) => result.nodeOf.get(id)).filter((n): n is number => n !== undefined);
    const reach = (adj: number[][]): (number | null)[] => {
      const d: (number | null)[] = nodes.map(() => null);
      const todo = [...chosenNodes];
      for (const c of chosenNodes) d[c] = 0;
      for (let i = 0; i < todo.length; i++) for (const w of adj[todo[i]] ?? []) if (d[w] === null) { d[w] = d[todo[i]]! + 1; todo.push(w); }
      return d;
    };
    const up = reach(result.parents);
    const down = reach(result.children);
    const owner = new Map<number, number>();
    for (const e of result.edges) if (e.kind === 'partner') owner.set(e.to, owner.get(e.from) ?? e.from);
    const rels: Rel[] = nodes.map((n) => {
      const u = up[n.id];
      const d = down[n.id];
      if (chosenNodes.includes(n.id)) return { t: 'focus' };
      const x = n.person ? extra.get(n.person.id) : undefined;
      if (x) return x;
      if (u !== null && (d === null || u <= d)) return { t: 'ancestor', n: u };
      if (d !== null) return { t: 'descendant', n: d };
      return { t: 'focus' };
    });
    nodes.forEach((n) => {
      if (up[n.id] !== null || down[n.id] !== null || chosenNodes.includes(n.id) || (n.person && extra.has(n.person.id))) return;
      const o = owner.get(n.id);
      const r = o === undefined ? null : rels[o];
      rels[n.id] = r?.t === 'descendant' ? { t: 'partner', of: r.n } : { t: 'partner', of: 'focus' };
    });
    for (const n of nodes) {
      const rel = rels[n.id];
      n.relation = relationLabel(rel, n.person!.sex);
      n.role = relationRole(rel, n.person!.sex);
      n.label = `${n.person!.name}${n.person!.lifespan ? `, ${n.person!.lifespan}` : ''}, ${n.relation}`;
    }
    const layout: RelLayout = { nodes, edges: result.edges, bounds: result.bounds, truncated: false, kids: nodes.map(() => []), labels: nodes.map((n) => n.label), rels, rootNode: chosenNodes[0], chosen: chosenNodes, adj: { parents: result.parents, children: result.children } };
    const chartId = this.nextChart++;
    this.charts.set(chartId, { kind: 'around', layout, version: this.version, reference: chosen[0] });
    return { chartId, kind: 'around', bounds: layout.bounds, nodeCount: nodes.length, personCount: new Set(nodes.map((n) => n.person?.id)).size, rootNode: chosenNodes[0], truncated: false, version: this.version, chosenNodes: [...chosenNodes] };
  }

  async chartOpenWhole(): Promise<WholeMeta> {
    return this.openWholeChart(null, null);
  }

  /** The chart of chosen people: them, their ancestors and their partners; with `allDescendants` also everyone below them. */
  async chartOpenSelection(people: Id[], allDescendants: boolean): Promise<WholeMeta> {
    const { on, kept } = this.selectionSubset(people, allDescendants);
    if (kept.length === 0) throw new MockApiError('not_found', 'None of the chosen people is in the project any more.');
    return this.openWholeChart({ people: kept, descendants: allDescendants }, on);
  }

  /** Who is on the chart of `people`, and the chosen people that exist (sorted, once each). Every walk marks before it follows. */
  private selectionSubset(people: Id[], allDescendants: boolean): { on: Set<Id>; kept: Id[] } {
    const real = (id: Id) => {
      const p = this.db.persons.get(id);
      return p !== undefined && !p.siblingMarker;
    };
    const kept = [...new Set(people.filter(real))].sort();
    const fams = [...this.db.families.values()];
    const partnersOf = (f: (typeof fams)[number]) => [f.partner1, f.partner2].filter((x): x is Id => x !== null && this.db.persons.has(x));
    const on = new Set<Id>(kept);
    const todo = [...kept];
    while (todo.length > 0) {
      const p = todo.pop()!;
      for (const f of fams) {
        if (!f.children.some((c) => c.person === p)) continue;
        for (const q of partnersOf(f)) if (!on.has(q)) { on.add(q); todo.push(q); }
      }
    }
    for (const f of fams) if (partnersOf(f).some((q) => kept.includes(q))) for (const q of partnersOf(f)) on.add(q);
    if (allDescendants) {
      const seen = new Set<Id>(kept);
      const down = [...kept];
      while (down.length > 0) {
        const p = down.pop()!;
        for (const f of fams) {
          if (!partnersOf(f).includes(p)) continue;
          for (const q of partnersOf(f)) on.add(q);
          for (const c of f.children) {
            if (!this.db.persons.has(c.person)) continue;
            on.add(c.person);
            if (!seen.has(c.person)) { seen.add(c.person); down.push(c.person); }
          }
        }
      }
    }
    // The unnamed-siblings marker of a family that is on the chart.
    for (const f of fams) {
      if (!partnersOf(f).some((q) => on.has(q))) continue;
      for (const c of f.children) if (this.db.persons.get(c.person)?.siblingMarker) on.add(c.person);
    }
    return { on, kept };
  }

  private async openWholeChart(selection: { people: Id[]; descendants: boolean } | null, subset: Set<Id> | null): Promise<WholeMeta> {
    // The mock draws a family of the whole-tree charts with the ancestors on top or at the bottom only.
    const reversed = ['bottom', 'right'].includes(this.settings.chartDirections.whole);
    // Two kinds are held side by side, one chart each: everyone and chosen people.
    const kindOf = (c: ChartState) => (c.whole!.selection ? 'selection' : 'plain');
    const kind = selection ? 'selection' : 'plain';
    const sameKind = (c: ChartState) => c.whole && kindOf(c) === kind;
    for (const c of this.charts.values()) {
      if (!sameKind(c) || c.version !== this.version || c.whole!.reversed !== reversed) continue;
      const have = c.whole!.selection;
      const sameSel = selection === null ? !have : !!have && have.descendants === selection.descendants && have.people.join() === selection.people.join();
      if (sameSel) return structuredClone(c.whole!.meta);
    }
    for (const [id, c] of [...this.charts]) if (sameKind(c)) this.charts.delete(id);
    const persons = [...this.db.persons.values()].filter((p) => subset === null || subset.has(p.id));
    const rows = new Map(this.rows().rows.map((r) => [r.id, r]));
    const result = layoutWhole({
      persons: persons.map((p) => ({ id: p.id, surname: rows.get(p.id)?.surname ?? '', birth: this.birthSort(p) })),
      families: [...this.db.families.values()].map((f) => ({ partners: [f.partner1, f.partner2], children: f.children.map((c) => c.person) })),
      ref: (id) => this.ref(this.person(id)),
    }, reversed);
    const surnames = result.nodes.map((n) => rows.get(n.person!.id)?.surname ?? '');
    for (const n of result.nodes) {
      const p = n.person!;
      n.label = `${p.siblingMarker ? 'more brothers and sisters, not named' : `${p.name}${p.lifespan ? `, ${p.lifespan}` : ''}`}, generation ${n.gen + 1}`;
    }
    // The markers are drawn like people but are not counted as people.
    const markerAt = result.nodes.filter((n) => n.person?.siblingMarker).map((n) => ({ x: n.x + n.w / 2, y: n.y + n.h / 2 }));
    // The main (largest, first) family block carries the name the user gave the tree.
    const mainAt = result.blocks.findIndex((b) => !b.unconnected);
    const blocks = result.blocks.map((b0, i) => {
      const b: WholeBlockDto = i === mainAt && this.settings.treeName ? { ...b0, customName: this.settings.treeName } : b0;
      const inside = markerAt.filter((m) => m.x >= b.rect.x && m.x <= b.rect.x + b.rect.w && m.y >= b.rect.y && m.y <= b.rect.y + b.rect.h + b.bodyH).length;
      const people = b.people - inside;
      const titled = b.customName ? { ...b, title: `${b.customName} (${b0.people} people)` } : b;
      return inside === 0 ? titled : { ...titled, people, title: titled.title.replace(/\(\d+/, `(${people}`) };
    });
    const chartId = this.nextChart++;
    const meta: WholeMeta = {
      meta: {
        chartId,
        kind: 'whole',
        bounds: result.bounds,
        nodeCount: result.nodes.length - markerAt.length,
        personCount: new Set(result.nodes.filter((n) => !n.person?.siblingMarker).map((n) => n.person?.id)).size,
        rootNode: 0,
        truncated: false,
        version: this.version,
      },
      blocks,
      chosen: selection ? selection.people.length : 0,
    };
    const layout: Layout = { nodes: result.nodes, edges: result.edges, bounds: result.bounds, truncated: false, kids: [], labels: [] };
    this.charts.set(chartId, { kind: 'whole', layout, version: this.version, reference: null, whole: { result, surnames, meta, reversed, ...(selection ? { selection } : {}) } });
    return structuredClone(meta);
  }

  async chartLineage(chartId: number, nodeId: number): Promise<number[]> {
    const c = this.wholeChart(chartId);
    if (!c.layout.nodes[nodeId]) throw missing('chart node', String(nodeId));
    return lineageOf(c.whole!.result, nodeId);
  }

  async chartLocate(chartId: number, person: Id): Promise<ChartLocation | null> {
    const c = this.wholeChart(chartId);
    const id = c.whole!.result.nodeOf.get(person);
    const n = id === undefined ? undefined : c.layout.nodes[id];
    return n ? { nodeId: n.id, rect: { x: n.x, y: n.y, w: n.w, h: n.h } } : null;
  }

  async chartSetReference(chartId: number, person: Id | null): Promise<void> {
    const c = this.chart(chartId);
    if (c.version !== this.version) throw new MockApiError('stale', 'The tree changed since this chart was opened. Reopen the chart.');
    if (person) this.person(person);
    c.reference = person;
  }

  async chartWindow(chartId: number, rect: Rect, zoom: number): Promise<ChartWindow> {
    const c = this.chart(chartId);
    if (c.version !== this.version) throw new MockApiError('stale', 'The tree changed since this chart was opened. Reopen the chart.');
    const nodes: ChartNode[] = c.layout.nodes.filter(
      (n) => n.x < rect.x + rect.w && n.x + n.w > rect.x && n.y < rect.y + rect.h && n.y + n.h > rect.y,
    );
    const edges = c.layout.edges.filter((e) => {
      let x0 = Infinity,
        y0 = Infinity,
        x1 = -Infinity,
        y1 = -Infinity;
      for (let i = 0; i < e.pts.length; i += 2) {
        x0 = Math.min(x0, e.pts[i]);
        x1 = Math.max(x1, e.pts[i]);
        y0 = Math.min(y0, e.pts[i + 1]);
        y1 = Math.max(y1, e.pts[i + 1]);
      }
      return x0 <= rect.x + rect.w && x1 >= rect.x && y0 <= rect.y + rect.h && y1 >= rect.y;
    });
    if (c.whole && zoom < 0.25) {
      // Far out the backend sends the surname and sex only.
      const surnames = c.whole.surnames;
      return structuredClone({
        nodes: nodes.map((n) => ({ ...n, label: '', person: { ...n.person!, name: surnames[n.id], lifespan: '', birthSurname: undefined, asRecorded: undefined } })),
        edges,
      });
    }
    const ref = c.reference;
    if (ref && zoom >= 0.25) {
      if (c.kin?.ref !== ref) c.kin = { ref, find: kinshipFinder({ families: () => this.db.families.values(), sex: (id) => this.db.persons.get(id)?.sex ?? 'unknown' }, ref) };
      const find = c.kin.find;
      const stamped = nodes.map((n) => {
        const k = n.person ? find(n.person.id) : null;
        return k && k.kind !== 'self_person' ? { ...n, kinship: k } : n;
      });
      return structuredClone({ nodes: stamped, edges });
    }
    return structuredClone({ nodes, edges });
  }

  async chartOutline(chartId: number, parentNode: number | null, limit: number): Promise<OutlineItem[]> {
    const c = this.chart(chartId);
    if (c.kind === 'whole') throw invalid('The whole tree has no outline. Use Find a person to move around it.');
    const lim = Math.max(0, Math.min(limit, MAX_PAGE));
    if (c.kind === 'relatives' || c.kind === 'around') {
      const rl = c.layout as RelLayout;
      if (parentNode !== null && parentNode < GROUP_BASE && !rl.nodes[parentNode]) throw missing('chart node', String(parentNode));
      return relativesOutline(rl, parentNode, lim);
    }
    if (parentNode !== null && !c.layout.nodes[parentNode]) throw missing('chart node', String(parentNode));
    const items = outlineOf(c.kind, c.layout, parentNode, lim);
    const ref = c.reference;
    if (!ref) return items;
    const find = kinshipFinder({ families: () => this.db.families.values(), sex: (id) => this.db.persons.get(id)?.sex ?? 'unknown' }, ref);
    return items.map((it) => {
      const k = it.person ? find(it.person.id) : null;
      return k && k.kind !== 'self_person' ? { ...it, kinship: k } : it;
    });
  }

  async chartClose(chartId: number): Promise<void> {
    this.charts.delete(chartId);
  }

  // ------------------------------------------------------------------- media

  async stageTraceImage(path: string): Promise<TraceImageInfo> {
    const name = path.replace(/[\\/]+$/, '').split(/[\\/]/).pop() ?? '';
    if (!/\.(jpe?g|png|webp|tiff?|gif)$/i.test(name)) throw invalid('That is not a picture file. Pick a JPG, PNG, WebP, TIFF or GIF.');
    this.trace.image = { name, width: 1200, height: 800 };
    this.trace.version++;
    return { ...this.trace.image };
  }

  traceImageUrl(version: number): string {
    const svg =
      '<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="800" viewBox="0 0 1200 800">' +
      '<rect width="1200" height="800" fill="#f3ecd9"/>' +
      '<g font-family="serif" font-size="34" fill="#3b2f1d" stroke="#3b2f1d" stroke-width="2">' +
      '<text x="220" y="120" stroke="none">Ivan Petrovych Koval</text><text x="640" y="120" stroke="none">Olena Koval</text>' +
      '<text x="420" y="420" stroke="none">Petro Koval</text><text x="120" y="640" stroke="none">Maria Koval</text>' +
      '<path d="M400 140V280M820 140V280M400 280H820M610 280V380" fill="none"/></g>' +
      '<!-- ' + version + ' --></svg>';
    return 'data:image/svg+xml;utf8,' + encodeURIComponent(svg);
  }

  async saveTraceDraft(json: string): Promise<void> {
    this.trace.json = json;
  }

  async loadTraceDraft(): Promise<TraceDraftState> {
    return { json: this.trace.json, image: this.trace.image ? { ...this.trace.image } : null };
  }

  async discardTraceDraft(): Promise<void> {
    this.trace.json = null;
    this.trace.image = null;
  }

  /** Mock keychain and reader: no network. Sends are counted so tests can check consent. */
  aiKeyStored = false;
  aiSends = 0;

  async aiKeyStatus(): Promise<boolean> {
    return this.aiKeyStored;
  }

  async aiRemoveKey(): Promise<void> {
    this.aiKeyStored = false;
  }

  aiCancels = 0;
  async aiCancelRead(): Promise<void> {
    this.aiCancels += 1;
  }
  async aiReadPicture(consent: boolean, key: string | null): Promise<AiReadResult> {
    if (!consent) throw invalid('Tick "I understand" before sending the picture.');
    if (!this.aiKeyStored && !key?.trim()) throw invalid('Enter your Anthropic API key first.');
    if (!this.trace.image) throw invalid('There is no picture to trace.');
    this.aiSends += 1;
    if (key?.trim()) this.aiKeyStored = true;
    const p = (id: string, given: string, surname: string, x: number, y: number, extra: Partial<AiReadResult['people'][number]> = {}) => ({
      id, given, surname, patronymic: '', sex: null, birth: null, death: null, x, y, w: 0.14, h: 0.05, confidence: 0.9, ...extra,
    });
    return {
      people: [
        p('p1', 'Ivan', 'Koval', 0.18, 0.15, { patronymic: 'Petrovych', sex: 'male', birth: '1901' }),
        p('p2', 'Olena', 'Koval', 0.53, 0.15, { sex: 'female', confidence: 0.8 }),
        p('p3', 'Petro', 'Koval', 0.35, 0.52, { sex: 'male', confidence: 0.6 }),
      ],
      relations: [
        { kind: 'partner', from: 'p1', to: 'p2' },
        { kind: 'parent', from: 'p1', to: 'p3' },
        { kind: 'parent', from: 'p2', to: 'p3' },
      ],
      skipped: 0,
      truncated: false,
    };
  }

  mediaUrl(id: Id, size: 'thumb' | 'preview' | 'original'): string {
    // A recording plays a second of silence, so the dev page has something to load.
    if (size === 'original' && this.db.media.get(id)?.mime?.startsWith('audio/')) return SILENT_WAV;
    const dim = size === 'thumb' ? 160 : size === 'preview' ? 640 : 1200;
    const hue = hashString(id) % 360;
    const svg =
      `<svg xmlns="http://www.w3.org/2000/svg" width="${dim}" height="${Math.round(dim * 0.75)}" viewBox="0 0 160 120">` +
      `<rect width="160" height="120" fill="hsl(${hue} 18% 82%)"/>` +
      `<path d="M0 96 40 60l28 26 30-38 62 56v10H0z" fill="hsl(${hue} 22% 62%)"/>` +
      `<circle cx="118" cy="30" r="10" fill="hsl(${hue} 30% 94%)"/></svg>`;
    return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
  }

  /** Records the call in `openedMedia` so tests can assert it. */
  readonly openedMedia: Id[] = [];

  async openMediaExternally(id: Id): Promise<void> {
    const m = this.db.media.get(id);
    if (m && !(m.mime ?? '').match(/^(image\/(jpeg|png|webp|gif|tiff)|application\/pdf)$/)) {
      throw new Error('Family Tree will only open photos and PDF documents. This file is some other kind of file, so it was not opened.');
    }
    this.openedMedia.push(id);
  }

  /** The primary of a media item's version group: the item itself when it is in none. */
  private versionHead(id: Id): Id {
    const p = this.db.versionOf?.get(id);
    return p && this.db.media.has(p) ? p : id;
  }

  /** The whole version group of a media item, primary first, then the most pixels first; one item when it has none. */
  private versionGroup(id: Id): Id[] {
    const head = this.versionHead(id);
    const px = (m: Id) => {
      const d = this.db.media.get(m);
      return (d?.width ?? 0) * (d?.height ?? 0);
    };
    const rest = [...(this.db.versionOf ?? new Map<Id, Id>()).entries()].filter(([m, p]) => p === head && m !== head && this.db.media.has(m)).map(([m]) => m);
    rest.sort((a, b) => px(b) - px(a));
    return [head, ...rest];
  }

  /** A media item as the backend sends it: the stored record, with its version group when it has one. */
  private shownMedia(id: Id): MediaDto | undefined {
    const m = this.db.media.get(id);
    if (!m) return undefined;
    const group = this.versionGroup(id);
    if (group.length < 2) return { ...m, versions: undefined };
    return { ...m, versions: group.map((g) => ({ ...this.db.media.get(g)!, versions: undefined })) };
  }

  async allPictures(): Promise<MediaDto[]> {
    const out = [...this.db.media.values()]
      .filter((m) => !(m.mime ?? '').startsWith('audio/') && this.versionHead(m.id) === m.id)
      .map((m) => {
        const shown = this.shownMedia(m.id)!;
        return { ...shown, transcription: [], versions: shown.versions?.map((v) => ({ ...v, transcription: [] })) };
      });
    const key = (m: MediaDto) => (m.title ?? m.originalName ?? '').toLowerCase();
    out.sort((a, b) => key(a).localeCompare(key(b)) || a.id.localeCompare(b.id));
    return structuredClone(out);
  }

  /** Media no record links to: left by a deleted note or person, kept so that Undo can bring the record back. */
  async unlinkedMedia(): Promise<MediaDto[]> {
    const linked = new Set<Id>();
    const take = (links: MediaLinkDto[] | undefined) => links?.forEach((l) => linked.add(l.media.id));
    for (const p of this.db.persons.values()) {
      take(p.media);
      for (const e of p.events) take(e.media);
      for (const n of p.notes) take(n.media);
    }
    return [...this.db.media.values()].filter((m) => !linked.has(m.id)).map((m) => structuredClone(m));
  }

  async exifSuggestion(media: Id): Promise<ExifSuggestion | null> {
    if (!this.db.media.has(media)) throw missing('media item', media);
    const h = hashString(media);
    if (h % 3 === 0) return null;
    return { date: `${1 + (h % 28)} Jun ${1900 + (h % 90)}`, lat: h % 2 ? 53.55 : null, lon: h % 2 ? 9.99 : null };
  }

  // ------------------------------------------------------------------ export

  async previewExport(opts: ExportOptions): Promise<PrivacyPreview> {
    let living = 0;
    let priv = 0;
    let privateEvents = 0;
    for (const p of this.db.persons.values()) {
      if (this.isLiving(p)) living++;
      else if (p.private) priv++;
      privateEvents += p.events.filter((e) => e.private).length;
    }
    const total = this.db.persons.size;
    const hidden = (opts.living === 'exclude' ? living : 0) + (opts.includePrivate ? 0 : priv);
    const placeholder = opts.living === 'placeholder' ? living : 0;
    const token = `pv-${hashString(JSON.stringify(opts)).toString(16)}-${this.version}`;
    this.previews.set(token, opts);
    return {
      options: opts,
      defaultVersion: '7.0',
      exportedPersons: total - hidden - placeholder,
      placeholderPersons: placeholder,
      hiddenPersons: hidden,
      removedRecords: hidden + (opts.includePrivate ? 0 : privateEvents),
      mentionWarnings: [],
      groupMedia: [],
      losslessNotes: opts.version === '5.5.1' ? ['Event kinds without a GEDCOM 5.5.1 tag are written as custom events.'] : [],
      token,
    };
  }

  async exportGedcom(opts: ExportOptions, _dest: string, previewToken: string): Promise<{ bytes: number }> {
    const made = this.previews.get(previewToken);
    if (!made || JSON.stringify(made) !== JSON.stringify(opts) || !previewToken.endsWith(`-${this.version}`)) {
      throw new MockApiError('stale', 'The preview no longer matches your choices or the tree. Preview again, then export.');
    }
    return { bytes: 2_000 + this.db.persons.size * 140 };
  }

  /** Paths taken before anything is made (tests set it); folders made by `makePictureFolder` are added. */
  existingPaths = new Set<string>();
  /** Folders made by `makePictureFolder`, in order. */
  madeFolders: string[] = [];

  async makePictureFolder(dir: string, name: string): Promise<string> {
    const clean = name.trim();
    if (!clean || clean === '.' || clean === '..' || /[\\/]/.test(clean)) throw invalid('There is no folder name for the sheets. Choose a file name and try again.');
    if (!dir.trim()) throw invalid('That folder does not exist. Choose another folder and try again.');
    const sep = dir.includes('\\') && !dir.includes('/') ? '\\' : '/';
    const base = dir.replace(/[\\/]+$/, '');
    for (let n = 1; n < 1000; n++) {
      const path = `${base}${sep}${n === 1 ? clean : `${clean} (${n})`}`;
      if (this.existingPaths.has(path)) continue;
      this.existingPaths.add(path);
      this.madeFolders.push(path);
      return path;
    }
    throw new MockApiError('io', 'Could not make a folder for the sheets: too many folders of that name. Choose another name and try again.');
  }

  /** Pictures "saved" by `savePicture`, newest last (the mock writes no file). */
  savedPictures: { dest: string; bytes: Uint8Array }[] = [];

  async savePicture(dest: string, bytes: Uint8Array): Promise<{ bytes: number }> {
    if (!dest.trim()) throw invalid('There is no file name to save the picture to. Choose where to save it and try again.');
    const ext = /\.([^./\\]+)$/.exec(dest)?.[1].toLowerCase();
    if (ext !== 'png' && ext !== 'svg') throw invalid('Pictures are saved as .png or .svg files. Add one of those endings to the file name and try again.');
    const png = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    const ok =
      ext === 'png'
        ? bytes.length > png.length && png.every((b, i) => bytes[i] === b)
        : /^\s*(<svg|<\?xml)/.test(new TextDecoder().decode(bytes.slice(0, 64)));
    if (!ok) throw invalid('The picture could not be made. Try saving it again.');
    this.savedPictures.push({ dest, bytes: bytes.slice() });
    return { bytes: bytes.length };
  }

  // ---------------------------------------------------------------- settings

  async getSettings(): Promise<Settings> {
    return { ...this.settings };
  }

  async setSettings(patch: Partial<Settings>): Promise<Settings> {
    const next = { ...this.settings, ...patch };
    if (patch.homePerson && !this.db.persons.has(patch.homePerson)) throw missing('person', patch.homePerson);
    next.backupsToKeep = Math.max(1, Math.min(100, Math.round(next.backupsToKeep)));
    next.livingMaxAge = Math.max(1, Math.min(150, Math.round(next.livingMaxAge)));
    next.treeName = next.treeName.trim();
    this.settings = next;
    this.touch();
    return { ...this.settings };
  }

  // ---------------------------------------------------------------- agent access (in memory)

  private agent: AgentAccessState = { enabled: false, port: 8765, running: false, error: null, requireKey: false, keylessCommand: null, connections: [] };
  private agentSeq = 0;
  /** Ports another program already holds, for tests. */
  agentTakenPorts = new Set<number>();

  private agentSnapshot(): AgentAccessState {
    const keylessCommand = this.agent.requireKey ? null : `claude mcp add --transport http gene-tree http://127.0.0.1:${this.agent.port}/mcp`;
    return { ...this.agent, keylessCommand, connections: this.agent.connections.map((c) => ({ ...c })) };
  }

  async agentAccessRequireKey(required: boolean): Promise<AgentAccessState> {
    this.agent = { ...this.agent, requireKey: required };
    return this.agentSnapshot();
  }

  async agentAccessState(): Promise<AgentAccessState> {
    return this.agentSnapshot();
  }

  async agentAccessSet(enabled: boolean, port: number): Promise<AgentAccessState> {
    if (!Number.isInteger(port) || port < 1024 || port > 65535) throw invalid('The port must be a number from 1024 to 65535.');
    const taken = enabled && this.agentTakenPorts.has(port);
    // Like the backend: a server that works keeps working when the move cannot happen.
    if (taken && this.agent.running) {
      this.agent = { ...this.agent, error: `Port ${port} is already used by another program. Choose another port.` };
      return this.agentSnapshot();
    }
    this.agent = {
      ...this.agent,
      enabled,
      port,
      running: enabled && !taken,
      error: taken ? `Port ${port} is already used by another program. Choose another port.` : null,
    };
    return this.agentSnapshot();
  }

  async agentConnectionCreate(name: string): Promise<NewConnectionDto> {
    const clean = name.trim();
    if (!clean) throw invalid('Give the connection a name.');
    if ([...clean].length > 100) throw invalid('Keep the connection name to 100 characters or fewer.');
    // eslint-disable-next-line no-control-regex
    if (/[\u0000-\u001f\u007f-\u009f]/.test(clean)) throw invalid('The connection name cannot contain line breaks or other control characters.');
    if (isBuiltinAgentName(clean) || this.agent.connections.some((c) => c.name.toLowerCase() === clean.toLowerCase())) {
      throw invalid(`A connection named "${clean}" already exists. Choose another name.`);
    }
    const bytes = new Uint8Array(16);
    globalThis.crypto.getRandomValues(bytes);
    const key = 'gt_' + Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
    const connection = { id: `agent-conn-${++this.agentSeq}`, name: clean, created: Date.now() };
    this.agent.connections = [...this.agent.connections, connection];
    const snippet = `claude mcp add --transport http gene-tree http://127.0.0.1:${this.agent.port}/mcp --header "Authorization: Bearer ${key}"`;
    return { connection: { ...connection }, key, snippet };
  }

  async agentConnectionRevoke(id: string): Promise<void> {
    if (!this.agent.connections.some((c) => c.id === id)) throw new MockApiError('not_found', `No connection with id "${id}".`);
    this.agent.connections = this.agent.connections.filter((c) => c.id !== id);
  }

  async agentRules(): Promise<string> {
    return [
      'Rules for agents working in this family tree',
      '',
      '1. Search before you add anyone, and never add a person who is already in the tree.',
      '2. State only what the source says. Mark anything you infer as an inference.',
      '3. Every fact you add names its source.',
      '4. You cannot delete people, change whether a person is living, or overwrite what the owner entered.',
    ].join('\n');
  }

  // ================================================================ internals

  private async info(dir: string, name?: string): Promise<ProjectInfo> {
    return {
      dir,
      name: name ?? (dir.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || this.projectName),
      personCount: [...this.db.persons.values()].filter((p) => !p.siblingMarker).length,
      homePerson: this.settings.homePerson,
      sourceVersion: '5.5.1',
      locationWarning: await this.checkLocation(dir),
    };
  }

  private now(): number {
    this.clock += 1000;
    return this.clock;
  }

  private touch(): void {
    this.version++;
    this.rowCache = null;
    this.sortCache.clear();
  }

  /** `before` is the database as the change found it: persons present in both versions get their field changes. */
  private record(label: string, op: HistoryEntry['op'], touched: Set<Id>, before?: Db): void {
    const fields = new Map<Id, FieldChangeDto[]>();
    for (const id of touched) {
      const was = before?.persons.get(id);
      const now = this.db.persons.get(id);
      if (was && now) fields.set(id, personFieldChanges(was, now));
    }
    this.log.push({ ts: this.now(), label, op, touched, fields });
    if (this.log.length > 2000) this.log.shift();
    const people = [...touched].flatMap((id) => {
      const p = this.db.persons.get(id);
      return p ? [{ id, name: this.ref(p).name, reviewed: null }] : [];
    });
    this.addLogEntry({
      actor: { kind: 'user' },
      batch: null,
      label,
      changes: people.map((p) => ({ kind: 'person', id: p.id, op, detail: p.name, fact: { kind: 'text', text: p.name }, sources: [], fields: fields.get(p.id) ?? [] })),
      people,
    });
  }

  private addLogEntry(row: Omit<AgentLogEntry, 'group' | 'ts' | 'reverted'>): AgentLogEntry {
    const entry: AgentLogEntry = { ...row, group: this.ids.next(), ts: this.now(), reverted: false };
    this.logRows.push(entry);
    return entry;
  }

  private logRow(e: AgentLogEntry): LogRowDto {
    return structuredClone({
      group: e.group,
      ts: e.ts,
      actor: e.actor,
      batch: e.batch,
      label: e.label,
      changes: e.changes,
      people: e.people.map((p) => ({ ...p, exists: this.db.persons.has(p.id), siblingMarker: this.db.persons.get(p.id)?.siblingMarker ?? false })),
      reverted: e.reverted,
      canUndo: e.actor.kind === 'agent' && !e.reverted,
    });
  }

  private logGroup(group: string): AgentLogEntry {
    const e = this.logRows.find((r) => r.group === group);
    if (!e) throw missing('change', group);
    return e;
  }

  /** A change that counts against the review: an agent's, not undone. */
  private standing(e: AgentLogEntry): boolean {
    return e.actor.kind === 'agent' && !e.reverted;
  }

  /** Undoes one agent change in place. Reports what stayed; a skipped change keeps the group open. */
  private revertGroup(e: AgentLogEntry): UndoReport {
    const report: UndoReport = { undone: 0, skipped: [] };
    if (!e.fact || e.reverted) return report;
    const { person, note, conflict } = e.fact;
    if (conflict) {
      const p = this.db.persons.get(person);
      const change = e.changes[0];
      report.skipped.push({ kind: change.kind, person: p ? this.ref(p) : null, detail: change.detail, fact: change.fact });
      return report;
    }
    if (this.db.persons.has(person)) this.execute({ type: 'deleteNote', target: { kind: 'person', id: person }, note });
    e.reverted = true;
    report.undone = 1;
    this.touch();
    this.addLogEntry({
      actor: { kind: 'user' },
      batch: null,
      label: `Undo: ${e.label}`,
      changes: e.changes.map((c) => ({ ...c, op: 'delete' })),
      people: e.people.map((p) => ({ ...p, reviewed: null })),
    });
    return report;
  }

  private currentUndo(): UndoState {
    const u = this.undoStack[this.undoStack.length - 1];
    const r = this.redoStack[this.redoStack.length - 1];
    return { canUndo: !!u, canRedo: !!r, undoLabel: u?.label ?? null, redoLabel: r?.label ?? null };
  }

  private relinkSources(p: PersonRec): void {
    const fix = (cs: CitationDto[]) => {
      for (const c of cs) if (c.source) c.source = this.db.sources.get(c.source.id) ?? c.source;
    };
    fix(p.citations);
    p.events.forEach((e) => fix(e.citations));
    p.names.forEach((n) => fix(n.citations));
  }

  private recountSources(): void {
    const counts = new Map<Id, number>();
    const add = (cs: CitationDto[]) => cs.forEach((c) => c.source && counts.set(c.source.id, (counts.get(c.source.id) ?? 0) + 1));
    for (const p of this.db.persons.values()) {
      add(p.citations);
      p.events.forEach((e) => add(e.citations));
      p.names.forEach((n) => add(n.citations));
    }
    for (const f of this.db.families.values()) {
      add(f.citations);
      f.events.forEach((e) => add(e.citations));
    }
    for (const s of this.db.sources.values()) s.citationCount = counts.get(s.id) ?? 0;
  }

  private person(id: Id): PersonRec {
    const p = this.db.persons.get(id);
    if (!p) throw missing('person', id);
    return p;
  }

  private family(id: Id): FamilyRec {
    const f = this.db.families.get(id);
    if (!f) throw missing('family', id);
    return f;
  }

  private chart(id: number): ChartState {
    const c = this.charts.get(id);
    if (!c) throw missing('chart', String(id));
    return c;
  }

  private wholeChart(id: number): ChartState {
    const c = this.chart(id);
    if (c.version !== this.version) throw new MockApiError('stale', 'The tree changed since this chart was opened. Reopen the chart.');
    if (!c.whole) throw invalid('This chart is not the whole tree. Open the whole tree first.');
    return c;
  }

  // --- derived person facts

  private prefName(p: PersonRec): NameDto {
    return p.names.find((n) => n.preferred) ?? p.names[0];
  }

  private birth(p: PersonRec): EventDto | undefined {
    return p.events.find((e) => e.kind === 'birth');
  }

  private death(p: PersonRec): EventDto | undefined {
    return p.events.find((e) => e.kind === 'death');
  }

  private birthSort(p: PersonRec): number | null {
    return this.birth(p)?.date?.sortMin ?? this.birth(p)?.date?.sortMax ?? null;
  }

  private computeLiving(p: PersonRec): boolean {
    if (p.livingOverride) return p.livingOverride === 'living';
    if (this.death(p)) return false;
    const y = yearOf(this.birth(p)?.date ?? null);
    return y !== null && REFERENCE_YEAR - y < this.settings.livingMaxAge;
  }

  private isLiving(p: PersonRec): boolean {
    return this.computeLiving(p);
  }

  private lifespan(p: PersonRec): string {
    return lifespanText(this.birth(p)?.date ?? null, this.death(p)?.date ?? null);
  }

  /** The surname of the birth name when the person is shown under another one (the same rule as the Rust index). */
  private birthSurname(p: PersonRec): string | undefined {
    if (p.siblingMarker) return undefined;
    const shown = this.prefName(p);
    if (!shown || shown.kind === 'birth') return undefined;
    const surnameOf = (n: NameDto) => [n.surnamePrefix, n.surnames.join(' ')].filter(Boolean).join(' ').trim();
    const birth = p.names.filter((n) => n.kind === 'birth' && !n.private).map(surnameOf).find((s) => s !== '');
    if (!birth || birth.toLowerCase() === surnameOf(shown).toLowerCase()) return undefined;
    return birth;
  }

  /** The earliest dated name of kind as_recorded (undated last, then their order); the same rule as the Rust index. */
  private asRecorded(p: PersonRec): string | undefined {
    if (p.siblingMarker) return undefined;
    const main = this.prefName(p);
    const list = p.names.filter((n) => n.kind === 'as_recorded' && !n.private && n !== main);
    const key = (n: NameDto) => n.date?.sortMin ?? Number.POSITIVE_INFINITY;
    const best = list.map((n, i) => ({ n, i })).sort((a, b) => key(a.n) - key(b.n) || a.i - b.i)[0]?.n;
    return best?.display?.trim() || undefined;
  }

  private ref(p: PersonRec): PersonRef {
    const birthSurname = this.birthSurname(p);
    const asRecorded = this.asRecorded(p);
    return {
      id: p.id,
      name: this.prefName(p)?.display ?? '',
      lifespan: this.lifespan(p),
      sex: p.sex,
      living: !p.siblingMarker && this.isLiving(p),
      private: p.private,
      ...(birthSurname ? { birthSurname } : {}),
      ...(asRecorded ? { asRecorded } : {}),
      siblingMarker: p.siblingMarker === true,
    };
  }

  private row(p: PersonRec): PersonRow {
    const n = this.prefName(p);
    const sources = new Set<Id>();
    const take = (cs: CitationDto[]) => cs.forEach((c) => sources.add(c.id));
    take(p.citations);
    p.events.forEach((e) => take(e.citations));
    p.names.forEach((nm) => take(nm.citations));
    return {
      ...this.ref(p),
      surname: n?.surnames.join(' ') ?? '',
      given: n?.given ?? '',
      birth: this.birth(p)?.date?.display ?? '',
      death: this.death(p)?.date?.display ?? '',
      sourceCount: sources.size,
      changed: p.changed,
    };
  }

  private rows() {
    if (!this.rowCache || this.rowCache.version !== this.version) {
      const rows: PersonRow[] = [];
      const folded = new Map<Id, string>();
      for (const p of this.db.persons.values()) {
        const r = this.row(p);
        rows.push(r);
        folded.set(p.id, fold(p.names.flatMap((n) => [n.display, ...modernVariants(n.display)]).join(' ')));
      }
      this.rowCache = { version: this.version, rows, folded };
    }
    return this.rowCache;
  }

  private sortedRows(q: PersonQuery): PersonRow[] {
    const key = JSON.stringify(q);
    const cached = this.sortCache.get(key);
    if (cached) return cached;
    const { rows, folded } = this.rows();
    const text = fold(q.text.trim());
    let out = rows.filter((r) => {
      if (r.siblingMarker) return false;
      if (text && !folded.get(r.id)!.includes(text)) return false;
      if (q.living === 'living' && !r.living) return false;
      if (q.living === 'deceased' && r.living) return false;
      if (q.noSource && r.sourceCount > 0) return false;
      if (q.noBirthDate && r.birth !== '') return false;
      if (q.privateOnly && !r.private) return false;
      return true;
    });
    const dir = q.desc ? -1 : 1;
    const birthKey = (r: PersonRow) => this.birthSort(this.db.persons.get(r.id)!) ?? Infinity;
    const cmp = (a: PersonRow, b: PersonRow): number => {
      let c = 0;
      switch (q.sort) {
        case 'surname':
          c = a.surname.localeCompare(b.surname, 'en') || a.given.localeCompare(b.given, 'en');
          break;
        case 'given':
          c = a.given.localeCompare(b.given, 'en') || a.surname.localeCompare(b.surname, 'en');
          break;
        case 'birth':
          c = birthKey(a) - birthKey(b);
          if (Number.isNaN(c)) c = 0;
          break;
        case 'changed':
          c = a.changed - b.changed;
          break;
      }
      return c * dir || a.id.localeCompare(b.id, 'en');
    };
    out = [...out].sort(cmp);
    this.sortCache.set(key, out);
    return out;
  }

  private summary(p: PersonRec): string {
    const part = (tag: string, e: EventDto | undefined) => {
      if (!e) return null;
      const bits = [e.date?.display, e.place?.name].filter(Boolean);
      return bits.length ? `${tag} ${bits.join(', ')}` : null;
    };
    return [part('b.', this.birth(p)), part('d.', this.death(p))].filter(Boolean).join('; ');
  }

  private unionSummary(f: FamilyRec): string | null {
    const m = f.events.find((e) => e.kind === 'marriage');
    if (!m) return null;
    const bits = [m.date?.display, m.place?.name].filter(Boolean);
    return bits.length ? `m. ${bits.join(', ')}` : null;
  }

  private parentSets(id: Id): ParentSetDto[] {
    const out: ParentSetDto[] = [];
    for (const f of this.db.families.values()) {
      const link = f.children.find((c) => c.person === id);
      if (!link) continue;
      const a = f.partner1 ? this.db.persons.get(f.partner1) : undefined;
      const b = f.partner2 ? this.db.persons.get(f.partner2) : undefined;
      out.push({
        familyId: f.id,
        parents: [a ? this.ref(a) : null, b ? this.ref(b) : null],
        rel: link.rel,
        preferred: link.preferred,
        summary: this.unionSummary(f),
      });
    }
    return out.sort((x, y) => Number(y.preferred) - Number(x.preferred));
  }

  private partnerships(id: Id): PartnershipDto[] {
    const out: PartnershipDto[] = [];
    for (const f of this.db.families.values()) {
      if (f.partner1 !== id && f.partner2 !== id) continue;
      const otherId = f.partner1 === id ? f.partner2 : f.partner1;
      const other = otherId ? this.db.persons.get(otherId) : undefined;
      out.push({
        familyId: f.id,
        partner: other ? this.ref(other) : null,
        kind: f.kind,
        summary: this.unionSummary(f),
        events: f.events,
        ...(f.partner1 !== id ? { focusSecond: true } : {}),
        children: f.children.flatMap((c) => {
          const cp = this.db.persons.get(c.person);
          return cp ? [{ person: this.ref(cp), rel: c.rel, birth: this.birth(cp)?.date?.display ?? '' }] : [];
        }),
      });
    }
    return out;
  }

  private preferredParents(id: Id): [Id | null, Id | null] {
    let best: FamilyRec | undefined;
    for (const f of this.db.families.values()) {
      const link = f.children.find((c) => c.person === id);
      if (!link) continue;
      if (!best || link.preferred) best = f;
      if (link.preferred) break;
    }
    return best ? [best.partner1, best.partner2] : [null, null];
  }

  private unionsOf(id: Id): { partner: Id | null; children: Id[]; married: boolean }[] {
    const out: { partner: Id | null; children: Id[]; married: boolean }[] = [];
    for (const f of this.db.families.values()) {
      if (f.partner1 !== id && f.partner2 !== id) continue;
      // The same rule as the backend's `Kin::married`: a couple is married unless recorded otherwise.
      out.push({ partner: f.partner1 === id ? f.partner2 : f.partner1, children: f.children.map((c) => c.person), married: f.kind !== 'unmarried' && f.kind !== 'other' });
    }
    return out;
  }

  // ------------------------------------------------------------------ map

  async placeEvents(q: PlaceEventsQuery): Promise<PlaceEventsPage> {
    const yearOf = (text: string): number | null => {
      const m = /(\d{4})/.exec(text);
      return m ? Number(m[1]) : null;
    };
    const place = this.db.places.get(q.place);
    if (!place) throw missing('place', q.place);
    const under = (id: Id | undefined) => {
      const p = id ? this.db.places.get(id) : undefined;
      return !!p && (p.id === place.id || p.full.endsWith(`, ${place.full}`));
    };
    const all: PlaceEventDto[] = [];
    for (const person of this.db.persons.values()) {
      if (person.private || person.siblingMarker) continue;
      const n = person.names.find((x) => x.preferred) ?? person.names[0];
      for (const e of person.events) {
        if (e.private || e.negative || !under(e.place?.id)) continue;
        const kind = e.kind === 'birth' || e.kind === 'death' ? e.kind : e.kind === 'marriage' ? 'marriage' : 'other';
        all.push({
          key: `e:${e.id}`, source: 'tree', ours: true, kind, kindDetail: e.kind, date: e.date?.display ?? '',
          year: yearOf(e.date?.display ?? ''), place: e.place?.id ?? null,
          people: [{ name: n?.display ?? '', asWritten: '', role: '', age: '', estate: '', note: '', person: person.id, living: false }],
          book: '', page: '', recId: '', note: '', unsure: false,
        });
      }
    }
    for (const r of this.placeRecords) if (under(r.place)) all.push(r);
    const fold1 = (x: string) => x.toLowerCase();
    const who = q.who ?? 'all';
    const base = all.filter((e) => (who === 'all' || (who === 'ours') === e.ours) && (!q.query || e.people.some((p) => fold1(`${p.name} ${p.asWritten}`).includes(fold1(q.query!)))));
    const byKind = new Map<string, number>();
    const byDecade = new Map<number, number>();
    for (const e of base) {
      byKind.set(e.kind, (byKind.get(e.kind) ?? 0) + 1);
      if (e.year != null) byDecade.set(Math.floor(e.year / 10) * 10, (byDecade.get(Math.floor(e.year / 10) * 10) ?? 0) + 1);
    }
    const shown = base
      .filter((e) => (!q.kinds?.length || q.kinds.includes(e.kind)) && (q.yearFrom == null || (e.year != null && e.year >= q.yearFrom)) && (q.yearTo == null || (e.year != null && e.year <= q.yearTo)))
      .sort((a, b) => (a.year ?? 1e9) - (b.year ?? 1e9));
    const offset = q.offset ?? 0;
    const limit = Math.min(q.limit ?? 100, 500);
    return structuredClone({
      place,
      total: shown.length,
      offset,
      // As the app answers: the name of a place under the one asked about, nothing for the place itself.
      items: shown.slice(offset, offset + limit).map((e) => ({ ...e, place: e.place && e.place !== place.id ? (this.db.places.get(e.place)?.name ?? null) : null })),
      overviewTotal: base.length,
      byKind: [...byKind].map(([kind, count]) => ({ kind, count })),
      byDecade: [...byDecade].sort((a, b) => a[0] - b[0]).map(([decade, count]) => ({ decade, count })),
    });
  }

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  async placeImportScan(_paths: string[]): Promise<PlaceImportScanDto> {
    return structuredClone(this.placeImportSample);
  }

  async placeImportPreview(_paths: string[], mapping: PlaceImportMapDto[]): Promise<PlaceImportPreviewDto> {
    const lines = this.placeImportSample.groups.map((g) => {
      const m = mapping.find((x) => x.book === g.book && x.village === g.village);
      const dup = m?.place && this.placeImported.has(`${g.book}|${g.village}`) ? g.records : 0;
      return { book: g.book, village: g.village, place: m?.place ?? null, records: g.records, new: m?.place ? g.records - dup : 0, duplicates: dup };
    });
    return {
      lines,
      totalNew: lines.reduce((n, l) => n + l.new, 0),
      totalDuplicates: lines.reduce((n, l) => n + l.duplicates, 0),
      unmapped: lines.filter((l) => !l.place).reduce((n, l) => n + l.records, 0),
    };
  }

  async placeImportRun(paths: string[], mapping: PlaceImportMapDto[], onProgress?: (p: PlaceImportProgressDto) => void): Promise<PlaceImportResultDto> {
    const pre = await this.placeImportPreview(paths, mapping);
    const total = pre.totalNew;
    onProgress?.({ done: total, total });
    for (const l of pre.lines) if (l.place) this.placeImported.add(`${l.book}|${l.village}`);
    return { added: pre.totalNew, duplicates: pre.totalDuplicates, unmapped: pre.unmapped };
  }

  async mapData(scope: MapScope): Promise<MapDataDto> {
    const parents = (p: Id): Id[] => [...new Set([...this.db.families.values()].filter((f) => f.children.some((c) => c.person === p)).flatMap((f) => [f.partner1, f.partner2]).filter((x): x is Id => !!x))];
    const real = (id: Id) => {
      const p = this.db.persons.get(id);
      return !!p && !p.siblingMarker && !p.private;
    };
    const gen = new Map<Id, number>();
    const links: MapLinkDto[] = [];
    if (scope.kind === 'all') {
      for (const p of this.db.persons.values()) if (real(p.id)) gen.set(p.id, 0);
    } else if (scope.person && real(scope.person)) {
      gen.set(scope.person, 0);
      const up = scope.kind === 'ancestors';
      const max = scope.kind === 'person' ? 0 : (scope.generations ?? 40);
      const todo = [scope.person];
      for (let i = 0; i < todo.length; i++) {
        const g = gen.get(todo[i])!;
        if (g >= max) continue;
        for (const q of up ? parents(todo[i]) : this.childrenOf(todo[i])) {
          if (!real(q)) continue;
          links.push(up ? { parent: q, child: todo[i] } : { parent: todo[i], child: q });
          if (!gen.has(q)) {
            gen.set(q, g + 1);
            todo.push(q);
          }
        }
      }
    }
    const places = new Map<Id, MapPlaceDto>();
    const events: MapEventDto[] = [];
    let without = 0;
    const take = (e: EventDto, person: Id) => {
      if (e.private) return;
      if (!e.place) {
        without++;
        return;
      }
      if (!places.has(e.place.id)) {
        const c = this.db.coords?.get(e.place.id);
        places.set(e.place.id, { id: e.place.id, label: e.place.full, name: e.place.name, lat: c ? c[0] : null, lon: c ? c[1] : null });
      }
      events.push({ id: e.id, kind: e.kind, year: yearOf(e.date), place: e.place.id, person });
    };
    for (const [id] of gen) {
      const p = this.person(id);
      for (const e of p.events) if (e.participants.length === 0 || e.participants.some((x) => x.person.id === id && x.role === 'principal')) take(e, id);
    }
    for (const f of this.db.families.values()) {
      for (const who of [f.partner1, f.partner2]) if (who && gen.has(who)) for (const e of f.events) take(e, who);
    }
    events.sort((a, b) => (a.year ?? -1e9) - (b.year ?? -1e9));
    const people: MapPersonDto[] = [...gen].map(([id, g]) => {
      const p = this.person(id);
      const n = p.names.find((x) => x.preferred) ?? p.names[0];
      return { id, name: n?.display ?? '', surname: n?.surnames[0] ?? '', generation: g, living: this.computeLiving(p) };
    });
    const layers = [...(this.db.layers?.values() ?? [])].map((l) => ({ ...l, title: this.db.media.get(l.media)?.title ?? l.title })).sort((a, b) => a.pos - b.pos);
    return { places: [...places.values()], people, events, links, eventsWithoutPlace: without, layers: structuredClone(layers) };
  }

  async addMapLayerFile(path: string, title: string | null, layer: MapLayerInput, oziMapPath?: string | null): Promise<Id> {
    const name = path.split(/[\\/]/).pop() ?? '';
    if (!/\.(jpe?g|png|webp|gif|tiff?)$/i.test(name)) throw invalid('Only a picture (JPEG, PNG, WebP, GIF or TIFF) can be laid on the Map.');
    const before = this.db;
    this.db = structuredClone(before);
    const id = this.ids.next();
    const m: MediaDto = {
      id,
      title: title?.trim() || name.replace(/\.[^.]+$/, ''),
      mime: mockMime(name),
      originalName: name,
      date: null,
      description: null,
      resolved: true,
      width: 2000,
      height: 1500,
      sha256: null,
      transcription: [],
      rotation: 0,
    };
    this.db.media.set(id, m);
    const layers = (this.db.layers ??= new Map());
    const pos = Math.max(-1, ...[...layers.values()].map((l) => l.pos)) + 1;
    const corners = layer.corners ?? (oziMapPath ? MOCK_OZI_CORNERS : null);
    layers.set(id, {
      media: id,
      title: m.title,
      series: layer.series ?? null,
      year: layer.year ?? null,
      source: layer.source ?? null,
      corners: corners ? structuredClone(corners) : null,
      opacity: layer.opacity,
      visible: layer.visible,
      pos,
      width: 2000,
      height: 1500,
    });
    this.undoStack.push({ label: 'Lay map on the Map', db: before });
    this.redoStack = [];
    this.touch();
    return id;
  }

  async findOziMap(path: string): Promise<string | null> {
    return /sheet/i.test(path) ? path.replace(/\.[^.]+$/, '_ozf.map') : null;
  }

  async oziCorners(mapPath: string, media: Id): Promise<OziPlacementDto> {
    if (!this.db.media.has(media)) throw missing('media', media);
    if (!/\.map$/i.test(mapPath)) throw invalid('This is not an OziExplorer .map file.');
    return { corners: structuredClone(MOCK_OZI_CORNERS), fits: !/crop/i.test(mapPath) };
  }

  /** In the mock a picture whose name says "crop" does not fit its .map file. */
  async oziCheck(mapPath: string, picturePath: string): Promise<OziPlacementDto> {
    if (!/\.map$/i.test(mapPath)) throw invalid('This is not an OziExplorer .map file.');
    return { corners: structuredClone(MOCK_OZI_CORNERS), fits: !/crop/i.test(picturePath) };
  }

  private gaz = new Set<string>();
  /** Place records about people outside the tree (a test or a screenshot seeds them). */
  placeRecords: (PlaceEventDto & { place: Id; ourPerson?: Id })[] = [];
  /** What `placeImportScan` answers, whatever the paths: the mock reads no files. */
  placeImportSample: PlaceImportScanDto = { files: 0, rows: 0, badRows: 0, groups: [] };
  private placeImported = new Set<string>();

  async gazetteerStatus(): Promise<GazetteerStatusDto> {
    return { countries: [...this.gaz].map((country) => ({ country, rows: MOCK_GAZETTEER.length, bytes: 1_200_000 })), attribution: 'Place data: GeoNames (geonames.org), CC BY 4.0.' };
  }

  async gazetteerRemoteSize(country: string): Promise<number | null> {
    return country.toUpperCase() === 'ZZ' ? 4_300_000 : null;
  }

  async gazetteerDownload(country: string, onProgress?: (p: GazetteerProgressDto) => void): Promise<GazetteerCountryDto> {
    const cc = country.toUpperCase();
    if (cc !== 'ZZ') throw new MockApiError('not_found', `GeoNames has no file for ${cc}.`);
    onProgress?.({ country: cc, phase: 'download', done: 4_300_000, total: 4_300_000 });
    this.gaz.add(cc);
    return { country: cc, rows: MOCK_GAZETTEER.length, bytes: 1_200_000 };
  }

  async gazetteerRemove(country: string): Promise<void> {
    this.gaz.delete(country.toUpperCase());
  }

  async gazetteerSuggest(place: Id, limit: number): Promise<GazetteerCandidateDto[]> {
    const pl = this.db.places.get(place);
    if (!pl || this.gaz.size === 0) return [];
    const parts = pl.full.split(',').map((x) => fold(x.trim()));
    const out: GazetteerCandidateDto[] = [];
    for (const g of MOCK_GAZETTEER) {
      if (fold(g.name) !== parts[0]) continue;
      const region_match = parts.slice(1).some((x) => fold(g.region).includes(x));
      out.push({ geonameId: g.id, country: 'ZZ', name: g.name, local: g.local, lat: g.lat, lon: g.lon, code: 'PPL', population: g.pop, region: g.region, regionLocal: g.region, score: 100 + (region_match ? 30 : 0), regionMatch: region_match });
    }
    return out.sort((a, b) => b.score - a.score).slice(0, limit);
  }

  private childrenOf(id: Id): Id[] {
    const seen = new Set<Id>();
    const out: Id[] = [];
    for (const f of this.db.families.values()) {
      if (f.partner1 !== id && f.partner2 !== id) continue;
      for (const c of f.children) {
        if (seen.has(c.person)) continue;
        seen.add(c.person);
        out.push(c.person);
      }
    }
    return out;
  }

  // ------------------------------------------------------------ edit helpers

  private ensurePlace(full: string): PlaceOption {
    const parts = full.split(',').map((s) => s.trim()).filter(Boolean);
    if (parts.length === 0) throw invalid('Enter a place name.');
    let leaf: PlaceOption | null = null;
    for (let i = parts.length - 1; i >= 0; i--) {
      const text = parts.slice(i).join(', ');
      let found: PlaceOption | undefined;
      for (const p of this.db.places.values()) if (p.full.toLowerCase() === text.toLowerCase()) found = p;
      if (!found) {
        found = { id: this.ids.next(), name: parts[i], full: text, kind: null };
        this.db.places.set(found.id, found);
      }
      leaf = found;
    }
    return leaf!;
  }

  private newPerson(draft: PersonDraft, sex: SexKind): PersonRec {
    const id = this.ids.next();
    const given = draft.given?.trim() || undefined;
    const surnames = draft.surnames.map((s) => s.trim()).filter(Boolean);
    const patronymic = draft.patronymic?.trim() || undefined;
    const events: EventDto[] = [];
    if (draft.birthDateRaw?.trim() || draft.birthPlaceText?.trim()) {
      events.push(
        makeEvent(this.ids.next(), 'birth', {
          dateRaw: draft.birthDateRaw?.trim() || null,
          place: draft.birthPlaceText?.trim() ? this.ensurePlace(draft.birthPlaceText) : null,
          order: this.settings.dayOrder,
          preferred: true,
        }),
      );
    }
    const p: PersonRec = {
      id,
      sex,
      sexText: null,
      livingOverride: null,
      private: false,
      names: [unnamedOk(makeName(this.ids.next(), { kind: 'birth', preferred: true, private: false, given, surnames, patronymic }))],
      events,
      notes: [],
      media: [],
      citations: [],
      changed: this.now(),
    };
    this.db.persons.set(id, p);
    return p;
  }

  private newFamily(partner1: Id | null, partner2: Id | null): FamilyRec {
    const f: FamilyRec = {
      id: this.ids.next(),
      partner1,
      partner2,
      kind: 'unknown',
      events: [],
      children: [],
      notes: [],
      citations: [],
    };
    this.db.families.set(f.id, f);
    return f;
  }

  private parentFamilies(id: Id): FamilyRec[] {
    return [...this.db.families.values()].filter((f) => f.children.some((c) => c.person === id));
  }

  private partnerFamilies(id: Id): FamilyRec[] {
    return [...this.db.families.values()].filter((f) => f.partner1 === id || f.partner2 === id);
  }

  private preferredParentFamily(id: Id): FamilyRec | undefined {
    const fams = this.parentFamilies(id);
    return fams.find((f) => f.children.find((c) => c.person === id)?.preferred) ?? fams[0];
  }

  private unsetPreferredElsewhere(child: Id, keep: Id): void {
    for (const f of this.db.families.values()) {
      if (f.id === keep) continue;
      for (const c of f.children) if (c.person === child) c.preferred = false;
    }
  }

  private findEvent(id: Id): { ev: EventDto; owner: { kind: 'person' | 'family'; rec: PersonRec | FamilyRec } } | null {
    for (const p of this.db.persons.values()) {
      const ev = p.events.find((e) => e.id === id);
      if (ev) return { ev, owner: { kind: 'person', rec: p } };
    }
    for (const f of this.db.families.values()) {
      const ev = f.events.find((e) => e.id === id);
      if (ev) return { ev, owner: { kind: 'family', rec: f } };
    }
    return null;
  }

  private findNote(id: Id): NoteDto | null {
    const scan = (ns: NoteDto[]) => ns.find((n) => n.id === id) ?? null;
    for (const p of this.db.persons.values()) {
      const hit = scan(p.notes) ?? p.events.map((e) => scan(e.notes)).find(Boolean) ?? null;
      if (hit) return hit;
    }
    for (const f of this.db.families.values()) {
      const hit = scan(f.notes) ?? f.events.map((e) => scan(e.notes)).find(Boolean) ?? null;
      if (hit) return hit;
    }
    return null;
  }

  /** The arrays a note, citation or media link can be attached to. */
  private owner(t: OwnerRef): OwnerBag {
    switch (t.kind) {
      case 'person': {
        const p = this.person(t.id);
        return { notes: p.notes, citations: p.citations, media: p.media };
      }
      case 'family': {
        const f = this.family(t.id);
        return { notes: f.notes, citations: f.citations, media: (f.media ??= []) };
      }
      case 'citation': {
        const c = this.findCitation(t.id);
        if (!c) throw missing('citation', t.id);
        return { media: (c.media ??= []) };
      }
      case 'event': {
        const e = this.findEvent(t.id);
        if (!e) throw missing('event', t.id);
        return { notes: e.ev.notes, citations: e.ev.citations, media: e.ev.media };
      }
      case 'name': {
        for (const p of this.db.persons.values()) {
          const n = p.names.find((x) => x.id === t.id);
          if (n) return { citations: n.citations };
        }
        throw missing('name', t.id);
      }
      case 'source': {
        const s = this.db.sources.get(t.id);
        if (!s) throw missing('source', t.id);
        return { media: s.media };
      }      case 'note': {
        const n = this.findNote(t.id);
        if (!n) throw missing('note', t.id);
        return { media: n.media };
      }
      default:
        throw invalid(`The mock does not attach records to a ${t.kind}.`);
    }
  }

  private findCitation(id: Id): CitationDto | null {
    const scan = (cs: CitationDto[]) => cs.find((c) => c.id === id) ?? null;
    for (const p of this.db.persons.values()) {
      const hit = scan(p.citations) ?? p.events.map((e) => scan(e.citations)).find(Boolean) ?? p.names.map((n) => scan(n.citations)).find(Boolean);
      if (hit) return hit;
    }
    for (const f of this.db.families.values()) {
      const hit = scan(f.citations) ?? f.events.map((e) => scan(e.citations)).find(Boolean);
      if (hit) return hit;
    }
    return null;
  }

  private buildCitation(c: CitationInput): CitationDto {
    let source: SourceDto;
    if (typeof c.source === 'string') {
      const s = this.db.sources.get(c.source);
      if (!s) throw missing('source', c.source);
      source = s;
    } else {
      const title = c.source.title.trim();
      if (!title) throw invalid('Enter a source title.');
      const existing = [...this.db.sources.values()].find((s) => (s.title ?? '').toLowerCase() === title.toLowerCase());
      source =
        existing ??
        (() => {
          const s: SourceDto = { id: this.ids.next(), title, author: null, publication: null, abbreviation: null, text: null, kind: null, issuer: null, docNumber: null, docDate: null, archive: null, callNumber: null, citationCount: 0, media: [] };
          this.db.sources.set(s.id, s);
          return s;
        })();
    }
    return {
      id: this.ids.next(),
      source,
      page: c.page ?? null,
      date: toDateDto(c.dateRaw, this.settings.dayOrder),
      text: c.text ?? null,
      quality: c.quality ?? null,
      evidence: {
        original: c.evidence?.original ?? null,
        primaryInfo: c.evidence?.primaryInfo ?? null,
        direct: c.evidence?.direct ?? null,
      },
    };
  }

  private buildEvent(input: EventInput, existing?: EventDto): EventDto {
    let place: PlaceOption | null = existing?.place ?? null;
    if (input.placeId) {
      place = this.db.places.get(input.placeId) ?? null;
      if (!place) throw missing('place', input.placeId);
    } else if (input.placeText !== undefined) {
      place = input.placeText.trim() ? this.ensurePlace(input.placeText) : null;
    }
    const ev = makeEvent(existing?.id ?? this.ids.next(), input.kind, {
      dateRaw: input.dateRaw,
      place,
      description: input.description ?? null,
      age: input.age ?? null,
      cause: input.cause ?? null,
      negative: input.negative,
      isPrivate: input.private,
      preferred: input.preferred,
      order: this.settings.dayOrder,
    });
    ev.participants = (input.participants ?? []).map((x) => ({ person: this.ref(this.person(x.person)), role: x.role }));
    const had = existing?.godparents ?? [];
    const principal = ev.participants.find((x) => x.role === 'principal')?.person.id ?? null;
    const gods = input.godparents ? input.godparents.map((g) => this.godparentFrom(g, had, principal)) : had;
    gods.forEach((g, i) => {
      if (gods.slice(0, i).some((o) => MockApi.sameGodparent(o, g))) throw invalid('The same godparent is in the list twice.');
    });
    if (gods.length > 0) ev.godparents = gods;
    if (existing) {
      ev.citations = existing.citations;
      ev.notes = existing.notes;
      ev.media = existing.media;
    }
    return ev;
  }

  // ---------------------------------------------------------------- execute

  private execute(cmd: EditCommand): { focus: Id | null; touched: Id[] } {
    switch (cmd.type) {
      case 'createPerson': {
        const p = this.newPerson(cmd.draft, cmd.draft.sex);
        return { focus: p.id, touched: [p.id] };
      }

      case 'updatePerson': {
        const p = this.person(cmd.id);
        if (cmd.sex !== undefined) p.sex = cmd.sex;
        if (cmd.sexText !== undefined) p.sexText = cmd.sexText || null;
        if ('livingOverride' in cmd) p.livingOverride = cmd.livingOverride ?? null;
        if (cmd.private !== undefined) p.private = cmd.private;
        return { focus: p.id, touched: [p.id] };
      }

      case 'deletePerson': {
        this.person(cmd.id);
        this.db.persons.delete(cmd.id);
        const touched: Id[] = [cmd.id];
        for (const f of [...this.db.families.values()]) {
          if (f.partner1 === cmd.id) f.partner1 = null;
          if (f.partner2 === cmd.id) f.partner2 = null;
          f.children = f.children.filter((c) => c.person !== cmd.id);
          if (!f.partner1 && !f.partner2 && f.children.length === 0) this.db.families.delete(f.id);
          else touched.push(f.id);
        }
        return { focus: null, touched };
      }

      case 'saveName': {
        const p = this.person(cmd.person);
        const input = cmd.name;
        if (!input.given?.trim() && !input.patronymic?.trim() && input.surnames.every((s) => !s.trim())) throw invalid('Enter a given name or a surname.');
        const idx = input.id ? p.names.findIndex((n) => n.id === input.id) : -1;
        if (input.id && idx < 0) throw missing('name', input.id);
        const made = makeName(input.id ?? this.ids.next(), input, this.settings.dayOrder);
        if (idx >= 0) {
          made.citations = p.names[idx].citations;
          p.names[idx] = made;
        } else p.names.push(made);
        if (made.preferred || p.names.length === 1) {
          made.preferred = true;
          for (const n of p.names) if (n !== made) n.preferred = false;
        } else if (!p.names.some((n) => n.preferred)) p.names[0].preferred = true;
        return { focus: p.id, touched: [p.id, made.id] };
      }

      case 'deleteName': {
        const p = this.person(cmd.person);
        const i = p.names.findIndex((n) => n.id === cmd.name);
        if (i < 0) throw missing('name', cmd.name);
        if (p.names.length === 1) throw invalid('A person needs at least one name. Edit this name instead.');
        const [gone] = p.names.splice(i, 1);
        if (gone.preferred) p.names[0].preferred = true;
        return { focus: p.id, touched: [p.id, cmd.name] };
      }

      case 'saveEvent': {
        const holder = cmd.owner.kind === 'person' ? this.person(cmd.owner.id) : this.family(cmd.owner.id);
        const i = cmd.event.id ? holder.events.findIndex((e) => e.id === cmd.event.id) : -1;
        if (cmd.event.id && i < 0) throw missing('event', cmd.event.id);
        const ev = this.buildEvent(cmd.event, i >= 0 ? holder.events[i] : undefined);
        if (i >= 0) holder.events[i] = ev;
        else holder.events.push(ev);
        return { focus: cmd.owner.kind === 'person' ? cmd.owner.id : null, touched: [cmd.owner.id, ev.id] };
      }

      case 'deleteEvent': {
        const holder = cmd.owner.kind === 'person' ? this.person(cmd.owner.id) : this.family(cmd.owner.id);
        const i = holder.events.findIndex((e) => e.id === cmd.event);
        if (i < 0) throw missing('event', cmd.event);
        holder.events.splice(i, 1);
        return { focus: cmd.owner.kind === 'person' ? cmd.owner.id : null, touched: [cmd.owner.id, cmd.event] };
      }

      case 'addGodparent': {
        const hit = this.findEvent(cmd.event);
        if (!hit) throw missing('event', cmd.event);
        if (hit.ev.kind !== 'baptism' && hit.ev.kind !== 'christening')
          throw invalid('Godparents go on a baptism or christening. Add that event first, then the godparents to it.');
        const principal = hit.owner.kind === 'person' ? hit.owner.rec.id : null;
        const g = this.godparentFrom({ ...cmd.godparent, id: undefined }, [], principal);
        const list = hit.ev.godparents ?? [];
        if (list.some((o) => MockApi.sameGodparent(o, g))) throw invalid('This godparent is on the event already.');
        hit.ev.godparents = [...list, g];
        return { focus: principal, touched: [hit.ev.id, ...(principal ? [principal] : [])] };
      }

      case 'removeGodparent': {
        const hit = this.findEvent(cmd.event);
        if (!hit) throw missing('event', cmd.event);
        const list = hit.ev.godparents ?? [];
        if (!list.some((g) => g.id === cmd.godparent)) throw missing('godparent', cmd.godparent);
        const rest = list.filter((g) => g.id !== cmd.godparent);
        if (rest.length > 0) hit.ev.godparents = rest;
        else delete hit.ev.godparents;
        const principal = hit.owner.kind === 'person' ? hit.owner.rec.id : null;
        return { focus: principal, touched: [hit.ev.id, ...(principal ? [principal] : [])] };
      }

      case 'quickAdd':
        return this.quickAdd(cmd);

      case 'linkExisting':
        return this.linkExisting(cmd);

      case 'saveFamily': {
        if (cmd.partner1 && cmd.partner1 === cmd.partner2) throw invalid('A family needs two different people.');
        if (cmd.partner1) this.person(cmd.partner1);
        if (cmd.partner2) this.person(cmd.partner2);
        const f = cmd.id ? this.family(cmd.id) : this.newFamily(null, null);
        if (f.children.some((c) => c.person === cmd.partner1 || c.person === cmd.partner2))
          throw invalid('A partner cannot also be a child of the same family. Unlink the child first.');
        f.partner1 = cmd.partner1;
        f.partner2 = cmd.partner2;
        f.kind = cmd.kind;
        return { focus: null, touched: [f.id, ...[cmd.partner1, cmd.partner2].filter((x): x is Id => !!x)] };
      }

      case 'deleteFamily': {
        const f = this.family(cmd.id);
        this.db.families.delete(cmd.id);
        for (const c of f.children) if (this.db.persons.get(c.person)?.siblingMarker) this.db.persons.delete(c.person);
        return { focus: null, touched: [f.id, ...[f.partner1, f.partner2, ...f.children.map((c) => c.person)].filter((x): x is Id => !!x)] };
      }

      case 'addSiblingMarker': {
        const f = this.family(cmd.family);
        if (f.children.some((c) => this.db.persons.get(c.person)?.siblingMarker))
          throw new MockApiError('conflict', 'This family already has an entry for unnamed brothers and sisters.');
        const id = this.ids.next();
        this.db.persons.set(id, { id, sex: 'unknown', sexText: null, livingOverride: null, private: false, names: [], events: [], notes: [], media: [], citations: [], changed: this.now(), siblingMarker: true });
        f.children.push({ person: id, rel: ['birth', 'birth'], preferred: true });
        return { focus: id, touched: [f.id, id] };
      }

      case 'linkChild': {
        const f = this.family(cmd.family);
        this.person(cmd.child);
        if (f.partner1 === cmd.child || f.partner2 === cmd.child) throw invalid('A person cannot be their own child.');
        if (f.children.some((c) => c.person === cmd.child)) throw new MockApiError('conflict', 'This person is already a child in the family.');
        f.children.push({ person: cmd.child, rel: cmd.rel, preferred: cmd.preferred });
        if (cmd.preferred) this.unsetPreferredElsewhere(cmd.child, f.id);
        return { focus: cmd.child, touched: [f.id, cmd.child] };
      }

      case 'updateChildLink': {
        const f = this.family(cmd.family);
        const link = f.children.find((c) => c.person === cmd.child);
        if (!link) throw missing('child link', cmd.child);
        link.rel = cmd.rel;
        link.preferred = cmd.preferred;
        if (cmd.preferred) this.unsetPreferredElsewhere(cmd.child, f.id);
        return { focus: cmd.child, touched: [f.id, cmd.child] };
      }

      case 'unlinkChild': {
        const f = this.family(cmd.family);
        const n = f.children.length;
        f.children = f.children.filter((c) => c.person !== cmd.child);
        if (f.children.length === n) throw missing('child link', cmd.child);
        if (this.db.persons.get(cmd.child)?.siblingMarker) this.db.persons.delete(cmd.child);
        return { focus: cmd.child, touched: [f.id, cmd.child] };
      }

      case 'reorderChildren': {
        const f = this.family(cmd.family);
        const have = new Set(f.children.map((c) => c.person));
        if (cmd.order.length !== have.size || !cmd.order.every((id) => have.has(id)) || new Set(cmd.order).size !== have.size)
          throw invalid('The new order must list each child of the family exactly once.');
        f.children = cmd.order.map((id) => f.children.find((c) => c.person === id)!);
        return { focus: null, touched: [f.id] };
      }

      case 'saveNote': {
        const bag = this.owner(cmd.target);
        if (!bag.notes) throw invalid('Notes cannot be attached to this record.');
        if (!cmd.note.text.trim()) throw invalid('Write some text for the note.');
        const i = cmd.note.id ? bag.notes.findIndex((n) => n.id === cmd.note.id) : -1;
        if (cmd.note.id && i < 0) throw missing('note', cmd.note.id);
        // A missing story leaves the note's story as it is; an empty one takes it off.
        const story = cmd.note.story === undefined ? undefined : cmd.note.story.trim() || null;
        if (i >= 0) {
          bag.notes[i] = { ...bag.notes[i], kind: cmd.note.kind, text: cmd.note.text, ...(story !== undefined ? { story } : {}) };
        } else {
          const made = makeNote(this.ids.next(), cmd.note.kind, cmd.note.text);
          bag.notes.push(story ? { ...made, story } : made);
        }
        const saved = bag.notes[i >= 0 ? i : bag.notes.length - 1];
        return { focus: cmd.target.kind === 'person' ? cmd.target.id : null, touched: [cmd.target.id, saved.id] };
      }

      case 'deleteNote': {
        const bag = this.owner(cmd.target);
        const i = bag.notes?.findIndex((n) => n.id === cmd.note) ?? -1;
        if (!bag.notes || i < 0) throw missing('note', cmd.note);
        bag.notes.splice(i, 1);
        return { focus: cmd.target.kind === 'person' ? cmd.target.id : null, touched: [cmd.target.id] };
      }

      case 'saveSource': {
        const input = cmd.source;
        if (input.id) {
          const s = this.db.sources.get(input.id);
          if (!s) throw missing('source', input.id);
          for (const k of ['title', 'author', 'publication', 'abbreviation', 'text', 'issuer'] as const) if (input[k] !== undefined) s[k] = input[k] || null;
          if (input.kind !== undefined) s.kind = input.kind.trim() || null;
          if (input.docNumber !== undefined) s.docNumber = input.docNumber || null;
          if (input.docDateRaw !== undefined) s.docDate = toDateDto(input.docDateRaw, this.settings.dayOrder);
          if (input.archive !== undefined) s.archive = input.archive.trim() || null;
          if (input.callNumber !== undefined) s.callNumber = input.callNumber.trim() || null;
          return { focus: null, touched: [s.id] };
        }
        if (!input.title?.trim()) throw invalid('Enter a source title.');
        const s: SourceDto = {
          id: this.ids.next(),
          title: input.title.trim(),
          author: input.author ?? null,
          publication: input.publication ?? null,
          abbreviation: input.abbreviation ?? null,
          text: input.text ?? null,
          kind: input.kind?.trim() || null,
          issuer: input.issuer || null,
          docNumber: input.docNumber || null,
          docDate: input.docDateRaw ? toDateDto(input.docDateRaw, this.settings.dayOrder) : null,
          archive: input.archive?.trim() || null,
          callNumber: input.callNumber?.trim() || null,
          citationCount: 0,
          media: [],
        };
        this.db.sources.set(s.id, s);
        return { focus: null, touched: [s.id] };
      }

      case 'addCitation': {
        const bag = this.owner(cmd.target);
        if (!bag.citations) throw invalid('Citations cannot be attached to this record.');
        let cit: CitationDto;
        const c = cmd.citation;
        if ('reuseLast' in c) {
          if (!this.lastCit) throw invalid('No citation to reuse.');
          cit = { ...structuredClone(this.lastCit), id: this.ids.next(), source: this.lastCit.source && (this.db.sources.get(this.lastCit.source.id) ?? this.lastCit.source) };
        } else if ('cloneOf' in c) {
          const src = this.findCitation(c.cloneOf);
          if (!src) throw missing('citation', c.cloneOf);
          cit = { ...structuredClone(src), id: this.ids.next(), source: src.source && (this.db.sources.get(src.source.id) ?? src.source) };
        } else cit = this.buildCitation(c);
        bag.citations.push(cit);
        this.lastCit = cit;
        return { focus: cmd.target.kind === 'person' ? cmd.target.id : null, touched: [cmd.target.id, cit.id] };
      }

      case 'updateCitation': {
        const cit = this.findCitation(cmd.id);
        if (!cit) throw missing('citation', cmd.id);
        const u = cmd.citation;
        if (u.source) {
          const s = this.db.sources.get(u.source);
          if (!s) throw missing('source', u.source);
          cit.source = s;
        }
        if (u.page !== undefined) cit.page = u.page || null;
        if (u.text !== undefined) cit.text = u.text || null;
        if (u.dateRaw !== undefined) cit.date = toDateDto(u.dateRaw, this.settings.dayOrder);
        if (u.quality !== undefined) cit.quality = u.quality;
        if (u.evidence) cit.evidence = { ...cit.evidence, ...Object.fromEntries(Object.entries(u.evidence).filter(([, v]) => v !== undefined)) };
        return { focus: null, touched: [cit.id] };
      }

      case 'removeCitation': {
        const bag = this.owner(cmd.target);
        const i = bag.citations?.findIndex((c) => c.id === cmd.citation) ?? -1;
        if (!bag.citations || i < 0) throw missing('citation', cmd.citation);
        bag.citations.splice(i, 1);
        return { focus: cmd.target.kind === 'person' ? cmd.target.id : null, touched: [cmd.target.id, cmd.citation] };
      }

      case 'attachMedia': {
        const bag = this.owner(cmd.target);
        if (!bag.media) throw invalid('Media cannot be attached to this record.');
        for (const id of cmd.media) {
          const m = this.db.media.get(id);
          if (!m) throw missing('media item', id);
          if (!bag.media.some((l) => l.media.id === id && !l.clip)) bag.media.push({ media: m, primary: !bag.media.some((l) => !l.clip) });
        }
        return { focus: cmd.target.kind === 'person' ? cmd.target.id : null, touched: [cmd.target.id, ...cmd.media] };
      }

      case 'detachMedia': {
        const bag = this.owner(cmd.target);
        const i = bag.media?.findIndex((l) => l.media.id === cmd.media && !l.clip) ?? -1;
        if (!bag.media || i < 0) throw missing('media link', cmd.media);
        const [gone] = bag.media.splice(i, 1);
        if (gone.primary && bag.media[0]) bag.media[0].primary = true;
        return { focus: cmd.target.kind === 'person' ? cmd.target.id : null, touched: [cmd.target.id, cmd.media] };
      }

      case 'deleteMedia': {
        if (!this.db.media.has(cmd.id)) throw missing('media item', cmd.id);
        const touched: Id[] = [cmd.id];
        // Every link to it goes, clips included, as in the backend.
        for (const { owner, links } of this.linkLists()) {
          const before = links.length;
          const wasPrimary = links.some((l) => l.primary && l.media.id === cmd.id);
          for (let i = links.length - 1; i >= 0; i--) if (links[i].media.id === cmd.id) links.splice(i, 1);
          if (links.length === before) continue;
          if (wasPrimary && links[0] && !links.some((l) => l.primary)) links[0].primary = true;
          touched.push(owner.id);
        }
        // The other versions of a deleted primary stay a group, under the largest of them.
        const versions = this.db.versionOf;
        if (versions && this.versionHead(cmd.id) === cmd.id) {
          const [next, ...more] = this.versionGroup(cmd.id).slice(1);
          if (next) {
            versions.delete(next);
            for (const v of more) versions.set(v, next);
          }
        }
        versions?.delete(cmd.id);
        this.db.media.delete(cmd.id);
        return { focus: null, touched };
      }

      case 'setPrimaryMedia': {
        const p = this.person(cmd.person);
        if (!p.media.some((l) => l.media.id === cmd.media)) throw missing('media link', cmd.media);
        for (const l of p.media) l.primary = l.media.id === cmd.media;
        return { focus: p.id, touched: [p.id, cmd.media] };
      }

      case 'importDraft':
        return this.importDraft(cmd);

      case 'updateMedia': {
        const m = this.db.media.get(cmd.id);
        if (!m) throw missing('media item', cmd.id);
        if (cmd.title !== undefined) m.title = cmd.title || null;
        if (cmd.description !== undefined) m.description = cmd.description || null;
        if (cmd.dateRaw !== undefined) m.date = toDateDto(cmd.dateRaw, this.settings.dayOrder);
        if (cmd.placeId) {
          const p = this.db.places.get(cmd.placeId);
          if (!p) throw missing('place', cmd.placeId);
          m.place = p;
        } else if (cmd.placeText !== undefined) {
          m.place = cmd.placeText.trim() ? this.ensurePlace(cmd.placeText) : undefined;
        }
        return { focus: null, touched: [m.id] };
      }

      case 'setMediaTranscription': {
        const m = this.db.media.get(cmd.id);
        if (!m) throw missing('media item', cmd.id);
        const seen = new Set<Id>();
        const sound = (m.mime ?? '').startsWith('audio/');
        const next = cmd.regions.map((r) => {
          if (sound) {
            if (r.area || r.rotation) throw invalid('A sound recording has no picture: give each piece its text, and its start and end times, without an area or a rotation.');
            if ((r.startMs == null) !== (r.endMs == null)) throw invalid('Give a piece of a transcript both its start and its end, or neither.');
            if (r.startMs != null && r.endMs != null && r.endMs <= r.startMs) throw invalid('A piece of a transcript must end after it starts.');
            const id = r.id ?? this.ids.next();
            if (seen.has(id)) throw invalid('Two pieces of the transcription have the same id.');
            seen.add(id);
            const speaker = r.speaker?.trim();
            return {
              id,
              area: null,
              rotation: 0 as const,
              text: r.text,
              ...(r.startMs != null ? { startMs: r.startMs, endMs: r.endMs } : {}),
              ...(speaker ? { speaker } : {}),
            };
          }
          if (r.startMs != null || r.endMs != null || r.speaker) throw invalid('Times and speakers belong to the transcript of a sound recording.');
          const rotation = r.rotation ?? 0;
          if (![0, 90, 180, 270].includes(rotation)) throw invalid('rotation must be 0, 90, 180 or 270.');
          const a = r.area;
          if (a && (![a.x, a.y, a.w, a.h].every(Number.isFinite) || a.w <= 0 || a.h <= 0 || a.x < 0 || a.y < 0 || a.x > 1 || a.y > 1))
            throw invalid('An area must lie on the picture.');
          const id = r.id ?? this.ids.next();
          if (seen.has(id)) throw invalid('Two pieces of the transcription have the same id.');
          seen.add(id);
          return { id, area: a ? { x: a.x, y: a.y, w: Math.min(a.w, 1 - a.x), h: Math.min(a.h, 1 - a.y) } : null, rotation, text: r.text };
        });
        // As the backend: pieces stored since the sender read the list stay, after the piece they followed.
        if (cmd.known) {
          let after: Id | null = null;
          for (const r of m.transcription) {
            if (!cmd.known.includes(r.id) && !next.some((n) => n.id === r.id)) {
              const i = after === null ? -1 : next.findIndex((n) => n.id === after);
              next.splice(i + 1, 0, structuredClone(r));
            }
            after = r.id;
          }
        }
        m.transcription = next;
        return { focus: null, touched: [m.id] };
      }

      case 'addAudioClip':
      case 'updateAudioClip': {
        const label = cmd.label?.trim() ? cmd.label.trim() : null;
        if (label && label.length > 200) throw invalid("A clip's label holds at most 200 characters.");
        if (cmd.endMs <= cmd.startMs || cmd.startMs < 0) throw invalid('A clip must end after it starts.');
        const clipBag = (t: OwnerRef) => {
          if (!['note', 'citation', 'event', 'person', 'family'].includes(t.kind)) throw invalid('A clip goes on a note, citation, event, person or family.');
          const bag = this.owner(t);
          if (!bag.media) throw invalid('A clip cannot be attached to this record.');
          return bag.media;
        };
        let media: MediaDto;
        let id: Id;
        if (cmd.type === 'addAudioClip') {
          const m = this.db.media.get(cmd.media);
          if (!m) throw missing('media item', cmd.media);
          if (!(m.mime ?? '').startsWith('audio/')) throw invalid('Only a sound recording has clips.');
          media = m;
          id = this.ids.next();
        } else {
          const links = clipBag(cmd.target);
          const i = links.findIndex((l) => l.clip?.id === cmd.clip);
          if (i < 0) throw missing('clip', cmd.clip);
          media = links[i].media;
          id = cmd.clip;
          links.splice(i, 1);
        }
        if (media.durationMs != null && cmd.endMs > media.durationMs) throw invalid(`The recording lasts ${clockMs(media.durationMs)}; a clip must end by then.`);
        const to = cmd.type === 'updateAudioClip' && cmd.moveTo ? cmd.moveTo : cmd.target;
        const links = clipBag(to);
        if (links.some((l) => l.media.id === media.id && l.clip && l.clip.startMs === cmd.startMs && l.clip.endMs === cmd.endMs && l.clip.label === label))
          throw invalid('That clip is already on this record.');
        links.push({ media, primary: false, clip: { id, startMs: cmd.startMs, endMs: cmd.endMs, label } });
        return { focus: null, touched: [to.id, media.id, id] };
      }

      case 'deleteAudioClip': {
        const links = this.owner(cmd.target).media;
        const i = links?.findIndex((l) => l.clip?.id === cmd.clip) ?? -1;
        if (!links || i < 0) throw missing('clip', cmd.clip);
        links.splice(i, 1);
        return { focus: null, touched: [cmd.target.id, cmd.clip] };
      }

      case 'groupMediaVersions': {
        const versions = (this.db.versionOf ??= new Map());
        const head = this.db.media.get(cmd.primary);
        if (!head) throw missing('media item', cmd.primary);
        const members = new Set<Id>();
        for (const o of [cmd.primary, ...cmd.others]) {
          const m = this.db.media.get(o);
          if (!m) throw missing('media item', o);
          if ((m.mime ?? '').startsWith('audio/') !== (head.mime ?? '').startsWith('audio/')) {
            throw invalid('A sound recording and a picture cannot be versions of one another.');
          }
          for (const g of this.versionGroup(o)) members.add(g);
        }
        members.delete(cmd.primary);
        if (members.size === 0) throw invalid('Choose another file of the same page.');
        versions.delete(cmd.primary);
        for (const m of members) versions.set(m, cmd.primary);
        return { focus: null, touched: [cmd.primary, ...members] };
      }

      case 'ungroupMediaVersion': {
        const versions = (this.db.versionOf ??= new Map());
        if (!this.db.media.has(cmd.media)) throw missing('media item', cmd.media);
        const group = this.versionGroup(cmd.media);
        if (group.length < 2) throw invalid('This file has no other versions.');
        if (group[0] !== cmd.media) {
          versions.delete(cmd.media);
          return { focus: null, touched: [cmd.media] };
        }
        const rest = group.slice(1);
        const next = cmd.newPrimary ?? rest[0];
        if (!rest.includes(next)) throw invalid('The new primary must be one of the other versions.');
        versions.delete(next);
        for (const v of rest) if (v !== next) versions.set(v, next);
        return { focus: null, touched: group };
      }

      case 'setMediaRotation': {
        const m = this.db.media.get(cmd.id);
        if (!m) throw missing('media item', cmd.id);
        if (![0, 90, 180, 270].includes(cmd.rotation)) throw invalid('rotation must be 0, 90, 180 or 270.');
        if ((m.mime ?? '').startsWith('audio/') || m.mime === 'application/pdf') throw invalid('Only a picture can be turned.');
        m.rotation = cmd.rotation;
        return { focus: null, touched: [m.id] };
      }

      case 'createPlace': {
        const place = this.ensurePlace(cmd.text);
        return { focus: place.id, touched: [place.id] };
      }

      case 'setPlaceCoordinates': {
        if (!this.db.places.has(cmd.place)) throw missing('place', cmd.place);
        const coords = (this.db.coords ??= new Map());
        if (cmd.latE6 == null && cmd.lonE6 == null) coords.delete(cmd.place);
        else if (cmd.latE6 == null || cmd.lonE6 == null) throw invalid('Give both the latitude and the longitude, or neither.');
        else if (Math.abs(cmd.latE6) > 90_000_000 || Math.abs(cmd.lonE6) > 180_000_000) throw invalid('The latitude is within 90 degrees and the longitude within 180.');
        else coords.set(cmd.place, [cmd.latE6 / 1e6, cmd.lonE6 / 1e6]);
        return { focus: null, touched: [cmd.place] };
      }

      case 'setMapLayer': {
        const m = this.db.media.get(cmd.media);
        if (!m) throw missing('media', cmd.media);
        const layers = (this.db.layers ??= new Map());
        if (cmd.title !== undefined) m.title = cmd.title.trim() || null;
        if (!cmd.layer) layers.delete(cmd.media);
        else {
          if (cmd.layer.opacity > 100) throw invalid('Opacity is 0 to 100.');
          if (cmd.layer.corners && cmd.layer.corners.length !== 4) throw invalid('A map is placed by its four corners.');
          const old = layers.get(cmd.media);
          const pos = old?.pos ?? Math.max(-1, ...[...layers.values()].map((l) => l.pos)) + 1;
          layers.set(cmd.media, {
            media: cmd.media,
            title: m.title,
            series: cmd.layer.series ?? null,
            year: cmd.layer.year ?? null,
            source: cmd.layer.source ?? null,
            corners: cmd.layer.corners ?? null,
            opacity: cmd.layer.opacity,
            visible: cmd.layer.visible,
            pos,
            width: m.width,
            height: m.height,
          });
        }
        return { focus: null, touched: [cmd.media] };
      }

      case 'setMapLayersVisible': {
        const layers = this.db.layers ?? new Map<Id, MapLayerDto>();
        if (cmd.shown.some((id) => cmd.hidden.includes(id))) throw invalid('A map is either shown or hidden, not both.');
        const all: [Id, boolean][] = [...cmd.shown.map((id): [Id, boolean] => [id, true]), ...cmd.hidden.map((id): [Id, boolean] => [id, false])];
        for (const [id] of all) if (!layers.has(id)) throw invalid('This picture is not on the Map.');
        const changed = all.filter(([id, v]) => layers.get(id)!.visible !== v);
        for (const [id, v] of changed) layers.get(id)!.visible = v;
        return { focus: null, touched: changed.map(([id]) => id) };
      }

      case 'reorderMapLayers': {
        const layers = this.db.layers ?? new Map<Id, MapLayerDto>();
        const all = [...layers.keys()].sort().join();
        if ([...cmd.order].sort().join() !== all) throw invalid('Give every map on the Map once, from the lowest to the top.');
        cmd.order.forEach((id, i) => (layers.get(id)!.pos = i));
        return { focus: null, touched: [...cmd.order] };
      }

      case 'addNumber': {
        if (!cmd.sourceFile.trim() || !cmd.number.trim()) throw invalid('Give both the source file and the number.');
        const p = this.person(cmd.person);
        return { focus: p.id, touched: [p.id] };
      }
    }
  }

  private importDraft(cmd: Extract<EditCommand, { type: 'importDraft' }>): { focus: Id | null; touched: Id[] } {
    const MAX = 500;
    if (cmd.persons.length > MAX || cmd.families.length > MAX)
      throw invalid(`A traced picture can add at most ${MAX} people at a time. Remove some and try again.`);
    if (cmd.persons.length === 0) throw invalid('There is nobody to add. Mark a person first.');
    // Check everything first; the caller restores the snapshot on any error.
    const ids = new Map<string, Id>();
    const usedExisting = new Set<Id>();
    cmd.persons.forEach((d, i) => {
      const key = d.key.trim();
      if (!key || ids.has(key)) throw invalid(`Card ${i + 1} has a missing or repeated label. Close the trace and open it again.`);
      if (d.existing) {
        this.person(d.existing);
        if (usedExisting.has(d.existing)) throw invalid('The same person from the tree is used on two cards. Merge the cards first.');
        usedExisting.add(d.existing);
        ids.set(key, d.existing);
      } else {
        if (!d.given?.trim() && !d.surname?.trim()) throw invalid(`Card ${i + 1} has no name. Enter a given name or a surname.`);
        ids.set(key, this.ids.next());
      }
    });
    const ref = (k: string): Id => {
      const id = ids.get(k.trim());
      if (!id) throw invalid('A link points at a card that is not in the draft.');
      return id;
    };
    const fams = cmd.families.map((f) => {
      const p1 = f.partner1 ? ref(f.partner1) : null;
      const p2 = f.partner2 ? ref(f.partner2) : null;
      const kids = f.children.map(ref);
      if (!p1 && !p2) throw invalid('A family needs at least one parent or partner.');
      if (p1 && p1 === p2) throw invalid('A family needs two different people.');
      if (new Set(kids).size !== kids.length) throw new MockApiError('conflict', 'A child is listed twice in one family.');
      if (kids.some((k) => k === p1 || k === p2)) throw invalid('A person cannot be their own child.');
      // Children of ONE parent from the tree join that parent's family.
      let into: FamilyRec | undefined;
      const single = p1 && !p2 ? p1 : p2 && !p1 ? p2 : null;
      if (f.existingFamily) {
        into = this.db.families.get(f.existingFamily);
        if (!into) throw invalid('The family chosen for the new people is not in the tree any more.');
        if (!single || (into.partner1 !== single && into.partner2 !== single))
          throw invalid('The family chosen for the new people does not include that parent.');
      } else if (single) {
        const mine = this.partnerFamilies(single);
        if (mine.length > 1)
          throw new MockApiError('conflict', 'A parent on the picture is in more than one family in the tree. Choose which family the new people belong to.');
        into = mine[0];
      }
      return { p1: into ? into.partner1 : p1, p2: into ? into.partner2 : p2, kids, into };
    });
    // A loop of parents and children is refused.
    const parents = new Map<Id, Id[]>();
    const addParents = (child: Id, ps: (Id | null)[]) => parents.set(child, [...(parents.get(child) ?? []), ...ps.filter((x): x is Id => !!x)]);
    for (const f of this.db.families.values()) for (const c of f.children) addParents(c.person, [f.partner1, f.partner2]);
    for (const f of fams) for (const k of f.kids) addParents(k, [f.p1, f.p2]);
    for (const f of fams)
      for (const k of f.kids) {
        const stack = [f.p1, f.p2].filter((x): x is Id => !!x);
        const seen = new Set<Id>();
        while (stack.length) {
          const n = stack.pop()!;
          if (n === k) throw invalid('These links make someone their own ancestor. Check the parent and child links.');
          if (seen.has(n)) continue;
          seen.add(n);
          stack.push(...(parents.get(n) ?? []));
        }
      }

    // Write.
    const touched: Id[] = [];
    let picture: MediaDto | null = null;
    if (cmd.attachPicture) {
      if (!this.trace.image) throw new MockApiError('not_found', 'The picture is missing. Pick it again or untick keeping it.');
      picture = {
        id: this.ids.next(),
        title: null,
        mime: 'image/png',
        originalName: this.trace.image.name,
        date: null,
        description: null,
        resolved: true,
        width: this.trace.image.width ?? null,
        height: this.trace.image.height ?? null,
        sha256: null,
        transcription: [],
        rotation: 0,
      };
      this.db.media.set(picture.id, picture);
      const sid = this.ids.next();
      this.db.sources.set(sid, {
        id: sid,
        title: 'Family tree picture',
        author: null,
        publication: null,
        abbreviation: null,
        text: `File: ${picture.originalName}`,
        kind: 'photo',
        issuer: null,
        docNumber: null,
        docDate: null,
        archive: null,
        callNumber: null,
        citationCount: 0,
        media: [],
      });
      touched.push(sid);
    }
    let focus: Id | null = null;
    for (const d of cmd.persons) {
      if (d.existing) continue;
      const id = ids.get(d.key.trim())!;
      const events: EventDto[] = [];
      if (d.birthDateRaw?.trim()) events.push(makeEvent(this.ids.next(), 'birth', { dateRaw: d.birthDateRaw.trim(), order: this.settings.dayOrder, preferred: true }));
      if (d.deathDateRaw?.trim()) events.push(makeEvent(this.ids.next(), 'death', { dateRaw: d.deathDateRaw.trim(), order: this.settings.dayOrder, preferred: true }));
      const name = makeName(this.ids.next(), {
        kind: 'birth',
        preferred: true,
        private: false,
        given: d.given?.trim() || undefined,
        surnames: d.surname?.trim() ? [d.surname.trim()] : [],
        patronymic: d.patronymic?.trim() || undefined,
      });
      this.db.persons.set(id, {
        id,
        sex: d.sex,
        sexText: null,
        livingOverride: null,
        private: false,
        names: [name],
        events,
        notes: d.notes?.trim() ? [makeNote(this.ids.next(), 'general', d.notes.trim())] : [],
        media: picture ? [{ media: picture, primary: false }] : [],
        citations: [],
        changed: this.now(),
      });
      touched.push(id);
      focus ??= id;
    }
    for (const f of fams) {
      const same =
        f.into ??
        (f.p1 && f.p2
          ? [...this.db.families.values()].find((x) => (x.partner1 === f.p1 && x.partner2 === f.p2) || (x.partner1 === f.p2 && x.partner2 === f.p1))
          : undefined);
      const fam = same ?? this.newFamily(f.p1, f.p2);
      for (const k of f.kids) {
        if (fam.children.some((c) => c.person === k)) continue;
        const hasOther = this.parentFamilies(k).length > 0 || [...this.db.families.values()].some((x) => x !== fam && x.children.some((c) => c.person === k));
        fam.children.push({ person: k, rel: ['birth', 'birth'], preferred: !hasOther });
      }
      touched.push(fam.id);
    }
    return { focus, touched };
  }

  /** Mirrors gt-session's add_linked_parent: a step-parent, adoptive parent, ... (the birth links stay). */
  private addLinkedParent(anchor: Id, relation: 'father' | 'mother', person: Id, kind: FamilyRec['children'][number]['rel'][number]): Id[] {
    const same = relation === 'father' ? 'male' : 'female';
    const current = this.preferredParentFamily(anchor);
    const step = kind === 'step';
    const setRel = (f: FamilyRec, child: Id, parent: Id, rel: typeof kind) => {
      const l = f.children.find((c) => c.person === child);
      if (l) l.rel = f.partner1 === parent ? [rel, l.rel[1]] : [l.rel[0], rel];
    };
    const alone = (f: FamilyRec) => !f.partner1 !== !f.partner2;
    const spouse = step ? [current?.partner1, current?.partner2].find((id) => id && this.person(id).sex !== same) : undefined;
    const join = step
      ? current && alone(current) && spouse
        ? current
        : undefined
      : this.parentFamilies(anchor).find((f) => {
          const l = f.children.find((c) => c.person === anchor);
          return alone(f) && l?.rel[f.partner1 ? 0 : 1] === kind;
        });
    const couple = (a: Id, b: Id) => this.partnerFamilies(a).find((f) => f.partner1 === b || f.partner2 === b);
    if (join) {
      const other = (join.partner1 ?? join.partner2)!;
      const target = couple(other, person);
      if (target) {
        const i = join.children.findIndex((c) => c.person === anchor);
        const [l] = join.children.splice(i, 1);
        const toOther = l.rel[join.partner1 === other ? 0 : 1];
        target.children.push(l);
        setRel(target, anchor, other, toOther);
        setRel(target, anchor, person, kind);
        if (join.children.length === 0) this.db.families.delete(join.id);
        return [join.id, target.id];
      }
      if (!join.partner1) join.partner1 = person;
      else join.partner2 = person;
      for (const c of join.children) setRel(join, c.person, person, kind);
      return [join.id];
    }
    if (spouse) {
      const f = couple(spouse, person) ?? this.newFamily(spouse, person);
      f.children.push({ person: anchor, rel: ['birth', 'birth'], preferred: false });
      setRel(f, anchor, person, kind);
      return [f.id];
    }
    const f = this.newFamily(person, null);
    f.children.push({ person: anchor, rel: step ? [kind, 'birth'] : [kind, kind], preferred: !current });
    return [f.id];
  }

  /** Mirrors gt-session's child_link: a stepchild is the birth child of the other partner. */
  private childRel(f: FamilyRec, anchor: Id, kind: FamilyRec['children'][number]['rel'][number] | undefined): FamilyRec['children'][number]['rel'] {
    if (!kind || kind === 'birth') return ['birth', 'birth'];
    if (kind !== 'step') return [kind, kind];
    return f.partner1 === anchor ? ['step', 'birth'] : ['birth', 'step'];
  }

  /** Mirrors gt-session's link_existing: quickAdd with someone already in the tree. */
  private linkExisting(cmd: Extract<EditCommand, { type: 'linkExisting' }>): { focus: Id; touched: Id[] } {
    const anchor = this.person(cmd.anchor).id;
    const person = this.person(cmd.person).id;
    if (person === anchor) throw invalid('A person cannot be their own relative.');
    const kindOf = cmd.parentRel && cmd.parentRel !== 'birth' ? cmd.parentRel : undefined;
    if (kindOf && cmd.relation !== 'father' && cmd.relation !== 'mother' && cmd.relation !== 'child') throw invalid('Only a parent or a child can be linked this way.');
    const descendants = (root: Id) => {
      const seen = new Set<Id>();
      const todo = [root];
      while (todo.length) {
        const id = todo.pop()!;
        for (const f of this.partnerFamilies(id))
          for (const c of f.children)
            if (!seen.has(c.person)) {
              seen.add(c.person);
              todo.push(c.person);
            }
      }
      return seen;
    };
    const couple = (a: Id, b: Id) => this.partnerFamilies(a).find((f) => f.partner1 === b || f.partner2 === b);
    const link = (p: Id, preferred: boolean) => ({ person: p, rel: ['birth', 'birth'] as FamilyRec['children'][number]['rel'], preferred });
    const has = (f: FamilyRec, p: Id) => f.partner1 === p || f.partner2 === p;
    const touched: Id[] = [anchor, person];
    switch (cmd.relation) {
      case 'father':
      case 'mother': {
        if (descendants(anchor).has(person)) throw invalid('A descendant cannot also be a parent.');
        if (this.parentFamilies(anchor).some((f) => has(f, person))) throw new MockApiError('conflict', 'This person is already a parent.');
        if (kindOf) {
          touched.push(...this.addLinkedParent(anchor, cmd.relation, person, kindOf));
          break;
        }
        const current = this.preferredParentFamily(anchor);
        const f = cmd.newParentSet ? undefined : current;
        if (f) {
          if (f.partner1 && f.partner2)
            throw new MockApiError('conflict', 'Both parents are already recorded. Add another set of parents to record a second pair.');
          const other = f.partner1 ?? f.partner2;
          const target = other ? couple(other, person) : undefined;
          if (target) {
            const i = f.children.findIndex((c) => c.person === anchor);
            if (i >= 0) {
              const [l] = f.children.splice(i, 1);
              if (!target.children.some((c) => c.person === anchor)) target.children.push(l);
            }
            if (f.children.length === 0) this.db.families.delete(f.id);
            touched.push(f.id, target.id);
          } else {
            if (!f.partner1) f.partner1 = person;
            else f.partner2 = person;
            touched.push(f.id);
          }
        } else {
          const nf = this.newFamily(person, null);
          nf.children.push(link(anchor, !current));
          touched.push(nf.id);
        }
        break;
      }
      case 'partner': {
        if (couple(anchor, person)) throw new MockApiError('conflict', 'These two are already partners.');
        if (cmd.family) {
          const f = this.family(cmd.family);
          if (f.partner1 && f.partner2) throw new MockApiError('conflict', 'This family already has two partners.');
          if (f.children.some((c) => c.person === person)) throw invalid('A partner cannot also be a child of the same family.');
          if (!f.partner1) f.partner1 = person;
          else f.partner2 = person;
          touched.push(f.id);
        } else touched.push(this.newFamily(anchor, person).id);
        break;
      }
      case 'child': {
        if (descendants(person).has(anchor)) throw invalid('An ancestor cannot also be a child.');
        let f: FamilyRec;
        if (cmd.family) {
          f = this.family(cmd.family);
          if (!has(f, anchor)) throw invalid('That family does not include this person.');
        } else {
          const fams = this.partnerFamilies(anchor);
          if (fams.length > 1) throw invalid('Choose which family the child belongs to.');
          f = fams[0] ?? this.newFamily(anchor, null);
        }
        if (has(f, person)) throw invalid('A partner cannot also be a child of the same family.');
        if (f.children.some((c) => c.person === person)) throw new MockApiError('conflict', 'This person is already a child in the family.');
        f.children.push({ ...link(person, !this.preferredParentFamily(person)), rel: this.childRel(f, anchor, kindOf) });
        touched.push(f.id);
        break;
      }
      case 'sibling': {
        const mine = this.preferredParentFamily(anchor);
        const theirs = this.preferredParentFamily(person);
        if (mine && theirs && mine.id === theirs.id) throw new MockApiError('conflict', 'These two are already siblings.');
        if (mine) {
          if (has(mine, person)) throw invalid('A parent cannot also be a sibling.');
          mine.children.push(link(person, !theirs));
          touched.push(mine.id);
        } else if (theirs) {
          if (has(theirs, anchor)) throw invalid('A parent cannot also be a sibling.');
          theirs.children.push(link(anchor, true));
          touched.push(theirs.id);
        } else {
          const nf = this.newFamily(null, null);
          nf.children.push(link(anchor, true), link(person, true));
          touched.push(nf.id);
        }
        break;
      }
    }
    return { focus: person, touched };
  }

  private quickAdd(cmd: Extract<EditCommand, { type: 'quickAdd' }>): { focus: Id; touched: Id[] } {
    const anchor = this.person(cmd.anchor);
    const sex: SexKind = cmd.relation === 'father' ? 'male' : cmd.relation === 'mother' ? 'female' : cmd.draft.sex;
    const touched: Id[] = [anchor.id];
    const kindOf = cmd.parentRel && cmd.parentRel !== 'birth' ? cmd.parentRel : undefined;
    if (kindOf && cmd.relation !== 'father' && cmd.relation !== 'mother' && cmd.relation !== 'child') throw invalid('Only a parent or a child can be linked this way.');

    // Validate and find the target before creating anything.
    let target: FamilyRec | undefined;
    switch (cmd.relation) {
      case 'father':
      case 'mother': {
        const pf = this.preferredParentFamily(anchor.id);
        if (pf && !cmd.newParentSet && !kindOf) {
          if (pf.partner1 && pf.partner2)
            throw new MockApiError('conflict', 'Both parents are already recorded. Add another set of parents to record a second pair.');
          target = pf;
        }
        break;
      }
      case 'partner':
        if (cmd.family) {
          target = this.family(cmd.family);
          if (target.partner1 && target.partner2) throw new MockApiError('conflict', 'This family already has two partners.');
        }
        break;
      case 'child':
        if (cmd.family) target = this.family(cmd.family);
        else {
          const fams = this.partnerFamilies(anchor.id);
          if (fams.length > 1) throw invalid('Choose which family the child belongs to.');
          target = fams[0];
        }
        break;
      case 'sibling':
        target = this.preferredParentFamily(anchor.id);
        break;
    }

    const created = this.newPerson(cmd.draft, sex);
    touched.push(created.id);

    switch (cmd.relation) {
      case 'father':
      case 'mother': {
        if (kindOf) {
          touched.push(...this.addLinkedParent(anchor.id, cmd.relation, created.id, kindOf));
        } else if (target) {
          if (!target.partner1) target.partner1 = created.id;
          else target.partner2 = created.id;
          touched.push(target.id);
        } else {
          const hadParents = this.parentFamilies(anchor.id).length > 0;
          const f = this.newFamily(created.id, null);
          f.children.push({ person: anchor.id, rel: ['birth', 'birth'], preferred: !hadParents });
          touched.push(f.id);
        }
        break;
      }
      case 'partner': {
        if (target) {
          if (!target.partner1) target.partner1 = created.id;
          else target.partner2 = created.id;
          touched.push(target.id);
        } else {
          const f = this.newFamily(anchor.id, created.id);
          touched.push(f.id);
        }
        break;
      }
      case 'child': {
        const f = target ?? this.newFamily(anchor.id, null);
        f.children.push({ person: created.id, rel: this.childRel(f, anchor.id, kindOf), preferred: true });
        touched.push(f.id);
        break;
      }
      case 'sibling': {
        const f = target ?? this.newFamily(null, null);
        if (!target) f.children.push({ person: anchor.id, rel: ['birth', 'birth'], preferred: true });
        f.children.push({ person: created.id, rel: ['birth', 'birth'], preferred: true });
        touched.push(f.id);
        break;
      }
    }
    return { focus: created.id, touched };
  }

  // --- About: no network. Tests set what the build reports and what GitHub "answers". ---
  about: AppInfo = { version: '0.3.0+12', base: '0.3.0', release: false, commit: 'a1b2c3d', date: '2026-11-08' };
  updateAnswer: UpdateStatus = { kind: 'latest', latest: '0.3.0' };
  updateChecks = 0;
  openedLinks: string[] = [];

  async appInfo(): Promise<AppInfo> {
    return { ...this.about };
  }

  async checkForUpdate(): Promise<UpdateStatus> {
    this.updateChecks += 1;
    return this.updateAnswer;
  }

  async openProjectLink(url: string): Promise<boolean> {
    this.openedLinks.push(url);
    return url.startsWith('https://github.com/nazymko/family-tree');
  }
}
