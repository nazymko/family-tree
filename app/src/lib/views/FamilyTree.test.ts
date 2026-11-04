import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import FamilyView from './FamilyView.svelte';
import { apiContext } from '../api/context';
import type { Api, ChildDto, FamilyViewDto, PartnershipDto, PersonRef } from '../api/api';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { en } from '../i18n/en';
import { uk } from '../i18n/uk';
import { nav } from '../state/nav.svelte';
import { project } from '../state/project.svelte';
import { ui } from '../state/ui.svelte';

const base = new MockApi(fixtureTree(300));

let n = 0;
function ref(name: string, sex: PersonRef['sex'] = 'unknown'): PersonRef {
  n++;
  return { id: `t${n}`, name, lifespan: '1850-1921', sex, living: false, private: false };
}
const kid = (p: PersonRef): ChildDto => ({ person: p, rel: ['birth', 'birth'], birth: '' });
function union(partner: PersonRef | null, kids: ChildDto[] = []): PartnershipDto {
  n++;
  return { familyId: `f${n}`, partner, kind: 'married', summary: '', events: [], children: kids };
}
function view(focus: PersonRef, over: Partial<FamilyViewDto> = {}): FamilyViewDto {
  return { focus, focusSummary: '', parentSets: [], partnerships: [], siblings: [], ...over };
}
const sets = (a: PersonRef | null, b: PersonRef | null) => [
  {
    familyId: `fp${a?.id}${b?.id}`,
    parents: [a, b] as [PersonRef | null, PersonRef | null],
    rel: ['birth', 'birth'] as never,
    preferred: true,
    summary: null,
  },
];

function setup(views: FamilyViewDto[]) {
  const byId = new Map(views.map((v) => [v.focus.id, v]));
  const getFamilyView = vi.fn(async (id: string) => {
    const v = byId.get(id);
    if (!v) throw new Error('none');
    return structuredClone(v);
  });
  const api = Object.assign(Object.create(base), { getFamilyView }) as Api;
  project.open({ dir: 'C:/x', name: 'x', personCount: 5, homePerson: null, sourceVersion: null, locationWarning: null });
  nav.reset();
  nav.go({ name: 'family' });
  nav.focus(views[0].focus.id);
  const user = userEvent.setup();
  const r = render(FamilyView, { context: apiContext(api, new MockPlatform()) });
  return { user, getFamilyView, ...r };
}

beforeEach(() => {
  localStorage.clear();
  ui.closeAdd();
  window.matchMedia = ((q: string) => ({ matches: false, media: q, addEventListener() {}, removeEventListener() {} })) as never;
});

const tree = () => screen.findByRole('group', { name: /^Family tree of/ });
const has = (name: string | RegExp) => screen.queryByRole('button', { name });

