import type { ChartEdge, ChartNode, Id, OutlineItem, PersonRef } from '../types';
import type { Layout } from './mockLayout';

// TypeScript port of crates/gt-layout (pedigree.rs, descendants.rs, types.rs) so the mock charts have the
// same shapes as the Rust ones: Ahnentafel columns for pedigrees, a Buchheim tidy tree over couple boxes
// for descendants. Edge `from` is the node nearer the root; partner nodes have an empty Nav.

export const M = { nodeW: 176, nodeH: 66, gapX: 40, gapY: 12, coupleGap: 8 };
export const MAX_NODES = 4000;

export interface RustLayoutSource {
  ref(id: Id): PersonRef;
  /** Sortable birth date, null when unknown (orders siblings on the relatives chart). */
  birth?(id: Id): number | null;
  /** Preferred parents, upper slot first. */
  parents(id: Id): [Id | null, Id | null];
  /** Partnerships in display order: (partner or null, children in birth order). */
  unions(id: Id): { partner: Id | null; children: Id[] }[];
}

export interface N extends ChartNode {
  key: Id;
}

export function newNode(id: number, key: Id, src: RustLayoutSource, gen: number): N {
  return {
    id,
    key,
    person: src.ref(key),
    x: 0,
    y: 0,
    w: M.nodeW,
    h: M.nodeH,
    gen,
    ahnen: null,
    dupOf: null,
    moreUp: false,
    moreDown: false,
    nav: { parent: null, child: null, prev: null, next: null },
    label: '',
  };
}

function linkChildren(nodes: N[], parent: number, kids: number[]): void {
  nodes[parent].nav.child = kids[0] ?? null;
  kids.forEach((k, i) => {
    const n = nodes[k].nav;
    n.parent = parent;
    n.prev = i > 0 ? kids[i - 1] : null;
    n.next = kids[i + 1] ?? null;
  });
}

export function dupEdge(nodes: N[], dup: number, orig: number): ChartEdge {
  const a = nodes[dup];
  const b = nodes[orig];
  const mx = Math.max(a.x + a.w, b.x + b.w) + M.gapX / 2;
  const ay = a.y + a.h / 2;
  const by = b.y + b.h / 2;
  return { from: dup, to: orig, kind: 'dup', pts: [a.x + a.w, ay, mx, ay, mx, by, b.x + b.w, by] };
}

export function bounds(nodes: N[]): { w: number; h: number } {
  let w = 0;
  let h = 0;
  for (const n of nodes) {
    w = Math.max(w, n.x + n.w);
    h = Math.max(h, n.y + n.h);
  }
  return { w, h };
}

function name(p: PersonRef | null): string {
  if (p?.siblingMarker) return 'more brothers and sisters, not named';
  return p ? `${p.name}${p.lifespan ? `, ${p.lifespan}` : ''}` : '';
}

/** "Father", "Mother's father", ... for an Ahnentafel number (three steps at most). */
export function ahnenRelation(a: number): string {
  if (a <= 1) return 'Starting person';
  const bits = a.toString(2).slice(1);
  if (bits.length > 3) return `Ancestor, generation ${bits.length}`;
  const words = [...bits].map((b) => (b === '0' ? 'father' : 'mother'));
  const last = words.pop()!;
  const text = [...words.map((w) => `${w}'s`), last].join(' ');
  return text.charAt(0).toUpperCase() + text.slice(1);
}

