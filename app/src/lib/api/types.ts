// The wire contract between the UI and the Rust shell. Field names are camelCase; Rust mirrors this
// with #[serde(rename_all = "camelCase")]. Keep in step with Task 3's brief.

export type Id = string; // UUID v7
export type DayOrder = 'dmy' | 'mdy';
export type NameOrder = 'given-first' | 'surname-first';
export type SexKind = 'male' | 'female' | 'intersex' | 'unknown' | 'other';

export interface PersonRef {
  id: Id;
  name: string;
  /**
   * The STATED years with a qualifier, never the range bounds of the parsed date: about/est/calc "c. 1968",
   * before "bef. 1968", after "aft. 1968", between/from..to the first stated year with "c.". Both years:
   * "c. 1968 – 2021" (en dash with spaces). Birth only: "b. 1850". Unknown birth with a death: "? – 2021".
   * Empty when neither is known.
   */
  lifespan: string;
  sex: SexKind;
  living: boolean;
  private: boolean;
}
export interface PersonRow extends PersonRef {
  surname: string;
  given: string;
  birth: string;
  death: string;
  sourceCount: number;
  changed: number; // unix ms
}
export interface PersonQuery {
  sort: 'surname' | 'given' | 'birth' | 'changed';
  desc: boolean;
  text: string;
  living: 'any' | 'living' | 'deceased';
  noSource: boolean;
  noBirthDate: boolean;
  privateOnly: boolean;
}
export interface Page<T> {
  total: number;
  offset: number;
  items: T[];
  letters?: { letter: string; offset: number }[];
}
export interface ApiError {
  code: 'not_found' | 'invalid' | 'conflict' | 'io' | 'no_project' | 'stale' | 'internal';
  message: string;
}

