import type { ChartNode } from '../api/types';
import type { Scene } from './picture';
import { descendantsSide, type ChartDirection } from './direction';
import type { SheetPlan } from './sheetPlan';

/**
 * Cutting a "built around" chart into pieces for the packed A3 sheets (spec 2026-11-21 "around print layouts", amendment
 * 21.11.2026 "packed sheets"). One scene, laid out in the screen's direction, is cut; `pieceView.ts` takes each piece out
 * of the four direction layouts and `pack.ts` packs them onto pages.
 *
 * Shapes of the scene this relies on (from `gt-layout`): a `parent` edge runs from the parent to the child, and a
 * couple's child gets one edge from each parent, the father's first; `partner` and `sibling` edges join people of one
 * row (a `sibling` bar only where no parent is drawn); a `dup` edge runs from a repeated box to its first occurrence,
 * which the repeated box names in `dupOf`. Generations are negative above the centred people.
 */

/** The shape of a "built around" scene the sheets are cut along (see `hangOf`). */
interface Hang {
  byId: Map<number, ChartNode>;
  isDup: (id: number) => boolean;
  /** Lineage node -> its place in the lineage: the centres and everything up their parent edges, depth-first, father's side first. */
  rank: Map<number, number>;
  /** Generation distance of a lineage node from the nearest centre. */
  dist: (id: number) => number;
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
  return { byId, isDup, rank, dist, up };
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
  /**
   * A piece of childless brothers and sisters (with their partners) moved out together: the brother or sister they
   * belong with who stayed behind (the line's person where there is one); null for any other piece.
   */
  siblingsOf: number | null;
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

  // Childless brothers and sisters: each with the partners beside them who have no family of their own here, by family.
  const parentsOf = new Map<number, number[]>();
  const partnersOf = new Map<number, number[]>();
  const hasKids = new Set<number>();
  for (const e of scene.edges) {
    if (!byId.has(e.from) || !byId.has(e.to)) continue;
    if (e.kind === 'parent') {
      push(parentsOf, e.to, e.from);
      hasKids.add(e.from);
    } else if (e.kind === 'partner') {
      push(partnersOf, e.from, e.to);
      push(partnersOf, e.to, e.from);
    }
  }
  const families = new Map<string, number[]>();
  for (const [c, ps] of parentsOf) if (!isDup(c)) push(families, [...ps].sort((a, b) => a - b).join(','), c);
  const loneSpouse = (sp: number, of: number) =>
    !isDup(sp) && !hasKids.has(sp) && !parentsOf.has(sp) && (partnersOf.get(sp) ?? []).every((x) => x === of);
  /** A childless child and their partners, or null when they have a family here. */
  const leafUnit = (c: number): number[] | null => {
    if (hasKids.has(c)) return null;
    const ps = partnersOf.get(c) ?? [];
    return ps.every((sp) => loneSpouse(sp, c)) ? [c, ...ps] : null;
  };
  const familyList = [...families.values()];

  const pieceOf = new Map<number, number>(ids.map((id) => [id, 0]));
  const tops: (number | null)[] = [null];
  const parentsOfPiece: (number | null)[] = [null];
  const unsplittable: boolean[] = [false];
  const siblingsOfPiece: (number | null)[] = [null];

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
    return { index: k, top: tops[k], parent: parentsOfPiece[k], full, refs, plain, unsplittable: unsplittable[k], siblingsOf: siblingsOfPiece[k] };
  };

  interface Block {
    top: number;
    /** A group of childless brothers and sisters: exactly these move (and who they stay with); else the hang subtree at `top`. */
    members: number[] | null;
    siblingsOf: number | null;
  }
  /**
   * The largest movable block of piece `k`, or null: a hang subtree (one post-order pass over the piece's hang forest),
   * or the childless brothers and sisters of one family with their partners.
   */
  const largestBlock = (k: number, total: number): Block | null => {
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
    let group: Block | null = null;
    let groupSize = 0;
    for (const children of familyList) {
      const members: number[] = [];
      for (const c of children) {
        const u = leafUnit(c);
        if (u && u.every((id) => pieceOf.get(id) === k && movableAt(id))) members.push(...u);
      }
      const n = members.length;
      // The piece left behind keeps a few people too (a couple and their many children stay one piece).
      if (n < minPeople || n > total - minPeople) continue;
      const left = children.filter((c) => !members.includes(c));
      const stay = [...left].sort((a, b) => (rank.get(a) ?? Infinity) - (rank.get(b) ?? Infinity) || a - b)[0] ?? null;
      const top = Math.min(...members.filter((id) => children.includes(id)));
      const beats =
        group === null ||
        n > groupSize ||
        (n === groupSize && ((rank.get(ownerOf(top)) ?? Infinity) < (rank.get(ownerOf(group.top)) ?? Infinity) || top < group.top));
      if (beats) {
        group = { top, members, siblingsOf: stay };
        groupSize = n;
      }
    }
    if (group && (best === null || groupSize > size.get(best)!)) return group;
    return best === null ? null : { top: best, members: null, siblingsOf: null };
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
      tops.push(x.top);
      parentsOfPiece.push(k);
      unsplittable.push(false);
      siblingsOfPiece.push(x.siblingsOf);
      if (x.members) {
        for (const id of x.members) pieceOf.set(id, nk);
        queue.push(nk);
        continue;
      }
      const todo = [x.top];
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
 * The frames of a packed set on the chart on screen: one per piece (the cover included), around the screen boxes of the
 * people the piece holds in full, labelled with the page it prints on. The plan is cut from print charts with their own
 * node ids and coordinates, so people are matched by node key: `keysOf` gives the key of a piece's box in its direction's
 * print chart. Frames may overlap. A set of one page has no frames.
 */
export function screenFrames(
  plan: SheetPlan,
  keysOf: (pieceNodeId: number, dir: ChartDirection) => NodeKey | undefined,
  screen: Scene,
): { no: number; x: number; y: number; w: number; h: number }[] {
  if (plan.pages.length <= 1) return [];
  const screenKeys = nodeKeys(screen);
  const out: { no: number; x: number; y: number; w: number; h: number }[] = [];
  for (const pg of plan.pages) {
    for (const pc of pg.pieces) {
      const full = new Set<NodeKey>();
      for (const n of pc.view.scene.nodes) {
        if (n.dupOf !== null || pc.view.refs.has(n.id)) continue;
        const k = keysOf(n.id, pc.view.direction);
        if (k !== undefined) full.add(k);
      }
      let x0 = Infinity;
      let y0 = Infinity;
      let x1 = -Infinity;
      let y1 = -Infinity;
      for (const n of screen.nodes) {
        if (n.dupOf !== null || !full.has(screenKeys.get(n.id)!)) continue;
        x0 = Math.min(x0, n.x);
        y0 = Math.min(y0, n.y);
        x1 = Math.max(x1, n.x + n.w);
        y1 = Math.max(y1, n.y + n.h);
      }
      if (x0 <= x1) out.push({ no: pg.no, x: x0, y: y0, w: x1 - x0, h: y1 - y0 });
    }
  }
  return out;
}
