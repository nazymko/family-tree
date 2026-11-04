import type { ChartEdge, ChartKind, ChartNode, Rect } from '../api/types';
import type { Camera } from './camera';

/** What a picture can show: the whole chart, the selected person's branch, or what is on the screen. */
export type PictureScope = 'whole' | 'branch' | 'screen';

/** The nodes and the lines between them that go into a picture. */
export interface Scene {
  nodes: ChartNode[];
  edges: ChartEdge[];
}

/** Space (px) around a whole or branch picture. */
export const MARGIN = 40;
/** Height (px, before scaling) of the strip with the chart name and the date. */
export const CAPTION_H = 30;
/** Largest side (px) of a picture. */
export const MAX_SIDE = 16384;
/** Largest area (px) of a picture. */
export const MAX_AREA = 200_000_000;
/** Side (world units) of one request when a whole chart is fetched in pieces. */
export const TILE = 4000;

// ------------------------------------------------------------ which people

function adjacency(edges: ChartEdge[], kind: ChartEdge['kind'], reverse: boolean): Map<number, number[]> {
  const out = new Map<number, number[]>();
  for (const e of edges) {
    if (e.kind !== kind) continue;
    const [a, b] = reverse ? [e.to, e.from] : [e.from, e.to];
    const list = out.get(a);
    if (list) list.push(b);
    else out.set(a, [b]);
  }
  return out;
}

/** Adds everything reachable from `set` along `next` (the start ids stay in). */
function close(set: Set<number>, next: Map<number, number[]>): void {
  const todo = [...set];
  while (todo.length) {
    for (const n of next.get(todo.pop()!) ?? []) {
      if (!set.has(n)) {
        set.add(n);
        todo.push(n);
      }
    }
  }
}

/**
 * The people of the selected person's branch, going away from the root of the chart, plus the partners of all of them.
 *  - pedigree: the person and their ancestors (edges run from a person to the ancestors);
 *  - descendants: the person and their descendants, and the partners of those people;
 *  - relatives: the direct line: ancestors, descendants and partners (no brothers, sisters or cousins);
 *  - whole tree: the lineage the backend gives (`lineage`) and the partners of those people.
 */
export function branchIds(kind: ChartKind, scene: Scene, selected: number, lineage?: Iterable<number>): Set<number> {
  const down = adjacency(scene.edges, 'parent', false);
  const up = adjacency(scene.edges, 'parent', true);
  const partners = new Map<number, number[]>();
  for (const [a, list] of adjacency(scene.edges, 'partner', false)) partners.set(a, [...(partners.get(a) ?? []), ...list]);
  for (const [a, list] of adjacency(scene.edges, 'partner', true)) partners.set(a, [...(partners.get(a) ?? []), ...list]);

  const set = new Set<number>([selected]);
  if (kind === 'pedigree') {
    close(set, down);
    return set;
  }
  if (kind === 'whole') {
    for (const id of lineage ?? []) set.add(id);
  } else if (kind === 'descendants') {
    // A child edge may leave from either partner of a couple, so partners and children are followed together.
    let size = -1;
    while (size !== set.size) {
      size = set.size;
      close(set, partners);
      close(set, down);
    }
    return set;
  } else {
    // Relatives: the ancestors of the person, and the descendants of the person and their partners.
    close(set, up);
    const below = new Set<number>([selected]);
    let size = -1;
    while (size !== below.size) {
      size = below.size;
      close(below, partners);
      close(below, down);
    }
    for (const id of below) set.add(id);
  }
  // The partners of everyone in the branch.
  close(set, partners);
  return set;
}

/** The scene for "whole" (everything) or "branch" (the selected person's branch only). */
export function sceneFor(scope: 'whole' | 'branch', kind: ChartKind, all: Scene, selected: number | null, lineage?: Iterable<number>): Scene {
  if (scope === 'whole') return all;
  if (selected === null || !all.nodes.some((n) => n.id === selected)) return { nodes: [], edges: [] };
  const ids = branchIds(kind, all, selected, lineage);
  return {
    nodes: all.nodes.filter((n) => ids.has(n.id)),
    edges: all.edges.filter((e) => ids.has(e.from) && ids.has(e.to)),
  };
}

// ------------------------------------------------------------ geometry

