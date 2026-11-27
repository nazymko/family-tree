import { describe, expect, it } from 'vitest';
import type { ChartEdge, ChartNode } from '../api/types';
import { makeApi, richestRoot, BIG } from './testing/helpers';
import { TWIG_SAMPLES, limb, planBranches, sCurve, type Family } from './branches';
import { boughWidth } from './renderer';

/** A top-down family line as the layouts draw it: down from the junction, across, down onto the child. */
function zEdge(from: number, to: number, sx: number, sy: number, cx: number, top: number): ChartEdge {
  const mid = sy + 20;
  return { from, to, kind: 'parent', pts: [sx, sy, sx, mid, cx, mid, cx, top] };
}

async function aroundScene(kin: 'line' | 'all') {
  const { api, fx } = makeApi();
  const root = await richestRoot(api, fx, 'around');
  const meta = await api.chartOpen({ kind: 'around', root, generations: 255, kin });
  const win = await api.chartWindow(meta.chartId, BIG, 1);
  return { nodes: win.nodes as ChartNode[], edges: win.edges as ChartEdge[] };
}

const touches = (pts: number[], n: ChartNode, inset = 1.5): boolean => {
  for (let i = 0; i + 1 < pts.length; i += 2) {
    if (pts[i] > n.x + inset && pts[i] < n.x + n.w - inset && pts[i + 1] > n.y + inset && pts[i + 1] < n.y + n.h - inset) return true;
  }
  return false;
};

describe('planBranches', () => {
  it('makes_one_family_of_the_lines_that_start_at_the_same_junction', () => {
    const edges = [zEdge(0, 1, 100, 50, 40, 100), zEdge(0, 2, 100, 50, 160, 100), zEdge(3, 4, 500, 50, 500, 100)];
    const fams = planBranches(edges);
    expect(fams).toHaveLength(2);
    expect(fams[0].twigs.map((t) => t.to)).toEqual([1, 2]);
    expect(fams[0].bough).toEqual([100, 50, 100, 70]);
    expect(fams[0].knot).toEqual([100, 70]);
    expect(fams[1].twigs.map((t) => t.to)).toEqual([4]);
  });

  it('draws_every_twig_from_the_knot_to_the_end_of_its_line', () => {
    const [f] = planBranches([zEdge(0, 1, 100, 50, 40, 100)]);
    const pts = f.twigs[0].pts;
    expect(f.twigs[0].curved).toBe(true);
    expect(pts).toHaveLength((TWIG_SAMPLES + 1) * 2);
    expect(pts.slice(0, 2)).toEqual([100, 70]);
    expect(pts.slice(-2)).toEqual([40, 100]);
  });

  it('leaves_and_arrives_along_the_axis_of_the_original_line', () => {
    const pts = sCurve([0, 0], [80, 100], 'y');
    // Vertical at both ends: the first and last steps hardly move sideways.
    expect(Math.abs(pts[2] - pts[0])).toBeLessThan(2);
    expect(Math.abs(pts[pts.length - 2] - pts[pts.length - 4])).toBeLessThan(2);
    const across = sCurve([0, 0], [100, 80], 'x');
    expect(Math.abs(across[3] - across[1])).toBeLessThan(2);
  });

  it('forks_a_limb_off_the_bough_and_drops_it_onto_the_card', () => {
    const pts = limb([0, 0], [100, 80], 'y');
    // Leaves the knot sideways (first step mostly along x), arrives straight down onto the card.
    expect(pts[2] - pts[0]).toBeGreaterThan(Math.abs(pts[3] - pts[1]));
    expect(Math.abs(pts[pts.length - 2] - pts[pts.length - 4])).toBeLessThan(2);
    for (let i = 0; i < pts.length; i += 2) {
      expect(pts[i]).toBeGreaterThanOrEqual(0);
      expect(pts[i]).toBeLessThanOrEqual(100);
      expect(pts[i + 1]).toBeGreaterThanOrEqual(0);
      expect(pts[i + 1]).toBeLessThanOrEqual(80);
    }
  });

  it('keeps_the_corners_of_a_line_of_another_shape', () => {
    const odd: ChartEdge = { from: 0, to: 1, kind: 'parent', pts: [0, 0, 0, 10, 50, 10, 50, 40, 20, 40, 20, 60] };
    const [f] = planBranches([odd]);
    expect(f.twigs[0].curved).toBe(false);
    expect(f.twigs[0].pts).toEqual([0, 10, 50, 10, 50, 40, 20, 40, 20, 60]);
  });

  it('ignores_partner_and_duplicate_lines', () => {
    const edges: ChartEdge[] = [
      { from: 0, to: 1, kind: 'partner', pts: [0, 0, 10, 0] },
      { from: 1, to: 2, kind: 'dup', pts: [0, 0, 10, 0, 10, 10, 0, 10] },
    ];
    expect(planBranches(edges)).toEqual([]);
  });

  it('is_deterministic', () => {
    const edges = [zEdge(0, 1, 100, 50, 40, 100), zEdge(0, 2, 100, 50, 160, 100), zEdge(3, 4, 500, 50, 500, 100)];
    expect(planBranches(edges)).toEqual(planBranches([...edges]));
  });
});

