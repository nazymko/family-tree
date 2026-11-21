import { describe, expect, it } from 'vitest';
import { nodeKeys } from './sheets';
import { planSheets, type PlanOptions } from './sheetPlan';
import { planSheetsAsync, sheetJob } from './sheetPlanJob';
import { loadFam } from './testing/famScenes';

describe('sheet plan job', () => {
  it('measured_captions_plan_as_the_live_ones', async () => {
    const dirs = (['top', 'bottom', 'left', 'right'] as const).map((d) => {
      const { scene } = loadFam('fam-b', d);
      return { direction: d, scene, keys: nodeKeys(scene) };
    });
    const root = loadFam('fam-b', 'top').root;
    const capW: PlanOptions['capW'] = (top, sib) => (sib ? 150 + (sib.id % 7) * 10 : 90 + (top.id % 5) * 20);
    const options = { choice: 'auto' as const, header: true, screenDir: 'top' as const, centres: [dirs[0].keys.get(root)!], tagW: 60, tagH: 14 };
    const live = planSheets(dirs, { ...options, capW });
    const viaJob = await planSheetsAsync(sheetJob(dirs, options, capW));
    const pages = (p: typeof live) => p.pages.map((pg) => pg.pieces.map((pc) => `${pc.index}:${pc.view.direction}@${pc.x.toFixed(1)},${pc.y.toFixed(1)}`).join(' '));
    expect(pages(viaJob)).toEqual(pages(live));
  });
});
