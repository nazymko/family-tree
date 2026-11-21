import type { ChartEdge, ChartNode, Rect } from '../api/types';
import { isHorizontal, type ChartDirection } from './direction';
import type { Scene } from './picture';

/**
 * Re-lays a piece of a "built around" print chart out on its own (packed sheets, round 2): the boxes keep their rows
 * (generations, and the sub-rows of folded groups) and their order in each row, but their places along the rows are
 * worked out again for this piece alone, so the gaps left by the people cut away close up instead of staying as a
 * staircase. Couples (and the rows of a folded group) move as one; the lines keep their shape, each point following the
 * box it belongs to; the bus lines of families sharing a channel never run over each other; and the tag of a reference
 * box never lands on a box of the next row.
 */

/** Room (world units) kept between two families' bus lines in one channel, and around a tag. */
const BUS_GAP = 12;
const TAG_GAP = 4;

interface Block {
  members: number[];
  /** Offset of each member from the block's anchor (its leftmost member), along the row. */
  off: Map<number, number>;
}

/** Swaps x and y of a scene (a left/right chart is laid out as a top one). */
function transpose(s: Scene): Scene {
  return {
    nodes: s.nodes.map((n) => ({ ...n, x: n.y, y: n.x, w: n.h, h: n.w })),
    edges: s.edges.map((e) => ({ ...e, pts: e.pts.map((_, i) => e.pts[i % 2 === 0 ? i + 1 : i - 1]) })),
  };
}

const swapRect = (r: Rect): Rect => ({ x: r.y, y: r.x, w: r.h, h: r.w });

/**
 * A child straight under its parents has a line of two points; it gets the bend of its brothers' and sisters' lines
 * (or one half-way between the parents' row and the child) so that it can bend when the child moves.
 */
function withBus(s: Scene): Scene {
  const bus = new Map<string, number>();
  for (const e of s.edges) if (e.kind === 'parent' && e.pts.length >= 8) bus.set(`${e.pts[0]}|${e.pts[1]}`, e.pts[3]);
  const byId = new Map(s.nodes.map((n) => [n.id, n]));
  return {
    nodes: s.nodes,
    edges: s.edges.map((e) => {
      if (e.kind !== 'parent' || e.pts.length !== 4) return e;
      const [x, y0, , y1] = e.pts;
      const from = byId.get(e.from);
      const b = bus.get(`${x}|${y0}`) ?? (from ? (from.y + from.h + y1) / 2 : (y0 + y1) / 2);
      return { ...e, pts: [x, y0, x, b, x, b, x, y1] };
    }),
  };
}

/** Turns a scene upside down (a bottom/right chart, its parents below their children, is laid out as a top one). */
function mirror(s: Scene): Scene {
  return {
    nodes: s.nodes.map((n) => ({ ...n, y: -(n.y + n.h) })),
    edges: s.edges.map((e) => ({ ...e, pts: e.pts.map((v, i) => (i % 2 === 1 ? -v : v)) })),
  };
}

/**
 * `scene` laid out again along its rows. `tags` gives, for each reference box, its tag's rectangle relative to the box's
 * top-left corner (scene units). `gap`: the most room kept between two neighbours in a row. Null when the constraints
 * cannot be met in order (the caller keeps the squeezed scene).
 */
