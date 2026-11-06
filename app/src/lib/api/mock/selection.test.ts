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
  /** Chosen people and every ancestor. */
  const ancestry = (ids: Id[]): Set<Id> => {
    const out = new Set<Id>(ids);
    const todo = [...ids];
    while (todo.length) for (const q of parentsOf(todo.pop()!)) if (!out.has(q)) (out.add(q), todo.push(q));
    return out;
  };
  const partnersOf = (id: Id): Id[] => fx.families.filter((f) => f.partner1 === id || f.partner2 === id).flatMap((f) => [f.partner1, f.partner2].filter(real).filter((p) => p !== id));
  const childrenOf = (id: Id): Id[] => fx.families.filter((f) => f.partner1 === id || f.partner2 === id).flatMap((f) => f.children.map((c) => c.person));
  /** A person with ancestors, a partner and children, and one more person without a link to them. */
  const rich = fx.persons.find((p) => !p.siblingMarker && parentsOf(p.id).length > 0 && partnersOf(p.id).length > 0 && childrenOf(p.id).length > 0)!;
  return { fx, api, parentsOf, ancestry, partnersOf, childrenOf, rich };
}

async function shown(api: MockApi, chartId: number): Promise<Id[]> {
  return (await api.chartWindow(chartId, BIG, 1)).nodes.map((n) => n.person!.id);
}

describe('mock chartOpenSelection', () => {
  it('shows_the_person_their_ancestors_and_partners_once_each', async () => {
    const s = setup();
    const m = await s.api.chartOpenSelection([s.rich.id], false);
    const ids = await shown(s.api, m.meta.chartId);
    expect(new Set(ids).size).toBe(ids.length);
    const want = s.ancestry([s.rich.id]);
    for (const p of s.partnersOf(s.rich.id)) want.add(p);
    expect(new Set(ids)).toEqual(want);
    expect(m.chosen).toBe(1);
    expect(m.meta.nodeCount).toBe(ids.length);
    // Their children are not on it.
    for (const c of s.childrenOf(s.rich.id)) expect(ids).not.toContain(c);
  });

  it('two_unconnected_people_make_two_groups_and_one_chart_for_connected_ones', async () => {
    const s = setup();
    const alone = s.fx.persons.find((p) => !p.siblingMarker && s.parentsOf(p.id).length === 0 && s.partnersOf(p.id).length === 0 && s.childrenOf(p.id).length === 0);
    expect(alone).toBeTruthy();
    const m = await s.api.chartOpenSelection([s.rich.id, alone!.id], false);
    expect(m.chosen).toBe(2);
    expect(m.blocks.length).toBe(2);
    expect(m.blocks.some((b) => b.unconnected)).toBe(true);
  });

  it('all_descendants_adds_the_children_and_their_partners', async () => {
    const s = setup();
    const without = await s.api.chartOpenSelection([s.rich.id], false);
    const withAll = await s.api.chartOpenSelection([s.rich.id], true);
    expect(withAll.meta.chartId).not.toBe(without.meta.chartId);
    const ids = new Set(await shown(s.api, withAll.meta.chartId));
    for (const c of s.childrenOf(s.rich.id)) expect(ids.has(c)).toBe(true);
    expect(withAll.meta.nodeCount).toBeGreaterThan(without.meta.nodeCount);
  });

  it('the_whole_tree_chart_is_kept_beside_it_and_the_same_request_is_served_again', async () => {
    const s = setup();
    const all = await s.api.chartOpenWhole();
    const a = await s.api.chartOpenSelection([s.rich.id], false);
    const b = await s.api.chartOpenSelection([s.rich.id], false);
    expect(b.meta.chartId).toBe(a.meta.chartId);
    expect(await s.api.chartOpenWhole()).toEqual(all);
    expect((await shown(s.api, all.meta.chartId)).length).toBe(all.meta.nodeCount);
    const c = await s.api.chartOpenSelection([s.rich.id], true);
    expect(c.meta.chartId).not.toBe(a.meta.chartId);
    expect((await shown(s.api, all.meta.chartId)).length).toBe(all.meta.nodeCount);
    await expect(s.api.chartLocate(a.meta.chartId, s.rich.id)).rejects.toBeTruthy();
    const loc = await s.api.chartLocate(c.meta.chartId, s.rich.id);
    expect(loc).not.toBeNull();
    expect((await s.api.chartLineage(c.meta.chartId, loc!.nodeId)).length).toBeGreaterThan(1);
  });

  it('a_deleted_person_is_dropped_and_nobody_left_is_not_found', async () => {
    const s = setup();
    const other = s.parentsOf(s.rich.id)[0];
    const before = await s.api.chartOpenSelection([s.rich.id, other], false);
    expect(before.chosen).toBe(2);
    await s.api.apply({ type: 'deletePerson', id: other });
    const after = await s.api.chartOpenSelection([s.rich.id, other], false);
    expect(after.chosen).toBe(1);
    await s.api.apply({ type: 'deletePerson', id: s.rich.id });
    await expect(s.api.chartOpenSelection([s.rich.id, other], false)).rejects.toMatchObject({ code: 'not_found' });
    await expect(s.api.chartOpenSelection([], false)).rejects.toMatchObject({ code: 'not_found' });
  });
});