describe('FamilyTree', () => {
  it('renders_grandparents_parents_partner_siblings_children', async () => {
    const gf = ref('Petro Koval', 'male');
    const gm = ref('Maria Koval', 'female');
    const f = ref('Ivan Koval', 'male');
    const m = ref('Olena Koval', 'female');
    const me = ref('Taras Koval', 'male');
    const sis = ref('Oksana Koval', 'female');
    const wife = ref('Halyna Koval', 'female');
    const son = ref('Andriy Koval', 'male');
    setup([
      view(me, { parentSets: sets(f, m), siblings: [sis], partnerships: [union(wife, [kid(son)])] }),
      view(f, { parentSets: sets(gf, gm) }),
      view(m),
    ]);
    await tree();
    await screen.findByRole('button', { name: 'Petro Koval, 1850-1921, Grandfather' });
    expect(has('Maria Koval, 1850-1921, Grandmother')).toBeInTheDocument();
    expect(has('Ivan Koval, 1850-1921, Father')).toBeInTheDocument();
    expect(has('Olena Koval, 1850-1921, Mother')).toBeInTheDocument();
    expect(has('Oksana Koval, 1850-1921, Sister')).toBeInTheDocument();
    expect(has('Halyna Koval, 1850-1921, Partner')).toBeInTheDocument();
    expect(has('Andriy Koval, 1850-1921, Son')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Taras Koval, 1850-1921' })).toHaveAttribute('aria-current', 'true');
  });

  it('empty_slots_appear_only_where_data_is_missing', async () => {
    const f = ref('Ivan Koval', 'male');
    const me = ref('Taras Koval', 'male');
    setup([view(me, { parentSets: sets(f, null) }), view(f)]);
    await tree();
    expect(has('Add mother of Taras Koval')).toBeInTheDocument();
    expect(has('Add father of Taras Koval')).not.toBeInTheDocument();
    expect(await screen.findByRole('button', { name: 'Add father of Ivan Koval' })).toBeInTheDocument();
    expect(has('Add mother of Ivan Koval')).toBeInTheDocument();
    expect(has('Add partner of Taras Koval')).toBeInTheDocument();
    expect(has('Add child of Taras Koval')).toBeInTheDocument();
    expect(has('Add brother or sister of Taras Koval')).toBeInTheDocument();
  });

  it('no_parent_means_no_sibling_or_grandparent_slots', async () => {
    const me = ref('Taras Koval');
    setup([view(me)]);
    await tree();
    expect(has('Add father of Taras Koval')).toBeInTheDocument();
    expect(has('Add mother of Taras Koval')).toBeInTheDocument();
    expect(has('Add brother or sister of Taras Koval')).not.toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: /^Add (father|mother) of/ })).toHaveLength(2);
  });

  it('slot_click_calls_requestAdd_with_relation_and_anchor_including_grandparents', async () => {
    const f = ref('Ivan Koval', 'male');
    const me = ref('Taras Koval', 'male');
    const { user } = setup([view(me, { parentSets: sets(f, null) }), view(f)]);
    await tree();
    const spy = vi.spyOn(ui, 'requestAdd');
    await user.click(screen.getByRole('button', { name: 'Add mother of Taras Koval' }));
    expect(spy).toHaveBeenLastCalledWith('mother', me.id);
    await user.click(await screen.findByRole('button', { name: 'Add father of Ivan Koval' }));
    expect(spy).toHaveBeenLastCalledWith('father', f.id);
    await user.click(screen.getByRole('button', { name: 'Add child of Taras Koval' }));
    expect(spy).toHaveBeenLastCalledWith('child', me.id);
    await user.click(screen.getByRole('button', { name: 'Add partner of Taras Koval' }));
    expect(spy).toHaveBeenLastCalledWith('partner', me.id);
    await user.click(screen.getByRole('button', { name: 'Add brother or sister of Taras Koval' }));
    expect(spy).toHaveBeenLastCalledWith('sibling', me.id);
  });

  it('box_click_and_enter_focus_the_person', async () => {
    const f = ref('Ivan Koval', 'male');
    const me = ref('Taras Koval', 'male');
    const { user } = setup([view(me, { parentSets: sets(f, null) }), view(f)]);
    await tree();
    await user.click(screen.getByRole('button', { name: /^Ivan Koval/ }));
    await waitFor(() => expect(nav.focusPerson).toBe(f.id));
    await screen.findByRole('heading', { level: 1, name: 'Ivan Koval' });
    nav.back();
    await screen.findByRole('heading', { level: 1, name: 'Taras Koval' });
    screen.getByRole('button', { name: /^Ivan Koval/ }).focus();
    await user.keyboard('{Enter}');
    await waitFor(() => expect(nav.focusPerson).toBe(f.id));
  });

  it('arrow_keys_move_between_boxes', async () => {
    const f = ref('Ivan Koval', 'male');
    const m = ref('Olena Koval', 'female');
    const me = ref('Taras Koval', 'male');
    const { user } = setup([view(me, { parentSets: sets(f, m) }), view(f), view(m)]);
    await tree();
    screen.getByRole('button', { name: /^Ivan Koval/ }).focus();
    await user.keyboard('{ArrowRight}');
    expect(screen.getByRole('button', { name: /^Olena Koval/ })).toHaveFocus();
    await user.keyboard('{ArrowDown}');
    expect(document.activeElement?.getAttribute('data-row')).toBe('2');
  });

  it('collapses_many_siblings_and_children_into_plus_n_more', async () => {
    const me = ref('Taras Koval');
    const f = ref('Ivan Koval', 'male');
    const sibs = Array.from({ length: 7 }, (_, i) => ref(`Sib${i}`));
    const kids = Array.from({ length: 9 }, (_, i) => ref(`Kid${i}`));
    const { user } = setup([
      view(me, { parentSets: sets(f, null), siblings: sibs, partnerships: [union(null, kids.map(kid))] }),
      view(f),
    ]);
    await tree();
    expect(has(/^Sib3/)).toBeInTheDocument();
    expect(has(/^Sib4/)).not.toBeInTheDocument();
    expect(has(/^Kid5/)).toBeInTheDocument();
    expect(has(/^Kid6/)).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Show 3 more brothers and sisters' }));
    expect(has(/^Sib6/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Show 3 more children' })).toHaveTextContent('+3 more');
    await user.click(screen.getByRole('button', { name: 'Show 3 more children' }));
    expect(has(/^Kid8/)).toBeInTheDocument();
  });

  it('toggle_is_remembered_and_cards_stay_available', async () => {
    const me = ref('Taras Koval');
    const { user, unmount } = setup([view(me)]);
    await tree();
    expect(screen.getByRole('button', { name: 'Tree' })).toHaveAttribute('aria-pressed', 'true');
    await user.click(screen.getByRole('button', { name: 'Cards' }));
    expect(screen.getByRole('region', { name: 'Parents' })).toBeInTheDocument();
    expect(screen.queryByRole('group', { name: /^Family tree of/ })).not.toBeInTheDocument();
    expect(localStorage.getItem('gt.familyMode')).toBe('cards');
    unmount();
    setup([view(me)]);
    await screen.findByRole('region', { name: 'Parents' });
    expect(screen.getByRole('button', { name: 'Cards' })).toHaveAttribute('aria-pressed', 'true');
  });

  it('tree_reloads_after_a_revision_bump', async () => {
    const me = ref('Taras Koval');
    const v = view(me);
    setup([v]);
    await tree();
    expect(has('Add partner of Taras Koval')).toBeInTheDocument();
    v.partnerships = [union(ref('Halyna Koval', 'female'))];
    project.rev++;
    await screen.findByRole('button', { name: /^Halyna Koval/ });
  });

  it('no_grandparent_slots_when_the_lookup_fails_or_is_pending', async () => {
    const f = ref('Ivan Koval', 'male');
    const me = ref('Taras Koval', 'male');
    // The father has no family view, so his lookup throws.
    setup([view(me, { parentSets: sets(f, null) })]);
    await tree();
    expect(has('Add father of Ivan Koval')).not.toBeInTheDocument();
    await new Promise((r) => setTimeout(r, 20));
    expect(has('Add father of Ivan Koval')).not.toBeInTheDocument();
    expect(has('Add mother of Ivan Koval')).not.toBeInTheDocument();
  });

  it('places_parents_by_sex_whatever_the_stored_order', async () => {
    const m = ref('Olena Koval', 'female');
    const me = ref('Taras Koval', 'male');
    setup([view(me, { parentSets: sets(m, null) }), view(m)]);
    await tree();
    expect(has('Add father of Taras Koval')).toBeInTheDocument();
    expect(has('Add mother of Taras Koval')).not.toBeInTheDocument();
    const boxes = screen.getAllByRole('button').filter((b) => b.getAttribute('data-row') === '1');
    expect(boxes[0]).toHaveAccessibleName('Add father of Taras Koval');
    expect(boxes[1]).toHaveAccessibleName(/^Olena Koval/);
  });

  it('unknown_sex_parent_takes_the_free_place', async () => {
    const f = ref('Ivan Koval', 'male');
    const x = ref('Sasha Koval');
    const me = ref('Taras Koval', 'male');
    setup([view(me, { parentSets: sets(x, f) }), view(f), view(x)]);
    await tree();
    expect(has('Sasha Koval, 1850-1921, Mother')).toBeInTheDocument();
    expect(has('Add father of Taras Koval')).not.toBeInTheDocument();
    expect(has('Add mother of Taras Koval')).not.toBeInTheDocument();
  });

  it('focus_box_is_not_announced_as_a_child', async () => {
    setup([view(ref('Taras Koval'))]);
    await tree();
    expect(screen.getByRole('button', { name: 'Taras Koval, 1850-1921' })).toHaveAttribute('aria-current', 'true');
  });

  it('keeps_a_tab_stop_after_more_is_used', async () => {
    const me = ref('Taras Koval');
    const sibs = Array.from({ length: 7 }, (_, i) => ref(`Sib${i}`));
    const { user } = setup([view(me, { siblings: sibs })]);
    await tree();
    await user.click(screen.getByRole('button', { name: 'Show 3 more brothers and sisters' }));
    await waitFor(() => expect(document.querySelectorAll('[data-nav][tabindex="0"]')).toHaveLength(1));
  });

  it('marks_half_siblings_and_offers_another_set_of_parents', async () => {
    const f = ref('Ivan Koval', 'male');
    const m = ref('Olena Koval', 'female');
    const f2 = ref('Bohdan Koval', 'male');
    const me = ref('Taras Koval', 'male');
    const full = ref('Oksana Koval', 'female');
    const half = ref('Dmytro Koval', 'male');
    const parentSets = [
      { ...sets(f, m)[0], familyId: 'fa', preferred: true },
      { ...sets(f2, null)[0], familyId: 'fb', preferred: false },
    ];
    const getFamily = vi.fn(async (id: string) => ({
      id,
      partners: [null, null],
      kind: 'married',
      summary: '',
      events: [],
      children: [me, full].map((p) => ({ preferred: true, person: p, rel: ['birth', 'birth'], birth: '' })),
    }));
    const byId = new Map([[me.id, view(me, { parentSets, siblings: [full, half] })], [f.id, view(f)], [m.id, view(m)], [f2.id, view(f2)]]);
    const api = Object.assign(Object.create(base), {
      getFamilyView: vi.fn(async (id: string) => structuredClone(byId.get(id)!)),
      getFamily,
    }) as Api;
    project.open({ dir: 'C:/x', name: 'x', personCount: 5, homePerson: null, sourceVersion: null, locationWarning: null });
    nav.reset();
    nav.go({ name: 'family' });
    nav.focus(me.id);
    const user = userEvent.setup();
    render(FamilyView, { context: apiContext(api, new MockPlatform()) });
    await tree();
    await screen.findByRole('button', { name: 'Dmytro Koval, 1850-1921, Half-brother' });
    expect(has('Oksana Koval, 1850-1921, Sister')).toBeInTheDocument();
    const spy = vi.spyOn(ui, 'requestAdd');
    await user.click(screen.getByRole('button', { name: 'Add another set of parents' }));
    expect(spy).toHaveBeenLastCalledWith('father', me.id, true);
    await user.click(screen.getByRole('button', { name: /Parents: Bohdan Koval/ }));
    expect(has('Bohdan Koval, 1850-1921, Father')).toBeInTheDocument();
    expect(has('Add mother of Taras Koval')).toBeInTheDocument();
    expect(has('Add another set of parents')).not.toBeInTheDocument();
  });

  it('person_row_reads_siblings_person_partner_then_add_partner', async () => {
    const f = ref('Ivan Koval', 'male');
    const me = ref('Taras Koval', 'male');
    const sis = ref('Oksana Koval', 'female');
    const wife = ref('Halyna Koval', 'female');
    setup([view(me, { parentSets: sets(f, null), siblings: [sis], partnerships: [union(wife)] }), view(f)]);
    await tree();
    const names = [...document.querySelectorAll('[data-row="2"]')].map((b) => b.getAttribute('aria-label'));
    expect(names).toEqual([
      'Add brother or sister of Taras Koval',
      'Oksana Koval, 1850-1921, Sister',
      'Taras Koval, 1850-1921',
      'Halyna Koval, 1850-1921, Partner',
      'Add partner of Taras Koval',
    ]);
  });

  it('grandparent_pairs_have_captions_and_other_parents_has_a_short_label', async () => {
    const gf = ref('Petro Koval', 'male');
    const gm = ref('Maria Koval', 'female');
    const f = ref('Ivan Koval', 'male');
    const m = ref('Olena Koval', 'female');
    const me = ref('Taras Koval', 'male');
    setup([view(me, { parentSets: sets(f, m) }), view(f, { parentSets: sets(gf, gm) }), view(m)]);
    await tree();
    await screen.findByText("Father's parents");
    expect(screen.getByRole('group', { name: "Father's parents" })).toBeInTheDocument();
    expect(await screen.findByText("Mother's parents")).toBeInTheDocument();
    const other = screen.getByRole('button', { name: 'Add another set of parents' });
    expect(other).toHaveTextContent('Other parents');
    expect(other).toHaveAttribute('title', 'Add another set of parents');
  });

  it('en_and_uk_have_the_same_tree_keys', () => {
    const keys = Object.keys(en.family).filter((k) => /^(tree|mode|viewMode|add\w+Of$)/.test(k));
    expect(keys.length).toBeGreaterThanOrEqual(13);
    for (const k of keys) expect((uk.family as Record<string, unknown>)[k], k).toBeTruthy();
  });
});
