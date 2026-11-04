import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/svelte';
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

function setup(views: FamilyViewDto[], over: Partial<Api> = {}) {
  const byId = new Map(views.map((v) => [v.focus.id, v]));
  const getFamilyView = vi.fn(async (id: string) => {
    const v = byId.get(id);
    if (!v) throw new Error('none');
    return structuredClone(v);
  });
  const api = Object.assign(Object.create(base), { getFamilyView }, over) as Api;
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
    expect(has('Halyna Koval, 1850-1921, Wife')).toBeInTheDocument();
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
      'Halyna Koval, 1850-1921, Wife',
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

  it('each_box_shows_its_relation_to_the_person_in_the_middle', async () => {
    const f = ref('Ivan Koval', 'male');
    const me = ref('Taras Koval', 'male');
    const sis = ref('Oksana Koval', 'female');
    const wife = ref('Halyna Koval', 'female');
    const partner = ref('Roman Shevchuk', 'male');
    const unmarried = { ...union(partner), familyId: 'u2', kind: 'unmarried' as const };
    setup([view(me, { parentSets: sets(f, null), siblings: [sis], partnerships: [union(wife), unmarried] }), view(f)]);
    await tree();
    const caption = (name: string) =>
      screen.getByRole('button', { name: new RegExp(`^${name}`) }).querySelector('.kin')?.textContent;
    expect(caption('Ivan Koval')).toBe('Father');
    expect(caption('Oksana Koval')).toBe('Sister');
    expect(caption('Halyna Koval')).toBe('Wife');
    expect(caption('Roman Shevchuk')).toBe('Partner');
    expect(caption('Taras Koval')).toBeUndefined();
  });

  it('a_partner_with_no_marriage_record_still_reads_as_husband_or_wife', async () => {
    const me = ref('Olena Koval', 'female');
    const husband = ref('Petro Bondar', 'male');
    const other = ref('Sasha Melnyk', 'unknown');
    const noRecord = { ...union(husband), kind: 'unknown' as const };
    const unknownSex = { ...union(other), kind: 'unknown' as const };
    setup([view(me, { partnerships: [noRecord, unknownSex] })]);
    await tree();
    const caption = (name: string) =>
      screen.getByRole('button', { name: new RegExp(`^${name}`) }).querySelector('.kin')?.textContent;
    expect(caption('Petro Bondar')).toBe('Husband');
    expect(caption('Sasha Melnyk')).toBe('Partner');
  });

  it('children_of_different_partners_are_captioned_with_their_other_parent', async () => {
    const me = ref('Ivan Koval', 'male');
    const first = ref('Olena Koval', 'female');
    const second = ref('Maria Bondar', 'female');
    const a = ref('Petro Koval', 'male');
    const b = ref('Anna Koval', 'female');
    const c = ref('Mykola Koval', 'male');
    setup([view(me, { partnerships: [union(first, [kid(a), kid(b)]), union(second, [kid(c)]), union(null, [])] })]);
    await tree();
    const groups = [...document.querySelectorAll('.kidsrow > .kids')];
    expect(groups.map((g) => g.querySelector('.kcap')?.textContent)).toEqual(['Mother: Olena Koval', 'Mother: Maria Bondar']);
    expect(groups[0].querySelectorAll('.kid .box:not(.slot)')).toHaveLength(2);
    expect(groups[1].querySelectorAll('.kid .box:not(.slot)')).toHaveLength(1);
  });

  it('a_stepfather_reads_as_stepfather_and_the_mother_as_mother', async () => {
    const me = ref('Taras Koval', 'male');
    const step = ref('Petro Bondar', 'male');
    const mom = ref('Olena Koval', 'female');
    const stepSet = { ...sets(step, mom)[0], rel: ['step', 'birth'] as never };
    setup([view(me, { parentSets: [stepSet] }), view(step), view(mom)]);
    await tree();
    const caption = (name: string) =>
      screen.getByRole('button', { name: new RegExp(`^${name}`) }).querySelector('.kin')?.textContent;
    expect(caption('Petro Bondar')).toBe('Step father');
    expect(caption('Olena Koval')).toBe('Mother');
  });

  it('a_single_partner_needs_no_caption', async () => {
    const me = ref('Ivan Koval', 'male');
    setup([view(me, { partnerships: [union(ref('Olena Koval', 'female'), [kid(ref('Petro Koval', 'male'))])] })]);
    await tree();
    expect(document.querySelector('.kcap')).toBeNull();
  });
});