export function layoutPedigree(root: Id, generationsIn: number, src: RustLayoutSource): Layout {
  const generations = Math.max(1, generationsIn);
  const nodes: N[] = [newNode(0, root, src, 0)];
  nodes[0].ahnen = 1;
  const kids: number[][] = [[]];
  const visited = new Map<Id, number>([[root, 0]]);
  let truncated = false;
  const queue = [0];
  for (let qi = 0; qi < queue.length; qi++) {
    const id = queue[qi];
    const n = nodes[id];
    const ps = src.parents(n.key);
    if (ps[0] === null && ps[1] === null) continue;
    if (n.gen + 1 >= generations) {
      n.moreUp = true;
      continue;
    }
    for (let slot = 0; slot < 2; slot++) {
      const pk = ps[slot];
      if (pk === null) continue;
      if (nodes.length >= MAX_NODES) {
        truncated = true;
        n.moreUp = true;
        continue;
      }
      const nid = nodes.length;
      const c = newNode(nid, pk, src, n.gen + 1);
      c.ahnen = n.ahnen !== null && n.gen < 50 ? n.ahnen * 2 + slot : null;
      const first = visited.get(pk);
      if (first !== undefined) c.dupOf = first;
      else {
        visited.set(pk, nid);
        queue.push(nid);
      }
      nodes.push(c);
      kids.push([]);
      kids[id].push(nid);
    }
  }

  const stepY = M.nodeH + M.gapY;
  const stepX = M.nodeW + M.gapX;
  const y = new Array<number>(nodes.length).fill(0);
  let row = 0;
  const stack = [0];
  while (stack.length) {
    const v = stack.pop()!;
    const k = kids[v];
    if (k.length === 0) y[v] = row++ * stepY;
    else for (let i = k.length - 1; i >= 0; i--) stack.push(k[i]);
  }
  for (let v = nodes.length - 1; v >= 0; v--) {
    const k = kids[v];
    if (k.length) y[v] = (y[k[0]] + y[k[k.length - 1]]) / 2;
  }
  nodes.forEach((n, i) => {
    n.x = n.gen * stepX;
    n.y = y[i];
  });

  const edges: ChartEdge[] = [];
  for (let v = 0; v < nodes.length; v++) {
    linkChildren(nodes, v, kids[v]);
    const a = nodes[v];
    for (const k of kids[v]) {
      const b = nodes[k];
      const mx = a.x + a.w + M.gapX / 2;
      edges.push({ from: v, to: k, kind: 'parent', pts: [a.x + a.w, a.y + a.h / 2, mx, a.y + a.h / 2, mx, b.y + b.h / 2, b.x, b.y + b.h / 2] });
    }
  }
  nodes.forEach((n, v) => {
    if (n.dupOf !== null) edges.push(dupEdge(nodes, v, n.dupOf));
  });
  for (const n of nodes) {
    n.label = `${n.ahnen ?? ''} ${ahnenRelation(n.ahnen ?? 0)}: ${name(n.person)}`.trim();
  }
  return { nodes: nodes.map(strip), edges, bounds: bounds(nodes), truncated, kids, labels: nodes.map((n) => n.label) };
}

export function strip(n: N): ChartNode {
  const { key, ...rest } = n;
  void key;
  return rest;
}

// ---------------------------------------------------------------- descendants

interface Unit {
  person: number;
  partners: number;
  depth: number;
  parent: number | null;
  attach: number | null;
  children: number[];
}

