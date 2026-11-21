import { layoutA3 } from './a3';
import type { ChartNode } from '../api/types';
import { compactX, sceneBounds, type Scene } from './picture';
import { descendantsSide, type ChartDirection } from './direction';

/**
 * Cutting a "built around" chart into A3 sheets by branch (spec 2026-11-21 "around print layouts", section B and
 * decisions 2-4). One scene, laid out once, is cut into sub-scenes the way the "branch" picture scope cuts one.
 *
 * Shapes of the scene this relies on (from `gt-layout`): a `parent` edge runs from the parent to the child, and a
 * couple's child gets one edge from each parent, the father's first; `partner` and `sibling` edges join people of one
 * row (a `sibling` bar only where no parent is drawn); a `dup` edge runs from a repeated box to its first occurrence,
 * which the repeated box names in `dupOf`. Generations are negative above the centred people.
 */

/** One printed sheet. */
export interface Sheet {
  /** 1-based; 1 is the cover. */
  no: number;
  /** Lineage node the sheet is cut at (its box is the reference mark at the foot of the sheet); null for the cover. */
  top: number | null;
  /** Nodes shown in full. */
  ids: Set<number>;
  /** Node id -> sheet number, never this sheet's own: drawn as a reference mark ("-> sheet N") on this sheet. */
  refs: Map<number, number>;
  /**
   * The sub-scene: the `ids` and `refs` nodes, repeated boxes (`dupOf`) whose first occurrence is on this sheet (in
   * neither set: drawn as ordinary repeats), and the edges between them, compacted with `compactX`.
   */
  scene: Scene;
}

export interface SplitOptions {
  /** A line sheet with fewer people in full is not made; its people stay on the sheet below. */
  minPeople?: number;
  /** Whether a sub-scene prints readably on one sheet. */
  fits?: (s: Scene) => boolean;
}

/** A line sheet needs at least this many people in full (decision 4). */
export const SHEET_MIN_PEOPLE = 8;

/** True when the scene prints on A3 with names of at least `A3_MIN_PT`. */
export function fitsA3(s: Scene): boolean {
  return !layoutA3(sceneBounds(s), 'auto', true).tooSmall;
}

interface Draft {
  top: number;
  nodes: Set<number>;
  kids: Draft[];
}

/** The shape of a "built around" scene the sheets are cut along (see `hangOf`). */
interface Hang {
  byId: Map<number, ChartNode>;
  isDup: (id: number) => boolean;
  parents: (id: number) => number[];
  childrenOf: Map<number, number[]>;
  /** Centres and everything up their parent edges, depth-first, father's side first. */
  lineage: number[];
  rank: Map<number, number>;
  /** Generation distance of a lineage node from the nearest centre. */
  dist: (id: number) => number;
  /** Lineage node -> the nodes hanging from it (itself included). */
  region: Map<number, number[]>;
  /** Node -> the node it hangs from (null for a centre and a node reached from nowhere). */
  up: Map<number, number | null>;
}

function push<K, V>(m: Map<K, V[]>, k: K, v: V): void {
  const list = m.get(k);
  if (!list) m.set(k, [v]);
  else if (!list.includes(v)) list.push(v);
}

/**
 * The lineage of `scene` around `centres` and which box everything hangs from. Lineage: the centres and everything up
 * their parent edges (a repeated box also brings its first occurrence), ranked depth-first, father's side first. A
 * lineage node hangs from its lineage child with the smaller generation distance (father's side first on a tie). Every
 * other node hangs from the node a multi-source search over parent, partner and sibling links reached it from; the
 * search never steps onto another lineage node, and a lineage person's brothers and sisters are linked to them as well.
 * Null for an empty scene.
 */
