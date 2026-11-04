import { describe, expect, it } from 'vitest';
import type { ChartNode } from '../types';
import { MockApi } from './MockApi';
import { fixtureTree } from './fixtures';

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

describe('mock relatives layout (port of gt-layout relatives)', () => {
  const fx = fixtureTree(500);
  const api = new MockApi(fx);

  async function richest() {
    let best = { id: fx.persons[0].id, count: 0 };
    for (const p of fx.persons.slice(0, 150)) {
      const m = await api.chartOpen({ kind: 'relatives', root: p.id, generations: 3 });
      await api.chartClose(m.chartId);
      if (m.nodeCount > best.count) best = { id: p.id, count: m.nodeCount };
    }
    return best.id;
  }

  it('has_siblings_on_the_focus_row_parents_above_and_no_overlap', async () => {
    const root = await richest();
    const meta = await api.chartOpen({ kind: 'relatives', root, generations: 3 });
    expect(meta.kind).toBe('relatives');
    const win = await api.chartWindow(meta.chartId, BIG, 1);
    expect(overlaps(win.nodes)).toBe(0);
    const focus = win.nodes[meta.rootNode];
    expect(focus.relation).toBe('starting person');
    expect(win.nodes.every((n) => n.x >= 0 && n.y >= 0)).toBe(true);
    const sibs = win.nodes.filter((n) => /brother|sister|sibling/.test(n.relation ?? ''));
    expect(sibs.length).toBeGreaterThan(0);
    for (const s of sibs) expect(s.y).toBe(focus.y);
    const parents = win.nodes.filter((n) => /^(father|mother|parent)$/.test(n.relation ?? ''));
    expect(parents.length).toBeGreaterThan(0);
    for (const p of parents) expect(p.y).toBeLessThan(focus.y);
    // Up goes to a parent and back down to the focus.
    expect(win.nodes.find((n) => n.id === focus.nav.parent)?.relation).toMatch(/^(father|mother|parent)$/);
  });

  it('can_hide_aunts_and_uncles', async () => {
    let seen = false;
    for (const p of fx.persons.slice(0, 150)) {
      const on = await api.chartOpen({ kind: 'relatives', root: p.id, generations: 3, auntsUncles: true });
      const off = await api.chartOpen({ kind: 'relatives', root: p.id, generations: 3, auntsUncles: false });
      const aunts = (await api.chartWindow(on.chartId, BIG, 1)).nodes.filter((n) => /aunt|uncle/.test(n.relation ?? ''));
      const none = (await api.chartWindow(off.chartId, BIG, 1)).nodes.filter((n) => /aunt|uncle/.test(n.relation ?? ''));
      expect(none).toHaveLength(0);
      if (aunts.length) seen = true;
      await api.chartClose(on.chartId);
      await api.chartClose(off.chartId);
      if (seen) break;
    }
    expect(seen).toBe(true);
  });

  it('shows_cousins_on_their_own_row_without_overlap', async () => {
    let found = false;
    for (const p of fx.persons.slice(0, 200)) {
      const m = await api.chartOpen({ kind: 'relatives', root: p.id, generations: 3 });
      const win = await api.chartWindow(m.chartId, BIG, 1);
      await api.chartClose(m.chartId);
      expect(overlaps(win.nodes)).toBe(0);
      const cousins = win.nodes.filter((n) => n.relation === 'cousin');
      if (!cousins.length) continue;
      found = true;
      const focus = win.nodes[m.rootNode];
      for (const c of cousins) expect(c.y).toBeLessThan(focus.y);
      break;
    }
    expect(found).toBe(true);
  });

  it('outline_references_are_not_expandable', async () => {
    const root = await richest();
    const meta = await api.chartOpen({ kind: 'relatives', root, generations: 3 });
    const top = (await api.chartOutline(meta.chartId, null, 10))[0];
    const groups = await api.chartOutline(meta.chartId, top.nodeId, 50);
    const parents = await api.chartOutline(meta.chartId, groups.find((g) => g.label === 'Parents')!.nodeId, 50);
    const gs = await api.chartOutline(meta.chartId, parents[0].nodeId, 50);
    const kids = await api.chartOutline(meta.chartId, gs.find((g) => g.label === 'Children')!.nodeId, 50);
    const back = kids.find((k) => k.nodeId === top.nodeId)!;
    expect(back.hasChildren).toBe(false);
    expect(back.label).toMatch(/also shown above/);
  });

  it('outline_items_carry_the_relation_and_reference_only_people_have_no_expander', async () => {
    const root = await richest();
    const meta = await api.chartOpen({ kind: 'relatives', root, generations: 3 });
    const top = (await api.chartOutline(meta.chartId, null, 10))[0];
    expect(top.relation).toBeUndefined();
    const groups = await api.chartOutline(meta.chartId, top.nodeId, 50);
    const parents = await api.chartOutline(meta.chartId, groups.find((g) => g.label === 'Parents')!.nodeId, 50);
    expect(parents.every((p) => /^(father|mother|parent)$/.test(p.relation ?? ''))).toBe(true);
    // No row may open into nothing but references.
    for (const p of parents) {
      if (!p.hasChildren) continue;
      const gs = await api.chartOutline(meta.chartId, p.nodeId, 50);
      let fresh = 0;
      for (const g of gs) {
        const rows = await api.chartOutline(meta.chartId, g.nodeId, 50);
        fresh += rows.filter((r) => !/also shown above/.test(r.label)).length;
      }
      expect(fresh).toBeGreaterThan(0);
    }
  });

  it('terminates_on_the_cycle_fixture', async () => {
    const meta = await api.chartOpen({ kind: 'relatives', root: fx.specials.cycleA!, generations: 5 });
    const win = await api.chartWindow(meta.chartId, BIG, 1);
    expect(win.nodes.length).toBeGreaterThan(1);
    expect(overlaps(win.nodes)).toBe(0);
    expect(win.nodes.some((n) => n.dupOf !== null)).toBe(true);
  });

  it('outline_groups_the_related_people', async () => {
    const root = await richest();
    const meta = await api.chartOpen({ kind: 'relatives', root, generations: 3 });
    const top = await api.chartOutline(meta.chartId, null, 10);
    expect(top).toHaveLength(1);
    const groups = await api.chartOutline(meta.chartId, top[0].nodeId, 50);
    const titles = groups.map((g) => g.label);
    expect(titles).toContain('Parents');
    expect(titles).toContain('Siblings');
    expect(groups.every((g) => g.person === null && g.hasChildren)).toBe(true);
    const parents = await api.chartOutline(meta.chartId, groups[titles.indexOf('Parents')].nodeId, 50);
    expect(parents.map((p) => p.label).every((l) => /^(Father|Mother|Parent)$/.test(l))).toBe(true);
    expect(parents.every((p) => p.person)).toBe(true);
  });
});