export interface DateDto {
  raw: string;
  display: string;
  status: 'ok' | 'phrase' | 'unparsed';
  sortMin: number | null;
  sortMax: number | null;
}
export interface DatePreview {
  raw: string;
  understoodAs: string;
  range: string | null;
  status: 'ok' | 'phrase' | 'unparsed';
  warnings: string[];
}
export interface PlaceOption {
  id: Id;
  name: string;
  full: string;
  kind: string | null;
}
export interface NameDto {
  id: Id;
  kind: 'birth' | 'married' | 'aka' | 'religious' | 'nickname' | 'immigrant' | 'professional' | 'other';
  kindText: string | null;
  preferred: boolean;
  private: boolean;
  display: string;
  prefix: string | null;
  given: string | null;
  call: string | null;
  surnamePrefix: string | null;
  surnames: string[];
  patronymic: string | null;
  suffix: string | null;
  nickname: string | null;
  lang: string | null;
  date: DateDto | null;
  citations: CitationDto[];
}
export type EventKindId = string; // 'birth' | 'death' | ... | 'custom:<text>' (Rust EventKind, lower_snake)
export interface ParticipantDto {
  person: PersonRef;
  role: string;
}
export interface EventDto {
  id: Id;
  kind: EventKindId;
  label: string;
  isFact: boolean;
  date: DateDto | null;
  place: PlaceOption | null;
  description: string | null;
  age: string | null;
  cause: string | null;
  negative: boolean;
  private: boolean;
  preferred: boolean;
  participants: ParticipantDto[];
  citations: CitationDto[];
  notes: NoteDto[];
  media: MediaLinkDto[];
}
export interface SourceDto {
  id: Id;
  title: string | null;
  author: string | null;
  publication: string | null;
  abbreviation: string | null;
  text: string | null;
  citationCount: number;
}
export interface CitationDto {
  id: Id;
  source: SourceDto | null;
  page: string | null;
  date: DateDto | null;
  text: string | null;
  quality: 0 | 1 | 2 | 3 | null;
  evidence: { original: boolean | null; primaryInfo: boolean | null; direct: boolean | null };
}
export interface NoteDto {
  id: Id;
  kind: 'general' | 'story' | 'research' | 'transcription' | 'conclusion' | 'other';
  text: string;
  shared: boolean;
  lang: string | null;
}
export interface MediaDto {
  id: Id;
  title: string | null;
  mime: string | null;
  originalName: string | null;
  date: DateDto | null;
  description: string | null;
  resolved: boolean; // file is in the project
  width: number | null;
  height: number | null;
  sha256: string | null;
}
export interface MediaLinkDto {
  media: MediaDto;
  primary: boolean;
}
export type ParentRelId = 'birth' | 'adopted' | 'step' | 'foster' | 'guardian' | 'sealed' | 'unknown' | 'other';
export type UnionKindId = 'married' | 'civil_union' | 'unmarried' | 'unknown' | 'other';
export interface ParentSetDto {
  familyId: Id;
  parents: [PersonRef | null, PersonRef | null];
  rel: [ParentRelId, ParentRelId];
  preferred: boolean;
  summary: string | null;
}
export interface ChildDto {
  person: PersonRef;
  rel: [ParentRelId, ParentRelId];
  birth: string;
}
export interface PartnershipDto {
  familyId: Id;
  partner: PersonRef | null;
  kind: UnionKindId;
  summary: string | null; // "m. 1 Jun 1878, Hamburg"
  events: EventDto[];
  children: ChildDto[];
}
export interface FamilyViewDto {
  focus: PersonRef;
  focusSummary: string;
  parentSets: ParentSetDto[];
  partnerships: PartnershipDto[];
  siblings: PersonRef[];
}
export interface PersonDetail {
  ref: PersonRef;
  sex: SexKind;
  sexText: string | null;
  livingOverride: 'living' | 'deceased' | null;
  livingComputed: boolean;
  private: boolean;
  names: NameDto[];
  events: EventDto[];
  parentSets: ParentSetDto[];
  partnerships: PartnershipDto[];
  notes: NoteDto[];
  media: MediaLinkDto[];
  citations: CitationDto[];
  changed: number;
}
export interface SearchHitDto {
  kind: 'person' | 'place' | 'source' | 'note';
  id: Id;
  label: string;
  snippet: string | null;
  person: PersonRef | null;
}
export interface RecentProject {
  dir: string;
  name: string;
  openedAt: number;
  missing: boolean;
}
export interface LocationWarningDto {
  kind: 'synced_folder' | 'network_share';
  provider: string | null;
}
export interface ProjectInfo {
  dir: string;
  name: string;
  personCount: number;
  homePerson: Id | null;
  sourceVersion: string | null;
  locationWarning: LocationWarningDto | null;
}
export interface Settings {
  dayOrder: DayOrder;
  nameOrder: NameOrder;
  theme: 'light' | 'dark' | 'system';
  backupsToKeep: number;
  livingMaxAge: number; // project
  homePerson: Id | null; // project
}
export interface ImportRequest {
  file: string;
  projectDir: string;
  projectName: string;
  mediaRoots: string[];
  dayOrder: DayOrder;
}
export interface MissingMedia {
  path: string;
  reason: 'notFound' | 'outsideRoots' | 'ambiguous';
  /** Files that could be the one meant (reason 'ambiguous'). */
  candidates: string[];
}
export interface ImportReportDto {
  project: ProjectInfo;
  encoding: string;
  version: string | null;
  producer: string | null;
  counts: Record<string, number>;
  repairs: { kind: string; line: number | null; detail: string }[];
  unmappedTags: Record<string, number>;
  unparsedDates: { line: number; text: string }[];
  missingMedia: MissingMedia[];
  /** Photos that are web links; they are kept as links, not copied. */
  mediaUrls: string[];
  truncated: boolean;
  text: string;
}
export interface ImportProgress {
  phase: 'reading' | 'parsing' | 'copying_media' | 'saving' | 'indexing';
  done: number;
  total: number;
}
export interface UndoState {
  canUndo: boolean;
  canRedo: boolean;
  undoLabel: string | null;
  redoLabel: string | null;
}
export interface EditResult {
  label: string;
  focus: Id | null;
  touched: Id[];
  undo: UndoState;
}
export interface HistoryEntry {
  ts: number;
  label: string;
  op: 'create' | 'update' | 'delete';
}
export interface ExportOptions {
  version: '5.5.1' | '7.0';
  living: 'exclude' | 'placeholder' | 'all';
  includePrivate: boolean;
}
export interface PrivacyPreview {
  options: ExportOptions;
  defaultVersion: '5.5.1' | '7.0';
  exportedPersons: number;
  placeholderPersons: number;
  hiddenPersons: number;
  removedRecords: number;
  mentionWarnings: { noteExcerpt: string; hiddenPerson: string }[];
  groupMedia: { title: string; originalName: string | null }[];
  losslessNotes: string[];
  token: string; // identifies this preview
}
export interface ChartRequest {
  kind: 'pedigree' | 'descendants';
  root: Id;
  generations: number;
}
export interface ChartMeta {
  chartId: number;
  kind: 'pedigree' | 'descendants';
  bounds: { w: number; h: number };
  nodeCount: number;
  rootNode: number;
  truncated: boolean;
  version: number;
}
export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}
export interface ChartNode {
  id: number;
  person: PersonRef | null;
  x: number;
  y: number;
  w: number;
  h: number;
  gen: number;
  ahnen: number | null;
  dupOf: number | null;
  moreUp: boolean;
  moreDown: boolean;
  nav: { parent: number | null; child: number | null; prev: number | null; next: number | null };
  label: string;
}
export interface ChartEdge {
  from: number;
  to: number;
  kind: 'parent' | 'partner' | 'dup';
  pts: number[]; // flat x,y pairs
}
export interface ChartWindow {
  nodes: ChartNode[];
  edges: ChartEdge[];
}
export interface OutlineItem {
  nodeId: number;
  person: PersonRef | null;
  label: string; // "2 Father", "Child 3"
  hasChildren: boolean;
}
export interface ExifSuggestion {
  date: string | null;
  lat: number | null;
  lon: number | null;
}
export interface AttachResult {
  attached: MediaDto[];
  failed: { path: string; reason: string }[];
}