function hangOf(scene: Scene, centres: number[]): Hang | null {
  const byId = new Map(scene.nodes.map((n) => [n.id, n]));
  if (!byId.size) return null;
  const isDup = (id: number): boolean => {
    const o = byId.get(id)!.dupOf;
    return o !== null && byId.has(o);
  };

  // Parents and children, in edge order (a couple's father first).
  const parentsOf = new Map<number, number[]>();
  const childrenOf = new Map<number, number[]>();
  for (const e of scene.edges) {
    if (e.kind !== 'parent' || !byId.has(e.from) || !byId.has(e.to)) continue;
    push(parentsOf, e.to, e.from);
    push(childrenOf, e.from, e.to);
  }
  const parents = (id: number) => parentsOf.get(id) ?? [];

  const rank = new Map<number, number>();
  const stack = [...centres].filter((id) => byId.has(id)).reverse();
  while (stack.length) {
    const id = stack.pop()!;
    if (rank.has(id)) continue;
    rank.set(id, rank.size);
    const next = [...parents(id)];
    const o = byId.get(id)!.dupOf;
    if (o !== null && byId.has(o)) next.push(o);
    for (let i = next.length - 1; i >= 0; i--) stack.push(next[i]);
  }
  const lineage = [...rank.keys()];
  // Generation distance of a lineage node: the fewest parent steps from any centre (a repeated box and its first
  // occurrence count as one). Centres need not share a row, so this, not `gen`, decides the cover.
  const depth = new Map<number, number>();
  const firstFrom = new Map<number, number>();
  let level = [...new Set(centres.filter((id) => byId.has(id)))];
  for (let d = 0; level.length; d++) {
    const next: number[] = [];
    for (let i = 0; i < level.length; i++) {
      const id = level[i];
      if (depth.has(id)) continue;
      depth.set(id, d);
      const o = byId.get(id)!.dupOf;
      if (o !== null && byId.has(o) && !depth.has(o)) {
        level.push(o);
        if (!firstFrom.has(o)) firstFrom.set(o, id);
      }
      for (const p of parents(id)) {
        if (depth.has(p)) continue;
        next.push(p);
        if (!firstFrom.has(p)) firstFrom.set(p, id);
      }
    }
    level = next;
  }
  const dist = (id: number) => depth.get(id) ?? 0;

  const adj = new Map<number, number[]>();
  const link = (a: number, b: number) => {
    push(adj, a, b);
    push(adj, b, a);
  };
  for (const e of scene.edges) {
    if (e.kind === 'dup' || !byId.has(e.from) || !byId.has(e.to)) continue;
    link(e.from, e.to);
  }
  for (const l of lineage)
    for (const p of parents(l)) for (const c of childrenOf.get(p) ?? []) if (c !== l && !rank.has(c)) link(l, c);
  // Ties: the source with the smaller generation distance (so a sibling reached both from an ancestor and from that
  // ancestor's parents hangs from the ancestor), then the father's side (the lower depth-first rank).
  const better = (a: number, b: number) => {
    const da = dist(a);
    const db = dist(b);
    return da !== db ? da < db : rank.get(a)! < rank.get(b)!;
  };
  const up = new Map<number, number | null>();
  const centreSet = new Set(centres);
  for (const l of lineage) {
    if (centreSet.has(l)) {
      up.set(l, null);
      continue;
    }
    let best: number | null = null;
    for (const c of childrenOf.get(l) ?? []) {
      if (!rank.has(c)) continue;
      if (best === null || dist(c) < dist(best) || (dist(c) === dist(best) && rank.get(c)! < rank.get(best)!)) best = c;
    }
    up.set(l, best ?? firstFrom.get(l) ?? null);
  }
  const owner = new Map<number, number>(lineage.map((l) => [l, l]));
  let frontier = lineage;
  while (frontier.length) {
    const best = new Map<number, { src: number; via: number }>();
    for (const u of frontier) {
      const src = owner.get(u)!;
      for (const v of adj.get(u) ?? []) {
        if (owner.has(v)) continue;
        const cur = best.get(v);
        if (cur === undefined || better(src, cur.src)) best.set(v, { src, via: u });
      }
    }
    for (const [v, b] of best) {
      owner.set(v, b.src);
      up.set(v, b.via);
    }
    frontier = [...best.keys()];
  }
  for (const n of scene.nodes) if (!up.has(n.id)) up.set(n.id, null);
  const region = new Map<number, number[]>();
  for (const n of scene.nodes) {
    const o = owner.get(n.id);
    if (o !== undefined) push(region, o, n.id);
  }
  return { byId, isDup, parents, childrenOf, lineage, rank, dist, region, up };
}

