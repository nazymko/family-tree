// The trace draft: cards marked on a picture, the links between them, and the step that turns them into one
// `importDraft` command. Pure functions on plain objects, so the screen and the tests share them.

import type { DraftFamily, DraftPerson, EditCommand, SexKind } from '../api/types';

/** A box on the picture, as fractions (0..1) of its width and height. */
export interface TraceBox {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface ExistingMatch {
  id: string;
  name: string;
}

export interface TraceCard extends TraceBox {
  id: string;
  given: string;
  surname: string;
  patronymic: string;
  sex: SexKind;
  birth: string;
  death: string;
  notes: string;
  /** Set when the card stands for a person already in the tree. */
  existing: ExistingMatch | null;
  /** Drawn dashed until the user accepts it (from the AI reader). */
  suggested: boolean;
}

/** `parent`: `from` is a parent of `to`. `partner`: the two are partners. */
export interface TraceLink {
  id: string;
  kind: 'parent' | 'partner';
  from: string;
  to: string;
  suggested: boolean;
}

export interface TraceDraft {
  version: 1;
  cards: TraceCard[];
  links: TraceLink[];
  /** Keep the picture as a media item and a source when the people are added. */
  keepPicture: boolean;
  nextId: number;
}

export const MAX_CARDS = 500;
const MIN_SIDE = 0.01;
export const DEFAULT_BOX = { w: 0.1, h: 0.04 };

export function newDraft(): TraceDraft {
  return { version: 1, cards: [], links: [], keepPicture: true, nextId: 1 };
}

const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));

/** Keeps a box inside the picture and not smaller than a sliver. */
export function fitBox(b: TraceBox): TraceBox {
  const w = clamp(Number.isFinite(b.w) ? b.w : DEFAULT_BOX.w, MIN_SIDE, 1);
  const h = clamp(Number.isFinite(b.h) ? b.h : DEFAULT_BOX.h, MIN_SIDE, 1);
  return { x: clamp(Number.isFinite(b.x) ? b.x : 0, 0, 1 - w), y: clamp(Number.isFinite(b.y) ? b.y : 0, 0, 1 - h), w, h };
}

export function blankCard(id: string, box: TraceBox): TraceCard {
  return {
    id,
    ...fitBox(box),
    given: '',
    surname: '',
    patronymic: '',
    sex: 'unknown',
    birth: '',
    death: '',
    notes: '',
    existing: null,
    suggested: false,
  };
}

/** Adds an empty card; returns it, or null when the draft is full. */
export function addCard(d: TraceDraft, box: TraceBox): TraceCard | null {
  if (d.cards.length >= MAX_CARDS) return null;
  const card = blankCard(`c${d.nextId++}`, box);
  d.cards.push(card);
  return card;
}

export function removeCard(d: TraceDraft, id: string): void {
  d.cards = d.cards.filter((c) => c.id !== id);
  d.links = d.links.filter((l) => l.from !== id && l.to !== id);
}

export function cardName(c: Pick<TraceCard, 'given' | 'patronymic' | 'surname' | 'existing'>): string {
  const own = [c.given, c.patronymic, c.surname]
    .map((s) => s.trim())
    .filter(Boolean)
    .join(' ');
  return own || c.existing?.name || '';
}

export const hasName = (c: TraceCard): boolean => !!c.existing || !!(c.given.trim() || c.surname.trim());

export type LinkKind = 'parent' | 'child' | 'partner';

/** Adds a link as the user words it: "A is parent of B", "A is child of B", "A is partner of B". */
export function addLink(d: TraceDraft, kind: LinkKind, a: string, b: string): { ok: true; link: TraceLink } | { ok: false; error: string } {
  if (a === b) return { ok: false, error: 'Pick a different card. A person cannot be linked to themselves.' };
  if (!d.cards.some((c) => c.id === a) || !d.cards.some((c) => c.id === b)) return { ok: false, error: 'That card is not on the picture any more.' };
  const [from, to] = kind === 'child' ? [b, a] : [a, b];
  const type = kind === 'partner' ? 'partner' : 'parent';
  const dup = d.links.find((l) => l.kind === type && (type === 'partner' ? (l.from === from && l.to === to) || (l.from === to && l.to === from) : l.from === from && l.to === to));
  if (dup) return { ok: false, error: 'These two cards are already linked that way.' };
  if (type === 'parent' && d.links.some((l) => l.kind === 'parent' && l.from === to && l.to === from))
    return { ok: false, error: 'These cards are already linked the other way round. Remove that link first.' };
  const link: TraceLink = { id: `l${d.nextId++}`, kind: type, from, to, suggested: false };
  d.links.push(link);
  return { ok: true, link };
}

