import { describe, expect, it } from 'vitest';
import type { ChartEdge, ChartNode } from '../api/types';
import type { ChartDirection } from './direction';
import type { DirScene } from './pieceView';
import { planSheets, type PlanOptions, type SheetPlan } from './sheetPlan';
import { nodeKeys } from './sheets';
import { loadFam } from './testing/famScenes';

const DIRS: ChartDirection[] = ['top', 'bottom', 'left', 'right'];
const nav = { parent: null, child: null, prev: null, next: null };

function famDirs(name: 'fam-b'): { dirs: DirScene[]; root: number } {
  const dirs = DIRS.map((d) => {
    const { scene } = loadFam(name, d);
    return { direction: d, scene, keys: nodeKeys(scene) };
  });
  return { dirs, root: loadFam(name, 'top').root };
}

const opts = (dirs: DirScene[], root: number): PlanOptions => ({
  choice: 'auto',
  header: true,
  screenDir: 'top',
  centres: [dirs[0].keys.get(root)!],
  capW: () => 200,
  tagW: 60,
  tagH: 14,
});

/** Node key -> the page that holds that person in full. */
function holders(plan: SheetPlan, dirs: DirScene[]): Map<string, number> {
  const out = new Map<string, number>();
  for (const pg of plan.pages)
    for (const p of pg.pieces) {
      const keys = dirs.find((d) => d.direction === p.view.direction)!.keys;
      for (const n of p.view.scene.nodes) if (n.dupOf === null && !p.view.refs.has(n.id)) out.set(keys.get(n.id)!, pg.no);
    }
  return out;
}

describe('planSheets', () => {
  it('small_chart_one_page', () => {
    // Twenty people in four rows of five.
    const nodes: ChartNode[] = Array.from({ length: 20 }, (_, i) => ({
      id: i + 1,
      person: { id: `s${i + 1}`, name: `Taras Bondar ${i + 1}`, lifespan: '', sex: 'male', living: false, private: false, siblingMarker: false },
      x: (i % 5) * 140,
      y: Math.floor(i / 5) * 90,
      w: 124,
      h: 56,
      gen: Math.floor(i / 5) - 2,
      ahnen: null,
      dupOf: null,
      moreUp: false,
      moreDown: false,
      nav,
      label: `Taras Bondar ${i + 1}`,
    }));
    const edges: ChartEdge[] = nodes.slice(1).map((n) => ({ from: 1, to: n.id, kind: 'partner', pts: [62, 28, n.x, n.y + 28] }));
    const scene = { nodes, edges };
    const dirs = DIRS.map((d) => ({ direction: d, scene, keys: nodeKeys(scene) }));
    const plan = planSheets(dirs, { ...opts(dirs, 1), centres: ['p:s1'] });
    expect(plan.single).toBe(true);
    expect(plan.pages.length).toBe(1);
    expect(plan.pages[0].pieces.length).toBe(1);
    expect(plan.pages[0].pieces[0].capY).toBeNull();
    expect(plan.pages[0].namePt).toBeGreaterThanOrEqual(8);
  });

  const b = famDirs('fam-b');
  const planB = planSheets(b.dirs, opts(b.dirs, b.root));

  it('cover_on_page_one', () => {
    expect(planB.single).toBe(false);
    expect(planB.pages[0].pieces[0].index).toBe(0);
    expect(planB.pages[0].pieces[0].top).toBeNull();
    expect(planB.pages[0].pieces[0].capY).toBeNull();
  });

  it('ref_pages_resolved', () => {
    const hold = holders(planB, b.dirs);
    let refs = 0;
    for (const pg of planB.pages)
      for (const p of pg.pieces) {
        const keys = b.dirs.find((d) => d.direction === p.view.direction)!.keys;
        const byId = new Map(p.view.scene.nodes.map((n) => [n.id, n]));
        for (const [id, page] of p.refPages) {
          refs++;
          const n = byId.get(id)!;
          // A repeat points to its first occurrence.
          const key = n.dupOf !== null ? `p:${n.person!.id}` : keys.get(id)!;
          expect(hold.get(key), key).toBe(page);
        }
      }
    expect(refs).toBeGreaterThan(0);
  });
});