/**
 * Cuts `scene` into sheets around `centres` (the root and any chosen people, not necessarily in one row). The cover
 * holds each centred person, their parents and grandparents and everything hanging from those, and the centred
 * people's descendants; each grandparent with ancestors gets a sheet of their ancestors and everything hanging from
 * them, split again one generation further up while it does not `fits` (a split sheet left with fewer than
 * `minPeople` people merges into the sheet below when that still fits). Numbered cover first, then depth-first, father's side first.
 */
export function splitSheets(scene: Scene, centres: number[], opts: SplitOptions = {}): Sheet[] {
  const minPeople = opts.minPeople ?? SHEET_MIN_PEOPLE;
  const fits = opts.fits ?? fitsA3;
  const h = hangOf(scene, centres);
  if (!h) return [];
  const { byId, isDup, parents, childrenOf, lineage, dist, region } = h;

  /** The ancestors of `t` (not `t`), along parent edges only, nearest first. */
  const above = (t: number): number[] => {
    const seen = new Set<number>([t]);
    for (const id of seen) for (const p of parents(id)) seen.add(p);
    seen.delete(t);
    return [...seen];
  };

  const sub = (keep: Set<number>): Scene =>
    compactX({
      nodes: scene.nodes.filter((n) => keep.has(n.id)),
      edges: scene.edges.filter((e) => keep.has(e.from) && keep.has(e.to)),
    });

  // The cover.
  const cover = new Set<number>();
  for (const l of lineage) if (dist(l) <= 2) for (const v of region.get(l) ?? []) cover.add(v);
  const down = centres.filter((id) => byId.has(id));
  for (let i = 0; i < down.length; i++) {
    for (const c of childrenOf.get(down[i]) ?? []) {
      if (!cover.has(c)) {
        cover.add(c);
        down.push(c);
      }
    }
  }
  const pool = new Set(scene.nodes.map((n) => n.id).filter((id) => !cover.has(id)));

  const collect = (d: Draft, out = new Set<number>()): Set<number> => {
    for (const v of d.nodes) out.add(v);
    for (const k of d.kids) collect(k, out);
    return out;
  };
  const fullCount = (nodes: Iterable<number>): number => {
    let full = 0;
    for (const v of nodes) if (!isDup(v)) full++;
    return full;
  };
  /**
   * Adds the split-off sheet `k` to `kids`, the sheets cut from the sheet below (`below`, cut at `belowTop`; the cover
   * when null). A sheet that was split further up and kept fewer than `minPeople` people in full merges into the sheet
   * below, which then also holds its split-off sheets, when the merged sheet still fits; otherwise it stays a sheet.
   */
  const place = (below: Set<number>, belowTop: number | null, k: Draft, kids: Draft[]): void => {
    if (k.kids.length && fullCount(k.nodes) < minPeople) {
      const merged = new Set([...below, ...k.nodes]);
      if (belowTop !== null) merged.add(belowTop);
      if (fits(sub(merged))) {
        for (const v of k.nodes) below.add(v);
        kids.push(...k.kids);
        return;
      }
    }
    kids.push(k);
  };
  /** Everyone of `pool` hanging from the ancestors of `t`. */
  const candidate = (t: number, from: Set<number>): Set<number> => {
    const nodes = new Set<number>();
    for (const a of above(t)) for (const v of region.get(a) ?? []) if (from.has(v)) nodes.add(v);
    return nodes;
  };
  /** The sheet cut at `t` from the people of `from`, split further up while it does not fit; null when too small. */
  const build = (t: number, from: Set<number>): Draft | null => {
    const nodes = candidate(t, from);
    if (fullCount(nodes) < minPeople) return null;
    const d: Draft = { top: t, nodes, kids: [] };
    if (!fits(sub(new Set([...nodes, t])))) {
      const cut: Draft[] = [];
      for (const p of parents(t)) {
        if (!nodes.has(p)) continue;
        const k = build(p, nodes);
        if (!k) continue;
        for (const v of collect(k)) nodes.delete(v);
        cut.push(k);
      }
      // Merging is decided once every split-off sheet has left this one.
      for (const k of cut) place(nodes, t, k, d.kids);
    }
    return d;
  };

  // A line sheet for every grandparent of a centre (generation distance 2) with ancestors beyond the cover.
  const cut: Draft[] = [];
  for (const g of lineage) {
    if (dist(g) !== 2 || !parents(g).some((p) => dist(p) > 2)) continue;
    const d = build(g, pool);
    if (d) {
      for (const v of collect(d)) pool.delete(v);
      cut.push(d);
    } else {
      for (const v of candidate(g, pool)) {
        cover.add(v);
        pool.delete(v);
      }
    }
  }
  // Anyone not reached from the lineage stays on the cover.
  for (const v of pool) cover.add(v);
  const lines: Draft[] = [];
  for (const d of cut) place(cover, null, d, lines);

  // Number the sheets: cover, then depth-first, father's side first.
  const parts: { top: number | null; nodes: Set<number> }[] = [{ top: null, nodes: cover }];
  const walk = (d: Draft) => {
    parts.push({ top: d.top, nodes: d.nodes });
    d.kids.forEach(walk);
  };
  lines.forEach(walk);
  const holder = new Map<number, number>();
  parts.forEach((p, i) => {
    for (const v of p.nodes) if (!isDup(v)) holder.set(v, i + 1);
  });

  return parts.map((p, i) => {
    const no = i + 1;
    const ids = new Set<number>();
    const refs = new Map<number, number>();
    // A repeated box whose first occurrence is on this same sheet: drawn as the chart draws it, not as a mark.
    const plain = new Set<number>();
    if (p.top !== null) refs.set(p.top, holder.get(p.top) ?? 1);
    for (const n of scene.nodes) {
      if (!p.nodes.has(n.id)) continue;
      if (!isDup(n.id)) ids.add(n.id);
      else if ((holder.get(n.dupOf!) ?? no) === no) plain.add(n.id);
      else refs.set(n.id, holder.get(n.dupOf!)!);
    }
    return { no, top: p.top, ids, refs, scene: sub(new Set([...ids, ...refs.keys(), ...plain])) };
  });
}

