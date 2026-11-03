import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import FamilyView from './FamilyView.svelte';
import * as flip from './flip';
import { apiContext } from '../api/context';
import type { Api, ChildDto, FamilyViewDto, ParentRelId, PartnershipDto, PersonRef } from '../api/api';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { nav } from '../state/nav.svelte';
import { project } from '../state/project.svelte';
import { ui } from '../state/ui.svelte';

vi.mock('./flip', async (orig) => {
  const actual = await orig<typeof import('./flip')>();
  return { ...actual, flipMove: vi.fn(actual.flipMove) };
});

const base = new MockApi(fixtureTree(300));

let n = 0;
function ref(name: string, over: Partial<PersonRef> = {}): PersonRef {
  n++;
  return { id: `p${n}`, name, lifespan: '1850-1921', sex: 'unknown', living: false, private: false, ...over };
}
function child(p: PersonRef, rel: [ParentRelId, ParentRelId] = ['birth', 'birth']): ChildDto {
  return { person: p, rel, birth: '1 Jan 1880' };
}
function union(partner: PersonRef | null, kids: ChildDto[] = [], summary: string | null = 'm. 1878, Hamburg'): PartnershipDto {
  n++;
  return { familyId: `f${n}`, partner, kind: 'married', summary, events: [], children: kids };
}
function view(focus: PersonRef, over: Partial<FamilyViewDto> = {}): FamilyViewDto {
  return { focus, focusSummary: 'b. Hamburg, farmer', parentSets: [], partnerships: [], siblings: [], ...over };
}

function setup(views: FamilyViewDto[], start?: string, extra: Partial<Api> = {}) {
  const byId = new Map(views.map((v) => [v.focus.id, v]));
  const getFamilyView = vi.fn(async (id: string) => {
    const v = byId.get(id);
    if (!v) throw new Error(`No person ${id}`);
    return structuredClone(v);
  });
  const api = Object.assign(Object.create(base), { getFamilyView }, extra) as Api;
  project.open({ dir: 'C:/x', name: 'x', personCount: 5, homePerson: null, sourceVersion: null, locationWarning: null });
  nav.reset();
  nav.go({ name: 'family' });
  nav.focus(start ?? views[0]?.focus.id ?? null);
  const user = userEvent.setup();
  render(FamilyView, { context: apiContext(api, new MockPlatform()) });
  return { api, user, getFamilyView };
}

beforeEach(() => {
  vi.mocked(flip.flipMove).mockClear();
  window.matchMedia = ((q: string) => ({ matches: false, media: q, addEventListener() {}, removeEventListener() {} })) as never;
});

const region = (name: string) => screen.getByRole('region', { name });

