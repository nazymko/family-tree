import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import FamilyView from './FamilyView.svelte';
import Settings from './Settings.svelte';
import FlipTreeButton from '../ui/FlipTreeButton.svelte';
import { apiContext } from '../api/context';
import type { Api, FamilyViewDto, PersonRef } from '../api/api';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { renderWith } from '../testing/renderWith';
import { settings } from '../state/settings.svelte';
import { nav } from '../state/nav.svelte';
import { project } from '../state/project.svelte';
import { en } from '../i18n/en';
import { uk } from '../i18n/uk';

const BIG = { x: -1e6, y: -1e6, w: 2e6, h: 2e6 };

afterEach(() => {
  settings.value = { ...settings.value, treeDirection: 'normal' };
});

describe('tree direction setting', () => {
  it('is_chosen_in_settings_and_persists', async () => {
    const api = new MockApi(fixtureTree(80));
    await settings.load(api);
    expect(settings.value.treeDirection).toBe('normal');
    renderWith(Settings, {}, { api });
    const user = userEvent.setup();
    await user.selectOptions(screen.getByRole('combobox', { name: 'Tree direction' }), 'Ancestors at the bottom, like a real tree (pedigree: on the left)');
    await waitFor(() => expect(settings.value.treeDirection).toBe('reversed'));
    expect((await api.getSettings()).treeDirection).toBe('reversed');
  });

  it('flip_button_toggles_the_same_setting_and_shows_its_state', async () => {
    const api = new MockApi(fixtureTree(80));
    await settings.load(api);
    renderWith(FlipTreeButton, {}, { api });
    const user = userEvent.setup();
    const btn = screen.getByRole('button', { name: 'Flip tree' });
    expect(btn).toHaveAttribute('aria-pressed', 'false');
    await user.click(btn);
    await waitFor(() => expect(btn).toHaveAttribute('aria-pressed', 'true'));
    expect((await api.getSettings()).treeDirection).toBe('reversed');
    await user.click(btn);
    await waitFor(() => expect(btn).toHaveAttribute('aria-pressed', 'false'));
    expect((await api.getSettings()).treeDirection).toBe('normal');
  });

  it('has_english_and_ukrainian_words', () => {
    for (const k of ['treeDirection', 'treeDirectionHint', 'treeNormal', 'treeReversed'] as const) {
      expect(en.settings[k]).toBeTruthy();
      expect(uk.settings[k]).toBeTruthy();
    }
    expect(uk.tree.flip).toBe('Перевернути дерево');
    expect(en.tree.flip).toBe('Flip tree');
  });
});

describe('MockApi honours the direction', () => {
  async function chart(api: MockApi, kind: 'pedigree' | 'descendants') {
    const home = (await api.getSettings()).homePerson!;
    const meta = await api.chartOpen({ kind, root: home, generations: 3 });
    return api.chartWindow(meta.chartId, BIG, 1);
  }

  it('pedigree_parents_move_to_the_left_and_descendants_children_above', async () => {
    const api = new MockApi(fixtureTree(300));
    const normal = await chart(api, 'pedigree');
    const root = normal.nodes.find((n) => n.ahnen === 1)!;
    const father = normal.nodes.find((n) => n.ahnen === 2)!;
    expect(father.x).toBeGreaterThan(root.x);
    await api.setSettings({ treeDirection: 'reversed' });
    const rev = await chart(api, 'pedigree');
    const r2 = rev.nodes.find((n) => n.ahnen === 1)!;
    const f2 = rev.nodes.find((n) => n.ahnen === 2)!;
    expect(f2.x).toBeLessThan(r2.x);
    expect(r2.y).toBe(root.y);
    // Edges follow their boxes: a parent edge starts at the near side of the root-side box.
    for (const e of rev.edges.filter((x) => x.kind === 'parent')) {
      const a = rev.nodes.find((n) => n.id === e.from)!;
      expect(e.pts[0]).toBe(a.x);
    }
    await api.setSettings({ treeDirection: 'normal' });
    const d = await chart(api, 'descendants');
    const top = d.nodes.find((n) => n.id === 0)!;
    const kid = d.nodes.find((n) => n.nav.parent === 0);
    if (kid) expect(kid.y).toBeGreaterThan(top.y);
    await api.setSettings({ treeDirection: 'reversed' });
    const dr = await chart(api, 'descendants');
    const top2 = dr.nodes.find((n) => n.id === 0)!;
    const kid2 = dr.nodes.find((n) => n.nav.parent === 0);
    if (kid2) expect(kid2.y).toBeLessThan(top2.y);
  });

  it('whole_tree_flips_each_family_but_headings_stay_above_their_people', async () => {
    const api = new MockApi(fixtureTree(300));
    const a = await api.chartOpenWhole();
    const wa = await api.chartWindow(a.meta.chartId, BIG, 1);
    await api.setSettings({ treeDirection: 'reversed' });
    const b = await api.chartOpenWhole();
    expect(b.meta.chartId).not.toBe(a.meta.chartId);
    expect(b.blocks.map((x) => x.rect)).toEqual(a.blocks.map((x) => x.rect));
    const win = await api.chartWindow(b.meta.chartId, BIG, 1);
    const first = b.blocks[0];
    expect(first.bodyH).toBeGreaterThan(0);
    const inFirst = win.nodes.filter((n) => n.x >= first.rect.x && n.x < first.rect.x + first.rect.w && n.y >= first.rect.y);
    for (const n of inFirst) expect(n.y).toBeGreaterThanOrEqual(first.rect.y + first.rect.h);
    // A parent is above its child normally and below it reversed.
    const pair = wa.nodes.find((n) => n.nav.parent !== null)!;
    const yOf = (w: typeof wa, id: number) => w.nodes.find((n) => n.id === id)!.y;
    expect(yOf(wa, pair.nav.parent!)).toBeLessThan(yOf(wa, pair.id));
    expect(yOf(win, pair.nav.parent!)).toBeGreaterThan(yOf(win, pair.id));
    // The same chart is served again while the direction does not change.
    expect((await api.chartOpenWhole()).meta.chartId).toBe(b.meta.chartId);
  });
});