// ------------------------------------------------------------ packed sheets: pieces

/** A block with fewer people in full is never moved to a piece of its own (spec amendment 21.11.2026, "Pieces"). */
export const PIECE_MIN_PEOPLE = 3;

/** One piece of a "built around" chart cut for packed A3 sheets (spec amendment 21.11.2026). */
export interface Piece {
  /** 0 = the cover; the others in the order they were moved out. */
  index: number;
  /** The node the block was cut at (its person names the piece's caption); null for the cover. */
  top: number | null;
  /** Index of the piece it was moved out of; null for the cover. */
  parent: number | null;
  /** Nodes shown in full. */
  full: Set<number>;
  /** Reference box node id -> index of the piece holding that person in full. */
  refs: Map<number, number>;
  /** Repeated boxes (`dupOf`) whose first occurrence is on this piece: drawn as plain repeats. */
  plain: Set<number>;
  /** It did not fit and had no block left to move. */
  unsplittable: boolean;
}

export interface CutOptions {
  /** Whether a piece prints readably on its own. */
  fits: (p: Piece) => boolean;
  /** The smallest block (people in full) that may move out; `PIECE_MIN_PEOPLE` by default. */
  minPeople?: number;
}

/**
 * Cuts `scene` into pieces around `centres` (spec amendment 21.11.2026, "Pieces" and "References"). Every box hangs
 * from one box (`hangOf`); a block is a hang subtree inside a piece, at a node off the lineage or at a lineage node three
 * or more generations above a centre. Starting from the whole chart as the cover, while a piece does not `fits`, its
 * largest block (people in full; ties: the lower rank of the lineage node it hangs from, then the lower node id) of at
 * least `minPeople` and fewer than the piece itself moves out into a new piece, which is treated the same way. A piece
 * with no such block left is `unsplittable`. Wherever a parent, partner or sibling line joins two people in full on
 * different pieces, each piece shows the other person as a reference box; a repeated box is plain where its first
 * occurrence is on the same piece, else a reference box to that piece.
 */