// ---- EditCommand: discriminated union on `type`. Rust mirrors it as #[serde(tag = "type", rename_all = "camelCase")].
export type OwnerRef = { kind: 'person' | 'family' | 'name' | 'event' | 'media' | 'citation' | 'childLink'; id: Id };
export interface NameInput {
  id?: Id;
  kind: NameDto['kind'];
  kindText?: string;
  preferred: boolean;
  private: boolean;
  prefix?: string;
  given?: string;
  call?: string;
  surnamePrefix?: string;
  surnames: string[];
  patronymic?: string;
  suffix?: string;
  nickname?: string;
  lang?: string;
  dateRaw?: string;
}
export interface EventInput {
  id?: Id;
  kind: EventKindId;
  dateRaw?: string;
  placeText?: string;
  placeId?: Id;
  description?: string;
  age?: string;
  cause?: string;
  negative: boolean;
  private: boolean;
  preferred: boolean;
  participants?: { person: Id; role: string }[];
}
export interface PersonDraft {
  given?: string;
  surnames: string[];
  sex: SexKind;
  birthDateRaw?: string;
  birthPlaceText?: string;
}
export interface CitationInput {
  source: Id | { title: string };
  page?: string;
  dateRaw?: string;
  text?: string;
  quality?: 0 | 1 | 2 | 3;
  evidence?: { original?: boolean; primaryInfo?: boolean; direct?: boolean };
}
export type EditCommand =
  | { type: 'createPerson'; draft: PersonDraft }
  | { type: 'updatePerson'; id: Id; sex?: SexKind; sexText?: string; livingOverride?: 'living' | 'deceased' | null; private?: boolean }
  | { type: 'deletePerson'; id: Id }
  | { type: 'saveName'; person: Id; name: NameInput }
  | { type: 'deleteName'; person: Id; name: Id }
  | { type: 'saveEvent'; owner: { kind: 'person' | 'family'; id: Id }; event: EventInput }
  | { type: 'deleteEvent'; owner: { kind: 'person' | 'family'; id: Id }; event: Id }
  | {
      type: 'quickAdd';
      anchor: Id;
      relation: 'father' | 'mother' | 'partner' | 'child' | 'sibling';
      draft: PersonDraft;
      family?: Id;
      newParentSet?: boolean;
    }
  | { type: 'saveFamily'; id?: Id; partner1: Id | null; partner2: Id | null; kind: UnionKindId }
  | { type: 'deleteFamily'; id: Id }
  | { type: 'linkChild'; family: Id; child: Id; rel: [ParentRelId, ParentRelId]; preferred: boolean }
  | { type: 'updateChildLink'; family: Id; child: Id; rel: [ParentRelId, ParentRelId]; preferred: boolean }
  | { type: 'unlinkChild'; family: Id; child: Id }
  | { type: 'reorderChildren'; family: Id; order: Id[] }
  | { type: 'saveNote'; target: OwnerRef; note: { id?: Id; kind: NoteDto['kind']; text: string } }
  | { type: 'deleteNote'; target: OwnerRef; note: Id }
  | {
      type: 'saveSource';
      source: { id?: Id; title?: string; author?: string; publication?: string; abbreviation?: string; text?: string };
    }
  | { type: 'addCitation'; target: OwnerRef; citation: CitationInput | { reuseLast: true } | { cloneOf: Id } }
  | { type: 'updateCitation'; id: Id; citation: Omit<CitationInput, 'source'> & { source?: Id } }
  | { type: 'removeCitation'; target: OwnerRef; citation: Id }
  | { type: 'attachMedia'; target: OwnerRef; media: Id[] }
  | { type: 'detachMedia'; target: OwnerRef; media: Id }
  | { type: 'setPrimaryMedia'; person: Id; media: Id }
  | { type: 'updateMedia'; id: Id; title?: string; description?: string; dateRaw?: string };
