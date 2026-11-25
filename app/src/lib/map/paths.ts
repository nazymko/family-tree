import type { MapDataDto, MapEventDto, MapPersonDto } from '../api/types';
import { mercator, type World } from './projection';

/** One stop of a life: a place, with the years the person had events there (consecutive events at one place are one stop). */
export interface Stop {
  place: string;
  name: string;
  x: number;
  y: number;
  from: number | null;
  to: number | null;
  kinds: string[];
}

export interface PersonPath {
  person: MapPersonDto;
  stops: Stop[];
}

/** An arrow from a parent's birth place to a child's. */
export interface BirthLink {
  parent: string;
  child: string;
  from: World;
  to: World;
  /** Generation of the child (the parent's is one more for ancestors, one less for descendants). */
  generation: number;
}

/** Undated events still have an order in a life: birth first, death and burial last, the rest between. */
function rank(e: MapEventDto): number {
  if (e.year !== null) return e.year;
  if (e.kind === 'birth' || e.kind === 'baptism' || e.kind === 'christening') return -1e9;
  if (e.kind === 'death' || e.kind === 'burial') return 1e9;
  return 0;
}

export function buildPaths(data: MapDataDto, events: readonly MapEventDto[]): PersonPath[] {
  const place = new Map(data.places.filter((p) => p.lat !== null && p.lon !== null).map((p) => [p.id, p]));
  const by = new Map<string, MapEventDto[]>();
  for (const e of events) {
    if (!place.has(e.place)) continue;
    const list = by.get(e.person);
    if (list) list.push(e);
    else by.set(e.person, [e]);
  }
  const out: PersonPath[] = [];
  for (const person of data.people) {
    const list = by.get(person.id);
    if (!list) continue;
    // A stable sort: events at the same rank keep the order the payload had.
    const ordered = list.map((e, i) => ({ e, i })).sort((a, b) => rank(a.e) - rank(b.e) || a.i - b.i);
    const stops: Stop[] = [];
    for (const { e } of ordered) {
      const p = place.get(e.place)!;
      const last = stops[stops.length - 1];
      if (last && last.place === p.id) {
        if (e.year !== null) {
          last.from = last.from === null ? e.year : Math.min(last.from, e.year);
          last.to = last.to === null ? e.year : Math.max(last.to, e.year);
        }
        if (!last.kinds.includes(e.kind)) last.kinds.push(e.kind);
        continue;
      }
      const w = mercator(p.lat!, p.lon!);
      stops.push({ place: p.id, name: p.name, x: w.x, y: w.y, from: e.year, to: e.year, kinds: [e.kind] });
    }
    out.push({ person, stops });
  }
  return out;
}

/** The first birth (or baptism) place of each person that is on the map. */
function birthPlaces(data: MapDataDto, events: readonly MapEventDto[]): Map<string, string> {
  const ok = new Set(data.places.filter((p) => p.lat !== null && p.lon !== null).map((p) => p.id));
  const out = new Map<string, string>();
  for (const e of events) if (e.kind === 'birth' && ok.has(e.place) && !out.has(e.person)) out.set(e.person, e.place);
  return out;
}

/** Parent to child arrows between birth places, only where the two places differ and both are on the map. */
export function birthLinks(data: MapDataDto, events: readonly MapEventDto[]): BirthLink[] {
  const born = birthPlaces(data, events);
  const place = new Map(data.places.map((p) => [p.id, p]));
  const gen = new Map(data.people.map((p) => [p.id, p.generation]));
  const out: BirthLink[] = [];
  const seen = new Set<string>();
  for (const l of data.links) {
    const a = born.get(l.parent);
    const b = born.get(l.child);
    if (!a || !b || a === b) continue;
    const key = `${l.parent}>${l.child}`;
    if (seen.has(key)) continue;
    seen.add(key);
    const pa = place.get(a)!;
    const pb = place.get(b)!;
    out.push({
      parent: l.parent,
      child: l.child,
      from: mercator(pa.lat!, pa.lon!),
      to: mercator(pb.lat!, pb.lon!),
      generation: Math.max(gen.get(l.parent) ?? 0, gen.get(l.child) ?? 0),
    });
  }
  return out;
}

export type ColorBy = 'generation' | 'surname';

/** Distinct, colour-blind-friendly hues; the first is for the person the scope is about. */
export const PALETTE = ['#0b6e99', '#d9730d', '#0f7b6c', '#ad1a72', '#6940a5', '#c4554d', '#8a6d1a', '#4a6b2c', '#2b59c3', '#9f6b53'];

export interface Legend {
  label: string;
  color: string;
}

/** Colours people by generation or by surname, and the legend that explains them (surnames: the nine most numerous, then "other"). */
export function colorScheme(people: readonly MapPersonDto[], by: ColorBy, other = 'other'): { color: (p: MapPersonDto) => string; legend: Legend[] } {
  if (by === 'generation') {
    const gens = [...new Set(people.map((p) => p.generation))].sort((a, b) => a - b);
    const at = (g: number) => PALETTE[Math.max(0, gens.indexOf(g)) % PALETTE.length];
    return { color: (p) => at(p.generation), legend: gens.map((g) => ({ label: String(g), color: at(g) })) };
  }
  const count = new Map<string, number>();
  for (const p of people) count.set(p.surname, (count.get(p.surname) ?? 0) + 1);
  const top = [...count].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).slice(0, PALETTE.length - 1).map(([s]) => s);
  const grey = '#8a8a8a';
  const legend: Legend[] = top.map((s, i) => ({ label: s || '?', color: PALETTE[i] }));
  if (count.size > top.length) legend.push({ label: other, color: grey });
  return { color: (p) => PALETTE[top.indexOf(p.surname)] ?? grey, legend };
}
