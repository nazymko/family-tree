import { describe, expect, it } from 'vitest';
import type { ChartDirection } from './direction';
import { nodeKeys } from './sheets';
import { FAMS, loadFam } from './testing/famScenes';

const DIRS: ChartDirection[] = ['top', 'bottom', 'left', 'right'];

describe('family-shaped fixtures', () => {
  it('loads_every_fixture', () => {
    for (const f of FAMS) {
      for (const d of DIRS) {
        const { scene, root } = loadFam(f, d);
        const ids = new Set(scene.nodes.map((n) => n.id));
        expect(ids.has(root), `${f} ${d}`).toBe(true);
        for (const e of scene.edges) if (e.kind === 'parent') expect(ids.has(e.from) && ids.has(e.to), `${f} ${d}`).toBe(true);
        for (const n of scene.nodes) expect(n.person?.id).toMatch(/^k\d+$/);
      }
    }
    expect(loadFam('fam-d', 'top').scene.nodes.length).toBeGreaterThanOrEqual(1500);
  });

  it('keys_match_across_directions', () => {
    for (const f of FAMS) {
      const sets = DIRS.map((d) => {
        const keys = [...nodeKeys(loadFam(f, d).scene).values()];
        expect(new Set(keys).size, `${f} ${d}`).toBe(keys.length);
        return new Set(keys);
      });
      for (const s of sets.slice(1)) expect([...s].sort()).toEqual([...sets[0]].sort());
    }
  });
});
