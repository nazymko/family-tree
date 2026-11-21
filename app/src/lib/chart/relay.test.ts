import { describe, expect, it } from 'vitest';
import type { ChartEdge, ChartNode } from '../api/types';
import type { ChartDirection } from './direction';
import { sceneBounds, type Scene } from './picture';
import { foldRuns, relayPiece } from './relay';
import { nodeKeys } from './sheets';
import { planSheets } from './sheetPlan';
import { GROUP_TAG_MORE, tagDir, tagRect } from './pieceView';
import { FAMS, loadFam } from './testing/famScenes';

const W = 124;
const H = 56;
const box = (id: number, x: number, y: number, gen: number): ChartNode =>
  ({ id, x, y, w: W, h: H, gen, dupOf: null, person: { id: `k${id}` } }) as unknown as ChartNode;
/** A top-down family line: parent couple's drop at `px`, bus at `bus`, to the child box. */
const line = (from: number, to: number, px: number, py: number, bus: number, kid: ChartNode): ChartEdge => ({
  from,
  to,
  kind: 'parent',
  pts: [px, py, px, bus, kid.x + W / 2, bus, kid.x + W / 2, kid.y],
});
const partner = (a: ChartNode, b: ChartNode): ChartEdge => ({ from: a.id, to: b.id, kind: 'partner', pts: [a.x + W, a.y + H / 2, b.x, b.y + H / 2] });

/** Boxes overlap; a line point lies inside a box it does not belong to; a line leaves its boxes. */
export function drawIssues(s: Scene): string[] {
  const out: string[] = [];
  const byId = new Map(s.nodes.map((n) => [n.id, n]));
  for (let i = 0; i < s.nodes.length; i++)
    for (let j = i + 1; j < s.nodes.length; j++) {
      const a = s.nodes[i];
      const b = s.nodes[j];
      if (a.x < b.x + b.w - 0.5 && b.x < a.x + a.w - 0.5 && a.y < b.y + b.h - 0.5 && b.y < a.y + a.h - 0.5) out.push(`overlap ${a.id} ${b.id}`);
    }
  const on = (n: ChartNode, x: number, y: number) => x >= n.x - 1 && x <= n.x + n.w + 1 && y >= n.y - 1 && y <= n.y + n.h + 1;
  // A couple's line to a child leaves from the middle of their partner line.
  const onPartner = (id: number, x: number, y: number) =>
    s.edges.some((p) => {
      if (p.kind !== 'partner' || (p.from !== id && p.to !== id)) return false;
      const [x1, y1, x2, y2] = [p.pts[0], p.pts[1], p.pts[p.pts.length - 2], p.pts[p.pts.length - 1]];
      return x >= Math.min(x1, x2) - 1 && x <= Math.max(x1, x2) + 1 && y >= Math.min(y1, y2) - 1 && y <= Math.max(y1, y2) + 1;
    });
  for (const e of s.edges) {
    const f = byId.get(e.from)!;
    const t = byId.get(e.to)!;
    const n = e.pts.length;
    if (!on(f, e.pts[0], e.pts[1]) && !on(t, e.pts[0], e.pts[1]) && !onPartner(e.from, e.pts[0], e.pts[1])) out.push(`loose start ${e.from}>${e.to}`);
    if (!on(t, e.pts[n - 2], e.pts[n - 1]) && !on(f, e.pts[n - 2], e.pts[n - 1])) out.push(`loose end ${e.from}>${e.to}`);
    // Every segment, sampled, stays out of the inside of other boxes.
    for (let i = 0; i + 3 < n; i += 2)
      for (let k = 1; k < 8; k++) {
        const x = e.pts[i] + ((e.pts[i + 2] - e.pts[i]) * k) / 8;
        const y = e.pts[i + 1] + ((e.pts[i + 3] - e.pts[i + 1]) * k) / 8;
        for (const b of s.nodes)
          if (b.id !== e.from && b.id !== e.to && x > b.x + 1 && x < b.x + b.w - 1 && y > b.y + 1 && y < b.y + b.h - 1) out.push(`line ${e.from}>${e.to} through ${b.id}`);
      }
  }
  return [...new Set(out)];
}

