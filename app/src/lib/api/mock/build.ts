import type { DayOrder, EventDto, Id, NameDto, NameInput, NoteDto, PlaceOption } from '../types';
import { toDateDto } from './mockParse';

export type Rng = () => number;

export function mulberry32(seed: number): Rng {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const HEX = '0123456789abcdef';

/** Deterministic UUID-v7-shaped ids: a rising 48-bit time prefix, then seeded random bits. */
export class IdGen {
  private n = 0;
  constructor(
    private rng: Rng,
    private base = 1_700_000_000_000,
  ) {}
  next(): Id {
    const ts = (this.base + this.n++).toString(16).padStart(12, '0');
    const r = (len: number) => Array.from({ length: len }, () => HEX[Math.floor(this.rng() * 16)]).join('');
    return `${ts.slice(0, 8)}-${ts.slice(8)}-7${r(3)}-${'89ab'[Math.floor(this.rng() * 4)]}${r(3)}-${r(12)}`;
  }
}

const KIND_LABELS: Record<string, string> = {
  birth: 'Birth',
  death: 'Death',
  baptism: 'Baptism',
  christening: 'Christening',
  burial: 'Burial',
  cremation: 'Cremation',
  marriage: 'Marriage',
  divorce: 'Divorce',
  engagement: 'Engagement',
  residence: 'Residence',
  occupation: 'Occupation',
  education: 'Education',
  emigration: 'Emigration',
  immigration: 'Immigration',
  naturalization: 'Naturalization',
  census: 'Census',
  military: 'Military service',
  religion: 'Religion',
  nationality: 'Nationality',
  title: 'Title',
  property: 'Property',
  illness: 'Illness',
  will: 'Will',
  probate: 'Probate',
};

const FACT_KINDS = new Set(['occupation', 'education', 'religion', 'nationality', 'title', 'property', 'residence', 'illness']);

export function eventLabel(kind: string): string {
  if (kind.startsWith('custom:')) {
    const t = kind.slice(7).trim();
    return t ? t.charAt(0).toUpperCase() + t.slice(1) : 'Event';
  }
  return KIND_LABELS[kind] ?? kind.replace(/_/g, ' ').replace(/^./, (c) => c.toUpperCase());
}

export function isFactKind(kind: string): boolean {
  return FACT_KINDS.has(kind);
}

export function makeEvent(
  id: Id,
  kind: string,
  opts: {
    dateRaw?: string | null;
    place?: PlaceOption | null;
    description?: string | null;
    order?: DayOrder;
    negative?: boolean;
    isPrivate?: boolean;
    preferred?: boolean;
    age?: string | null;
    cause?: string | null;
  } = {},
): EventDto {
  return {
    id,
    kind,
    label: eventLabel(kind),
    isFact: isFactKind(kind),
    date: toDateDto(opts.dateRaw, opts.order ?? 'dmy'),
    place: opts.place ?? null,
    description: opts.description ?? null,
    age: opts.age ?? null,
    cause: opts.cause ?? null,
    negative: opts.negative ?? false,
    private: opts.isPrivate ?? false,
    preferred: opts.preferred ?? false,
    participants: [],
    citations: [],
    notes: [],
    media: [],
  };
}

export function nameDisplay(n: {
  prefix?: string | null;
  given?: string | null;
  patronymic?: string | null;
  surnamePrefix?: string | null;
  surnames: string[];
  suffix?: string | null;
  cjk?: boolean;
}): string {
  if (n.cjk) return `${n.surnames.join('')}${n.given ?? ''}`;
  return [n.prefix, n.given, n.patronymic, n.surnamePrefix, n.surnames.join(' '), n.suffix]
    .filter((s): s is string => !!s && s.trim() !== '')
    .join(' ');
}

const CJK_RE = /[぀-ヿ㐀-鿿]/;

export function makeName(
  id: Id,
  input: Omit<NameInput, 'id'>,
  order: DayOrder = 'dmy',
): NameDto {
  const cjk = CJK_RE.test(`${input.given ?? ''}${input.surnames.join('')}`);
  return {
    id,
    kind: input.kind,
    kindText: input.kindText ?? null,
    preferred: input.preferred,
    private: input.private,
    display: nameDisplay({ ...input, cjk }),
    prefix: input.prefix ?? null,
    given: input.given ?? null,
    call: input.call ?? null,
    surnamePrefix: input.surnamePrefix ?? null,
    surnames: input.surnames,
    patronymic: input.patronymic ?? null,
    suffix: input.suffix ?? null,
    nickname: input.nickname ?? null,
    lang: input.lang ?? null,
    date: toDateDto(input.dateRaw, order),
    citations: [],
  };
}

export function makeNote(id: Id, kind: NoteDto['kind'], text: string): NoteDto {
  return { id, kind, text, shared: false, lang: null, media: [] };
}

/** Lower-case and strip diacritics so "Müller" matches "muller". */
export function fold(s: string): string {
  return s.normalize('NFD').replace(/\p{M}+/gu, '').toLowerCase();
}

export function hashString(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}
