import type { MapDataDto, MapEventDto } from '../api/types';
import { mercator, toScreen, type Camera } from './projection';

/** One point on the map: a place with at least one selected event. */
export interface PlacePoint {
  id: string;
  name: string;
  label: string;
  x: number;
  y: number;
  count: number;
  people: Set<string>;
  byKind: Map<string, number>;
  events: MapEventDto[];
}

/** Groups events by place; places without a point are left out (they are counted by `missingCoordinates`). */
export function placePoints(data: MapDataDto, events: readonly MapEventDto[]): PlacePoint[] {
  const where = new Map<string, PlacePoint>();
  const base = new Map(
    data.places.filter((p) => p.lat !== null && p.lon !== null).map((p) => [p.id, { p, w: mercator(p.lat!, p.lon!) }] as const),
  );
  for (const e of events) {
    const at = base.get(e.place);
    if (!at) continue;
    let pt = where.get(e.place);
    if (!pt) {
      pt = { id: at.p.id, name: at.p.name, label: at.p.label, x: at.w.x, y: at.w.y, count: 0, people: new Set(), byKind: new Map(), events: [] };
      where.set(e.place, pt);
    }
    pt.count++;
    pt.people.add(e.person);
    pt.byKind.set(e.kind, (pt.byKind.get(e.kind) ?? 0) + 1);
    pt.events.push(e);
  }
  return [...where.values()];
}

/** The dot radius in px for `count` events: area grows with the count, from 4 to 26 px. */
export const radiusFor = (count: number): number => Math.min(26, 4 + 3 * Math.sqrt(Math.max(0, count - 1)));

/** The point under a screen position (the nearest dot whose circle holds it), or null. */
export function hitPoint(points: readonly PlacePoint[], cam: Camera, w: number, h: number, sx: number, sy: number): PlacePoint | null {
  let best: PlacePoint | null = null;
  let bestD = Infinity;
  for (const p of points) {
    const [px, py] = toScreen(cam, w, h, p.x, p.y);
    const d = Math.hypot(px - sx, py - sy);
    if (d <= Math.max(radiusFor(p.count), 9) && d < bestD) {
      best = p;
      bestD = d;
    }
  }
  return best;
}

export interface Label {
  text: string;
  x: number;
  y: number;
  w: number;
}

/** Labels for the busiest points first, each only when its box does not overlap one already placed. `measure` gives the text width. */
export function pickLabels(points: readonly PlacePoint[], cam: Camera, w: number, h: number, measure: (s: string) => number, lineH = 14): Label[] {
  const placed: { x0: number; x1: number; y0: number; y1: number }[] = [];
  const out: Label[] = [];
  const order = [...points].sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
  for (const p of order) {
    const [sx, sy] = toScreen(cam, w, h, p.x, p.y);
    if (sx < -40 || sy < -40 || sx > w + 40 || sy > h + 40) continue;
    const tw = measure(p.name);
    const x = sx + radiusFor(p.count) + 3;
    const box = { x0: x - 2, x1: x + tw + 2, y0: sy - lineH / 2, y1: sy + lineH / 2 };
    if (placed.some((q) => box.x0 < q.x1 && box.x1 > q.x0 && box.y0 < q.y1 && box.y1 > q.y0)) continue;
    placed.push(box);
    out.push({ text: p.name, x, y: sy, w: tw });
  }
  return out;
}