describe('relayPiece', { timeout: 30_000 }, () => {
  it('closes_the_staircase', () => {
    // A couple far to the right of their two children (the gap left by cousins cut away).
    const a = box(1, 2000, 0, 0);
    const b = box(2, 2000 + W + 24, 0, 0);
    const c = box(3, 0, 88, 1);
    const d = box(4, W + 28, 88, 1);
    const px = 2000 + W + 12;
    const s: Scene = { nodes: [a, b, c, d], edges: [partner(a, b), ...[c, d].flatMap((k) => [line(1, k.id, px, H / 2, 72, k), line(2, k.id, px, H / 2, 72, k)])] };
    const r = relayPiece(s, 'top')!;
    expect(sceneBounds(r, []).w).toBeLessThan(2 * W + 60);
    expect(drawIssues(r)).toEqual([]);
  });

  it('folds_a_long_run_of_leaves', () => {
    const p = box(1, 0, 0, 0);
    const kids = [...Array(12).keys()].map((i) => box(10 + i, i * (W + 28), 88, 1));
    const below = box(30, 0, 176, 2);
    const s: Scene = { nodes: [p, ...kids, below], edges: kids.map((k) => line(1, k.id, W / 2, H, 72, k)) };
    const { scene, groups } = foldRuns(s);
    expect(groups).toEqual([kids.map((k) => k.id)]);
    expect(new Set(scene.nodes.filter((n) => n.gen === 1).map((n) => n.y)).size).toBe(2);
    expect(scene.nodes.find((n) => n.id === 30)!.y).toBe(176 + H + 8);
    expect(drawIssues(scene)).toEqual([]);
    const r = relayPiece(s, 'top', new Map(), 40, true)!;
    expect(sceneBounds(r, []).w).toBeLessThan(sceneBounds(s, []).w * 0.6);
    expect(drawIssues(r)).toEqual([]);
  });

  it('keeps_a_couple_together_in_a_fold', () => {
    const p = box(1, 0, 0, 0);
    const kids = [...Array(9).keys()].map((i) => box(10 + i, i * (W + 28), 88, 1));
    const spouse = box(40, 9 * (W + 28), 88, 1);
    kids.push(box(19, 10 * (W + 28), 88, 1));
    const s: Scene = { nodes: [p, ...kids, spouse], edges: [...kids.map((k) => line(1, k.id, W / 2, H, 72, k)), partner(kids[8], spouse)] };
    const { scene } = foldRuns(s);
    const at = new Map(scene.nodes.map((n) => [n.id, n]));
    expect(at.get(40)!.y).toBe(at.get(18)!.y);
    expect(at.get(40)!.x).toBeGreaterThan(at.get(18)!.x);
    expect(drawIssues(scene)).toEqual([]);
  });

  it('every_piece_of_every_fam_draws_clean', () => {
    for (const f of FAMS) {
      const dirs = (['top', 'bottom', 'left', 'right'] as ChartDirection[]).map((d) => {
        const { scene } = loadFam(f, d);
        return { direction: d, scene, keys: nodeKeys(scene) };
      });
      const root = loadFam(f, 'top').root;
      const plan = planSheets(dirs, { choice: 'auto', header: true, screenDir: 'top', centres: [dirs[0].keys.get(root)!], capW: () => 200, tagW: 60, tagH: 14 });
      for (const pg of plan.pages)
        for (const p of pg.pieces) {
          const v = p.view;
          expect(drawIssues(v.scene), `${f} piece ${p.index} ${v.direction}`).toEqual([]);
          // A reference's tag lands on no box.
          for (const n of v.scene.nodes) {
            if (!v.refs.has(n.id)) continue;
            const t = tagRect(n, tagDir(v, n.id), 60 * (1 + GROUP_TAG_MORE * ((v.groups.get(n.id)?.length ?? 1) - 1)), 14);
            const hit = v.scene.nodes.filter((b) => t.x < b.x + b.w - 0.5 && b.x < t.x + t.w - 0.5 && t.y < b.y + b.h - 0.5 && b.y < t.y + t.h - 0.5);
            expect(hit.map((b) => b.id), `${f} piece ${p.index} ${v.direction} tag of ${n.id}`).toEqual([]);
          }
        }
    }
  });
});
