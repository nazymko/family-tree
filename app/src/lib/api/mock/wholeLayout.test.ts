import { describe, expect, it } from 'vitest';
import { fixtureTree } from './fixtures';
import { MockApi } from './MockApi';

describe('mock whole-tree layout', () => {
  it('places_everyone_once_without_overlap_and_is_stable', async () => {
    const api = new MockApi(fixtureTree(300));
    const m = await api.chartOpenWhole();
    const win = await api.chartWindow(m.meta.chartId, { x: 0, y: 0, w: 1e7, h: 1e7 }, 1);
    expect(win.nodes).toHaveLength(m.meta.nodeCount);
    expect(new Set(win.nodes.map((n) => n.person!.id)).size).toBe(win.nodes.length);
    const sorted = [...win.nodes].sort((a, b) => a.x - b.x);
    for (let i = 0; i < sorted.length; i++) {
      for (let j = i + 1; j < sorted.length && sorted[j].x < sorted[i].x + sorted[i].w; j++) {
        const a = sorted[i];
        const b = sorted[j];
        expect(a.y + a.h <= b.y || b.y + b.h <= a.y, `${a.id} overlaps ${b.id}`).toBe(true);
      }
    }
    const again = await new MockApi(fixtureTree(300)).chartOpenWhole();
    expect(again.blocks).toEqual(m.blocks);
  });
});