export function relayPiece(scene: Scene, dir: ChartDirection, tags: Map<number, Rect> = new Map(), gap = 40, fold = false): Scene | null {
  if (scene.nodes.length < 2) return scene;
  const across = isHorizontal(dir);
  const flip = dir === 'bottom' || dir === 'right';
  const t0 = across ? transpose(scene) : scene;
  const t = flip ? mirror(t0) : t0;
  const hOf = new Map(t.nodes.map((n) => [n.id, n.h]));
  const folded = fold ? foldRuns(t, new Set(tags.keys())) : { scene: t, groups: [] };
  const s = withBus(folded.scene);
  const grid = new Set(folded.groups.flat());
  const tagRel = new Map(
    [...tags].map(([id, r]) => {
      const q = across ? swapRect(r) : r;
      return [id, flip ? { ...q, y: (hOf.get(id) ?? 0) - q.y - q.h } : q];
    }),
  );
  const byId = new Map(s.nodes.map((n) => [n.id, n]));

  // Rows: boxes at one height, in order along the row.
  const rowKey = (n: ChartNode) => Math.round(n.y * 2) / 2;
  const rows = new Map<number, ChartNode[]>();
  for (const n of s.nodes) {
    const k = rowKey(n);
    const r = rows.get(k);
    if (r) r.push(n);
    else rows.set(k, [n]);
  }
  for (const r of rows.values()) r.sort((a, b) => a.x - b.x || a.id - b.id);

  // Blocks: a couple side by side moves as one; so do the children of a family whose lines do not run straight (a folded group).
  const parent = new Map<number, number>(s.nodes.map((n) => [n.id, n.id]));
  const find = (x: number): number => {
    let r = x;
    while (parent.get(r) !== r) r = parent.get(r)!;
    let c = x;
    while (parent.get(c) !== r) {
      const nx = parent.get(c)!;
      parent.set(c, r);
      c = nx;
    }
    return r;
  };
  const unite = (a: number, b: number) => parent.set(find(a), find(b));
  const partners = new Set<string>();
  for (const e of s.edges) if (e.kind === 'partner') partners.add(`${Math.min(e.from, e.to)}:${Math.max(e.from, e.to)}`);
  for (const r of rows.values())
    for (let i = 0; i + 1 < r.length; i++) if (partners.has(`${Math.min(r[i].id, r[i + 1].id)}:${Math.max(r[i].id, r[i + 1].id)}`)) unite(r[i].id, r[i + 1].id);
  // A grid the layout folded (a family whose children stand in more than one row) moves whole and stays under its
  // parents: their line may run on down through it.
  const kidsOf = new Map<number, Set<number>>();
  for (const e of s.edges) {
    if (e.kind !== 'parent' || !byId.has(e.from) || !byId.has(e.to) || grid.has(e.to)) continue;
    const k = find(e.from);
    (kidsOf.get(k) ?? kidsOf.set(k, new Set()).get(k)!).add(e.to);
  }
  for (const [k, kids] of kidsOf)
    if (new Set([...kids].map((c) => rowKey(byId.get(c)!))).size > 1) for (const c of kids) unite(k, c);
  for (const g of folded.groups) for (const k of g.slice(1)) unite(g[0], k);

  const blocks = new Map<number, Block>();
  for (const n of s.nodes) {
    const k = find(n.id);
    const b = blocks.get(k);
    if (b) b.members.push(n.id);
    else blocks.set(k, { members: [n.id], off: new Map() });
  }
  for (const b of blocks.values()) {
    const x0 = Math.min(...b.members.map((id) => byId.get(id)!.x));
    for (const id of b.members) b.off.set(id, byId.get(id)!.x - x0);
  }
  const blockOf = (id: number) => find(id);
  const offOf = (id: number) => blocks.get(find(id))!.off.get(id)!;

  // Constraints: P[b] >= P[a] + d.
  const cons: { a: number; b: number; d: number }[] = [];
  /** Box `v` (its left side plus `vAt`) at least `d` after box `u` (its left side plus `uAt`). */
  const after = (u: number, uAt: number, v: number, vAt: number, d: number) => {
    const a = blockOf(u);
    const b = blockOf(v);
    if (a === b) return;
    cons.push({ a, b, d: offOf(u) + uAt + d - offOf(v) - vAt });
  };
  // Neighbours in a row keep their order, at most `gap` apart (closer when they were).
  for (const r of rows.values())
    for (let i = 0; i + 1 < r.length; i++) {
      const u = r[i];
      const v = r[i + 1];
      after(u.id, u.w, v.id, 0, Math.max(0, Math.min(gap, v.x - (u.x + u.w))));
    }
  // Families sharing a bus channel keep their bus lines apart, in the order they had.
  const fams = new Map<string, { nodes: Set<number>; lo: number; hi: number }>();
  for (const e of s.edges) {
    if (e.kind !== 'parent' || e.pts.length < 6 || !byId.has(e.from) || !byId.has(e.to)) continue;
    const key = `${Math.round(e.pts[3] * 2) / 2}|${blockOf(e.from)}`;
    let f = fams.get(key);
    if (!f) {
      f = { nodes: new Set(), lo: Infinity, hi: -Infinity };
      fams.set(key, f);
    }
    for (const id of [e.from, e.to]) {
      f.nodes.add(id);
      const n = byId.get(id)!;
      f.lo = Math.min(f.lo, n.x + n.w / 2);
      f.hi = Math.max(f.hi, n.x + n.w / 2);
    }
  }
  const channels = new Map<string, { nodes: Set<number>; lo: number; hi: number }[]>();
  for (const [key, f] of fams) {
    const ch = key.split('|')[0];
    const list = channels.get(ch);
    if (list) list.push(f);
    else channels.set(ch, [f]);
  }
  for (const list of channels.values()) {
    list.sort((p, q) => p.lo - q.lo);
    for (let i = 0; i + 1 < list.length; i++) {
      const F = list[i];
      const G = list[i + 1];
      if (F.hi >= G.lo) continue;
      // Per block, the last centre of F and the first of G are enough.
      const ends = (fam: Set<number>, other: Set<number>, last: boolean) => {
        const m = new Map<number, number>();
        for (const id of fam) {
          if (other.has(id)) continue;
          const k = blockOf(id);
          const c = offOf(id) + byId.get(id)!.w / 2;
          const was = m.get(k);
          if (was === undefined || (last ? c > offOf(was) + byId.get(was)!.w / 2 : c < offOf(was) + byId.get(was)!.w / 2)) m.set(k, id);
        }
        return [...m.values()];
      };
      for (const a of ends(F.nodes, G.nodes, true)) for (const b of ends(G.nodes, F.nodes, false)) after(a, byId.get(a)!.w / 2, b, byId.get(b)!.w / 2, BUS_GAP);
    }
  }
  // A tag reaching into another row keeps that row's boxes off it, on the side they were.
  const rowList = [...rows.entries()]
    .sort((p, q) => p[0] - q[0])
    .map(([k, r]) => [k, r, Math.min(...r.map((m) => m.y)), Math.max(...r.map((m) => m.y + m.h))] as const);
  for (const [id, rel] of tagRel) {
    const n = byId.get(id);
    if (!n) continue;
    const ty0 = n.y + rel.y;
    const ty1 = ty0 + rel.h;
    const tc = n.x + rel.x + rel.w / 2;
    for (const [k, r, ry0, ry1] of rowList) {
      if (k === rowKey(n)) continue;
      if (ry1 <= ty0 || ry0 >= ty1) continue;
      const before = r.filter((m) => m.x + m.w / 2 < tc);
      const later = r.filter((m) => m.x + m.w / 2 >= tc);
      if (before.length) {
        const m = before[before.length - 1];
        after(m.id, m.w, id, rel.x, TAG_GAP);
      }
      if (later.length) after(id, rel.x + rel.w, later[0].id, 0, TAG_GAP);
    }
  }

  // Order the blocks (the constraints follow the old order, so they form no cycle unless a tag forbids it).
  const ids = [...blocks.keys()];
  const out = new Map<number, { b: number; d: number }[]>();
  const inn = new Map<number, { a: number; d: number }[]>();
  const indeg = new Map<number, number>(ids.map((b) => [b, 0]));
  for (const c of cons) {
    (out.get(c.a) ?? out.set(c.a, []).get(c.a)!).push({ b: c.b, d: c.d });
    (inn.get(c.b) ?? inn.set(c.b, []).get(c.b)!).push({ a: c.a, d: c.d });
    indeg.set(c.b, indeg.get(c.b)! + 1);
  }
  const order: number[] = [];
  const queue = ids.filter((b) => indeg.get(b) === 0).sort((p, q) => byId.get(p)!.x - byId.get(q)!.x);
  for (let i = 0; i < queue.length; i++) {
    const a = queue[i];
    order.push(a);
    for (const { b } of out.get(a) ?? []) {
      indeg.set(b, indeg.get(b)! - 1);
      if (indeg.get(b) === 0) queue.push(b);
    }
  }
  if (order.length !== ids.length) return null;

  const extent = new Map(ids.map((b) => [b, Math.max(...blocks.get(b)!.members.map((m) => blocks.get(b)!.off.get(m)! + byId.get(m)!.w))]));
  const P = new Map<number, number>();
  for (const b of order) P.set(b, Math.max(0, ...(inn.get(b) ?? []).map((c) => P.get(c.a)! + c.d)));
  const W = Math.max(...ids.map((b) => P.get(b)! + extent.get(b)!));
  const R = new Map<number, number>();
  for (let i = order.length - 1; i >= 0; i--) {
    const b = order[i];
    R.set(b, Math.min(W - extent.get(b)!, ...(out.get(b) ?? []).map((c) => R.get(c.b)! - c.d)));
  }

  // Balance: each block moves towards the middle of the boxes it is joined to by a parent line, inside its room.
  const links = new Map<number, { self: number; other: number }[]>();
  for (const e of s.edges) {
    if (e.kind !== 'parent' || !byId.has(e.from) || !byId.has(e.to) || blockOf(e.from) === blockOf(e.to)) continue;
    for (const [u, v] of [
      [e.from, e.to],
      [e.to, e.from],
    ]) {
      const b = blockOf(u);
      (links.get(b) ?? links.set(b, []).get(b)!).push({ self: u, other: v });
    }
  }
  const centre = (id: number) => P.get(blockOf(id))! + offOf(id) + byId.get(id)!.w / 2;
  for (let sweep = 0; sweep < 4; sweep++) {
    for (const b of order) {
      const lower = Math.max(0, ...(inn.get(b) ?? []).map((c) => P.get(c.a)! + c.d));
      const ls = links.get(b);
      let want = P.get(b)!;
      if (ls?.length) want = ls.reduce((acc, l) => acc + centre(l.other) - offOf(l.self) - byId.get(l.self)!.w / 2, 0) / ls.length;
      P.set(b, Math.min(R.get(b)!, Math.max(lower, want)));
    }
  }

  const dx = new Map(s.nodes.map((n) => [n.id, P.get(blockOf(n.id))! + offOf(n.id) - n.x]));
  const nodes = s.nodes.map((n) => ({ ...n, x: n.x + dx.get(n.id)! }));
  const edges: ChartEdge[] = s.edges.map((e) => {
    const df = dx.get(e.from) ?? 0;
    const dt = dx.get(e.to) ?? 0;
    // A parent line's drop (its first two points) follows the parents, the rest the child; a partner line's ends follow their boxes.
    const lead = e.kind === 'parent' ? 2 : 1;
    return { ...e, pts: e.pts.map((v, i) => (i % 2 === 1 ? v : v + (i < 2 * lead ? df : dt))) };
  });
  const res = { nodes, edges };
  const back = flip ? mirror(res) : res;
  return across ? transpose(back) : back;
}

