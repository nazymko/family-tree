import { describe, expect, it } from 'vitest';
import type { ChartNode } from '../types';
import { MockApi } from './MockApi';
import { fixtureTree } from './fixtures';
import { ahnenRelation, layoutDescendants } from './layout';

describe('mock descendants layout: a person with a partnered and a partnerless family', () => {
  // Invented people: M has partner P with child C1, and a family alone with child C2.
  const src = {
    ref: (id: string) => ({ id, name: `Person ${id}` }),
    parents: () => [null, null],
    unions: (id: string) =>
      id === 'M'
        ? [
            { partner: 'P', children: ['C1'] },
            { partner: null, children: ['C2'] },
          ]
        : [],
  };

  it('the partnerless family stands left of the couple family, no child line crosses', () => {
    const l = layoutDescendants('M', 5, src as never);
    const x = (key: string) => {
      const n = l.nodes.find((q) => q.person.id === key)!;
      return n.x + n.w / 2;
    };
    expect(x('C2')).toBeLessThan(x('C1'));
    const conn = l.edges.find((e) => e.kind === 'partner')!;
    const mid = (conn.pts[0] + conn.pts[2]) / 2;
    const c1 = l.edges.find((e) => e.kind === 'parent' && l.nodes[e.to].person.id === 'C1')!;
    const c2 = l.edges.find((e) => e.kind === 'parent' && l.nodes[e.to].person.id === 'C2')!;
    expect(c1.pts[0]).toBeCloseTo(mid, 1);
    expect(c2.pts[0]).toBeLessThan(mid);
  });
});

const BIG = { x: -1e6, y: -1e6, w: 2e6, h: 2e6 };

function overlaps(nodes: ChartNode[]): number {
  let n = 0;
  for (let i = 0; i < nodes.length; i++)
    for (let j = i + 1; j < nodes.length; j++) {
      const a = nodes[i];
      const b = nodes[j];
      if (a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h) n++;
    }
  return n;
}

describe('mock layout (port of gt-layout)', () => {
  const fx = fixtureTree(500);
  const api = new MockApi(fx);

  it('pedigree_uses_ahnentafel_columns', async () => {
    const home = (await api.getSettings()).homePerson!;
    const meta = await api.chartOpen({ kind: 'pedigree', root: home, generations: 4 });
    const win = await api.chartWindow(meta.chartId, BIG, 1);
    const root = win.nodes[meta.rootNode];
    expect(root.ahnen).toBe(1);
    expect(root.x).toBe(0);
    for (const n of win.nodes) {
      expect(n.x).toBe(n.gen * 216);
      expect(n.gen).toBeLessThan(4);
      expect(n.w).toBe(176);
      expect(n.h).toBe(66);
    }
    expect(overlaps(win.nodes)).toBe(0);
    let sawMore = false;
    for (const p of fx.persons.slice(0, 200)) {
      const m = await api.chartOpen({ kind: 'pedigree', root: p.id, generations: 2 });
      if ((await api.chartWindow(m.chartId, BIG, 1)).nodes.some((n) => n.moreUp)) sawMore = true;
    }
    expect(sawMore).toBe(true);
    const father = win.nodes.find((n) => n.ahnen === 2)!;
    expect(root.nav.child).toBe(father.id);
    expect(father.nav.parent).toBe(root.id);
    expect(father.nav.next).toBe(win.nodes.find((n) => n.ahnen === 3)!.id);
    // Edge `from` is the node nearer the root.
    for (const e of win.edges.filter((e) => e.kind === 'parent')) expect(win.nodes[e.from].gen).toBeLessThan(win.nodes[e.to].gen);
  });

  it('relation_labels', () => {
    expect(ahnenRelation(2)).toBe('Father');
    expect(ahnenRelation(3)).toBe('Mother');
    expect(ahnenRelation(6)).toBe("Mother's father");
  });

  it('descendants_partners_have_empty_nav_and_no_overlap', async () => {
    const meta = await api.chartOpen({ kind: 'descendants', root: fx.specials.manyPartners!, generations: 3 });
    const win = await api.chartWindow(meta.chartId, BIG, 1);
    const partners = win.edges.filter((e) => e.kind === 'partner');
    expect(partners.length).toBeGreaterThanOrEqual(12);
    for (const e of partners) {
      expect(win.nodes[e.to].nav).toEqual({ parent: null, child: null, prev: null, next: null });
      expect(win.nodes[e.to].y).toBe(win.nodes[e.from].y);
    }
    expect(overlaps(win.nodes)).toBe(0);
  });

  it('cycle_terminates_with_dup_nodes', async () => {
    const meta = await api.chartOpen({ kind: 'pedigree', root: fx.specials.cycleA!, generations: 8 });
    expect(Number.isFinite(meta.nodeCount)).toBe(true);
    const win = await api.chartWindow(meta.chartId, BIG, 1);
    expect(win.nodes.length).toBe(meta.nodeCount);
  });

  it('stale_chart_window_after_edit', async () => {
    const a = new MockApi(fixtureTree(60));
    const home = (await a.getSettings()).homePerson!;
    const meta = await a.chartOpen({ kind: 'pedigree', root: home, generations: 3 });
    await a.apply({ type: 'createPerson', draft: { given: 'New', surnames: ['Person'], sex: 'unknown' } } as never);
    await expect(a.chartWindow(meta.chartId, BIG, 1)).rejects.toMatchObject({ code: 'stale' });
  });
});
