import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import PersonEditor from './PersonEditor.svelte';
import EventEditor from '../editors/EventEditor.svelte';
import { findPerson, newApi, renderWithApi } from '../editors/testing/helpers';
import type { MockApi } from '../api/mock/MockApi';
import type { EventDto, GodparentInput, PersonDetail } from '../api/types';
import { settings } from '../state/settings.svelte';
import { nav } from '../state/nav.svelte';
import { setLanguage } from '../i18n';
import { shortGodparentName } from '../ui/godparents';

let api: MockApi;
let child: PersonDetail;
let godfather: PersonDetail;

async function baptize(godparents: GodparentInput[], place?: string) {
  await api.apply({
    type: 'saveEvent',
    owner: { kind: 'person', id: child.ref.id },
    event: {
      kind: 'baptism',
      dateRaw: '6 Feb 1906',
      ...(place ? { placeText: place } : {}),
      negative: false,
      private: false,
      preferred: true,
      godparents,
    },
  });
  return (await api.getPerson(child.ref.id)).events.find((e) => e.kind === 'baptism')!;
}

const WIDOW = 'Hanna Bondar, a widow of Ivanivka';

beforeEach(async () => {
  api = newApi();
  child = await findPerson(api, (d) => d.events.some((e) => e.kind === 'birth' && !!e.place) && !d.events.some((e) => e.kind === 'baptism' || e.kind === 'christening'));
  godfather = await findPerson(api, (d) => d.sex === 'male' && d.ref.id !== child.ref.id && d.events.every((e) => e.kind !== 'baptism'));
  await settings.load(api);
});

afterEach(() => setLanguage('en'));

async function openPerson(id: string) {
  renderWithApi(api, PersonEditor, { id });
  return (await screen.findAllByRole('heading', { level: 1 }))[0];
}

describe('baptism on the card', () => {
  it('shows_the_baptism_after_the_birth_and_its_place_only_when_it_differs', async () => {
    await baptize([], 'Ivanivka');
    await openPerson(child.ref.id);
    const bap = await screen.findByText((_, el) => el?.hasAttribute('data-baptism') ?? false);
    expect(bap.textContent).toMatch(/^· bap. .*1906, Ivanivka$/);
    expect(bap.closest('p')!.textContent).toContain('Born');
  });

  it('leaves_out_the_place_when_it_is_the_birth_place', async () => {
    const birth = child.events.find((e) => e.kind === 'birth')!;
    await api.apply({
      type: 'saveEvent',
      owner: { kind: 'person', id: child.ref.id },
      event: { kind: 'baptism', dateRaw: '6 Feb 1906', placeId: birth.place!.id, negative: false, private: false, preferred: true },
    });
    await openPerson(child.ref.id);
    const bap = await screen.findByText((_, el) => el?.hasAttribute('data-baptism') ?? false);
    expect(bap.textContent).not.toContain(',');
    expect(bap.textContent).toContain('1906');
  });

  it('lists_godparents_on_one_line_the_linked_one_opens_their_page_and_the_tooltip_has_the_rest', async () => {
    await baptize([
      { person: godfather.ref.id, role: 'godfather', note: 'the uncle' },
      { name: WIDOW, role: 'godmother' },
    ]);
    const go = vi.spyOn(nav, 'go');
    await openPerson(child.ref.id);
    const line = document.querySelector('p[data-godparents]') as HTMLElement;
    expect(line).toBeTruthy();
    expect(line.textContent).toContain('Godparents');
    expect(line.textContent).toContain('Hanna Bondar');
    expect(line.textContent).not.toContain('widow');
    expect(line.title).toContain(WIDOW);
    expect(line.title).toContain('the uncle');
    await userEvent.setup().click(within(line).getByRole('button', { name: godfather.ref.name }));
    expect(go).toHaveBeenCalledWith({ name: 'person', id: godfather.ref.id });
  });

  it('a_person_with_only_a_baptism_shows_it_in_place_of_the_birth', async () => {
    const birth = child.events.find((e) => e.kind === 'birth')!;
    await api.apply({ type: 'deleteEvent', owner: { kind: 'person', id: child.ref.id }, event: birth.id });
    await baptize([]);
    expect((await api.getPerson(child.ref.id)).events.some((e) => e.kind === 'birth')).toBe(false);
    await openPerson(child.ref.id);
    expect(await screen.findByText('Baptized')).toBeTruthy();
  });

  it('reads_in_ukrainian', async () => {
    await baptize([{ name: WIDOW, role: 'godmother' }], 'Ivanivka');
    setLanguage('uk');
    await openPerson(child.ref.id);
    const bap = await screen.findByText((_, el) => el?.hasAttribute('data-baptism') ?? false);
    expect(bap.textContent).toContain('хрещ.');
    expect(document.querySelector('p[data-godparents]')!.textContent).toContain('Хрещені');
  });

  it('cuts_a_written_name_at_its_first_comma', () => {
    expect(shortGodparentName(WIDOW)).toBe('Hanna Bondar');
    expect(shortGodparentName('  Ivan Koval ')).toBe('Ivan Koval');
    expect(shortGodparentName(null)).toBe('');
  });
});

