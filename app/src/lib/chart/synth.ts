import type { ChartEdge, ChartMeta, ChartNode, ChartWindow, Rect } from '../api/types';

const NODE_W = 176;
const NODE_H = 56;
const GAP_X = 40;
const GAP_Y = 12;
const CELL = 512;

function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const GIVEN = ['Anna', 'Piotr', 'Maria', 'Jan', 'Elena', 'Ivan', 'Sofia', 'Karl', 'Olga', 'Henryk', 'Greta', 'Tomas'];
const SURNAME = ['Kowalski', 'Novak', 'Müller', 'Ivanova', 'Schmidt', 'Horvat', 'Lindqvist', 'Moreau', 'Rossi', 'Dvořák'];

const cellOf = (v: number): number => Math.floor(v / CELL);

/** Balanced binary tree laid out like a pedigree (Ahnentafel heap order); no Api involved. */
export function synthChart(
  n: number,
  seed = 1,
): { meta: ChartMeta; all: ChartNode[]; edges: ChartEdge[]; query(r: Rect): ChartWindow } {
  const rnd = mulberry32(seed);
  const count = Math.max(1, Math.floor(n));
  const stepY = NODE_H + GAP_Y;
  const stepX = NODE_W + GAP_X;
  const ys = new Float64Array(count + 1);
  const gens = new Int16Array(count + 1);
  let row = 0;
  // Leaves take consecutive rows left to right (iterative DFS), parents sit midway.
  const stack = [1];
  const post: number[] = [];
  while (stack.length) {
    const i = stack.pop()!;
    post.push(i);
    if (2 * i + 1 <= count) stack.push(2 * i + 1);
    if (2 * i <= count) stack.push(2 * i);
  }
  for (const i of post) {
    gens[i] = i === 1 ? 0 : gens[i >> 1] + 1;
    if (2 * i > count) ys[i] = row++ * stepY;
  }
  for (let k = post.length - 1; k >= 0; k--) {
    const i = post[k];
    if (2 * i <= count) ys[i] = (ys[2 * i] + ys[Math.min(2 * i + 1, count)]) / 2;
  }

  const all: ChartNode[] = [];
  for (let i = 1; i <= count; i++) {
    const name = `${GIVEN[Math.floor(rnd() * GIVEN.length)]} ${SURNAME[Math.floor(rnd() * SURNAME.length)]}`;
    const birth = 1990 - gens[i] * 28 + Math.floor(rnd() * 6);
    all.push({
      id: i - 1,
      person: {
        id: `p${i}`,
        name,
        lifespan: `${birth}-${birth + 55 + Math.floor(rnd() * 30)}`,
        sex: i % 2 === 0 ? 'male' : 'female',
        living: false,
        private: false,
        siblingMarker: false,
      },
      x: gens[i] * stepX,
      y: ys[i],
      w: NODE_W,
      h: NODE_H,
      gen: gens[i],
      ahnen: i,
      dupOf: null,
      moreUp: false,
      moreDown: false,
      nav: {
        parent: i === 1 ? null : (i >> 1) - 1,
        child: 2 * i <= count ? 2 * i - 1 : null,
        prev: i > 1 && i % 2 === 1 ? i - 2 : null,
        next: i > 1 && i % 2 === 0 && i + 1 <= count ? i : null,
      },
      label: `${name}, ancestor ${i}`,
    });
  }
  const edges: ChartEdge[] = [];
  for (let i = 2; i <= count; i++) {
    const a = all[(i >> 1) - 1];
    const b = all[i - 1];
    const mx = a.x + a.w + GAP_X / 2;
    edges.push({
      from: a.id,
      to: b.id,
      kind: 'parent',
      pts: [a.x + a.w, a.y + a.h / 2, mx, a.y + a.h / 2, mx, b.y + b.h / 2, b.x, b.y + b.h / 2],
    });
  }

  // 512-unit grid index.
  const nodeCells = new Map<string, number[]>();
  const edgeCells = new Map<string, number[]>();
  const put = (m: Map<string, number[]>, r: Rect, id: number) => {
    for (let cx = cellOf(r.x); cx <= cellOf(r.x + r.w); cx++)
      for (let cy = cellOf(r.y); cy <= cellOf(r.y + r.h); cy++) {
        const k = `${cx},${cy}`;
        const l = m.get(k);
        if (l) l.push(id);
        else m.set(k, [id]);
      }
  };
  all.forEach((nd) => put(nodeCells, nd, nd.id));
  edges.forEach((e, id) => {
    const xs = e.pts.filter((_, k) => k % 2 === 0);
    const yv = e.pts.filter((_, k) => k % 2 === 1);
    const x0 = Math.min(...xs);
    const y0 = Math.min(...yv);
    put(edgeCells, { x: x0, y: y0, w: Math.max(...xs) - x0, h: Math.max(...yv) - y0 }, id);
  });
  const collect = (m: Map<string, number[]>, r: Rect): number[] => {
    const out = new Set<number>();
    for (let cx = cellOf(r.x); cx <= cellOf(r.x + r.w); cx++)
      for (let cy = cellOf(r.y); cy <= cellOf(r.y + r.h); cy++) for (const id of m.get(`${cx},${cy}`) ?? []) out.add(id);
    return [...out].sort((a, b) => a - b);
  };

  const maxGen = gens.reduce((m, g) => Math.max(m, g), 0);
  const meta: ChartMeta = {
    chartId: 1,
    kind: 'pedigree',
    bounds: { w: (maxGen + 1) * stepX - GAP_X, h: Math.max(1, row) * stepY - GAP_Y },
    nodeCount: count,
    rootNode: 0,
    truncated: false,
    version: 0,
  };
  return {
    meta,
    all,
    edges,
    query(r: Rect): ChartWindow {
      const nodes = collect(nodeCells, r)
        .map((id) => all[id])
        .filter((nd) => nd.x < r.x + r.w && nd.x + nd.w > r.x && nd.y < r.y + r.h && nd.y + nd.h > r.y);
      const es = collect(edgeCells, r).map((id) => edges[id]);
      return { nodes, edges: es };
    },
  };
}
