// Which pictures of the old maps to have in memory. A tree can hold dozens of scanned sheets; decoding them all (the
// originals are up to 11 000 pixels across) froze the Map. So: a map that is hidden, not placed, or off the screen asks
// for nothing; one on the screen gets its 1600-pixel preview; only the few largest on the screen that the view shows
// bigger than the preview get their original; and pictures not wanted for a while are let go.
import type { Id, MapCorner, MapLayerDto } from '../api/types';
import { cornersToWorld, screenQuad, wantsMore, type Quad } from './layers';
import type { Camera } from './projection';

/** At most this many originals are kept at once. */
export const MAX_FULL = 2;
/** A preview not wanted for this long is let go. */
export const KEEP_PREVIEW_MS = 30_000;
/** An original not wanted for this long is let go. */
export const KEEP_FULL_MS = 8_000;
/** How far past the view's edges (as a share of its size) a map counts as on the screen, so a short pan finds it loaded. */
export const MARGIN = 0.25;

/** What is held for one map, and when it was last wanted. */
export interface Held {
  preview: boolean;
  /** The longest side of the preview held, in pixels. */
  previewSide: number;
  full: boolean;
  seen: number;
  fullSeen: number;
}

export interface LoadPlan {
  /** Maps whose preview is wanted now (shown, placed, on the screen). */
  wanted: Set<Id>;
  /** Maps whose original is wanted now. */
  wantedFull: Set<Id>;
  /** Previews to ask for. */
  preview: Id[];
  /** Originals to ask for. */
  full: Id[];
  /** Maps to let go of entirely. */
  drop: Id[];
  /** Maps to keep only the preview of. */
  dropFull: Id[];
}

/** Whether a screen quad touches the view grown by `margin` of its size on every side. */
export function nearScreen(q: Quad, w: number, h: number, margin = MARGIN): boolean {
  const mx = w * margin;
  const my = h * margin;
  let x0 = Infinity;
  let x1 = -Infinity;
  let y0 = Infinity;
  let y1 = -Infinity;
  for (const [x, y] of q) {
    x0 = Math.min(x0, x);
    x1 = Math.max(x1, x);
    y0 = Math.min(y0, y);
    y1 = Math.max(y1, y);
  }
  return x1 >= -mx && x0 <= w + mx && y1 >= -my && y0 <= h + my;
}

/** The part of the view a quad covers, in square pixels (its bounding box clipped to the view): the bigger, the more an original helps. */
function onScreenArea(q: Quad, w: number, h: number): number {
  const xs = q.map((p) => p[0]);
  const ys = q.map((p) => p[1]);
  const cw = Math.max(0, Math.min(w, Math.max(...xs)) - Math.max(0, Math.min(...xs)));
  const ch = Math.max(0, Math.min(h, Math.max(...ys)) - Math.max(0, Math.min(...ys)));
  return cw * ch;
}

export interface PlanInput {
  /** Every layer, in any order. */
  layers: MapLayerDto[];
  /** The map being placed and its corners now: shown even when hidden, and loaded first. */
  placing: { media: Id; corners: MapCorner[] } | null;
  cam: Camera;
  w: number;
  h: number;
  /** Device pixels per CSS pixel. */
  scale: number;
  held: ReadonlyMap<Id, Held>;
  now: number;
  /** While the view moves, no original is asked for (it would be stale before it arrives). */
  moving?: boolean;
  maxFull?: number;
}

/** What to load and what to let go of, for the view as it is. The caller marks `wanted` maps as seen. */
export function planLoads(input: PlanInput): LoadPlan {
  const { layers, placing, cam, w, h, scale, held, now } = input;
  const maxFull = input.maxFull ?? MAX_FULL;
  const wanted = new Set<Id>();
  const candidates: { id: Id; area: number }[] = [];
  for (const l of layers) {
    const isPlacing = placing?.media === l.media;
    const corners = isPlacing ? placing.corners : l.corners;
    if (!corners || corners.length !== 4 || (!l.visible && !isPlacing)) continue;
    const q = screenQuad(cornersToWorld(corners), cam, w, h);
    if (!nearScreen(q, w, h)) continue;
    wanted.add(l.media);
    const have = held.get(l.media);
    const side = have?.previewSide ?? 0;
    const fullSide = Math.max(l.width ?? 0, l.height ?? 0) || Infinity;
    // A held original counts half again as large, so two maps of about the same size do not swap originals back
    // and forth as the view moves a little.
    const area = isPlacing ? Infinity : onScreenArea(q, w, h) * (have?.full ? 1.5 : 1);
    if (side > 0 && wantsMore(q, scale, side, fullSide)) candidates.push({ id: l.media, area });
  }
  candidates.sort((a, b) => b.area - a.area);
  const wantedFull = new Set(candidates.slice(0, maxFull).map((c) => c.id));
  const out: LoadPlan = { wanted, wantedFull, preview: [], full: [], drop: [], dropFull: [] };
  for (const id of wanted) if (!held.get(id)?.preview) out.preview.push(id);
  if (!input.moving) for (const id of wantedFull) if (!held.get(id)?.full) out.full.push(id);
  let fulls = 0;
  for (const [id, hld] of held) {
    if (!wanted.has(id) && now - hld.seen > KEEP_PREVIEW_MS) {
      out.drop.push(id);
      continue;
    }
    if (hld.full && !wantedFull.has(id) && now - hld.fullSeen > KEEP_FULL_MS) out.dropFull.push(id);
    else if (hld.full) fulls++;
  }
  // Never more originals than the cap plus the ones arriving: the oldest unwanted go first.
  const over = fulls + out.full.length - maxFull;
  if (over > 0) {
    const spare = [...held]
      .filter(([id, hld]) => hld.full && !wantedFull.has(id) && !out.dropFull.includes(id) && !out.drop.includes(id))
      .sort((a, b) => a[1].fullSeen - b[1].fullSeen)
      .slice(0, over)
      .map(([id]) => id);
    out.dropFull.push(...spare);
  }
  return out;
}