export function cutPieces(scene: Scene, centres: number[], o: CutOptions): Piece[] {
  const minPeople = o.minPeople ?? PIECE_MIN_PEOPLE;
  const h = hangOf(scene, centres);
  if (!h) return [];
  const { byId, isDup, rank, dist, up } = h;
  const ids = scene.nodes.map((n) => n.id);
  const kids = new Map<number, number[]>();
  for (const id of ids) {
    const u = up.get(id);
    if (u !== null && u !== undefined) push(kids, u, id);
  }
  // The lineage node each box hangs from in the end (itself for a lineage node).
  const lineOwner = new Map<number, number>();
  const ownerOf = (id: number): number => {
    const hit = lineOwner.get(id);
    if (hit !== undefined) return hit;
    let at = id;
    const path: number[] = [];
    while (!rank.has(at) && !lineOwner.has(at)) {
      path.push(at);
      const u = up.get(at);
      if (u === null || u === undefined) break;
      at = u;
    }
    const res = lineOwner.get(at) ?? (rank.has(at) ? at : -1);
    for (const p of path) lineOwner.set(p, res);
    lineOwner.set(at, res);
    return res;
  };
  const movableAt = (id: number): boolean => !rank.has(id) || dist(id) >= 3;

  const pieceOf = new Map<number, number>(ids.map((id) => [id, 0]));
  const tops: (number | null)[] = [null];
  const parentsOfPiece: (number | null)[] = [null];
  const unsplittable: boolean[] = [false];

  const edges = scene.edges.filter((e) => (e.kind === 'parent' || e.kind === 'partner' || e.kind === 'sibling') && byId.has(e.from) && byId.has(e.to));
  /** The piece `k` as it stands now: its people, references and plain repeats. */
  const pieceNow = (k: number): Piece => {
    const full = new Set<number>();
    const refs = new Map<number, number>();
    const plain = new Set<number>();
    for (const id of ids) {
      if (pieceOf.get(id) !== k) continue;
      if (!isDup(id)) full.add(id);
      else {
        const holder = pieceOf.get(byId.get(id)!.dupOf!)!;
        if (holder === k) plain.add(id);
        else refs.set(id, holder);
      }
    }
    for (const e of edges) {
      if (isDup(e.from) || isDup(e.to)) continue;
      const a = pieceOf.get(e.from)!;
      const b = pieceOf.get(e.to)!;
      if (a === b) continue;
      if (a === k) refs.set(e.to, b);
      else if (b === k) refs.set(e.from, a);
    }
    return { index: k, top: tops[k], parent: parentsOfPiece[k], full, refs, plain, unsplittable: unsplittable[k] };
  };

  /** The largest movable block of piece `k`, or null. One post-order pass over the piece's hang forest. */
  const largestBlock = (k: number, total: number): number | null => {
    const roots = ids.filter((id) => {
      if (pieceOf.get(id) !== k) return false;
      const u = up.get(id);
      return u === null || u === undefined || pieceOf.get(u) !== k;
    });
    const size = new Map<number, number>();
    let best: number | null = null;
    const better = (a: number, b: number): boolean => {
      const sa = size.get(a)!;
      const sb = size.get(b)!;
      if (sa !== sb) return sa > sb;
      const ra = rank.get(ownerOf(a)) ?? Infinity;
      const rb = rank.get(ownerOf(b)) ?? Infinity;
      if (ra !== rb) return ra < rb;
      return a < b;
    };
    for (const r of roots) {
      // Iterative post-order.
      const stack: [number, boolean][] = [[r, false]];
      while (stack.length) {
        const [id, done] = stack.pop()!;
        if (done) {
          let s = isDup(id) ? 0 : 1;
          for (const c of kids.get(id) ?? []) if (pieceOf.get(c) === k) s += size.get(c)!;
          size.set(id, s);
          if (id !== r && movableAt(id) && s >= minPeople && s < total && (best === null || better(id, best))) best = id;
          continue;
        }
        stack.push([id, true]);
        for (const c of kids.get(id) ?? []) if (pieceOf.get(c) === k) stack.push([c, false]);
      }
    }
    return best;
  };

  const queue = [0];
  for (let qi = 0; qi < queue.length; qi++) {
    const k = queue[qi];
    for (;;) {
      const p = pieceNow(k);
      if (o.fits(p)) break;
      const x = largestBlock(k, p.full.size);
      if (x === null) {
        unsplittable[k] = true;
        break;
      }
      const nk = tops.length;
      tops.push(x);
      parentsOfPiece.push(k);
      unsplittable.push(false);
      const todo = [x];
      while (todo.length) {
        const id = todo.pop()!;
        pieceOf.set(id, nk);
        for (const c of kids.get(id) ?? []) if (pieceOf.get(c) === k) todo.push(c);
      }
      queue.push(nk);
    }
  }
  return tops.map((_, k) => pieceNow(k));
}