describe('FamilyTree detaching a wrong link', () => {
  const famDto = (id: string, partners: [PersonRef | null, PersonRef | null], kids: PersonRef[]) => ({
    id,
    partners,
    kind: 'married' as const,
    summary: null,
    events: [],
    children: kids.map((p) => ({ person: p, rel: ['birth', 'birth'] as never, preferred: true, birth: '' })),
  });

  function family() {
    const dad = ref('Petro Koval', 'male');
    const mum = ref('Maria Koval', 'female');
    const me = ref('Ivan Koval', 'male');
    const sis = ref('Anna Koval', 'female');
    const wife = ref('Olena Koval', 'female');
    const son = ref('Taras Koval', 'male');
    const ex = ref('Iryna Bondar', 'female');
    const u = union(wife, [kid(son)]);
    const empty = union(ex);
    const ps = sets(dad, mum);
    const v = view(me, { parentSets: ps, siblings: [sis], partnerships: [u, empty] });
    const fams: Record<string, ReturnType<typeof famDto>> = {
      [ps[0].familyId]: famDto(ps[0].familyId, [dad, mum], [me, sis]),
      [u.familyId]: famDto(u.familyId, [me, wife], [son]),
    };
    const apply = vi.fn(async (cmd: unknown) => void cmd as never);
    const getFamily = vi.fn(async (id: string) => structuredClone(fams[id]) as never);
    return { dad, mum, me, sis, wife, son, ex, u, empty, ps, v, apply, getFamily };
  }

  async function choose(user: ReturnType<typeof userEvent.setup>, box: RegExp, item: RegExp) {
    const b = await screen.findByRole('button', { name: box });
    await user.pointer({ keys: '[MouseRight]', target: b });
    await user.click(await screen.findByRole('menuitem', { name: item }));
    return screen.findByRole('dialog');
  }

  it('a_child_is_detached_from_the_person_and_partner_after_confirming', async () => {
    const f = family();
    const { user } = setup([f.v], { apply: f.apply, getFamily: f.getFamily });
    await tree();
    const dlg = await choose(user, /^Taras Koval/, /Not a child of Ivan Koval/);
    expect(dlg).toHaveTextContent('Taras Koval will no longer be a child of Ivan Koval and Olena Koval.');
    expect(f.apply).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Detach' }));
    await waitFor(() => expect(f.apply).toHaveBeenCalledWith({ type: 'unlinkChild', family: f.u.familyId, child: f.son.id }));
  });

  it('cancel_changes_nothing', async () => {
    const f = family();
    const { user } = setup([f.v], { apply: f.apply, getFamily: f.getFamily });
    await tree();
    await choose(user, /^Taras Koval/, /Not a child of/);
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(f.apply).not.toHaveBeenCalled();
  });

  it('the_person_is_detached_from_wrong_parents', async () => {
    const f = family();
    const { user } = setup([f.v], { apply: f.apply, getFamily: f.getFamily });
    await tree();
    await choose(user, /^Petro Koval/, /Not a child of these parents: detach Ivan Koval/);
    await user.click(screen.getByRole('button', { name: 'Detach' }));
    await waitFor(() => expect(f.apply).toHaveBeenCalledWith({ type: 'unlinkChild', family: f.ps[0].familyId, child: f.me.id }));
  });

  it('a_wrong_parent_leaves_the_couple_and_the_dialog_names_the_other_children', async () => {
    const f = family();
    const { user } = setup([f.v], { apply: f.apply, getFamily: f.getFamily });
    await tree();
    const dlg = await choose(user, /^Maria Koval/, /Not a parent: remove Maria Koval/);
    expect(dlg).toHaveTextContent('other children of this couple: Anna Koval');
    await user.click(screen.getByRole('button', { name: 'Remove' }));
    await waitFor(() =>
      expect(f.apply).toHaveBeenCalledWith({ type: 'saveFamily', id: f.ps[0].familyId, partner1: f.dad.id, partner2: null, kind: 'married' }),
    );
  });

  it('a_partner_with_children_leaves_them_with_the_person_and_one_without_children_drops_the_family', async () => {
    const f = family();
    const { user } = setup([f.v], { apply: f.apply, getFamily: f.getFamily });
    await tree();
    await choose(user, /^Olena Koval/, /Not a partner: remove Olena Koval/);
    await user.click(screen.getByRole('button', { name: 'Remove' }));
    await waitFor(() =>
      expect(f.apply).toHaveBeenCalledWith({ type: 'saveFamily', id: f.u.familyId, partner1: f.me.id, partner2: null, kind: 'married' }),
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    await choose(user, /^Iryna Bondar/, /Not a partner: remove Iryna Bondar/);
    await user.click(screen.getByRole('button', { name: 'Remove' }));
    await waitFor(() => expect(f.apply).toHaveBeenCalledWith({ type: 'deleteFamily', id: f.empty.familyId }));
  });

  it('a_sibling_can_be_detached_and_the_person_box_has_no_menu', async () => {
    const f = family();
    const { user } = setup([f.v], { apply: f.apply, getFamily: f.getFamily });
    await tree();
    await choose(user, /^Anna Koval/, /detach Anna Koval/);
    await user.click(screen.getByRole('button', { name: 'Detach' }));
    await waitFor(() => expect(f.apply).toHaveBeenCalledWith({ type: 'unlinkChild', family: f.ps[0].familyId, child: f.sis.id }));
    expect(document.querySelector('.box.current')).not.toHaveAttribute('aria-haspopup');
  });

  it('the_only_child_of_a_single_parent_takes_the_empty_family_with_it', async () => {
    const dad = ref('Petro Koval', 'male');
    const me = ref('Ivan Koval', 'male');
    const ps = sets(dad, null);
    const apply = vi.fn(async (cmd: unknown) => void cmd as never);
    const getFamily = vi.fn(async () => famDto(ps[0].familyId, [dad, null], [me]) as never);
    const { user } = setup([view(me, { parentSets: ps })], { apply, getFamily });
    await tree();
    const b = await screen.findByRole('button', { name: /^Petro Koval/ });
    await user.pointer({ keys: '[MouseRight]', target: b });
    expect(screen.queryByRole('menuitem', { name: /Not a parent/ })).toBeNull();
    await user.click(screen.getByRole('menuitem', { name: /Not a child of these parents/ }));
    await user.click(await screen.findByRole('button', { name: 'Detach' }));
    await waitFor(() => expect(apply).toHaveBeenCalledWith({ type: 'deleteFamily', id: ps[0].familyId }));
  });

  it('the_menu_also_opens_from_the_keyboard_and_escape_returns_focus', async () => {
    const f = family();
    const { user } = setup([f.v], { apply: f.apply, getFamily: f.getFamily });
    await tree();
    const b = await screen.findByRole('button', { name: /^Taras Koval/ });
    b.focus();
    b.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    expect(await screen.findByRole('menu', { name: 'Actions for Taras Koval' })).toBeInTheDocument();
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('menu')).toBeNull();
    expect(document.activeElement).toBe(b);
  });
});

