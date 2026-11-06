import type { Api } from '../api';
import type { AgentAccessState, NewConnectionDto } from '../types';
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
} from '../types';
import { personFieldChanges } from './fieldDiff';
import { IdGen, fold, hashString, makeEvent, makeName, makeNote, mulberry32 } from './build';
import type { Db, FamilyRec, Fixture, PersonRec } from './model';
import { MockApiError } from './model';
import { mirrorLayout, type Layout } from './mockLayout';
import { layoutDescendants, layoutPedigree, outlineOf, type RustLayoutSource } from './layout';
import { GROUP_BASE, layoutRelatives, relativesOutline, type RelLayout } from './relatives';
import { layoutWhole, lineageOf, type WholeResult } from './wholeLayout';
import { kinshipFinder } from './kinship';
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
  whole?: { result: WholeResult; surnames: string[]; meta: WholeMeta; reversed: boolean; selection?: { people: Id[]; descendants: boolean }; around?: Id };
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
  importDraft: 'Add traced people',
  addNumber: 'Record number',
};

const missing = (what: string, id: Id) => new MockApiError('not_found', `No ${what} with id "${id}" in this project.`);
const invalid = (message: string) => new MockApiError('invalid', message);
const markerEntry = (id: Id) => new MockApiError('not_found', `"${id}" is the entry for unnamed brothers and sisters. It has no details.`);

