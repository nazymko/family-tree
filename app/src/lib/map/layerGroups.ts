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
