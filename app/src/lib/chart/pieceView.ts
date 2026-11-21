import type { ChartNode, Rect } from '../api/types';
import { layoutA3, targetScale, type A3Choice, type A3Layout } from './a3';
import type { ChartDirection } from './direction';
import { compactBoth, sceneBounds, type Scene } from './picture';
import { refTagAt, type NodeKey, type Piece } from './sheets';

/**
 * A piece of a packed "sheets by branch" set in each chart direction (spec amendment 21.11.2026, "Compacted on both
 * axes" and "Direction per piece"): the piece is cut once, on the screen direction's layout; here the same boxes (matched
 * by node key) are taken out of each direction's layout, squeezed on both axes, and the direction that suits it is picked.
 */

/** The print chart laid out in one direction, with the key of every box. */
export interface DirScene {
  direction: ChartDirection;
  scene: Scene;
  keys: Map<number, NodeKey>;
}

/** One piece in one direction. */
export interface PieceView {
  direction: ChartDirection;
  /** The piece's boxes (in full, reference boxes, plain repeats) in this direction, compacted on both axes. */
  scene: Scene;
  /** Reference box node id (in `scene`) -> index of the piece holding that person in full. */
  refs: Map<number, number>;
  /** The scene's bounds with every reference tag. */
  bounds: Rect;
  /** The piece alone on an A3 page. */
  standalone: A3Layout;
}

export interface ViewOptions {
  choice: A3Choice;
  header: boolean;
  /** Size of a reference tag's text («→ арк. 99»), world units at the target scale. */
  tagW: number;
  tagH: number;
}

/** The gap (world units) between a reference box and its tag, as the print draws it. */
export const TAG_GAP = 2;

/** The rectangle a reference box's tag takes (see `refTagAt`), `w` x `h` world units. */
export function tagRect(box: { x: number; y: number; w: number; h: number }, dir: ChartDirection, w: number, h: number): Rect {
  const at = refTagAt(box, dir, TAG_GAP);
  const x = at.align === 'center' ? at.x - w / 2 : at.align === 'left' ? at.x : at.x - w;
  const y = at.baseline === 'top' ? at.y : at.baseline === 'bottom' ? at.y - h : at.y - h / 2;
  return { x, y, w, h };
}

const union = (a: Rect, b: Rect): Rect => {
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  return { x, y, w: Math.max(a.x + a.w, b.x + b.w) - x, h: Math.max(a.y + a.h, b.y + b.h) - y };
};

/** Node key -> node id of a direction scene (built once per scene). */
const inverse = new WeakMap<Map<number, NodeKey>, Map<NodeKey, number>>();
function idsByKey(keys: Map<number, NodeKey>): Map<NodeKey, number> {
  let m = inverse.get(keys);
  if (!m) {
    m = new Map([...keys].map(([id, k]) => [k, id]));
    inverse.set(keys, m);
  }
  return m;
}

/** The piece as it prints in each direction of `dirs`; `cut` is the scene its node ids belong to. */
export function viewsOf(piece: Piece, cut: DirScene, dirs: DirScene[], o: ViewOptions): PieceView[] {
  const keyOf = (id: number) => cut.keys.get(id)!;
  const all = [...piece.full, ...piece.refs.keys(), ...piece.plain].map(keyOf);
  const refKeys = new Map([...piece.refs].map(([id, k]) => [keyOf(id), k]));
  return dirs.map((d) => {
    const byKey = idsByKey(d.keys);
    const keep = new Set<number>();
    for (const k of all) {
      const id = byKey.get(k);
      if (id !== undefined) keep.add(id);
    }
    const refs = new Map<number, number>();
    for (const [k, p] of refKeys) {
      const id = byKey.get(k);
      if (id !== undefined) refs.set(id, p);
    }
    const occupied = (n: ChartNode): Rect => (refs.has(n.id) ? union(n, tagRect(n, d.direction, o.tagW, o.tagH)) : n);
    const scene = compactBoth(
      { nodes: d.scene.nodes.filter((n) => keep.has(n.id)), edges: d.scene.edges.filter((e) => keep.has(e.from) && keep.has(e.to)) },
      40,
      occupied,
    );
    const tags = scene.nodes.filter((n) => refs.has(n.id)).map((n) => tagRect(n, d.direction, o.tagW, o.tagH));
    const bounds = sceneBounds(scene, tags);
    return { direction: d.direction, scene, refs, bounds, standalone: layoutA3(bounds, o.choice, o.header) };
  });
}

const EPS = 1e-9;

/** Whether some view prints the piece alone on a page with names at the target size or larger. */
export function fitsTarget(views: PieceView[]): boolean {
  const t = targetScale();
  return views.some((v) => v.standalone.scale >= t - EPS);
}

/**
 * Whether a piece reads at the target size in some direction. Rejects at once, without cutting it out of the four
 * layouts, a piece whose cards alone (in `cut`) take more than the printable A3 area at the target scale.
 */
export function pieceFits(piece: Piece, cut: DirScene, dirs: DirScene[], o: ViewOptions): boolean {
  const byId = cutIndex(cut);
  let cards = 0;
  for (const set of [piece.full, piece.refs.keys(), piece.plain]) {
    for (const id of set) {
      const n = byId.get(id);
      if (n) cards += n.w * n.h;
    }
  }
  const t = targetScale();
  const one = { x: 0, y: 0, w: 1, h: 1 };
  const area = Math.max(
    ...(o.choice === 'auto' ? (['landscape', 'portrait'] as const) : [o.choice]).map((c) => {
      const a = layoutA3(one, c, o.header).area;
      return a.w * a.h;
    }),
  );
  if (cards * t * t > area) return false;
  return fitsTarget(viewsOf(piece, cut, dirs, o));
}

const nodeIndex = new WeakMap<Scene, Map<number, ChartNode>>();
function cutIndex(cut: DirScene): Map<number, ChartNode> {
  let m = nodeIndex.get(cut.scene);
  if (!m) {
    m = new Map(cut.scene.nodes.map((n) => [n.id, n]));
    nodeIndex.set(cut.scene, m);
  }
  return m;
}

const ORDER: ChartDirection[] = ['top', 'bottom', 'left', 'right'];

/**
 * The direction a piece prints in: among the views that reach the target size, the smallest area; when none does, the
 * one that prints largest. Values within 0.1% are a tie: the screen direction stays, then top, bottom, left, right.
 */
export function pickView(views: PieceView[], screenDir: ChartDirection): PieceView {
  if (!views.length) throw new Error('no view of the piece');
  const t = targetScale();
  const fit = views.filter((v) => v.standalone.scale >= t - EPS);
  const pref = (v: PieceView) => (v.direction === screenDir ? -1 : ORDER.indexOf(v.direction));
  const ordered = [...(fit.length ? fit : views)].sort((a, b) => pref(a) - pref(b));
  let best = ordered[0];
  for (const v of ordered.slice(1)) {
    if (fit.length) {
      if (v.bounds.w * v.bounds.h < best.bounds.w * best.bounds.h * (1 - 1e-3)) best = v;
    } else if (v.standalone.scale > best.standalone.scale * (1 + 1e-3)) best = v;
  }
  return best;
}
