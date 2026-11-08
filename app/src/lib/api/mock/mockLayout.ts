import type { ChartEdge, ChartKind, ChartNode, Id, OutlineItem, PersonRef } from '../types';
import { isHorizontal, type ChartDirection } from '../../chart/direction';

// A deliberately simple chart layout for the mock (a generation grid for pedigrees, an indented
// list for descendants). It is not Walker; it only has to give the views real shapes to draw.

export const NODE_W = 176;
export const NODE_H = 66;
export const GAP_X = 40;
export const GAP_Y = 12;
const MAX_NODES = 4000;

export interface LayoutSource {
  ref(id: Id): PersonRef;
  /** Preferred parent set as [father, mother]; either may be null. */
  parents(id: Id): [Id | null, Id | null];
  /** Children across all partnerships, in order, without duplicates. */
  children(id: Id): Id[];
}

export interface Layout {
  nodes: ChartNode[];
  edges: ChartEdge[];
  bounds: { w: number; h: number };
  truncated: boolean;
  /** Tree-children of each node (pedigree: its parents; descendants: its children). */
  kids: number[][];
  labels: string[];
}

/**
 * Mirrors a layout inside its own extent, like the real `Layout::mirror`: `vertical` swaps top and bottom
 * (descendants, relatives), otherwise left and right (the left-to-right pedigree). Ids and nav links stay.
 */
export function mirrorLayout<T extends { nodes: ChartNode[]; edges: ChartEdge[] }>(layout: T, vertical: boolean): T {
  const lo = Math.min(...layout.nodes.map((n) => (vertical ? n.y : n.x)));
  const hi = Math.max(...layout.nodes.map((n) => (vertical ? n.y + n.h : n.x + n.w)));
  for (const n of layout.nodes) {
    if (vertical) n.y = lo + hi - n.y - n.h;
    else n.x = lo + hi - n.x - n.w;
  }
  for (const e of layout.edges) e.pts = e.pts.map((v, i) => (i % 2 === (vertical ? 1 : 0) ? lo + hi - v : v));
  return layout;
}

/**
 * Turns a layout the way the real `Layout::orient` does: a chart is laid out with the pedigree growing right and the
 * others with the oldest generation on top; here the turned axes are stretched to the new spacing (the mock does not
 * lay out again), cards keep their size, then the layout is mirrored to put the ancestors on the asked side.
 */
export function orientLayout<T extends { nodes: ChartNode[]; edges: ChartEdge[]; bounds: { w: number; h: number } }>(layout: T, kind: ChartKind, dir: ChartDirection): T {
  const native = (kind === 'pedigree' ? 'right' : 'top') as ChartDirection;
  if (dir === native) return layout;
  let side: ChartDirection = native;
  if (isHorizontal(native) !== isHorizontal(dir)) {
    // Generation step along x: a card width plus a gap; along y: about a card height plus a gap.
    const toX = isHorizontal(native) ? 1 : (NODE_W + GAP_X) / (NODE_H + GAP_Y * 4);
    const toY = isHorizontal(native) ? (NODE_H + GAP_Y * 4) / (NODE_W + GAP_X) : (NODE_H + GAP_Y * 2) / (NODE_W + GAP_X);
    const fromRows = isHorizontal(native) ? (NODE_H + GAP_Y * 4) / (NODE_H + GAP_Y) : toX;
    for (const n of layout.nodes) {
      const { x, y } = n;
      if (isHorizontal(native)) {
        n.x = y * fromRows;
        n.y = x * toY;
      } else {
        n.x = y * toX;
        n.y = x * toY;
      }
    }
    for (const e of layout.edges) {
      const next = e.pts.slice();
      for (let i = 0; i + 1 < next.length; i += 2) {
        const x = e.pts[i];
        const y = e.pts[i + 1];
        next[i] = isHorizontal(native) ? y * fromRows : y * toX;
        next[i + 1] = x * toY;
      }
      e.pts = next;
    }
    side = native === 'top' ? 'left' : native === 'left' ? 'top' : native === 'bottom' ? 'right' : 'bottom';
    layout.bounds = { w: Math.max(0, ...layout.nodes.map((n) => n.x + n.w)), h: Math.max(0, ...layout.nodes.map((n) => n.y + n.h)) };
  }
  if (side !== dir && layout.nodes.length > 0) mirrorLayout(layout, !isHorizontal(dir));
  return layout;
}

