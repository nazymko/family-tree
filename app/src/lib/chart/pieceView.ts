import type { ChartNode, Rect } from '../api/types';
import { layoutA3, targetScale, type A3Choice, type A3Layout } from './a3';
import { isHorizontal, opposite, type ChartDirection } from './direction';
import { compactBoth, sceneBounds, type Scene } from './picture';
import { relayPiece } from './relay';
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
  /**
   * Reference boxes whose tag goes on the ancestors' side: the parents of someone on this piece (their line to the child
   * leaves the descendants' side, where the tag would sit on it). See `tagDir`.
   */
  flip: Set<number>;
  /**
   * Children of one family here who are on other pieces share one reference box: that box's node id -> the pieces
   * holding them (sorted). The other children's boxes are left out of `scene` and `refs`.
   */
  groups: Map<number, number[]>;
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

/** How much wider a reference tag gets for each page more it lists (a tag of `tagW` holds one page number). */
export const GROUP_TAG_MORE = 0.45;

/** The re-lays tried for a piece: [shortest run folded (0: none), grid ratio]; the smallest wins. */
const FOLDS: [number, number][] = [
  [0, 3],
  [6, 3],
  [3, 3],
];

/** The rectangle a reference box's tag takes (see `refTagAt`), `w` x `h` world units. */
export function tagRect(box: { x: number; y: number; w: number; h: number }, dir: ChartDirection, w: number, h: number): Rect {
  const at = refTagAt(box, dir, TAG_GAP);
  const x = at.align === 'center' ? at.x - w / 2 : at.align === 'left' ? at.x : at.x - w;
  const y = at.baseline === 'top' ? at.y : at.baseline === 'bottom' ? at.y - h : at.y - h / 2;
  return { x, y, w, h };
}

/** The direction to give `refTagAt` / `tagRect` for the tag of reference box `id` of a view. */
export function tagDir(v: Pick<PieceView, 'direction' | 'flip'>, id: number): ChartDirection {
  return v.flip.has(id) ? opposite(v.direction) : v.direction;
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
    // Children of one family (their parents shown here) who are on other pieces: one box stands for them all.
    const groups = new Map<number, number[]>();
    const parentsOf = new Map<number, number[]>();
    for (const e of d.scene.edges)
      if (e.kind === 'parent' && refs.has(e.to) && keep.has(e.from) && !refs.has(e.from)) parentsOf.set(e.to, [...(parentsOf.get(e.to) ?? []), e.from]);
    const fams = new Map<string, number[]>();
    for (const [c, ps] of parentsOf) {
      const k = [...ps].sort((a, b) => a - b).join(',');
      fams.set(k, [...(fams.get(k) ?? []), c]);
    }
    const along = isHorizontal(d.direction) ? (n: ChartNode) => n.y : (n: ChartNode) => n.x;
    const nodeOf = new Map(d.scene.nodes.map((n) => [n.id, n]));
    for (const kids of fams.values()) {
      if (kids.length < 2) continue;
      const rep = kids.reduce((a, b) => (along(nodeOf.get(b)!) < along(nodeOf.get(a)!) ? b : a));
      groups.set(rep, [...new Set(kids.map((c) => refs.get(c)!))].sort((a, b) => a - b));
      for (const c of kids)
        if (c !== rep) {
          keep.delete(c);
          refs.delete(c);
        }
    }
    // A reference box that is a parent of someone here has its line leave on the descendants' side: its tag goes on the
    // ancestors' side, which is free (its own parents are not on this piece).
    const flip = new Set<number>();
    for (const e of d.scene.edges) if (e.kind === 'parent' && refs.has(e.from) && keep.has(e.to)) flip.add(e.from);
    const dirOf = (id: number) => (flip.has(id) ? opposite(d.direction) : d.direction);
    // A group's tag lists several pages: wider.
    const tagWOf = (id: number) => o.tagW * (1 + GROUP_TAG_MORE * ((groups.get(id)?.length ?? 1) - 1));
    const occupied = (n: ChartNode): Rect => (refs.has(n.id) ? union(n, tagRect(n, dirOf(n.id), tagWOf(n.id), o.tagH)) : n);
    const kept = { nodes: d.scene.nodes.filter((n) => keep.has(n.id)), edges: d.scene.edges.filter((e) => keep.has(e.from) && keep.has(e.to)) };
    const boundsOf = (sc: Scene) => sceneBounds(sc, sc.nodes.filter((n) => refs.has(n.id)).map((n) => tagRect(n, dirOf(n.id), tagWOf(n.id), o.tagH)));
    // Laid out again for this piece alone; the plain squeeze stays when it comes out smaller.
    const rel = new Map<number, Rect>();
    for (const n of kept.nodes) {
      if (!refs.has(n.id)) continue;
      const r = tagRect(n, dirOf(n.id), tagWOf(n.id), o.tagH);
      rel.set(n.id, { ...r, x: r.x - n.x, y: r.y - n.y });
    }
    const squeezed = compactBoth(kept, 40, occupied);
    let scene = squeezed;
    let bounds = boundsOf(squeezed);
    // Unfolded, the layout's own folds (runs of six), and shorter runs too (a piece of many small families): see FOLDS.
    for (const [fold, ratio] of FOLDS) {
      const relaid = relayPiece(kept, d.direction, rel, 16, fold, ratio);
      if (!relaid) continue;
      const dense = compactBoth(relaid, 40, occupied);
      const b = boundsOf(dense);
      if (b.w * b.h <= bounds.w * bounds.h) {
        scene = dense;
        bounds = b;
      }
    }
    return { direction: d.direction, scene, refs, flip, groups, bounds, standalone: layoutA3(bounds, o.choice, o.header) };
  });
}

const EPS = 1e-9;

/** Whether some view prints the piece alone on a page with names at the target size or larger. */
export function fitsTarget(views: PieceView[]): boolean {
  const t = targetScale();
  return views.some((v) => v.standalone.scale >= t - EPS);
}

/**
 * Whether a piece's cards alone (in `cut`) fit the printable A3 area at the target scale: a quick test that rejects a
 * piece far too big without cutting it out of the four layouts.
 */
export function cardsFit(piece: Piece, cut: DirScene, o: Pick<ViewOptions, 'choice' | 'header'>): boolean {
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
  return cards * t * t <= area;
}

/** Whether a piece reads at the target size in some direction (`cardsFit` first, then the four views). */
export function pieceFits(piece: Piece, cut: DirScene, dirs: DirScene[], o: ViewOptions): boolean {
  return cardsFit(piece, cut, o) && fitsTarget(viewsOf(piece, cut, dirs, o));
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
 * `fits` replaces the test "reaches the target size alone on a page" (the planner also counts the caption band).
 */
export function pickView(views: PieceView[], screenDir: ChartDirection, fits?: (v: PieceView) => boolean): PieceView {
  if (!views.length) throw new Error('no view of the piece');
  const t = targetScale();
  const fit = views.filter(fits ?? ((v) => v.standalone.scale >= t - EPS));
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
