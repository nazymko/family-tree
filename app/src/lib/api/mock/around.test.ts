import { describe, expect, it } from 'vitest';
import type { ChartNode } from '../types';
import { MockApi } from './MockApi';
import { fixtureTree } from './fixtures';

const BIG = { x: -1e6, y: -1e6, w: 2e6, h: 2e6 };
const RELATIVES_OFF_THE_LINE = /brother|sister|sibling|aunt|uncle|cousin|nephew|niece/;

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

describe('mock chart built around a person (port of gt-layout around)', () => {
  const fx = fixtureTree(500);
  const api = new MockApi(fx);

  /** A person who has brothers or sisters, parents and children on the relatives chart. */
  async function withSiblings() {
    for (const p of fx.persons.slice(0, 200)) {
      const m = await api.chartOpen({ kind: 'relatives', root: p.id, generations: 3 });
      const nodes = (await api.chartWindow(m.chartId, BIG, 1)).nodes;
      await api.chartClose(m.chartId);
      const has = (re: RegExp) => nodes.some((n) => re.test(n.relation ?? ''));
      if (has(/brother|sister|sibling/) && has(/^(father|mother|parent)$/) && has(/son|daughter|child/)) return p.id;
    }
    throw new Error('the fixture has nobody with siblings, parents and children');
  }

  it('shows_the_line_only_ancestors_above_and_descendants_below_and_nothing_overlaps', async () => {
    const root = await withSiblings();
    const m = await api.chartOpen({ kind: 'around', root, generations: 255 });
    expect(m.kind).toBe('around');
    expect(m.truncated).toBe(false);
    const nodes = (await api.chartWindow(m.chartId, BIG, 1)).nodes;
    expect(overlaps(nodes)).toBe(0);
    const me = nodes[m.rootNode];
    expect(me.relation).toBe('starting person');
    for (const n of nodes) expect(n.relation ?? '', `${n.person?.name}`).not.toMatch(RELATIVES_OFF_THE_LINE);
    const above = nodes.filter((n) => /father|mother|parent|grandparent|grandfather|grandmother/.test(n.relation ?? ''));
    const below = nodes.filter((n) => /\b(son|daughter|child|grandson|granddaughter|grandchild)\b/.test(n.relation ?? ''));
    expect(above.length).toBeGreaterThan(0);
    expect(below.length).toBeGreaterThan(0);
    for (const n of above) expect(n.y).toBeLessThan(me.y);
    for (const n of below) expect(n.y).toBeGreaterThan(me.y);
    // Fewer people than the relatives chart, which also has the brothers and sisters.
    const rel = await api.chartOpen({ kind: 'relatives', root, generations: 6, auntsUncles: false });
    const six = await api.chartOpen({ kind: 'around', root, generations: 6 });
    expect(six.nodeCount).toBeLessThan(rel.nodeCount);
  });

  it('puts_the_person_over_the_middle_of_their_parents', async () => {
    const root = await withSiblings();
    const m = await api.chartOpen({ kind: 'around', root, generations: 255 });
    const win = await api.chartWindow(m.chartId, BIG, 1);
    const me = win.nodes[m.rootNode];
    const parents = win.edges.filter((e) => e.kind === 'parent' && e.to === me.id).map((e) => win.nodes[e.from]);
    expect(parents.length).toBeGreaterThan(0);
    const mid = parents.reduce((s, p) => s + p.x + p.w / 2, 0) / parents.length;
    expect(Math.abs(me.x + me.w / 2 - mid)).toBeLessThan(0.01);
  });

  it('all_relatives_adds_every_ancestors_family_without_overlap_with_the_parents_over_the_person', async () => {
    const root = await withSiblings();
    const sibs = await api.chartOpen({ kind: 'around', root, generations: 255, kin: 'siblings' });
    const m = await api.chartOpen({ kind: 'around', root, generations: 255, kin: 'all' });
    expect(m.nodeCount).toBeGreaterThanOrEqual(sibs.nodeCount);
    const win = await api.chartWindow(m.chartId, BIG, 1);
    expect(overlaps(win.nodes)).toBe(0);
    const me = win.nodes[m.rootNode];
    const parents = win.edges.filter((e) => e.kind === 'parent' && e.to === me.id).map((e) => win.nodes[e.from]);
    expect(parents.length).toBeGreaterThan(0);
    const mid = parents.reduce((s, p) => s + p.x + p.w / 2, 0) / parents.length;
    expect(Math.abs(me.x + me.w / 2 - mid)).toBeLessThan(0.01);
    // Everyone stands in their generation's row: all the person's own brothers and sisters share the person's row.
    for (const n of win.nodes.filter((x) => x.role?.kind === 'sibling')) expect(n.y).toBe(me.y);
    // Great-uncles and cousins are named by generations up and down.
    for (const n of win.nodes.filter((x) => x.role?.kind === 'kin')) expect(n.relation).toBeTruthy();
  });

  it('several_people_grow_from_the_line_to_brothers_and_sisters_to_all_relatives_without_overlap_or_repeats', async () => {
    const small = new MockApi(fixtureTree(40));
    const people = fixtureTree(40).persons.filter((p) => !p.siblingMarker).map((p) => p.id);
    let grew = false;
    for (let i = 0; i < people.length && !grew; i++) {
      const roots = [people[i], people[(i + 7) % people.length]];
      const count: number[] = [];
      for (const kin of ['line', 'siblings', 'all'] as const) {
        const m = await small.chartOpen({ kind: 'around', root: roots[0], generations: 255, kin, roots });
        expect(m.chosenNodes).toHaveLength(2);
        const win = await small.chartWindow(m.chartId, BIG, 1);
        expect(overlaps(win.nodes)).toBe(0);
        const ids = win.nodes.map((n) => n.person?.id);
        expect(new Set(ids).size).toBe(ids.length);
        count.push(m.nodeCount);
        await small.chartClose(m.chartId);
      }
      expect(count[1]).toBeGreaterThanOrEqual(count[0]);
      expect(count[2]).toBeGreaterThanOrEqual(count[1]);
      grew = count[1] > count[0] && count[2] > count[1];
    }
    expect(grew).toBe(true);
  });

  it('a_limit_marks_the_cut_and_a_missing_person_is_not_found', async () => {
    const root = await withSiblings();
    const all = await api.chartOpen({ kind: 'around', root, generations: 255 });
    const one = await api.chartOpen({ kind: 'around', root, generations: 1 });
    expect(one.nodeCount).toBeLessThanOrEqual(all.nodeCount);
    await expect(api.chartOpen({ kind: 'around', root: '00000000-0000-4000-8000-000000000000', generations: 255 })).rejects.toMatchObject({ code: 'not_found' });
  });
});