export function layoutDescendants(root: Id, generationsIn: number, src: RustLayoutSource): Layout {
  const generations = Math.max(1, generationsIn);
  const nodes: N[] = [];
  const visited = new Map<Id, number>();
  const units: Unit[] = [];
  const pending: { at: number | null; children: Id[] }[][] = [];
  let truncated = false;

  const addNode = (key: Id, depth: number): number => {
    const id = nodes.length;
    const n = newNode(id, key, src, depth);
    const first = visited.get(key);
    if (first !== undefined) n.dupOf = first;
    else visited.set(key, id);
    nodes.push(n);
    return id;
  };
  const makeUnit = (key: Id, depth: number, parent: number | null, attach: number | null): number | null => {
    if (nodes.length >= MAX_NODES) {
      truncated = true;
      return null;
    }
    const person = addNode(key, depth);
    let partners = 0;
    const pend: { at: number | null; children: Id[] }[] = [];
    if (nodes[person].dupOf === null) {
      for (const u of src.unions(key)) {
        let at: number | null = null;
        if (u.partner !== null) {
          if (nodes.length >= MAX_NODES) truncated = true;
          else {
            at = addNode(u.partner, depth);
            partners++;
          }
        }
        pend.push({ at, children: u.children });
      }
    }
    pending.push(pend);
    units.push({ person, partners, depth, parent, attach, children: [] });
    return units.length - 1;
  };

  makeUnit(root, 0, null, null);
  const queue = [0];
  const treeKids = new Map<number, number[]>();
  for (let qi = 0; qi < queue.length; qi++) {
    const u = queue[qi];
    const unions = pending[u];
    pending[u] = [];
    const person = units[u].person;
    const depth = units[u].depth;
    if (unions.every((x) => x.children.length === 0)) continue;
    if (depth + 1 >= generations) {
      nodes[person].moreDown = true;
      continue;
    }
    const kidNodes: number[] = [];
    outer: for (const { at, children } of unions) {
      for (const key of children) {
        const cu = makeUnit(key, depth + 1, u, at);
        if (cu === null) {
          nodes[person].moreDown = true;
          break outer;
        }
        units[u].children.push(cu);
        kidNodes.push(units[cu].person);
        queue.push(cu);
      }
    }
    linkChildren(nodes, person, kidNodes);
    treeKids.set(person, kidNodes);
  }

  const widths = units.map((u) => M.nodeW * (1 + u.partners) + M.coupleGap * u.partners);
  const centres = tidy(units, widths, M.gapX);
  const minLeft = centres.reduce((m, c, i) => Math.min(m, c - widths[i] / 2), Infinity);
  const stepY = M.nodeH + M.gapY * 4;
  const stepP = M.nodeW + M.coupleGap;
  units.forEach((u, i) => {
    const left = centres[i] - widths[i] / 2 - minLeft;
    for (let k = 0; k <= u.partners; k++) {
      const n = nodes[u.person + k];
      n.x = left + k * stepP;
      n.y = u.depth * stepY;
    }
  });

  const edges: ChartEdge[] = [];
  for (const u of units) {
    for (let k = 1; k <= u.partners; k++) {
      const to = u.person + k;
      const a = nodes[to - 1];
      const b = nodes[to];
      edges.push({ from: to - 1, to, kind: 'partner', pts: [a.x + a.w, a.y + a.h / 2, b.x, b.y + b.h / 2] });
    }
    for (const c of u.children) {
      const cu = units[c];
      let sx: number;
      let sy: number;
      if (cu.attach !== null) {
        const a = nodes[cu.attach - 1];
        const b = nodes[cu.attach];
        sx = (a.x + a.w + b.x) / 2;
        sy = b.y + b.h / 2;
      } else {
        const a = nodes[u.person];
        sx = a.x + a.w / 2;
        sy = a.y + a.h;
      }
      const t = nodes[cu.person];
      const mid = nodes[u.person].y + nodes[u.person].h + M.gapY * 2;
      edges.push({ from: u.person, to: cu.person, kind: 'parent', pts: [sx, sy, sx, mid, t.x + t.w / 2, mid, t.x + t.w / 2, t.y] });
    }
  }
  nodes.forEach((n, v) => {
    if (n.dupOf !== null) edges.push(dupEdge(nodes, v, n.dupOf));
  });

  const kids = nodes.map((n) => treeKids.get(n.id) ?? []);
  const persons = new Set(units.map((u) => u.person));
  for (const n of nodes) {
    const isPartner = !persons.has(n.id);
    n.label = `${name(n.person)}${n.id === 0 ? ', starting person' : isPartner ? ', partner' : `, generation ${n.gen}`}`;
  }
  return { nodes: nodes.map(strip), edges, bounds: bounds(nodes), truncated, kids, labels: nodes.map((n) => n.label) };
}