describe('FamilyView', () => {
  it('shows_parents_partners_children', async () => {
    const f = ref('Hans Schmidt', { sex: 'male' });
    const m = ref('Anna Weber', { sex: 'female' });
    const me = ref('Karl Schmidt', { sex: 'male' });
    const wife = ref('Emma Koch', { sex: 'female' });
    const kids = [ref('A One'), ref('B Two'), ref('C Three')];
    setup([
      view(me, {
        parentSets: [{ familyId: 'fp', parents: [f, m], rel: ['birth', 'birth'], preferred: true, summary: null }],
        partnerships: [union(wife, kids.map((k) => child(k)))],
      }),
    ]);
    await screen.findByRole('heading', { level: 1, name: 'Karl Schmidt' });
    expect(within(region('Parents')).getAllByRole('button')).toHaveLength(2);
    expect(within(region('Partners')).getAllByRole('button')).toHaveLength(1);
    expect(within(region('Children')).getAllByRole('listitem')).toHaveLength(3);
    expect(within(region('Children')).getAllByRole('button')).toHaveLength(3);
  });

  it('card_accessible_name_has_name_lifespan_relationship', async () => {
    const f = ref('Hans Schmidt', { sex: 'male', lifespan: '1820-1890' });
    setup([
      view(ref('Karl'), {
        parentSets: [{ familyId: 'fp', parents: [f, null], rel: ['birth', 'birth'], preferred: true, summary: null }],
      }),
    ]);
    expect(await screen.findByRole('button', { name: 'Hans Schmidt, 1820-1890, Father' })).toBeInTheDocument();
  });

  it('unknown_parent_slot_shows_add_prompt', async () => {
    const ui0 = ui.add;
    const { user } = setup([view(ref('Karl'))]);
    expect(await screen.findByText('Father not recorded')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Add father' }));
    expect(ui.add).toMatchObject({ open: true, relation: 'father' });
    ui.closeAdd();
    void ui0;
  });

  it('click_card_refocuses_and_loads', async () => {
    const kid = ref('Greta Schmidt', { sex: 'female' });
    const me = ref('Karl Schmidt');
    const { user, getFamilyView } = setup([view(me, { partnerships: [union(null, [child(kid)])] }), view(kid)]);
    await screen.findByRole('heading', { level: 1, name: 'Karl Schmidt' });
    await user.click(screen.getByRole('button', { name: /Greta Schmidt, 1850-1921, Daughter/ }));
    await screen.findByRole('heading', { level: 1, name: 'Greta Schmidt' });
    expect(getFamilyView).toHaveBeenLastCalledWith(kid.id);
    expect(nav.focusPerson).toBe(kid.id);
    expect(flip.flipMove).toHaveBeenCalledTimes(1);
    expect(vi.mocked(flip.flipMove).mock.calls[0][2]).toBe(260);
    expect(vi.mocked(flip.flipMove).mock.calls[0][3]).toBe(false);
  });

  it('refocus_back_returns_to_previous_person', async () => {
    const kid = ref('Greta Schmidt', { sex: 'female' });
    const me = ref('Karl Schmidt');
    const { user } = setup([view(me, { partnerships: [union(null, [child(kid)])] }), view(kid)]);
    await screen.findByRole('heading', { level: 1, name: 'Karl Schmidt' });
    await user.click(screen.getByRole('button', { name: /Greta Schmidt, 1850-1921, Daughter/ }));
    await screen.findByRole('heading', { level: 1, name: 'Greta Schmidt' });
    expect(nav.canBack).toBe(true);
    nav.back();
    await screen.findByRole('heading', { level: 1, name: 'Karl Schmidt' });
    expect(nav.focusPerson).toBe(me.id);
  });

  it('reduced_motion_skips_flip', async () => {
    window.matchMedia = ((q: string) => ({ matches: true, media: q, addEventListener() {}, removeEventListener() {} })) as never;
    const animate = vi.fn();
    (HTMLElement.prototype as unknown as { animate: unknown }).animate = animate;
    const kid = ref('Greta');
    const { user } = setup([view(ref('Karl'), { partnerships: [union(null, [child(kid)])] }), view(kid)]);
    await screen.findByRole('heading', { level: 1, name: 'Karl' });
    await user.click(screen.getByRole('button', { name: /Greta/ }));
    await screen.findByRole('heading', { level: 1, name: 'Greta' });
    expect(flip.flipMove).toHaveBeenCalledTimes(1);
    expect(vi.mocked(flip.flipMove).mock.calls[0][3]).toBe(true);
    expect(animate).not.toHaveBeenCalled();
    delete (HTMLElement.prototype as unknown as { animate?: unknown }).animate;
  });

  it('many_partners_and_children_do_not_break', async () => {
    const unions = Array.from({ length: 12 }, (_, i) =>
      union(ref(`Partner ${i}`), i === 0 ? Array.from({ length: 40 }, (_, k) => child(ref(`Kid ${k}`))) : []),
    );
    const { user } = setup([view(ref('Casanova'), { partnerships: unions })]);
    const list = await screen.findByRole('listbox', { name: 'Choose a partner' });
    expect(within(list).getAllByRole('option')).toHaveLength(12);
    expect(within(region('Children')).getAllByRole('listitem')).toHaveLength(40);
    expect(document.querySelector('.family')!.getAttribute('style')).toBeNull();
    await user.click(within(list).getAllByRole('option')[1]);
    expect(await screen.findByText('No children recorded.')).toBeInTheDocument();
    expect(within(list).getAllByRole('option')[1]).toHaveAttribute('aria-selected', 'true');
  });

  it('cycle_fixture_renders', async () => {
    const cyc = new MockApi(fixtureTree(300));
    const special = (fixtureTree(300).specials.cycleA)!;
    project.open({ dir: 'C:/x', name: 'x', personCount: 300, homePerson: null, sourceVersion: null, locationWarning: null });
    nav.reset();
    nav.go({ name: 'family' });
    nav.focus(special);
    render(FamilyView, { context: apiContext(cyc, new MockPlatform()) });
    expect(await screen.findByRole('heading', { level: 1 })).toBeInTheDocument();
  });

  it('rtl_name_in_bdi', async () => {
    setup([view(ref('יעקב כהן'))]);
    const h = await screen.findByRole('heading', { level: 1 });
    const bdi = within(h).getByText('יעקב כהן');
    expect(bdi.tagName).toBe('BDI');
    expect(bdi).toHaveAttribute('dir', 'auto');
  });

  it('living_flag_text_not_colour', async () => {
    const p = ref('Lena Weber', { living: true, lifespan: 'b. 1990' });
    const m = ref('Marta Weber', { living: true, sex: 'female' });
    setup([view(p, { parentSets: [{ familyId: 'x', parents: [null, m], rel: ['birth', 'birth'], preferred: true, summary: null }] })]);
    await screen.findByRole('heading', { level: 1, name: 'Lena Weber' });
    const card = screen.getByRole('button', { name: /Marta Weber/ });
    expect(within(card).getByText('Living')).toBeInTheDocument();
    expect(screen.getAllByText('Living').length).toBeGreaterThanOrEqual(2);
  });

  it('adopted_chip_present', async () => {
    const f = ref('Otto Adler', { sex: 'male' });
    setup([view(ref('Karl'), { parentSets: [{ familyId: 'x', parents: [f, null], rel: ['adopted', 'birth'], preferred: true, summary: null }] })]);
    expect(await screen.findByText('Adopted')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Otto Adler, 1850-1921, Adopted father/ })).toBeInTheDocument();
  });

  it('other_parent_sets_are_collapsed', async () => {
    const a = ref('Birth Father', { sex: 'male' });
    const b = ref('Foster Father', { sex: 'male' });
    setup([
      view(ref('Karl'), {
        parentSets: [
          { familyId: 'a', parents: [a, null], rel: ['birth', 'birth'], preferred: true, summary: null },
          { familyId: 'b', parents: [b, null], rel: ['foster', 'birth'], preferred: false, summary: null },
        ],
      }),
    ]);
    const sum = await screen.findByText('Other parents (1)');
    expect(sum.closest('details')).not.toHaveAttribute('open');
  });

  it('private_person_shows_lock_label', async () => {
    const p = ref('Hidden Person', { private: true });
    setup([view(p)]);
    await screen.findByRole('heading', { level: 1, name: 'Hidden Person' });
    expect(screen.getAllByRole('img', { name: 'Private' }).length).toBeGreaterThan(0);
  });

  it('add_menu_offers_another_set_when_parents_full', async () => {
    const f = ref('F', { sex: 'male' });
    const m = ref('M', { sex: 'female' });
    const { user } = setup([
      view(ref('Karl'), { parentSets: [{ familyId: 'a', parents: [f, m], rel: ['birth', 'birth'], preferred: true, summary: null }] }),
    ]);
    await screen.findByRole('heading', { level: 1, name: 'Karl' });
    await user.click(screen.getByRole('button', { name: 'Add' }));
    expect(screen.getByRole('menuitem', { name: 'Add another set of parents' })).toBeInTheDocument();
    expect(screen.queryByRole('menuitem', { name: 'Add father' })).toBeNull();
    await user.click(screen.getByRole('menuitem', { name: 'Add another set of parents' }));
    expect(ui.add).toMatchObject({ open: true, relation: 'father', newParentSet: true });
    ui.closeAdd();
  });

  it('add_sibling_disabled_without_parents', async () => {
    const { user } = setup([view(ref('Karl'))]);
    await screen.findByRole('heading', { level: 1, name: 'Karl' });
    await user.click(screen.getByRole('button', { name: 'Add' }));
    const item = screen.getByRole('menuitem', { name: 'Add sibling' });
    expect(item).toHaveAttribute('aria-disabled', 'true');
    expect(item).toHaveAttribute('title', expect.stringContaining('Add a parent first'));
  });

  it('empty_tree_state', async () => {
    const empty = Object.assign(Object.create(base), {
      listPersons: async () => ({ total: 0, offset: 0, items: [] }),
    }) as Api;
    project.open({ dir: 'C:/x', name: 'x', personCount: 0, homePerson: null, sourceVersion: null, locationWarning: null });
    nav.reset();
    nav.go({ name: 'family' });
    const user = userEvent.setup();
    render(FamilyView, { context: apiContext(empty, new MockPlatform()) });
    expect(await screen.findByText('No one to show. Add the first person.')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Add person' }));
    expect(ui.add.open).toBe(true);
    ui.closeAdd();
  });

  it('works_with_mock_api_home_person', async () => {
    const api = new MockApi(fixtureTree(300));
    const info = await api.openProject('C:/x');
    project.open(info);
    nav.reset();
    nav.go({ name: 'family' });
    render(FamilyView, { context: apiContext(api, new MockPlatform()) });
    await waitFor(() => expect(screen.getByRole('heading', { level: 1 })).toBeInTheDocument());
    expect(nav.focusPerson).toBe(info.homePerson);
  });
});