function label(p: PersonRef, extra: string): string {
  const who = p.siblingMarker ? 'more brothers and sisters, not named' : `${p.name}${p.lifespan ? `, ${p.lifespan}` : ''}`;
  return `${who}${extra ? `, ${extra}` : ''}`;
}

function blank(id: number, person: PersonRef, gen: number): ChartNode {
  return {
    id,
    person,
    x: 0,
    y: 0,
    w: NODE_W,
    h: NODE_H,
    gen,
    ahnen: null,
    dupOf: null,
    moreUp: false,
    moreDown: false,
    nav: { parent: null, child: null, prev: null, next: null },
    label: '',
  };
}

function elbow(from: ChartNode, to: ChartNode): number[] {
  const x1 = from.x + from.w;
  const y1 = from.y + from.h / 2;
  const x2 = to.x;
  const y2 = to.y + to.h / 2;
  const mid = x1 + GAP_X / 2;
  return [x1, y1, mid, y1, mid, y2, x2, y2];
}

export function layoutPedigree(root: Id, generations: number, src: LayoutSource): Layout {
  const nodes: ChartNode[] = [];
  const slots: number[] = [];
  const kids: number[][] = [];
  const seen = new Map<Id, number>();
  const toChild: (number | null)[] = [];
  let truncated = false;

  const add = (id: Id, gen: number, slot: number, child: number | null): number => {
    const idx = nodes.length;
    const n = blank(idx, src.ref(id), gen);
    n.ahnen = 2 ** gen + slot;
    const dup = seen.get(id);
    if (dup === undefined) seen.set(id, idx);
    else n.dupOf = dup;
    nodes.push(n);
    slots.push(slot);
    kids.push([]);
    toChild.push(child);
    if (child !== null) kids[child].push(idx);
    return idx;
  };

  const queue: number[] = [add(root, 0, 0, null)];
  for (let qi = 0; qi < queue.length; qi++) {
    const idx = queue[qi];
    const n = nodes[idx];
    if (n.dupOf !== null) continue;
    const parents = src.parents(n.person!.id);
    if (n.gen >= generations) {
      n.moreUp = parents[0] !== null || parents[1] !== null;
      continue;
    }
    for (let j = 0; j < 2; j++) {
      const pid = parents[j];
      if (pid === null) continue;
      if (nodes.length >= MAX_NODES) {
        truncated = true;
        n.moreUp = true;
        break;
      }
      queue.push(add(pid, n.gen + 1, slots[idx] * 2 + j, idx));
    }
  }

  const maxGen = nodes.reduce((m, n) => Math.max(m, n.gen), 0);
  const unit = NODE_H + GAP_Y;
  for (const n of nodes) {
    const slot = slots[n.id];
    const centre = (slot + 0.5) * unit * 2 ** (maxGen - n.gen);
    n.x = n.gen * (NODE_W + GAP_X);
    n.y = centre - NODE_H / 2;
  }

  const edges: ChartEdge[] = [];
  for (const n of nodes) {
    const c = toChild[n.id];
    if (c !== null) edges.push({ from: c, to: n.id, kind: 'parent', pts: elbow(nodes[c], n) });
    if (n.dupOf !== null) {
      const o = nodes[n.dupOf];
      edges.push({
        from: n.id,
        to: o.id,
        kind: 'dup',
        pts: [n.x + n.w / 2, n.y + n.h / 2, o.x + o.w / 2, o.y + o.h / 2],
      });
    }
  }

  // Navigation: parent = the first recorded parent node, child = the node it descends to,
  // prev/next = neighbours in the same generation by vertical order.
  const byGen = new Map<number, ChartNode[]>();
  for (const n of nodes) byGen.set(n.gen, [...(byGen.get(n.gen) ?? []), n]);
  for (const list of byGen.values()) {
    list.sort((a, b) => a.y - b.y);
    list.forEach((n, i) => {
      n.nav.prev = list[i - 1]?.id ?? null;
      n.nav.next = list[i + 1]?.id ?? null;
    });
  }
  for (const n of nodes) {
    n.nav.parent = kids[n.id][0] ?? null;
    n.nav.child = toChild[n.id];
    n.label = label(n.person!, n.ahnen === 1 ? 'starting person' : `ancestor ${n.ahnen}`);
  }

  return {
    nodes,
    edges,
    bounds: { w: (maxGen + 1) * (NODE_W + GAP_X) - GAP_X, h: 2 ** maxGen * unit },
    truncated,
    kids,
    labels: nodes.map((n) => n.label),
  };
}