describe('the branches of a chart built around a person (synthetic tree)', () => {
  const families = (edges: ChartEdge[]) => new Set(edges.filter((e) => e.kind === 'parent' || e.kind === 'sibling').map((e) => `${e.from}|${e.pts[0]}|${e.pts[1]}`));

  for (const kin of ['line', 'all'] as const) {
    it(`places_every_family_and_every_child_${kin}`, async () => {
      const { edges } = await aroundScene(kin);
      const fams = planBranches(edges);
      expect(fams.length).toBeGreaterThan(0);
      expect(fams).toHaveLength(families(edges).size);
      const lines = edges.filter((e) => e.kind === 'parent' || e.kind === 'sibling').length;
      expect(fams.reduce((n, f: Family) => n + f.twigs.length, 0)).toBe(lines);
      // Each twig ends where its line ends.
      for (const e of edges.filter((x) => x.kind === 'parent' || x.kind === 'sibling')) {
        const fam = fams.find((f) => f.from === e.from && f.bough[0] === e.pts[0] && f.bough[1] === e.pts[1])!;
        const tw = fam.twigs.find((x) => x.to === e.to)!;
        expect(tw.pts.slice(-2)).toEqual(e.pts.slice(-2));
      }
    });

    it(`draws_no_branch_over_a_card_${kin}`, async () => {
      const { nodes, edges } = await aroundScene(kin);
      for (const f of planBranches(edges)) {
        for (const n of nodes) {
          expect(touches([f.bough[0], f.bough[1], f.bough[2], f.bough[3]], n)).toBe(false);
          for (const tw of f.twigs) expect(touches(tw.pts, n)).toBe(false);
        }
      }
    });
  }

  it('never_extends_a_bough_beyond_its_own_junction_and_children', async () => {
    const { edges } = await aroundScene('all');
    for (const f of planBranches(edges)) {
      const lines = edges.filter((e) => (e.kind === 'parent' || e.kind === 'sibling') && e.from === f.from && e.pts[0] === f.bough[0] && e.pts[1] === f.bough[1]);
      const xs = [f.bough[0], ...lines.map((e) => e.pts[e.pts.length - 2])];
      const lo = Math.min(...xs);
      const hi = Math.max(...xs);
      const all = [f.bough, ...f.twigs.map((t) => t.pts)];
      for (const pts of all) {
        for (let i = 0; i < pts.length; i += 2) {
          expect(pts[i]).toBeGreaterThanOrEqual(lo - 0.01);
          expect(pts[i]).toBeLessThanOrEqual(hi + 0.01);
        }
      }
      expect(f.twigs).toHaveLength(lines.length);
    }
  });

  it('gives_the_same_plan_every_time', async () => {
    const a = await aroundScene('all');
    const b = await aroundScene('all');
    expect(planBranches(a.edges)).toEqual(planBranches(b.edges));
  });
});

describe('the thickness of a bough', () => {
  it('is_thickest_under_the_centre_and_thinner_every_generation', () => {
    const w = [0, 1, 2, 3, 4, 8].map((g) => boughWidth(g));
    for (let i = 1; i < w.length; i++) expect(w[i]).toBeLessThanOrEqual(w[i - 1]);
    expect(w[0]).toBeGreaterThan(w[3]);
    expect(boughWidth(-2)).toBe(boughWidth(2));
    expect(w[5]).toBeGreaterThan(0);
  });
});
