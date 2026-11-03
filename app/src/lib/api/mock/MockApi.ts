import type { Api } from '../api';
import type {
  RelinkResult,
  AttachResult,
  ChartMeta,
  ChartNode,
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
  UndoState,
} from '../types';
import { IdGen, fold, hashString, makeEvent, makeName, makeNote, mulberry32 } from './build';
import type { Db, FamilyRec, Fixture, PersonRec } from './model';
import { MockApiError } from './model';
import type { Layout } from './mockLayout';
import { layoutDescendants, layoutPedigree, outlineOf, type RustLayoutSource } from './layout';
import { toDateDto, yearOf } from './mockParse';
import { lifespanText, previewDate } from './mockDate';

export { MockApiError } from './model';

/** Fixed so living/deceased results do not drift with the calendar. */
const REFERENCE_YEAR = 2026;
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
}

interface ChartState {
  kind: 'pedigree' | 'descendants';
  layout: Layout;
  version: number;
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
  saveFamily: 'Save family',
  deleteFamily: 'Delete family',
  linkChild: 'Add child to family',
  updateChildLink: 'Edit child link',
  unlinkChild: 'Remove child from family',
  reorderChildren: 'Reorder children',
  saveNote: 'Save note',
  deleteNote: 'Delete note',
  saveSource: 'Save source',
  addCitation: 'Add citation',
  updateCitation: 'Edit citation',
  removeCitation: 'Remove citation',
  attachMedia: 'Attach media',
  detachMedia: 'Detach media',
  setPrimaryMedia: 'Set primary media',
  updateMedia: 'Edit media',
};

