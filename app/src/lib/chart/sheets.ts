import { layoutA3 } from './a3';
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
  const byId = new Map(scene.nodes.map((n) => [n.id, n]));
  if (!byId.size) return [];
  const isDup = (id: number): boolean => {
    const o = byId.get(id)!.dupOf;
    return o !== null && byId.has(o);
  };

  // Parents and children, in edge order (a couple's father first).
  const parentsOf = new Map<number, number[]>();
  const childrenOf = new Map<number, number[]>();
  const push = (m: Map<number, number[]>, k: number, v: number) => {
    const list = m.get(k);
    if (!list) m.set(k, [v]);
    else if (!list.includes(v)) list.push(v);
  };
  for (const e of scene.edges) {
    if (e.kind !== 'parent' || !byId.has(e.from) || !byId.has(e.to)) continue;
    push(parentsOf, e.to, e.from);
    push(childrenOf, e.from, e.to);
  }
  const parents = (id: number) => parentsOf.get(id) ?? [];

  // Lineage: the centres and everything up their parent edges (a repeated box also brings its first occurrence),
  // ranked in depth-first order, father's side first.
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
  let level = [...new Set(centres.filter((id) => byId.has(id)))];
  for (let d = 0; level.length; d++) {
    const next: number[] = [];
    for (let i = 0; i < level.length; i++) {
      const id = level[i];
      if (depth.has(id)) continue;
      depth.set(id, d);
      const o = byId.get(id)!.dupOf;
      if (o !== null && byId.has(o) && !depth.has(o)) level.push(o);
      for (const p of parents(id)) if (!depth.has(p)) next.push(p);
    }
    level = next;
  }
  const dist = (id: number) => depth.get(id) ?? 0;

  // Which lineage node everyone hangs from: a multi-source search over parent, partner and sibling links that never
  // steps onto another lineage node. A lineage person's brothers and sisters are linked to them as well (with their
  // parents drawn the scene joins them only through the parents), so they hang from that person, not from the
  // parents. Ties: the source with the smaller generation distance (`dist`: parent steps from the nearest centre, so
  // a sibling reached both from an ancestor and from that ancestor's parents hangs from the ancestor), then the father's
  // side (the lower depth-first rank).
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
  const better = (a: number, b: number) => {
    const da = dist(a);
    const db = dist(b);
    return da !== db ? da < db : rank.get(a)! < rank.get(b)!;
  };
  const owner = new Map<number, number>(lineage.map((l) => [l, l]));
  let frontier = lineage;
  while (frontier.length) {
    const best = new Map<number, number>();
    for (const u of frontier) {
      const src = owner.get(u)!;
      for (const v of adj.get(u) ?? []) {
        if (owner.has(v)) continue;
        const cur = best.get(v);
        if (cur === undefined || better(src, cur)) best.set(v, src);
      }
    }
    for (const [v, s] of best) owner.set(v, s);
    frontier = [...best.keys()];
  }
  const region = new Map<number, number[]>();
  for (const n of scene.nodes) {
    const o = owner.get(n.id);
    if (o !== undefined) push(region, o, n.id);
  }

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
 * Where each sheet of `sheets` falls on the screen scene `scene` it was cut from: the bounds of the nodes the sheet
 * shows in full (reference marks and repeats belong to other sheets' areas), in the scene's own coordinates. The
 * sheets' own scenes are compacted, so their coordinates are not these.
 */
export function sheetBounds(scene: Scene, sheets: readonly Sheet[]): { no: number; x: number; y: number; w: number; h: number }[] {
  const out: { no: number; x: number; y: number; w: number; h: number }[] = [];
  for (const sh of sheets) {
    let x0 = Infinity;
    let y0 = Infinity;
    let x1 = -Infinity;
    let y1 = -Infinity;
    for (const n of scene.nodes) {
      if (!sh.ids.has(n.id)) continue;
      x0 = Math.min(x0, n.x);
      y0 = Math.min(y0, n.y);
      x1 = Math.max(x1, n.x + n.w);
      y1 = Math.max(y1, n.y + n.h);
    }
    if (x0 <= x1) out.push({ no: sh.no, x: x0, y: y0, w: x1 - x0, h: y1 - y0 });
  }
  return out;
}
