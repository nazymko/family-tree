import { describe, expect, it } from 'vitest';
import type { Id } from '../types';
import { fixtureTree } from './fixtures';
import { MockApi } from './MockApi';

const BIG = { x: -1e7, y: -1e7, w: 2e7, h: 2e7 };

function setup() {
  const fx = fixtureTree(200);
  const api = new MockApi(fx);
  const real = (id: Id | null): id is Id => id !== null && fx.persons.some((p) => p.id === id && !p.siblingMarker);
  const parentsOf = (id: Id): Id[] =>
    fx.families.filter((f) => f.children.some((c) => c.person === id)).flatMap((f) => [f.partner1, f.partner2].filter(real));
  const partnersOf = (id: Id): Id[] => fx.families.filter((f) => f.partner1 === id || f.partner2 === id).flatMap((f) => [f.partner1, f.partner2].filter(real).filter((p) => p !== id));
  const childrenOf = (id: Id): Id[] => fx.families.filter((f) => f.partner1 === id || f.partner2 === id).flatMap((f) => f.children.map((c) => c.person));
  const rich = fx.persons.find((p) => !p.siblingMarker && parentsOf(p.id).length > 0 && partnersOf(p.id).length > 0 && childrenOf(p.id).length > 0)!;
  return { fx, api, rich };
}

async function nodes(api: MockApi, chartId: number) {
  return (await api.chartWindow(chartId, BIG, 1)).nodes;
}

describe('mock chartOpenArranged', () => {
  it('draws_the_same_people_once_each_and_leaves_the_plain_chart_alone', async () => {
    const s = setup();
    const plain = await s.api.chartOpenWhole();
    const a = await s.api.chartOpenArranged(s.rich.id);
    expect(a.meta.chartId).not.toBe(plain.meta.chartId);
    expect(a.meta.nodeCount).toBe(plain.meta.nodeCount);
    const ids = (await nodes(s.api, a.meta.chartId)).map((n) => n.person!.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(new Set(ids)).toEqual(new Set((await nodes(s.api, plain.meta.chartId)).map((n) => n.person!.id)));
    expect(await s.api.chartOpenWhole()).toEqual(plain);
    expect(await s.api.chartOpenArranged(s.rich.id)).toEqual(a);
  });

  it('holds_one_arranged_chart_and_the_chosen_people_chart_stays', async () => {
    const s = setup();
    const sel = await s.api.chartOpenSelection([s.rich.id], false);
    const a = await s.api.chartOpenArranged(s.rich.id, [s.rich.id], false);
    expect(a.chosen).toBe(1);
    expect(a.meta.nodeCount).toBe(sel.meta.nodeCount);
    expect(await s.api.chartOpenSelection([s.rich.id], false)).toEqual(sel);
    const b = await s.api.chartOpenArranged(s.rich.id);
    expect(b.meta.chartId).not.toBe(a.meta.chartId);
    await expect(s.api.chartWindow(a.meta.chartId, BIG, 1)).rejects.toBeTruthy();
  });

  it('puts_the_person_with_their_family_in_the_first_block_and_says_not_found_for_nobody', async () => {
    const s = setup();
    const a = await s.api.chartOpenArranged(s.rich.id);
    const ns = await nodes(s.api, a.meta.chartId);
    const mine = ns.find((n) => n.person!.id === s.rich.id)!;
    const first = a.blocks[0];
    expect(mine.y).toBeGreaterThanOrEqual(first.rect.y);
    expect(mine.y).toBeLessThanOrEqual(first.rect.y + first.rect.h + first.bodyH);
    await expect(s.api.chartOpenArranged('00000000-0000-4000-8000-000000000000')).rejects.toMatchObject({ code: 'not_found' });
    await expect(s.api.chartOpenArranged(s.rich.id, [])).rejects.toMatchObject({ code: 'not_found' });
  });
});