export function removeLink(d: TraceDraft, id: string): void {
  d.links = d.links.filter((l) => l.id !== id);
}

/** What will be added: suggestions the user has not accepted are left out. */
export function activeCards(d: TraceDraft): TraceCard[] {
  return d.cards.filter((c) => !c.suggested);
}

export function activeLinks(d: TraceDraft): TraceLink[] {
  const ok = new Set(activeCards(d).map((c) => c.id));
  return d.links.filter((l) => !l.suggested && ok.has(l.from) && ok.has(l.to));
}

export interface Built {
  families: DraftFamily[];
  /** Things the user must fix before adding. */
  problems: string[];
}

/**
 * Groups the links into families. Two partners and their shared children make one family; children
 * whose only known parent has exactly one partner join that couple; other single parents get a family
 * of their own.
 */
export function buildFamilies(d: TraceDraft): Built {
  const cards = new Map(activeCards(d).map((c) => [c.id, c]));
  const links = activeLinks(d);
  const problems: string[] = [];
  const label = (id: string) => cardName(cards.get(id)!) || 'An unnamed card';

  const parents = new Map<string, string[]>();
  const partners = new Map<string, string[]>();
  for (const l of links) {
    if (l.kind === 'parent') parents.set(l.to, [...(parents.get(l.to) ?? []), l.from]);
    else {
      partners.set(l.from, [...(partners.get(l.from) ?? []), l.to]);
      partners.set(l.to, [...(partners.get(l.to) ?? []), l.from]);
    }
  }
  const families = new Map<string, DraftFamily>();
  const family = (p1: string, p2?: string): DraftFamily => {
    const key = p2 ? [p1, p2].sort().join('|') : p1;
    let f = families.get(key);
    if (!f) {
      f = p2 ? { partner1: p1, partner2: p2, children: [] } : { partner1: p1, children: [] };
      families.set(key, f);
    }
    return f;
  };
  // Couples first, so a couple without children still becomes a family.
  for (const l of links) if (l.kind === 'partner') family(l.from, l.to);
  for (const [child, ps] of parents) {
    let list = ps;
    if (ps.length > 2) {
      problems.push(`${label(child)} has more than two parents. Remove a link.`);
      list = ps.slice(0, 2);
    }
    if (list.length === 2) family(list[0], list[1]).children.push(child);
    else {
      const only = partners.get(list[0]) ?? [];
      (only.length === 1 ? family(list[0], only[0]) : family(list[0])).children.push(child);
    }
  }
  for (const c of cards.values()) if (!hasName(c)) problems.push('A card has no name. Enter a given name or a surname, or remove the card.');
  return { families: [...families.values()], problems: [...new Set(problems)] };
}

function draftPerson(c: TraceCard): DraftPerson {
  if (c.existing) return { key: c.id, existing: c.existing.id, sex: 'unknown' };
  const p: DraftPerson = { key: c.id, sex: c.sex };
  if (c.given.trim()) p.given = c.given.trim();
  if (c.surname.trim()) p.surname = c.surname.trim();
  if (c.patronymic.trim()) p.patronymic = c.patronymic.trim();
  if (c.birth.trim()) p.birthDateRaw = c.birth.trim();
  if (c.death.trim()) p.deathDateRaw = c.death.trim();
  if (c.notes.trim()) p.notes = c.notes.trim();
  return p;
}

export function toCommand(d: TraceDraft): Extract<EditCommand, { type: 'importDraft' }> {
  return {
    type: 'importDraft',
    persons: activeCards(d).map(draftPerson),
    families: buildFamilies(d).families,
    attachPicture: d.keepPicture,
  };
}

export interface Counts {
  /** New people that will be created. */
  newPeople: number;
  /** Cards that stand for people already in the tree. */
  matched: number;
  families: number;
  links: number;
  suggestions: number;
}

