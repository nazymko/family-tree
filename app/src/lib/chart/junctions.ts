import type { ChartEdge } from '../api/types';

/**
 * Where lines of the connectors meet and where they only cross, worked out from the geometry alone (the lines are
 * axis-aligned), so the picture export, which draws through the same renderer, shows the same.
 *
 * - A dot goes where a line branches: three or more directions leave the point (a bus meets a drop, a children's
 *   line leaves the partner connector).
 * - A bridge is a small gap in a horizontal line where a vertical line of another family crosses it without joining.
 */

interface Seg {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  partner: boolean;
}

export interface Marks {
  /** Flat `x, y` of every branch. */
  dots: number[];
  /** Horizontal parent lines that are crossed: `segKey` of the line -> x of each crossing, ascending. */
  crossings: Map<string, number[]>;
}

const EPS = 0.01;
const UP = 1;
const DOWN = 2;
const LEFT = 4;
const RIGHT = 8;

/** Key of a horizontal piece of a line, ends in any order. */
export function segKey(xa: number, xb: number, y: number): string {
  return `${Math.min(xa, xb)},${Math.max(xa, xb)},${y}`;
}

const q = (v: number) => Math.round(v * 100);

function bits(m: number): number {
  let n = 0;
  for (let b = m; b; b &= b - 1) n++;
  return n;
}

export function connectorMarks(edges: ChartEdge[]): Marks {
  const hs: Seg[] = [];
  const vs: Seg[] = [];
  const seen = new Set<string>();
  for (const e of edges) {
    const partner = e.kind === 'partner';
    if (!partner && e.kind !== 'parent' && e.kind !== 'sibling') continue;
    const p = e.pts;
    for (let i = 0; i + 3 < p.length; i += 2) {
      const dx = Math.abs(p[i + 2] - p[i]);
      const dy = Math.abs(p[i + 3] - p[i + 1]);
      const horizontal = dy < EPS && dx >= EPS;
      const vertical = dx < EPS && dy >= EPS;
      if (!horizontal && !vertical) continue;
      const s: Seg = {
        x0: Math.min(p[i], p[i + 2]),
        y0: Math.min(p[i + 1], p[i + 3]),
        x1: Math.max(p[i], p[i + 2]),
        y1: Math.max(p[i + 1], p[i + 3]),
        partner,
      };
      const key = `${horizontal ? 'h' : 'v'}${s.x0},${s.y0},${s.x1},${s.y1}`;
      if (seen.has(key)) continue;
      seen.add(key);
      (horizontal ? hs : vs).push(s);
    }
  }

  // Branches: points where three or more directions meet.
  const byY = new Map<number, Seg[]>();
  for (const s of hs) {
    const k = q(s.y0);
    const l = byY.get(k);
    if (l) l.push(s);
    else byY.set(k, [s]);
  }
  const byX = new Map<number, Seg[]>();
  for (const s of vs) {
    const k = q(s.x0);
    const l = byX.get(k);
    if (l) l.push(s);
    else byX.set(k, [s]);
  }
  const points = new Map<string, { x: number; y: number; mask: number }>();
  const touch = (x: number, y: number, dir: number) => {
    const k = `${q(x)},${q(y)}`;
    const pt = points.get(k);
    if (pt) pt.mask |= dir;
    else points.set(k, { x, y, mask: dir });
  };
  for (const s of hs) {
    touch(s.x0, s.y0, RIGHT);
    touch(s.x1, s.y0, LEFT);
  }
  for (const s of vs) {
    touch(s.x0, s.y0, DOWN);
    touch(s.x0, s.y1, UP);
  }
  const dots: number[] = [];
  for (const pt of points.values()) {
    let mask = pt.mask;
    for (const s of byY.get(q(pt.y)) ?? []) if (s.x0 + EPS < pt.x && pt.x < s.x1 - EPS) mask |= LEFT | RIGHT;
    for (const s of byX.get(q(pt.x)) ?? []) if (s.y0 + EPS < pt.y && pt.y < s.y1 - EPS) mask |= UP | DOWN;
    if (bits(mask) >= 3) dots.push(pt.x, pt.y);
  }

  // Crossings: a vertical parent line strictly inside a horizontal one, and the other way round.
  const crossings = new Map<string, number[]>();
  const sorted = vs.filter((s) => !s.partner).sort((a, b) => a.x0 - b.x0);
  for (const h of hs) {
    if (h.partner) continue;
    // First vertical line right of the left end.
    let lo = 0;
    let hi = sorted.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (sorted[mid].x0 <= h.x0 + EPS) lo = mid + 1;
      else hi = mid;
    }
    const xs: number[] = [];
    for (let i = lo; i < sorted.length && sorted[i].x0 < h.x1 - EPS; i++) {
      const v = sorted[i];
      if (v.y0 + EPS < h.y0 && h.y0 < v.y1 - EPS) xs.push(v.x0);
    }
    if (xs.length > 0) crossings.set(segKey(h.x0, h.x1, h.y0), xs);
  }
  return { dots, crossings };
}
