// Choosing which old maps the Map shows: the layers grouped by series, a group shown or hidden as a whole, one map shown
// alone, a filter by title or sheet, and the maps that lie under a point. Plain functions over the layer list, so the
// panel stays thin and the rules are tested on their own.
import type { Id, MapLayerDto } from '../api/types';

/** One series of maps, the top map first. `series` is null for the maps that name none. */
export interface LayerGroup {
  key: string;
  series: string | null;
  layers: MapLayerDto[];
  shown: number;
  /** Set in place mode: the place's id and leaf name, or `other` for the maps that cover no place. */
  placeId?: string;
  placeName?: string;
  other?: boolean;
}

/** Whether a group is all shown, all hidden or partly shown (the group checkbox's three states). */
export type Tri = 'all' | 'none' | 'some';

/** What one change of visibility does: the maps to show and the maps to hide, only those that change. */
export interface VisibilityChange {
  shown: Id[];
  hidden: Id[];
}

/** The key two spellings of one series share: trimmed, spaces folded, case ignored. Empty for no series. */
export function seriesKey(series: string | null | undefined): string {
  return (series ?? '').trim().replace(/\s+/g, ' ').toLocaleLowerCase();
}

const fold = (s: string) => s.toLocaleLowerCase().replace(/\s+/g, ' ');

/** Whether a layer matches the filter text: every word in its title, series, year or source. */
export function matches(l: MapLayerDto, filter: string): boolean {
  const words = fold(filter).trim().split(' ').filter(Boolean);
  if (words.length === 0) return true;
  const hay = fold([l.title, l.series, l.year, l.source].filter(Boolean).join(' '));
  return words.every((w) => hay.includes(w));
}

/**
 * The layers (top first) grouped by series. A group's place is that of its top map; the maps without a series come
 * last. Only maps matching `filter` are listed; a group with none is left out.
 */
export function groupLayers(top: MapLayerDto[], filter = ''): LayerGroup[] {
  const groups = new Map<string, LayerGroup>();
  for (const l of top) {
    if (!matches(l, filter)) continue;
    const key = seriesKey(l.series);
    let g = groups.get(key);
    if (!g) {
      g = { key, series: key ? l.series!.trim().replace(/\s+/g, ' ') : null, layers: [], shown: 0 };
      groups.set(key, g);
    }
    g.layers.push(l);
    if (l.visible) g.shown++;
  }
  const out = [...groups.values()];
  return [...out.filter((g) => g.key), ...out.filter((g) => !g.key)];
}

export function triState(layers: MapLayerDto[]): Tri {
  const shown = layers.filter((l) => l.visible).length;
  return shown === 0 ? 'none' : shown === layers.length ? 'all' : 'some';
}

/** Shows (or hides) every one of `layers`; the group checkbox shows a partly shown group in full. */
export function setAll(layers: MapLayerDto[], visible: boolean): VisibilityChange {
  const ids = layers.filter((l) => l.visible !== visible).map((l) => l.media);
  return visible ? { shown: ids, hidden: [] } : { shown: [], hidden: ids };
}

/** What a click on a group checkbox does: a fully shown group is hidden, any other is shown in full. */
export function toggleGroup(layers: MapLayerDto[]): VisibilityChange {
  return setAll(layers, triState(layers) !== 'all');
}

/** Only this map shown: it, if hidden, is shown and every other shown map is hidden. */
export function onlyThis(all: MapLayerDto[], media: Id): VisibilityChange {
  return {
    shown: all.filter((l) => l.media === media && !l.visible).map((l) => l.media),
    hidden: all.filter((l) => l.media !== media && l.visible).map((l) => l.media),
  };
}

export const isNoChange = (c: VisibilityChange) => c.shown.length === 0 && c.hidden.length === 0;