/** Names a box across layouts of one chart in different directions (node ids differ between them). */
export type NodeKey = string;

/**
 * The key of every box of `scene`: `p:<person id>` for a person's box in full; for a repeated box (`dupOf` naming a box
 * of the scene) `d:<person id>><key of the box at the other end of its first parent edge, else of its first partner
 * edge>`; `l:<label>:<gen>` for a box without a person. A key that would repeat gets `#2`, `#3`… in scene order.
 */
export function nodeKeys(scene: Scene): Map<number, NodeKey> {
  const byId = new Map(scene.nodes.map((n) => [n.id, n]));
  const firstLink = new Map<number, number>();
  for (const kind of ['parent', 'partner'] as const) {
    for (const e of scene.edges) {
      if (e.kind !== kind || !byId.has(e.from) || !byId.has(e.to)) continue;
      if (!firstLink.has(e.from)) firstLink.set(e.from, e.to);
      if (!firstLink.has(e.to)) firstLink.set(e.to, e.from);
    }
  }
  const raw = new Map<number, string>();
  const visiting = new Set<number>();
  const base = (id: number): string => {
    const hit = raw.get(id);
    if (hit !== undefined) return hit;
    const n = byId.get(id)!;
    let k: string;
    if (!n.person) k = `l:${n.label}:${n.gen}`;
    else if (n.dupOf === null || !byId.has(n.dupOf)) k = `p:${n.person.id}`;
    else {
      const other = firstLink.get(id);
      let at = '';
      if (other !== undefined && !visiting.has(other)) {
        visiting.add(id);
        at = base(other);
        visiting.delete(id);
      }
      k = `d:${n.person.id}>${at}`;
    }
    raw.set(id, k);
    return k;
  };
  const out = new Map<number, NodeKey>();
  const seen = new Map<string, number>();
  for (const n of scene.nodes) {
    const k = base(n.id);
    const c = (seen.get(k) ?? 0) + 1;
    seen.set(k, c);
    out.set(n.id, c === 1 ? k : `${k}#${c}`);
  }
  return out;
}