/** An unnamed person is valid in genealogy; show it as Unknown. */
function unnamedOk(n: NameDto): NameDto {
  return n.display ? n : { ...n, display: 'Unknown' };
}

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
      treeDirection: 'normal',
      livingMaxAge: 100,
      homePerson: fixture.homePerson,
    };
  }

  // ---------------------------------------------------------------- projects

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
    return { bytes: this.copyBytes, photos: 38, people };
  }

  /** Zip and folder passed to `openReceivedCopy`, for tests. */
  readonly receivedCopies: { zip: string; dest: string }[] = [];

  async openReceivedCopy(zip: string, dest: string): Promise<ProjectInfo> {
    if (!/\.zip$/i.test(zip)) throw invalid('This is not a zip file Gene Tree can read.');
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
    if (/bad|broken/i.test(source)) throw invalid('This is not a copy of a Gene Tree tree.');
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
      media: p.media,
      citations: p.citations,
      changed: p.changed,
    });
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
    });
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
      const person: PersonRef = { id: r.id, name: r.name, lifespan: r.lifespan, sex: r.sex, living: r.living, private: r.private, siblingMarker: false };
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
      if (!q || fold(s.title ?? '').includes(q)) out.push(s);
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
    const label = addedStep ? `Add step${cmd.relation}` : cmd.type === 'quickAdd' || cmd.type === 'linkExisting' ? `Add ${cmd.relation}` : LABELS[cmd.type];
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
      else if (target.kind !== 'note' && audioName.test(name)) failed.push({ path, reason: 'sound recordings can only be attached to a note' });
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
    const generations = Math.max(1, Math.min(req.generations, req.kind === 'pedigree' ? 255 : 20));
    const src: RustLayoutSource = {
      ref: (id) => this.ref(this.person(id)),
      birth: (id) => this.birthSort(this.person(id)),
      parents: (id) => this.preferredParents(id),
      unions: (id) => this.unionsOf(id),
    };
    const laid =
      req.kind === 'pedigree'
        ? layoutPedigree(req.root, generations, src)
        : req.kind === 'descendants'
          ? layoutDescendants(req.root, generations, src)
          : layoutRelatives(req.root, Math.min(generations, 6), req.auntsUncles ?? true, src);
    // Reversed tree: ancestors at the bottom (the left-to-right pedigree grows leftward instead).
    const layout = this.settings.treeDirection === 'reversed' ? mirrorLayout(laid, req.kind !== 'pedigree') : laid;
    const chartId = this.nextChart++;
    this.charts.set(chartId, { kind: req.kind, layout, version: this.version, reference: req.root });
    return {
      chartId,
      kind: req.kind,
      bounds: layout.bounds,
      nodeCount: layout.nodes.length,
      rootNode: 0,
      truncated: layout.truncated,
      version: this.version,
    };
  }

  async chartOpenWhole(): Promise<WholeMeta> {
    return this.openWholeChart(null, null, null);
  }

  /** The same people in another order: `around`'s closest relatives sit near them. */
  async chartOpenArranged(around: Id, people?: Id[], allDescendants = false): Promise<WholeMeta> {
    const p = this.db.persons.get(around);
    const gone = () => new MockApiError('not_found', 'The person to arrange the chart around is not on the chart any more.');
    if (!p || p.siblingMarker) throw gone();
    if (people === undefined) return this.openWholeChart(null, null, around);
    const { on, kept } = this.selectionSubset(people, allDescendants);
    if (kept.length === 0) throw new MockApiError('not_found', 'None of the chosen people is in the project any more.');
    if (!on.has(around)) throw gone();
    return this.openWholeChart({ people: kept, descendants: allDescendants }, on, around);
  }

  /** The chart of chosen people: them, their ancestors and their partners; with `allDescendants` also everyone below them. */
  async chartOpenSelection(people: Id[], allDescendants: boolean): Promise<WholeMeta> {
    const { on, kept } = this.selectionSubset(people, allDescendants);
    if (kept.length === 0) throw new MockApiError('not_found', 'None of the chosen people is in the project any more.');
    return this.openWholeChart({ people: kept, descendants: allDescendants }, on, null);
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

  private async openWholeChart(selection: { people: Id[]; descendants: boolean } | null, subset: Set<Id> | null, around: Id | null): Promise<WholeMeta> {
    const reversed = this.settings.treeDirection === 'reversed';
    // Three kinds are held side by side, one chart each: everyone, chosen people, and an arranged one (of either).
    const kindOf = (c: ChartState) => (c.whole!.around ? 'arranged' : c.whole!.selection ? 'selection' : 'plain');
    const kind = around ? 'arranged' : selection ? 'selection' : 'plain';
    const sameKind = (c: ChartState) => c.whole && kindOf(c) === kind;
    for (const c of this.charts.values()) {
      if (!sameKind(c) || c.version !== this.version || c.whole!.reversed !== reversed) continue;
      const have = c.whole!.selection;
      const sameSel = selection === null ? !have : !!have && have.descendants === selection.descendants && have.people.join() === selection.people.join();
      if (sameSel && (c.whole!.around ?? null) === around) return structuredClone(c.whole!.meta);
    }
    for (const [id, c] of [...this.charts]) if (sameKind(c)) this.charts.delete(id);
    const persons = [...this.db.persons.values()].filter((p) => subset === null || subset.has(p.id));
    const rows = new Map(this.rows().rows.map((r) => [r.id, r]));
    const result = layoutWhole({
      persons: persons.map((p) => ({ id: p.id, surname: rows.get(p.id)?.surname ?? '', birth: this.birthSort(p) })),
      families: [...this.db.families.values()].map((f) => ({ partners: [f.partner1, f.partner2], children: f.children.map((c) => c.person) })),
      ref: (id) => this.ref(this.person(id)),
    }, reversed, around ?? undefined);
    const surnames = result.nodes.map((n) => rows.get(n.person!.id)?.surname ?? '');
    for (const n of result.nodes) {
      const p = n.person!;
      n.label = `${p.siblingMarker ? 'more brothers and sisters, not named' : `${p.name}${p.lifespan ? `, ${p.lifespan}` : ''}`}, generation ${n.gen + 1}`;
    }
    // The markers are drawn like people but are not counted as people.
    const markerAt = result.nodes.filter((n) => n.person?.siblingMarker).map((n) => ({ x: n.x + n.w / 2, y: n.y + n.h / 2 }));
    const blocks = result.blocks.map((b) => {
      const inside = markerAt.filter((m) => m.x >= b.rect.x && m.x <= b.rect.x + b.rect.w && m.y >= b.rect.y && m.y <= b.rect.y + b.rect.h + b.bodyH).length;
      const people = b.people - inside;
      return inside === 0 ? b : { ...b, people, title: b.title.replace(/\(\d+/, `(${people}`) };
    });
    const chartId = this.nextChart++;
    const meta: WholeMeta = {
      meta: {
        chartId,
        kind: 'whole',
        bounds: result.bounds,
        nodeCount: result.nodes.length - markerAt.length,
        rootNode: 0,
        truncated: false,
        version: this.version,
      },
      blocks,
      chosen: selection ? selection.people.length : 0,
    };
    const layout: Layout = { nodes: result.nodes, edges: result.edges, bounds: result.bounds, truncated: false, kids: [], labels: [] };
    this.charts.set(chartId, { kind: 'whole', layout, version: this.version, reference: null, whole: { result, surnames, meta, reversed, ...(selection ? { selection } : {}), ...(around ? { around } : {}) } });
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
        nodes: nodes.map((n) => ({ ...n, label: '', person: { ...n.person!, name: surnames[n.id], lifespan: '' } })),
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
    if (c.kind === 'relatives') {
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
      throw new Error('Gene Tree will only open photos and PDF documents. This file is some other kind of file, so it was not opened.');
    }
    this.openedMedia.push(id);
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

  private ref(p: PersonRec): PersonRef {
    return {
      id: p.id,
      name: this.prefName(p)?.display ?? '',
      lifespan: this.lifespan(p),
      sex: p.sex,
      living: !p.siblingMarker && this.isLiving(p),
      private: p.private,
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
        folded.set(p.id, fold(p.names.map((n) => n.display).join(' ')));
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

  private unionsOf(id: Id): { partner: Id | null; children: Id[] }[] {
    const out: { partner: Id | null; children: Id[] }[] = [];
    for (const f of this.db.families.values()) {
      if (f.partner1 !== id && f.partner2 !== id) continue;
      out.push({ partner: f.partner1 === id ? f.partner2 : f.partner1, children: f.children.map((c) => c.person) });
    }
    return out;
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
        return { notes: f.notes, citations: f.citations };
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
      case 'note': {
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
          const s: SourceDto = { id: this.ids.next(), title, author: null, publication: null, abbreviation: null, text: null, citationCount: 0 };
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
        if (i >= 0) bag.notes[i] = { ...bag.notes[i], kind: cmd.note.kind, text: cmd.note.text };
        else bag.notes.push(makeNote(this.ids.next(), cmd.note.kind, cmd.note.text));
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
          for (const k of ['title', 'author', 'publication', 'abbreviation', 'text'] as const) if (input[k] !== undefined) s[k] = input[k] || null;
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
          citationCount: 0,
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
          if (!bag.media.some((l) => l.media.id === id)) bag.media.push({ media: m, primary: bag.media.length === 0 });
        }
        return { focus: cmd.target.kind === 'person' ? cmd.target.id : null, touched: [cmd.target.id, ...cmd.media] };
      }

      case 'detachMedia': {
        const bag = this.owner(cmd.target);
        const i = bag.media?.findIndex((l) => l.media.id === cmd.media) ?? -1;
        if (!bag.media || i < 0) throw missing('media link', cmd.media);
        const [gone] = bag.media.splice(i, 1);
        if (gone.primary && bag.media[0]) bag.media[0].primary = true;
        return { focus: cmd.target.kind === 'person' ? cmd.target.id : null, touched: [cmd.target.id, cmd.media] };
      }

      case 'deleteMedia': {
        if (!this.db.media.has(cmd.id)) throw missing('media item', cmd.id);
        const touched: Id[] = [cmd.id];
        const sweep = (links: MediaLinkDto[] | undefined, owner: Id) => {
          const i = links?.findIndex((l) => l.media.id === cmd.id) ?? -1;
          if (!links || i < 0) return;
          const [gone] = links.splice(i, 1);
          if (gone.primary && links[0]) links[0].primary = true;
          touched.push(owner);
        };
        for (const p of this.db.persons.values()) {
          sweep(p.media, p.id);
          for (const e of p.events) sweep(e.media, e.id);
          for (const n of p.notes) sweep(n.media, n.id);
        }
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
        return { focus: null, touched: [m.id] };
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
        citationCount: 0,
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
}