/** The box around the nodes, the points of the lines and any extra rectangles (block headings). */
export function sceneBounds(scene: Scene, extra: Rect[] = []): Rect {
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -Infinity;
  let y1 = -Infinity;
  const add = (x: number, y: number) => {
    if (x < x0) x0 = x;
    if (x > x1) x1 = x;
    if (y < y0) y0 = y;
    if (y > y1) y1 = y;
  };
  for (const n of scene.nodes) {
    add(n.x, n.y);
    add(n.x + n.w, n.y + n.h);
  }
  for (const e of scene.edges) for (let i = 0; i + 1 < e.pts.length; i += 2) add(e.pts[i], e.pts[i + 1]);
  for (const r of extra) {
    add(r.x, r.y);
    add(r.x + r.w, r.y + r.h);
  }
  return x0 === Infinity ? { x: 0, y: 0, w: 0, h: 0 } : { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}

export interface Fit {
  /** Multiplier (at most 1) that makes the picture fit the safe size. */
  scale: number;
  w: number;
  h: number;
  reduced: boolean;
  /** The size relative to what was asked for, as a whole percent (below 100 when reduced). */
  percent: number;
}

/** Scales a picture of `w` x `h` px down so each side is at most 16,384 px and the area at most about 200 MP. */
export function fitScale(w: number, h: number): Fit {
  const s = Math.min(1, MAX_SIDE / Math.max(1, w, h), Math.sqrt(MAX_AREA / Math.max(1, w * h)));
  const reduced = s < 1;
  return {
    scale: s,
    w: Math.max(1, Math.floor(w * s + 1e-6)),
    h: Math.max(1, Math.floor(h * s + 1e-6)),
    reduced,
    percent: reduced ? Math.max(1, Math.min(99, Math.round(s * 100))) : 100,
  };
}

/** Pieces of `bounds`, at most `tile` wide and high, to fetch a big chart with several requests. */
export function tileRects(bounds: Rect, tile = TILE): Rect[] {
  const out: Rect[] = [];
  const cols = Math.max(1, Math.ceil(bounds.w / tile));
  const rows = Math.max(1, Math.ceil(bounds.h / tile));
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      const x = bounds.x + c * tile;
      const y = bounds.y + r * tile;
      out.push({ x, y, w: Math.min(tile, bounds.x + bounds.w - x), h: Math.min(tile, bounds.y + bounds.h - y) });
    }
  }
  return out;
}

/** Joins the pieces of a chart fetched in tiles: a node or line that reaches into two tiles is kept once. */
export function mergeScenes(parts: Scene[]): Scene {
  const nodes = new Map<number, ChartNode>();
  const edges = new Map<string, ChartEdge>();
  for (const p of parts) {
    for (const n of p.nodes) if (!nodes.has(n.id)) nodes.set(n.id, n);
    for (const e of p.edges) {
      const key = `${e.from}>${e.to}:${e.kind}`;
      if (!edges.has(key)) edges.set(key, e);
    }
  }
  return { nodes: [...nodes.values()], edges: [...edges.values()] };
}

export interface PicturePlan {
  /** Where the chart is looked at from, in world units. */
  cam: Camera;
  /** Size of the chart area in CSS-like px, before scaling. */
  view: { w: number; h: number };
  /** Pixels per px of `view`. */
  scale: number;
  /** Pixel size of the finished picture, caption strip included. */
  width: number;
  height: number;
  reduced: boolean;
  percent: number;
}

/**
 * How a picture is laid out. Whole and branch pictures show the scene at 100% with a margin and are cropped to it.
 * A screen picture is exactly the viewport at its zoom, at the screen's pixel ratio.
 */
export function planPicture(
  scope: PictureScope,
  scene: Scene,
  screen: { cam: Camera; size: { w: number; h: number }; dpr: number } | null,
  extra: Rect[] = [],
): PicturePlan {
  let cam: Camera;
  let view: { w: number; h: number };
  let base = 1;
  if (scope === 'screen' && screen) {
    cam = { ...screen.cam };
    view = { ...screen.size };
    base = Math.max(1, screen.dpr);
  } else {
    const b = sceneBounds(scene, extra);
    cam = { x: b.x - MARGIN, y: b.y - MARGIN, zoom: 1 };
    view = { w: Math.ceil(b.w + 2 * MARGIN), h: Math.ceil(b.h + 2 * MARGIN) };
  }
  const f = fitScale(view.w * base, (view.h + CAPTION_H) * base);
  return { cam, view, scale: base * f.scale, width: f.w, height: f.h, reduced: f.reduced, percent: f.percent };
}

/** A safe file name for a picture: no characters Windows or macOS refuse, and a .png ending. */
export function pictureFileName(base: string): string {
  // eslint-disable-next-line no-control-regex
  const clean = base.replace(/[\\/:*?"<>|\u0000-\u001f]/g, ' ').replace(/\s+/g, ' ').trim().slice(0, 120).trim();
  return `${clean.replace(/^\.+$/, '') || 'Chart'}.png`;
}