// Buchheim, Juenger, Leipert: linear-time tidy tree over variable-width units (units are in BFS order).
function tidy(units: Unit[], w: number[], gap: number): number[] {
  const n = units.length;
  const x = new Array<number>(n).fill(0);
  const md = new Array<number>(n).fill(0);
  const thread = new Array<number | null>(n).fill(null);
  const anc = Array.from({ length: n }, (_, i) => i);
  const change = new Array<number>(n).fill(0);
  const shift = new Array<number>(n).fill(0);
  const number = new Array<number>(n).fill(0);
  for (const u of units) u.children.forEach((c, i) => (number[c] = i));

  const dist = (a: number, b: number) => (w[a] + w[b]) / 2 + gap;
  const nextLeft = (v: number) => units[v].children[0] ?? thread[v];
  const nextRight = (v: number) => units[v].children[units[v].children.length - 1] ?? thread[v];
  const leftSibling = (v: number): number | null => {
    const p = units[v].parent;
    if (p === null || number[v] === 0) return null;
    return units[p].children[number[v] - 1];
  };
  const executeShifts = (v: number) => {
    let s = 0;
    let ch = 0;
    const cs = units[v].children;
    for (let i = cs.length - 1; i >= 0; i--) {
      const c = cs[i];
      x[c] += s;
      md[c] += s;
      ch += change[c];
      s += shift[c] + ch;
    }
  };
  const moveSubtree = (wl: number, wr: number, sh: number) => {
    const subtrees = number[wr] - number[wl];
    change[wr] -= sh / subtrees;
    shift[wr] += sh;
    change[wl] += sh / subtrees;
    x[wr] += sh;
    md[wr] += sh;
  };
  const apportion = (v: number, defaultIn: number): number => {
    let defaultAnc = defaultIn;
    const wv = leftSibling(v);
    const parent = units[v].parent;
    if (wv === null || parent === null) return defaultAnc;
    let vir = v;
    let vor = v;
    let vil = wv;
    let vol = units[parent].children[0];
    let sir = md[v];
    let sor = md[v];
    let sil = md[vil];
    let sol = md[vol];
    for (;;) {
      const nr = nextRight(vil);
      const nl = nextLeft(vir);
      if (nr === null || nr === undefined || nl === null || nl === undefined) break;
      vil = nr;
      vir = nl;
      vol = nextLeft(vol) ?? vol;
      vor = nextRight(vor) ?? vor;
      anc[vor] = v;
      const sh = x[vil] + sil - (x[vir] + sir) + dist(vil, vir);
      if (sh > 0) {
        const a = units[anc[vil]].parent === parent ? anc[vil] : defaultAnc;
        moveSubtree(a, v, sh);
        sir += sh;
        sor += sh;
      }
      sil += md[vil];
      sir += md[vir];
      sol += md[vol];
      sor += md[vor];
    }
    const nr = nextRight(vil);
    if (nr !== null && nr !== undefined && (nextRight(vor) ?? null) === null) {
      thread[vor] = nr;
      md[vor] += sil - sor;
    } else {
      const nl = nextLeft(vir);
      if (nl !== null && nl !== undefined && (nextLeft(vol) ?? null) === null) {
        thread[vol] = nl;
        md[vol] += sir - sol;
      }
      defaultAnc = v;
    }
    return defaultAnc;
  };

  const defaultAnc = units.map((u) => u.children[0] ?? 0);
  const order: number[] = [];
  const stack = [0];
  while (stack.length) {
    const v = stack.pop()!;
    order.push(v);
    stack.push(...units[v].children);
  }
  order.reverse();
  for (const v of order) {
    const left = leftSibling(v);
    const ch = units[v].children;
    if (ch.length) {
      executeShifts(v);
      const mid = (x[ch[0]] + x[ch[ch.length - 1]]) / 2;
      if (left !== null) {
        x[v] = x[left] + dist(v, left);
        md[v] = x[v] - mid;
      } else x[v] = mid;
    } else if (left !== null) x[v] = x[left] + dist(v, left);
    const p = units[v].parent;
    if (p !== null) defaultAnc[p] = apportion(v, defaultAnc[p]);
  }
  const out = [...x];
  const msum = new Array<number>(n).fill(0);
  for (let v = 0; v < n; v++) {
    out[v] = x[v] + msum[v];
    for (const c of units[v].children) msum[c] = msum[v] + md[v];
  }
  return out;
}

export function outlineOf(
  kind: 'pedigree' | 'descendants' | 'relatives',
  layout: Layout,
  parentNode: number | null,
  limit: number,
): OutlineItem[] {
  const ids = parentNode === null ? [0] : (layout.kids[parentNode] ?? []);
  return ids.slice(0, limit).map((id, i) => {
    const n = layout.nodes[id];
    let text: string;
    if (id === 0) text = kind === 'pedigree' ? '1 Starting person' : 'Starting person';
    else if (kind === 'pedigree') text = `${n.ahnen ?? ''} ${ahnenRelation(n.ahnen ?? 0)}`.trim();
    else text = `Child ${i + 1}`;
    return { nodeId: id, person: n.person, label: text, hasChildren: layout.kids[id].length > 0 || n.moreUp || n.moreDown, appearsEarlier: false, alsoAbove: false };
  });
}
