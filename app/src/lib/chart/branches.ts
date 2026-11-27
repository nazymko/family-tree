import type { ChartEdge } from '../api/types';

/**
 * The "branches" display of a chart: every family (the parents' junction and its children) becomes a branch of a
 * tree. The cards and the layout do not move; the connector lines of the chart are replaced by a short bough from the
 * family's junction and a curved twig to each child, so the people, the selection and the pan and zoom stay as they are.
 *
 * Only the edges of the chart are read. A family is every `parent` or `sibling` edge that starts at the same point of
 * the same card; its bough is the first segment of the line, shared by all its children, and each twig is an S curve
 * from the end of the bough to the child's card, leaving along the bough's axis and arriving along the same axis as
 * the original line. A line of any other shape (a folded or spined route) keeps its own corners, so a twig never
 * crosses a card the original line did not cross.
 */

/** Segments a twig's curve is cut into; the same for every twig, so the plan is deterministic. */
export const TWIG_SAMPLES = 16;

export interface Twig {
  /** The child's box. */
  to: number;
  /** Flat `x0, y0, x1, y1, ...`, from the knot to the child. */
  pts: number[];
  /** False when the original corners were kept. */
  curved: boolean;
}

export interface Family {
  /** The card the family hangs from. */
  from: number;
  /** The first segment of the family's lines: `x0, y0, x1, y1`. */
  bough: [number, number, number, number];
  /** Where the twigs start (the end of the bough). */
  knot: [number, number];
  twigs: Twig[];
}

const same = (a: number, b: number): boolean => Math.abs(a - b) < 0.01;

/** The S curve from `a` to `b` leaving and arriving along `axis`, cut into `n` segments. */
export function sCurve(a: [number, number], b: [number, number], axis: 'x' | 'y', n: number = TWIG_SAMPLES): number[] {
  const k = axis === 'y' ? 1 : 0;
  const mid = (a[k] + b[k]) / 2;
  const c1: [number, number] = k === 1 ? [a[0], mid] : [mid, a[1]];
  const c2: [number, number] = k === 1 ? [b[0], mid] : [mid, b[1]];
  const out: number[] = [];
  for (let i = 0; i <= n; i++) {
    const t = i / n;
    const u = 1 - t;
    const w0 = u * u * u;
    const w1 = 3 * u * u * t;
    const w2 = 3 * u * t * t;
    const w3 = t * t * t;
    out.push(w0 * a[0] + w1 * c1[0] + w2 * c2[0] + w3 * b[0], w0 * a[1] + w1 * c1[1] + w2 * c2[1] + w3 * b[1]);
  }
  // The end points are exact, not the result of the arithmetic above.
  out[0] = a[0];
  out[1] = a[1];
  out[out.length - 2] = b[0];
  out[out.length - 1] = b[1];
  return out;
}

/**
 * The limb from `a` (the knot) to `b` (a child's card): it leaves the knot across the bough's `axis` and turns into
 * the child along that axis, so it forks away from the bough like a branch and drops onto the card. It stays inside
 * the box of `a` and `b`, where the layout keeps the cards clear.
 */
export function limb(a: [number, number], b: [number, number], axis: 'x' | 'y', n: number = TWIG_SAMPLES): number[] {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const c1: [number, number] = axis === 'y' ? [a[0] + dx * 0.6, a[1]] : [a[0], a[1] + dy * 0.6];
  const c2: [number, number] = axis === 'y' ? [b[0], b[1] - dy * 0.6] : [b[0] - dx * 0.6, b[1]];
  const out: number[] = [];
  for (let i = 0; i <= n; i++) {
    const t = i / n;
    const u = 1 - t;
    const w0 = u * u * u;
    const w1 = 3 * u * u * t;
    const w2 = 3 * u * t * t;
    const w3 = t * t * t;
    out.push(w0 * a[0] + w1 * c1[0] + w2 * c2[0] + w3 * b[0], w0 * a[1] + w1 * c1[1] + w2 * c2[1] + w3 * b[1]);
  }
  out[0] = a[0];
  out[1] = a[1];
  out[out.length - 2] = b[0];
  out[out.length - 1] = b[1];
  return out;
}

/**
 * The families among a set of edges, as branches. Families come out in the order of their first edge and twigs in the
 * order of the edges; nothing depends on anything but the edges.
 */
export function planBranches(edges: readonly ChartEdge[]): Family[] {
  const families: Family[] = [];
  const byKey = new Map<string, Family>();
  for (const e of edges) {
    if ((e.kind !== 'parent' && e.kind !== 'sibling') || e.pts.length < 4) continue;
    const p = e.pts;
    const key = `${e.from}|${Math.round(p[0] * 100)}|${Math.round(p[1] * 100)}`;
    let fam = byKey.get(key);
    if (!fam) {
      fam = { from: e.from, bough: [p[0], p[1], p[2], p[3]], knot: [p[2], p[3]], twigs: [] };
      byKey.set(key, fam);
      families.push(fam);
    }
    const rest = p.slice(2);
    if (rest.length < 4) {
      // The line is one segment long: the bough already reaches the child.
      fam.twigs.push({ to: e.to, pts: [...rest], curved: false });
      continue;
    }
    // The family line of the layouts: along, across, along (a Z), the first and last segments on one axis.
    const vertical = same(p[0], p[2]);
    const horizontal = same(p[1], p[3]);
    const z =
      p.length === 8 &&
      vertical !== horizontal &&
      (vertical ? same(p[4], p[6]) && same(p[5], p[3]) : same(p[5], p[7]) && same(p[4], p[2]));
    if (z) fam.twigs.push({ to: e.to, pts: limb(fam.knot, [p[6], p[7]], vertical ? 'y' : 'x'), curved: true });
    else fam.twigs.push({ to: e.to, pts: rest, curved: false });
  }
  return families;
}