export function layoutDescendants(root: Id, generations: number, src: LayoutSource): Layout {
  const nodes: ChartNode[] = [];
  const kids: number[][] = [];
  const parentOf: (number | null)[] = [];
  const seen = new Map<Id, number>();
  let truncated = false;
  let row = 0;
  const unit = NODE_H + GAP_Y;

  const visit = (id: Id, gen: number, parent: number | null): void => {
    const idx = nodes.length;
    const n = blank(idx, src.ref(id), gen);
    n.x = gen * (NODE_W + GAP_X);
    n.y = row++ * unit;
    const dup = seen.get(id);
    if (dup === undefined) seen.set(id, idx);
    else n.dupOf = dup;
    nodes.push(n);
    kids.push([]);
    parentOf.push(parent);
    if (parent !== null) kids[parent].push(idx);
    if (n.dupOf !== null) return;
    const children = src.children(id);
    if (gen >= generations) {
      n.moreDown = children.length > 0;
      return;
    }
    for (const c of children) {
      if (nodes.length >= MAX_NODES) {
        truncated = true;
        n.moreDown = true;
        return;
      }
      visit(c, gen + 1, idx);
    }
  };
  visit(root, 0, null);

  const edges: ChartEdge[] = [];
  for (const n of nodes) {
    const p = parentOf[n.id];
    if (p !== null) edges.push({ from: p, to: n.id, kind: 'parent', pts: elbow(nodes[p], n) });
    if (n.dupOf !== null) {
      const o = nodes[n.dupOf];
      edges.push({
        from: n.id,
        to: o.id,
        kind: 'dup',
        pts: [n.x + n.w / 2, n.y + n.h / 2, o.x + o.w / 2, o.y + o.h / 2],
      });
    }
  }
  for (const n of nodes) {
    const p = parentOf[n.id];
    n.nav.parent = p;
    n.nav.child = kids[n.id][0] ?? null;
    if (p !== null) {
      const sibs = kids[p];
      const i = sibs.indexOf(n.id);
      n.nav.prev = sibs[i - 1] ?? null;
      n.nav.next = sibs[i + 1] ?? null;
    }
    n.label = label(n.person!, n.gen === 0 ? 'starting person' : `generation ${n.gen}`);
  }

  const maxGen = nodes.reduce((m, n) => Math.max(m, n.gen), 0);
  return {
    nodes,
    edges,
    bounds: { w: (maxGen + 1) * (NODE_W + GAP_X) - GAP_X, h: Math.max(1, row) * unit - GAP_Y },
    truncated,
    kids,
    labels: nodes.map((n) => n.label),
  };
}

export function outlineOf(
  kind: 'pedigree' | 'descendants',
  layout: Layout,
  parentNode: number | null,
  limit: number,
): OutlineItem[] {
  const ids = parentNode === null ? [0] : (layout.kids[parentNode] ?? []);
  return ids.slice(0, limit).map((id, i) => {
    const n = layout.nodes[id];
    let text: string;
    if (parentNode === null) text = kind === 'pedigree' ? '1 Starting person' : 'Starting person';
    else if (kind === 'pedigree') text = `${n.ahnen} ${(n.ahnen ?? 0) % 2 === 0 ? 'Father' : 'Mother'}`;
    else text = `Child ${i + 1}`;
    return {
      nodeId: id,
      person: n.person,
      label: text,
      hasChildren: layout.kids[id].length > 0 || n.moreUp || n.moreDown,
      appearsEarlier: false,
      alsoAbove: false,
    };
  });
}