/** The shortest run of leaf boxes that folds, and the gap between two sub-rows of a fold (as the layout's own folds). */
const FOLD_MIN = 6;
const FOLD_GAP = 8;
const FOLD_RATIO = 3;

/** The grid of a run of `n` boxes: the fewest sub-rows (two or more) keeping it at most `FOLD_RATIO` times as many boxes across as deep. */
function foldShape(n: number): { k: number; c: number } {
  let c = 2;
  while (c < Math.ceil(n / 2) && Math.ceil(n / c) > FOLD_RATIO * c) c++;
  return { k: Math.ceil(n / c), c };
}

/**
 * `s` (laid out top-down) with every run of at least `FOLD_MIN` leaf children of one family that stand side by side
 * folded into a grid the way the layout folds them: sub-rows `FOLD_GAP` apart, the lines to the lower sub-rows running
 * down the gap after the grid's first box. The rows below move down to make room. `skip`: boxes that never fold (a
 * reference box, whose tag hangs below it).
 */
export function foldRuns(s: Scene, skip: Set<number> = new Set()): { scene: Scene; groups: number[][] } {
  const byId = new Map(s.nodes.map((n) => [n.id, n]));
  const hasKids = new Set<number>();
  const hasParents = new Set<number>();
  const partners = new Map<number, number[]>();
  for (const e of s.edges) {
    if (e.kind === 'parent') {
      hasKids.add(e.from);
      hasParents.add(e.to);
    } else
      for (const [u, v] of [
        [e.from, e.to],
        [e.to, e.from],
      ])
        (partners.get(u) ?? partners.set(u, []).get(u)!).push(v);
  }
  // Families: the lines that share a parent's drop, all of the standard shape.
  const bent = new Set<string>();
  const famOf = new Map<number, Set<string>>();
  for (const e of s.edges) {
    if (e.kind !== 'parent' || !byId.has(e.to)) continue;
    const key = `${e.pts[0]}|${e.pts[1]}`;
    if (e.pts.length > 8) bent.add(key);
    (famOf.get(e.to) ?? famOf.set(e.to, new Set()).get(e.to)!).add(key);
  }
  const rowKey = (n: ChartNode) => Math.round(n.y * 2) / 2;
  const rows = new Map<number, ChartNode[]>();
  for (const n of s.nodes) (rows.get(rowKey(n)) ?? rows.set(rowKey(n), []).get(rowKey(n))!).push(n);
  const moved = new Map<number, { dx: number; dy: number; gx: number; sub: number }>();
  const extraBelow: { y: number; d: number }[] = [];
  const groups: number[][] = [];
  const free = (n: ChartNode) => !hasKids.has(n.id) && !skip.has(n.id);
  for (const [y, r] of rows) {
    r.sort((a, b) => a.x - b.x);
    let extra = 0;
    let i = 0;
    // Units of a run: a leaf child alone, or with the leaf partner beside it (who has no parents here).
    const unitAt = (i: number, key: string): ChartNode[] | null => {
      const n = r[i];
      const fam = famOf.get(n.id);
      const ps = partners.get(n.id) ?? [];
      if (free(n) && fam?.size === 1 && fam.has(key)) {
        if (ps.length === 0) return [n];
        const m = r[i + 1];
        if (ps.length === 1 && m && m.id === ps[0] && free(m) && !hasParents.has(m.id) && (partners.get(m.id) ?? []).length === 1) return [n, m];
        return null;
      }
      // The partner first, then the child.
      const m = r[i + 1];
      if (ps.length === 1 && m && m.id === ps[0] && free(n) && !hasParents.has(n.id) && free(m) && (partners.get(m.id) ?? []).length === 1) {
        const fm = famOf.get(m.id);
        if (fm?.size === 1 && fm.has(key)) return [n, m];
      }
      return null;
    };
    const keyAt = (i: number): string | null => {
      for (const n of r.slice(i, i + 2)) {
        const fam = famOf.get(n.id);
        if (fam?.size === 1) return [...fam][0];
      }
      return null;
    };
    while (i < r.length) {
      const key = keyAt(i);
      const first = key !== null && !bent.has(key) ? unitAt(i, key) : null;
      if (!first) {
        i++;
        continue;
      }
      const units: ChartNode[][] = [];
      let j = i;
      for (let u = unitAt(j, key!); u; u = j < r.length ? unitAt(j, key!) : null) {
        units.push(u);
        j += u.length;
      }
      i = j;
      const slots = units.reduce((a, u) => a + u.length, 0);
      if (slots < FOLD_MIN || units.length < 3) continue;
      const w = units[0][0].w;
      const h = units[0][0].h;
      const all = units.flat();
      if (all.some((n) => n.w !== w || n.h !== h)) continue;
      const gaps = all.slice(1).map((n, m) => n.x - all[m].x - w);
      const sib = Math.min(...gaps);
      if (sib < 4) continue;
      const { k } = foldShape(slots);
      const x0 = all[0].x;
      // Fill sub-row by sub-row; a couple never splits.
      const place: { u: ChartNode[]; x: number; sub: number }[] = [];
      let sub = 0;
      let x = x0;
      let used = 0;
      for (const u of units) {
        const uw = u[u.length - 1].x + w - u[0].x;
        if (used > 0 && used + u.length > k) {
          sub++;
          x = x0;
          used = 0;
        }
        place.push({ u, x, sub });
        x += uw + sib;
        used += u.length;
      }
      const subs = sub + 1;
      const width = (q: number) => Math.max(...place.filter((p) => p.sub === q).map((p) => p.x + p.u[p.u.length - 1].x + w - p.u[0].x));
      // A lower sub-row reaching past the first would run under the next box's line.
      if (subs < 2 || [...Array(subs).keys()].some((q) => width(q) > width(0) + w / 2 - 4)) continue;
      const gx = x0 - sib / 2;
      for (const p of place)
        for (const n of p.u) moved.set(n.id, { dx: p.x + (n.x - p.u[0].x) - n.x, dy: p.sub * (h + FOLD_GAP), gx, sub: p.sub });
      groups.push(all.map((n) => n.id));
      extra = Math.max(extra, sub * (h + FOLD_GAP));
    }
    if (extra > 0) extraBelow.push({ y: y + Math.max(...r.map((n) => n.h)), d: extra });
  }
  if (moved.size === 0) return { scene: s, groups };
  const down = (v: number) => extraBelow.reduce((a, b) => a + (v > b.y + 0.5 ? b.d : 0), 0);
  const nodes = s.nodes.map((n) => {
    const m = moved.get(n.id);
    return m ? { ...n, x: n.x + m.dx, y: n.y + m.dy } : { ...n, y: n.y + down(n.y) };
  });
  const at = new Map(nodes.map((n) => [n.id, n]));
  const edges: ChartEdge[] = s.edges.map((e) => {
    const m = moved.get(e.to);
    const kid = at.get(e.to)!;
    if (e.kind === 'parent' && m) {
      const cx = kid.x + kid.w / 2;
      const [px, py, , bus] = e.pts;
      if (m.sub === 0) return { ...e, pts: [px, py, px, bus, cx, bus, cx, kid.y] };
      return { ...e, pts: [px, py, px, bus, m.gx, bus, m.gx, kid.y - FOLD_GAP / 2, cx, kid.y - FOLD_GAP / 2, cx, kid.y] };
    }
    if (e.kind === 'partner' && m) return { ...e, pts: e.pts.map((v, i) => v + (i % 2 === 0 ? m.dx : m.dy)) };
    return { ...e, pts: e.pts.map((v, i) => (i % 2 === 1 ? v + down(v) : v)) };
  });
  return { scene: { nodes, edges }, groups };
}
