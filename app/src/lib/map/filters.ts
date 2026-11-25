import type { MapDataDto, MapEventDto, MapPlaceDto } from '../api/types';

/** The event kinds the Map offers by name, in the order of a life; other kinds appear after them under their own names. */
export const KIND_ORDER = ['birth', 'baptism', 'christening', 'marriage', 'residence', 'death', 'burial'];

export interface MapFilter {
  /** Event kinds shown; empty means every kind. */
  kinds: ReadonlySet<string>;
  /** First and last year shown; null is open. Undated events are shown only while both are open. */
  from: number | null;
  to: number | null;
}

export const noFilter = (): MapFilter => ({ kinds: new Set(), from: null, to: null });

/** The kinds that occur in the data, life order first. */
export function kindsIn(events: readonly MapEventDto[]): string[] {
  const seen = new Set(events.map((e) => e.kind));
  const known = KIND_ORDER.filter((k) => seen.has(k));
  const other = [...seen].filter((k) => !KIND_ORDER.includes(k)).sort();
  return [...known, ...other];
}

export function yearRange(events: readonly MapEventDto[]): [number, number] | null {
  let lo = Infinity;
  let hi = -Infinity;
  for (const e of events) {
    if (e.year === null) continue;
    lo = Math.min(lo, e.year);
    hi = Math.max(hi, e.year);
  }
  return lo <= hi ? [lo, hi] : null;
}

export function filterEvents(events: readonly MapEventDto[], f: MapFilter): MapEventDto[] {
  const open = f.from === null && f.to === null;
  return events.filter((e) => {
    if (f.kinds.size > 0 && !f.kinds.has(e.kind)) return false;
    if (e.year === null) return open;
    return (f.from === null || e.year >= f.from) && (f.to === null || e.year <= f.to);
  });
}

export interface Missing {
  /** Events of the selection whose place has no point on the map. */
  events: number;
  /** Those places, most events first. */
  places: { place: MapPlaceDto; events: number }[];
  /** Events with no place at all (nothing to set). */
  noPlace: number;
}

export function missingCoordinates(data: MapDataDto, events: readonly MapEventDto[]): Missing {
  const byId = new Map(data.places.map((p) => [p.id, p]));
  const count = new Map<string, number>();
  for (const e of events) {
    const p = byId.get(e.place);
    if (p && (p.lat === null || p.lon === null)) count.set(p.id, (count.get(p.id) ?? 0) + 1);
  }
  const places = [...count].map(([id, n]) => ({ place: byId.get(id)!, events: n })).sort((a, b) => b.events - a.events || a.place.label.localeCompare(b.place.label));
  return { events: places.reduce((n, p) => n + p.events, 0), places, noPlace: data.eventsWithoutPlace };
}