describe('FamilyTree direction', () => {
  const ref = (id: string, name: string, sex: PersonRef['sex']): PersonRef => ({ id, name, lifespan: '1850-1921', sex, living: false, private: false });
  const dad = ref('d', 'Ivan Koval', 'male');
  const mum = ref('m', 'Olena Koval', 'female');
  const me = ref('me', 'Taras Koval', 'male');
  const son = ref('s', 'Andriy Koval', 'male');
  const views: Record<string, FamilyViewDto> = {
    me: {
      focus: me,
      focusSummary: '',
      parentSets: [{ familyId: 'fp', parents: [dad, mum], rel: ['birth', 'birth'] as never, preferred: true, summary: null }],
      partnerships: [{ familyId: 'fu', partner: null, kind: 'married', summary: '', events: [], children: [{ person: son, rel: ['birth', 'birth'], birth: '' }] }],
      siblings: [],
    },
    d: { focus: dad, focusSummary: '', parentSets: [], partnerships: [], siblings: [] },
    m: { focus: mum, focusSummary: '', parentSets: [], partnerships: [], siblings: [] },
  };
  const base = new MockApi(fixtureTree(80));

  beforeEach(() => {
    localStorage.clear();
    window.matchMedia = ((q: string) => ({ matches: false, media: q, addEventListener() {}, removeEventListener() {} })) as never;
    project.open({ dir: 'C:/x', name: 'x', personCount: 5, homePerson: null, sourceVersion: null, locationWarning: null });
    nav.reset();
    nav.go({ name: 'family' });
    nav.focus('me');
  });

  function mount() {
    const getFamilyView = vi.fn(async (id: string) => structuredClone(views[id]));
    const api = Object.assign(Object.create(base), { getFamilyView }) as Api;
    return render(FamilyView, { context: apiContext(api, new MockPlatform()) });
  }

  it('reversed_marks_the_tree_and_the_button_flips_it', async () => {
    const user = userEvent.setup();
    mount();
    const tree = await screen.findByRole('group', { name: /^Family tree of/ });
    expect(tree).toHaveAttribute('data-direction', 'normal');
    expect(tree).not.toHaveClass('reversed');
    const btn = screen.getByRole('button', { name: 'Flip tree' });
    await user.click(btn);
    await waitFor(() => expect(tree).toHaveAttribute('data-direction', 'reversed'));
    expect(tree).toHaveClass('reversed');
    expect(btn).toHaveAttribute('aria-pressed', 'true');
    // The DOM (and tab) order is unchanged: parents still come before the children.
    const dadBox = screen.getByRole('button', { name: /^Ivan Koval/ });
    const sonBox = screen.getByRole('button', { name: /^Andriy Koval/ });
    expect(dadBox.compareDocumentPosition(sonBox) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('arrow_up_goes_to_the_row_that_is_visually_above', async () => {
    const user = userEvent.setup();
    settings.value = { ...settings.value, treeDirection: 'reversed' };
    mount();
    await screen.findByRole('group', { name: /^Family tree of/ });
    const self = await screen.findByRole('button', { name: 'Taras Koval, 1850-1921' });
    self.focus();
    await user.keyboard('{ArrowUp}');
    // Reversed: the children are above the person.
    await waitFor(() => expect(screen.getByRole('button', { name: /^Andriy Koval/ })).toHaveFocus());
  });
});
