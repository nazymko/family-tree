import { describe, expect, it } from 'vitest';
import { MockApi } from './MockApi';
import { fixtureTree } from './fixtures';
import { kinshipText } from '../../family/kinship';

const all = { x: -1e6, y: -1e6, w: 2e6, h: 2e6 };

describe('MockApi kinship', () => {
  it('labels_a_pedigree_against_its_starting_person', async () => {
    const api = new MockApi(fixtureTree(500));
    const home = (await api.getSettings()).homePerson!;
    const parents = (await api.getFamilyView(home)).parentSets[0].parents;
    const meta = await api.chartOpen({ kind: 'pedigree', root: home, generations: 3 });
    const win = await api.chartWindow(meta.chartId, all, 1);
    const byId = new Map(win.nodes.map((n) => [n.person?.id, n]));
    expect(byId.get(home)?.kinship).toBeUndefined();
    for (const p of parents) {
      const k = byId.get(p!.id)?.kinship;
      expect(k).toMatchObject({ kind: 'blood', up: 1, down: 0 });
      expect(kinshipText(k)).toBe(p!.sex === 'male' ? 'father' : 'mother');
    }
    const far = await api.chartWindow(meta.chartId, all, 0.1);
    expect(far.nodes.every((n) => n.kinship === undefined)).toBe(true);
    const outline = await api.chartOutline(meta.chartId, 0, 10);
    expect(outline.some((i) => i.kinship?.up === 1)).toBe(true);
  });

  it('labels_the_whole_tree_only_after_a_reference_is_named', async () => {
    const api = new MockApi(fixtureTree(500));
    const home = (await api.getSettings()).homePerson!;
    const parents = (await api.getFamilyView(home)).parentSets[0].parents;
    const m = await api.chartOpenWhole();
    const id = m.meta.chartId;
    const before = await api.chartWindow(id, all, 1);
    expect(before.nodes.some((n) => n.kinship)).toBe(false);
    await api.chartSetReference(id, home);
    const after = await api.chartWindow(id, all, 1);
    const dad = after.nodes.find((n) => n.person?.id === parents[0]!.id);
    expect(dad?.kinship?.kind).toBe('blood');
    expect(after.nodes.find((n) => n.person?.id === home)?.kinship).toBeUndefined();
    await api.chartSetReference(id, null);
    expect((await api.chartWindow(id, all, 1)).nodes.some((n) => n.kinship)).toBe(false);
  });
});