/** Where the "→ sheet N" tag of a reference box is drawn, and how the text is anchored there. */
export interface RefTag {
  x: number;
  y: number;
  align: 'center' | 'left' | 'right';
  baseline: 'top' | 'bottom' | 'middle';
}

/**
 * The place of a reference box's tag, `gap` away from the box (in the box's units) on the side away from its ancestors
 * (`dir` is the side the ancestors are drawn on): the line to the ancestors leaves the box on their side, so the tag never
 * sits on it, and the box's descendants are on another sheet, so that side is free.
 */
export function refTagAt(box: { x: number; y: number; w: number; h: number }, dir: ChartDirection, gap: number): RefTag {
  const cx = box.x + box.w / 2;
  const cy = box.y + box.h / 2;
  switch (descendantsSide(dir)) {
    case 'bottom':
      return { x: cx, y: box.y + box.h + gap, align: 'center', baseline: 'top' };
    case 'top':
      return { x: cx, y: box.y - gap, align: 'center', baseline: 'bottom' };
    case 'right':
      return { x: box.x + box.w + gap, y: cy, align: 'left', baseline: 'middle' };
    case 'left':
      return { x: box.x - gap, y: cy, align: 'right', baseline: 'middle' };
  }
}

/** How a "built around" chart is laid out for the page: as it always was, folded compactly, or cut into sheets by branch. */
export type PrintLayout = 'today' | 'compact' | 'sheets';

/**
 * The frames of the printed sheets on the chart on screen. `sheets` are cut from the print chart `print`, which has its
 * own node ids and coordinates (compact cards, stretched, maybe turned to the best-fit direction), so each sheet is framed
 * around the screen nodes of the same people in `screen` (matched by person id) and keeps its printed number. Only the
 * line sheets get a frame: the cover (sheet 1) holds the middle of the chart, so its bounds would span the whole chart and
 * cross every other frame; it is what lies outside the frames. One sheet: no frames.
 */
export function screenFrames(print: Scene, sheets: readonly Sheet[], screen: Scene): { no: number; x: number; y: number; w: number; h: number }[] {
  if (sheets.length <= 1) return [];
  const personOf = new Map(print.nodes.map((n) => [n.id, n.person?.id ?? null]));
  const out: { no: number; x: number; y: number; w: number; h: number }[] = [];
  for (const sh of sheets) {
    if (sh.top === null) continue;
    const people = new Set<string>();
    for (const id of sh.ids) {
      const p = personOf.get(id);
      if (p) people.add(p);
    }
    let x0 = Infinity;
    let y0 = Infinity;
    let x1 = -Infinity;
    let y1 = -Infinity;
    for (const n of screen.nodes) {
      if (n.dupOf !== null || !n.person || !people.has(n.person.id)) continue;
      x0 = Math.min(x0, n.x);
      y0 = Math.min(y0, n.y);
      x1 = Math.max(x1, n.x + n.w);
      y1 = Math.max(y1, n.y + n.h);
    }
    if (x0 <= x1) out.push({ no: sh.no, x: x0, y: y0, w: x1 - x0, h: y1 - y0 });
  }
  return out;
}

/**
 * The names of the line sheets, told apart: a surname that two or more sheets share gets the top person's given name
 * («Bondar, Taras»), and a name still shared gets « (2)», « (3)» in sheet order (the first keeps it plain).
 */
export function branchNames(tops: readonly { surname: string; given: string }[]): string[] {
  const count = (list: string[], x: string) => list.filter((y) => y === x).length;
  const surnames = tops.map((t) => t.surname);
  const named = tops.map((t) => (count(surnames, t.surname) > 1 && t.given ? `${t.surname}, ${t.given}` : t.surname));
  const seen = new Map<string, number>();
  return named.map((n) => {
    const k = (seen.get(n) ?? 0) + 1;
    seen.set(n, k);
    return k === 1 ? n : `${n} (${k})`;
  });
}