/** Whether a point (degrees) lies inside the quadrilateral of a layer's corners, by the even-odd rule. */
export function contains(l: MapLayerDto, lat: number, lon: number): boolean {
  const c = l.corners;
  if (!c || c.length !== 4) return false;
  let inside = false;
  for (let i = 0, j = 3; i < 4; j = i++) {
    const yi = c[i].latE6 / 1e6;
    const xi = c[i].lonE6 / 1e6;
    const yj = c[j].latE6 / 1e6;
    const xj = c[j].lonE6 / 1e6;
    if (yi > lat !== yj > lat && lon < ((xj - xi) * (lat - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

/** The placed maps (top first) whose corners hold the point. */
export function layersUnder(top: MapLayerDto[], lat: number, lon: number): MapLayerDto[] {
  return top.filter((l) => contains(l, lat, lon));
}

// ---- Years and places ------------------------------------------------------------------------------------------------

/** The years a map covers, both ends included. */
export interface YearSpan {
  from: number;
  to: number;
}

const ROMAN: Record<string, number> = { I: 1, V: 5, X: 10 };
function roman(s: string): number {
  let n = 0;
  for (let i = 0; i < s.length; i++) {
    const v = ROMAN[s[i]];
    n += v < (ROMAN[s[i + 1]] ?? 0) ? -v : v;
  }
  return n;
}

/** Cyrillic look-alikes of the roman digits and of "c" are typed all the time in «ХІХ ст.». */
const latin = (s: string) => s.replace(/[Хх]/g, 'X').replace(/[Іі]/g, 'I').replace(/[Сс]/g, 'c');

function spanOf(text: string): YearSpan | null {
  const t = latin(text);
  const range = t.match(/(?<!\d)(1\d{3}|20\d{2})\s*[–—-]\s*(\d{4}|\d{2})(?!\d)/);
  if (range) {
    const from = Number(range[1]);
    let to = Number(range[2]);
    if (range[2].length === 2) to = Math.floor(from / 100) * 100 + to;
    if (to >= from) return { from, to };
  }
  const years = [...t.matchAll(/(?<!\d)(1\d{3}|20\d{2})(?!\d)/g)].map((m) => Number(m[1]));
  if (years.length > 0) return { from: Math.min(...years), to: Math.max(...years) };
  const rc = t.match(/(?<![\p{L}\d])([IVX]{1,5})\s*(?:cent|c[тt]?|в)\.?(?![\p{L}])/iu);
  const ord = t.match(/(?<!\d)(\d{1,2})(?:st|nd|rd|th)\s*c/i);
  const c = rc ? roman(rc[1].toUpperCase()) : ord ? Number(ord[1]) : 0;
  if (c >= 1 && c <= 21) return { from: (c - 1) * 100 + 1, to: c * 100 };
  return null;
}

/** The years a layer covers, read from its year text, else from its title; null when neither holds a year. */
export function yearSpan(l: Pick<MapLayerDto, 'year' | 'title'>): YearSpan | null {
  return (l.year ? spanOf(l.year) : null) ?? (l.title ? spanOf(l.title) : null);
}

/** What the panel filters by. `from`/`to` are years or null (open); `place` is a place id or null (all places). */
export interface LayerFilters {
  text: string;
  from: number | null;
  to: number | null;
  noYear: boolean;
  place: string | null;
}

export const noFilters: LayerFilters = { text: '', from: null, to: null, noYear: true, place: null };

/** Whether a layer's years overlap the chosen range; a layer with no readable year follows `noYear`. */
export function yearMatches(l: MapLayerDto, f: Pick<LayerFilters, 'from' | 'to' | 'noYear'>): boolean {
  if (f.from === null && f.to === null) return true;
  const s = yearSpan(l);
  if (!s) return f.noYear;
  return (f.from === null || s.to >= f.from) && (f.to === null || s.from <= f.to);
}

/** A place of the родовід as the panel needs it: `count` is the number of family events there. */
export interface PlaceRef {
  id: string;
  name: string;
  label: string;
  lat: number | null;
  lon: number | null;
  count: number;
}

/** Place id -> the maps (top first) whose corners hold it. Computed once per layer or place change. */
export function mapsByPlace(top: MapLayerDto[], places: readonly PlaceRef[]): Map<string, MapLayerDto[]> {
  const out = new Map<string, MapLayerDto[]>();
  const placed = top.filter((l) => l.corners?.length === 4);
  for (const p of places) {
    if (p.lat === null || p.lon === null) continue;
    const under = placed.filter((l) => contains(l, p.lat!, p.lon!));
    if (under.length > 0) out.set(p.id, under);
  }
  return out;
}

export interface PlaceOption {
  id: string;
  name: string;
  label: string;
  maps: number;
}

/** The places that lie under at least one map, by name. */
export function placeOptions(index: Map<string, MapLayerDto[]>, places: readonly PlaceRef[]): PlaceOption[] {
  return places
    .filter((p) => index.has(p.id))
    .map((p) => ({ id: p.id, name: p.name, label: p.label, maps: index.get(p.id)!.length }))
    .sort((a, b) => a.name.localeCompare(b.name) || a.label.localeCompare(b.label));
}

/** The layers that pass text AND years AND place. */
export function filterLayers(top: MapLayerDto[], index: Map<string, MapLayerDto[]>, f: LayerFilters): MapLayerDto[] {
  const inPlace = f.place ? new Set((index.get(f.place) ?? []).map((l) => l.media)) : null;
  return top.filter((l) => matches(l, f.text) && yearMatches(l, f) && (!inPlace || inPlace.has(l.media)));
}

/**
 * The layers grouped by place: each place (most family events first, then by name) lists the maps that cover it, a map
 * under several places in each; the maps covering none go last as "other" (key `other`). With a place chosen only that
 * place's group is returned.
 */
export function groupByPlace(top: MapLayerDto[], places: readonly PlaceRef[], index: Map<string, MapLayerDto[]>, f: LayerFilters): LayerGroup[] {
  const keep = new Set(filterLayers(top, index, f).map((l) => l.media));
  const out: LayerGroup[] = [];
  const covered = new Set<string>();
  const ordered = [...places].sort((a, b) => b.count - a.count || a.name.localeCompare(b.name) || a.id.localeCompare(b.id));
  for (const p of ordered) {
    const all = index.get(p.id);
    if (!all) continue;
    for (const l of all) covered.add(l.media);
    if (f.place && f.place !== p.id) continue;
    const layers = all.filter((l) => keep.has(l.media));
    if (layers.length === 0) continue;
    out.push({ key: `p:${p.id}`, series: null, placeId: p.id, placeName: p.name, layers, shown: layers.filter((l) => l.visible).length });
  }
  if (!f.place) {
    const rest = top.filter((l) => keep.has(l.media) && !covered.has(l.media));
    if (rest.length > 0) out.push({ key: 'other', series: null, other: true, layers: rest, shown: rest.filter((l) => l.visible).length });
  }
  return out;
}

/** Only these maps shown: those of `layers` that are hidden are shown and every other shown map is hidden. */
export function onlyThese(all: MapLayerDto[], layers: MapLayerDto[]): VisibilityChange {
  const keep = new Set(layers.map((l) => l.media));
  return {
    shown: all.filter((l) => keep.has(l.media) && !l.visible).map((l) => l.media),
    hidden: all.filter((l) => !keep.has(l.media) && l.visible).map((l) => l.media),
  };
}