describe('godchildren', () => {
  it('a_godparent_s_page_lists_the_godchildren', async () => {
    await baptize([{ person: godfather.ref.id, role: 'godfather' }]);
    await openPerson(godfather.ref.id);
    const section = (await screen.findByRole('heading', { level: 2, name: 'Godchildren' })).closest('section')!;
    expect(section.textContent).toContain('Godfather to:');
    expect(within(section).getByRole('button', { name: child.ref.name })).toBeTruthy();
    expect(section.textContent).toMatch(/godfather, bap\. .*1906/);
  });

  it('is_called_хрещеники_in_ukrainian', async () => {
    await baptize([{ person: godfather.ref.id, role: 'godfather' }]);
    setLanguage('uk');
    await openPerson(godfather.ref.id);
    expect(await screen.findByRole('heading', { level: 2, name: 'Хрещеники' })).toBeTruthy();
  });

  it('nobody_else_has_the_section', async () => {
    await openPerson(child.ref.id);
    await screen.findByRole('heading', { level: 2, name: 'Names' });
    expect(screen.queryByRole('heading', { level: 2, name: 'Godchildren' })).toBeNull();
  });
});

describe('godparents in the event editor', () => {
  function show(event?: EventDto, newKind = 'baptism') {
    const onSaved = vi.fn();
    renderWithApi(api, EventEditor, { owner: { kind: 'person', id: child.ref.id }, event, newKind, onSaved, onCancel: vi.fn() });
    return onSaved;
  }
  const sent = (spy: { mock: { calls: unknown[][] } }) =>
    (spy.mock.calls.at(-1)![0] as { event: { godparents?: GodparentInput[] } }).event.godparents;

  it('a_new_baptism_takes_godparents_by_name_with_role_and_note', async () => {
    const user = userEvent.setup();
    const spy = vi.spyOn(api, 'apply');
    const onSaved = show();
    const box = document.querySelector('fieldset[data-godparents]') as HTMLElement;
    expect(box).toBeTruthy();
    await user.click(within(box).getByRole('button', { name: 'Add a godparent by name' }));
    await user.type(within(box).getByRole('textbox', { name: 'Name of godparent 1' }), WIDOW);
    await user.selectOptions(within(box).getByRole('combobox', { name: `Role of ${WIDOW}` }), 'godmother');
    await user.type(within(box).getByRole('textbox', { name: `Note on ${WIDOW}` }), 'could not sign');
    await user.click(screen.getByRole('button', { name: 'Save event' }));
    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect(sent(spy)).toEqual([{ name: WIDOW, role: 'godmother', note: 'could not sign' }]);
    const bap = (await api.getPerson(child.ref.id)).events.find((e) => e.kind === 'baptism')!;
    expect(bap.godparents?.map((g) => g.name)).toEqual([WIDOW]);
  });

  it('editing_keeps_the_ids_and_a_removed_one_goes', async () => {
    const user = userEvent.setup();
    const bap = await baptize([
      { person: godfather.ref.id, role: 'godfather' },
      { name: WIDOW, role: 'godmother' },
    ]);
    const spy = vi.spyOn(api, 'apply');
    const onSaved = show(bap);
    const box = document.querySelector('fieldset[data-godparents]') as HTMLElement;
    expect(within(box).getByText(godfather.ref.name)).toBeTruthy();
    await user.click(within(box).getByRole('button', { name: `Remove ${WIDOW}` }));
    await user.click(screen.getByRole('button', { name: 'Save event' }));
    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect(sent(spy)).toEqual([{ id: bap.godparents![0].id, person: godfather.ref.id, role: 'godfather' }]);
  });

  it('a_birth_has_no_godparents_box', async () => {
    show(undefined, 'birth');
    expect(document.querySelector('fieldset[data-godparents]')).toBeNull();
  });
});