const missing = (what: string, id: Id) => new MockApiError('not_found', `No ${what} with id "${id}" in this project.`);
const invalid = (message: string) => new MockApiError('invalid', message);

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
  private charts = new Map<number, ChartState>();
  private nextChart = 1;
  private version = 0;
  private previews = new Map<string, ExportOptions>();
  private projectName: string;
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
      backupsToKeep: 10,
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
      const f = folded.get(r.id)!;
      const at = f.indexOf(q);
      if (at < 0) continue;
      const person: PersonRef = { id: r.id, name: r.name, lifespan: r.lifespan, sex: r.sex, living: r.living, private: r.private };
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
      .map((h) => ({ ts: h.ts, label: h.label, op: h.op }));
  }

  // ------------------------------------------------------------------- edits

  async apply(cmd: EditCommand): Promise<EditResult> {
    const label = cmd.type === 'quickAdd' ? `Add ${cmd.relation}` : LABELS[cmd.type];
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
    this.record(label, op, new Set(touched));
    return { label, focus: outcome.focus, touched, undo: this.currentUndo() };
  }

  async attachFiles(target: OwnerRef, paths: string[]): Promise<AttachResult> {
    const failed: AttachResult['failed'] = [];
    const accepted: { path: string; name: string }[] = [];
    const known = new Set<string>();
    for (const p of this.db.persons.values()) for (const l of p.media) known.add((l.media.originalName ?? '').toLowerCase());
    for (const path of paths) {
      const name = path.replace(/[\\/]+$/, '').split(/[\\/]/).pop() ?? '';
      if (!name) failed.push({ path, reason: 'The file has no name.' });
      else if (/[\\/]$/.test(path) || !/\.[A-Za-z0-9]{1,8}$/.test(name)) failed.push({ path, reason: 'Folders cannot be attached. Choose files.' });
      else if (/\.(exe|bat|cmd|msi|dll|com|scr)$/i.test(name)) failed.push({ path, reason: 'This file type cannot be attached.' });
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
          mime: /\.(jpe?g)$/i.test(a.name) ? 'image/jpeg' : /\.png$/i.test(a.name) ? 'image/png' : /\.pdf$/i.test(a.name) ? 'application/pdf' : 'application/octet-stream',
          originalName: a.name,
          date: null,
          description: null,
          resolved: true,
          width: 1024,
          height: 768,
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
    this.person(req.root);
    const generations = Math.max(1, Math.min(req.generations, 20));
    const src: RustLayoutSource = {
      ref: (id) => this.ref(this.person(id)),
      parents: (id) => this.preferredParents(id),
      unions: (id) => this.unionsOf(id),
    };
    const layout = req.kind === 'pedigree' ? layoutPedigree(req.root, generations, src) : layoutDescendants(req.root, generations, src);
    const chartId = this.nextChart++;
    this.charts.set(chartId, { kind: req.kind, layout, version: this.version });
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

  async chartWindow(chartId: number, rect: Rect, zoom: number): Promise<ChartWindow> {
    void zoom;
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
    return structuredClone({ nodes, edges });
  }

  async chartOutline(chartId: number, parentNode: number | null, limit: number): Promise<OutlineItem[]> {
    const c = this.chart(chartId);
    if (parentNode !== null && !c.layout.nodes[parentNode]) throw missing('chart node', String(parentNode));
    return outlineOf(c.kind, c.layout, parentNode, Math.max(0, Math.min(limit, MAX_PAGE)));
  }

  async chartClose(chartId: number): Promise<void> {
    this.charts.delete(chartId);
  }

  // ------------------------------------------------------------------- media

  mediaUrl(id: Id, size: 'thumb' | 'preview' | 'original'): string {
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

  // ================================================================ internals

  private async info(dir: string, name?: string): Promise<ProjectInfo> {
    return {
      dir,
      name: name ?? (dir.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || this.projectName),
      personCount: this.db.persons.size,
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

  private record(label: string, op: HistoryEntry['op'], touched: Set<Id>): void {
    this.log.push({ ts: this.now(), label, op, touched });
    if (this.log.length > 2000) this.log.shift();
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
      living: this.isLiving(p),
      private: p.private,
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
      surname: n.surnames.join(' '),
      given: n.given ?? '',
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
      names: [unnamedOk(makeName(this.ids.next(), { kind: 'birth', preferred: true, private: false, given, surnames }))],
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
        if (!input.given?.trim() && input.surnames.every((s) => !s.trim())) throw invalid('Enter a given name or a surname.');
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
        return { focus: null, touched: [f.id, ...[f.partner1, f.partner2, ...f.children.map((c) => c.person)].filter((x): x is Id => !!x)] };
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
        return { focus: cmd.target.kind === 'person' ? cmd.target.id : null, touched: [cmd.target.id] };
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

      case 'setPrimaryMedia': {
        const p = this.person(cmd.person);
        if (!p.media.some((l) => l.media.id === cmd.media)) throw missing('media link', cmd.media);
        for (const l of p.media) l.primary = l.media.id === cmd.media;
        return { focus: p.id, touched: [p.id, cmd.media] };
      }

      case 'updateMedia': {
        const m = this.db.media.get(cmd.id);
        if (!m) throw missing('media item', cmd.id);
        if (cmd.title !== undefined) m.title = cmd.title || null;
        if (cmd.description !== undefined) m.description = cmd.description || null;
        if (cmd.dateRaw !== undefined) m.date = toDateDto(cmd.dateRaw, this.settings.dayOrder);
        return { focus: null, touched: [m.id] };
      }
    }
  }

  private quickAdd(cmd: Extract<EditCommand, { type: 'quickAdd' }>): { focus: Id; touched: Id[] } {
    const anchor = this.person(cmd.anchor);
    const sex: SexKind = cmd.relation === 'father' ? 'male' : cmd.relation === 'mother' ? 'female' : cmd.draft.sex;
    const touched: Id[] = [anchor.id];

    // Validate and find the target before creating anything.
    let target: FamilyRec | undefined;
    switch (cmd.relation) {
      case 'father':
      case 'mother': {
        const pf = this.preferredParentFamily(anchor.id);
        if (pf && !cmd.newParentSet) {
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
        if (target) {
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
        f.children.push({ person: created.id, rel: ['birth', 'birth'], preferred: true });
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