describe('FamilyTree couple end marks', () => {
  const endEvent = (kind: string, year: string, roles: [PersonRef, string][]) => ({
    id: 'ev1',
    kind,
    label: kind,
    date: { raw: year, display: year },
    description: null,
    cause: null,
    negative: false,
    private: false,
    preferred: true,
    participants: roles.map(([person, role]) => ({ person, role })),
  });

  it('a_died_partner_and_the_one_who_raised_the_children_alone_are_marked', async () => {
    const me = ref('Ivan Koval', 'male');
    const wife = ref('Olena Koval', 'female');
    const son = ref('Taras Koval', 'male');
    const u = union(wife, [kid(son)]);
    u.events = [endEvent('custom:widowed', '1944', [[wife, 'left_family'], [me, 'raised_alone']])] as never;
    setup([view(me, { partnerships: [u] })]);
    await tree();
    expect(await screen.findByRole('button', { name: /^Olena Koval, .*died 1944/ })).toBeInTheDocument();
    expect(screen.getByText('Raised alone by Ivan Koval')).toBeInTheDocument();
    expect(document.querySelector('.join.ended')).not.toBeNull();
  });

  it('the_parents_divorce_is_shown_on_both_parent_boxes', async () => {
    const dad = ref('Petro Koval', 'male');
    const mum = ref('Maria Koval', 'female');
    const me = ref('Ivan Koval', 'male');
    const ps = sets(dad, mum);
    const getFamily = vi.fn(async (id: string) => ({
      id,
      partners: [dad, mum],
      kind: 'married',
      summary: null,
      events: [endEvent('divorce', '1935', [[dad, 'left_family'], [mum, 'raised_alone']])],
      children: [],
    }) as never);
    setup([view(me, { parentSets: ps })], { getFamily });
    await tree();
    expect(await screen.findByRole('button', { name: /^Petro Koval, .*divorced 1935/ })).toBeInTheDocument();
    expect(await screen.findByRole('button', { name: /^Maria Koval, .*raised the children alone/ })).toBeInTheDocument();
  });

  it('the_menu_records_who_left_and_who_raised_the_children', async () => {
    const me = ref('Ivan Koval', 'male');
    const wife = ref('Olena Koval', 'female');
    const u = union(wife, [kid(ref('Taras Koval', 'male'))]);
    const apply = vi.fn(async (cmd: unknown) => void cmd as never);
    const { user } = setup([view(me, { partnerships: [u] })], { apply });
    await tree();
    const b = await screen.findByRole('button', { name: /^Olena Koval/ });
    await user.pointer({ keys: '[MouseRight]', target: b });
    await user.click(await screen.findByRole('menuitem', { name: /How did this couple end/ }));
    const dlg = await screen.findByRole('dialog');
    await user.click(within(dlg).getByLabelText('Divorce'));
    await user.click(within(dlg).getByLabelText('Ivan Koval'));
    await user.click(within(dlg).getByLabelText('Olena Koval raised the children alone afterwards'));
    await user.click(within(dlg).getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(apply).toHaveBeenCalledWith(
        expect.objectContaining({
          type: 'saveEvent',
          owner: { kind: 'family', id: u.familyId },
          event: expect.objectContaining({
            kind: 'divorce',
            participants: [
              { person: me.id, role: 'left_family' },
              { person: wife.id, role: 'raised_alone' },
            ],
          }),
        }),
      ),
    );
  });

  it('an_existing_mark_can_be_removed', async () => {
    const me = ref('Ivan Koval', 'male');
    const wife = ref('Olena Koval', 'female');
    const u = union(wife);
    u.events = [endEvent('separation', '1950', [[me, 'left_family']])] as never;
    const apply = vi.fn(async (cmd: unknown) => void cmd as never);
    const { user } = setup([view(me, { partnerships: [u] })], { apply });
    await tree();
    const b = await screen.findByRole('button', { name: /^Olena Koval, .*separated 1950/ });
    await user.pointer({ keys: '[MouseRight]', target: b });
    await user.click(await screen.findByRole('menuitem', { name: /Edit how this couple ended/ }));
    await user.click(await screen.findByRole('button', { name: 'Remove the mark' }));
    await waitFor(() => expect(apply).toHaveBeenCalledWith({ type: 'deleteEvent', owner: { kind: 'family', id: u.familyId }, event: 'ev1' }));
  });
});