export function counts(d: TraceDraft): Counts {
  const cards = activeCards(d);
  return {
    newPeople: cards.filter((c) => !c.existing).length,
    matched: cards.filter((c) => c.existing).length,
    families: buildFamilies(d).families.length,
    links: activeLinks(d).length,
    suggestions: d.cards.filter((c) => c.suggested).length + d.links.filter((l) => l.suggested).length,
  };
}

/** Accepts one suggested card, together with the suggested links whose other end is already accepted. */
export function acceptCard(d: TraceDraft, id: string): void {
  const c = d.cards.find((x) => x.id === id);
  if (!c) return;
  c.suggested = false;
  for (const l of d.links) {
    if (!l.suggested || (l.from !== id && l.to !== id)) continue;
    const other = d.cards.find((x) => x.id === (l.from === id ? l.to : l.from));
    if (other && !other.suggested) l.suggested = false;
  }
}

export function acceptAll(d: TraceDraft): void {
  for (const c of d.cards) c.suggested = false;
  for (const l of d.links) l.suggested = false;
}

/** Drops every suggestion, and the links that hung on suggested cards. */
export function rejectAll(d: TraceDraft): void {
  const gone = new Set(d.cards.filter((c) => c.suggested).map((c) => c.id));
  d.cards = d.cards.filter((c) => !c.suggested);
  d.links = d.links.filter((l) => !l.suggested && !gone.has(l.from) && !gone.has(l.to));
}

const SEXES: SexKind[] = ['male', 'female', 'intersex', 'unknown', 'other'];
const str = (v: unknown, max = 500): string => (typeof v === 'string' ? v.slice(0, max) : '');
const num = (v: unknown): number => (typeof v === 'number' && Number.isFinite(v) ? v : NaN);

/** Reads a saved draft defensively: anything malformed is skipped, never thrown. */
export function parseDraft(json: string | null): TraceDraft | null {
  if (!json) return null;
  let raw: unknown;
  try {
    raw = JSON.parse(json);
  } catch {
    return null;
  }
  if (!raw || typeof raw !== 'object') return null;
  const r = raw as Record<string, unknown>;
  const d = newDraft();
  d.keepPicture = r.keepPicture !== false;
  const ids = new Set<string>();
  for (const x of Array.isArray(r.cards) ? r.cards.slice(0, MAX_CARDS) : []) {
    if (!x || typeof x !== 'object') continue;
    const c = x as Record<string, unknown>;
    const id = str(c.id, 40);
    if (!id || ids.has(id)) continue;
    ids.add(id);
    const ex = c.existing as Record<string, unknown> | null | undefined;
    d.cards.push({
      ...blankCard(id, { x: num(c.x), y: num(c.y), w: num(c.w), h: num(c.h) }),
      given: str(c.given),
      surname: str(c.surname),
      patronymic: str(c.patronymic),
      sex: SEXES.includes(c.sex as SexKind) ? (c.sex as SexKind) : 'unknown',
      birth: str(c.birth, 60),
      death: str(c.death, 60),
      notes: str(c.notes, 5000),
      existing: ex && typeof ex.id === 'string' ? { id: ex.id, name: str(ex.name) } : null,
      suggested: c.suggested === true,
    });
  }
  const seen = new Set<string>();
  for (const x of Array.isArray(r.links) ? r.links.slice(0, MAX_CARDS * 4) : []) {
    if (!x || typeof x !== 'object') continue;
    const l = x as Record<string, unknown>;
    const id = str(l.id, 40);
    const kind = l.kind === 'parent' || l.kind === 'partner' ? l.kind : null;
    const from = str(l.from, 40);
    const to = str(l.to, 40);
    if (!id || !kind || !ids.has(from) || !ids.has(to) || from === to || seen.has(id)) continue;
    seen.add(id);
    d.links.push({ id, kind, from, to, suggested: l.suggested === true });
  }
  // Keep new ids clear of the ones in use.
  d.nextId = Math.max(1, ...[...ids, ...seen].map((s) => Number(s.slice(1)) + 1).filter(Number.isFinite), num(r.nextId) || 1);
  return d;
}
