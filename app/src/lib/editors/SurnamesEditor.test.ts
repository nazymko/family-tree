import { beforeEach, describe, expect, it } from 'vitest';
import { screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import PersonEditor from '../views/PersonEditor.svelte';
import { findPerson, newApi, renderWithApi } from './testing/helpers';
import { settings } from '../state/settings.svelte';
import type { MockApi } from '../api/mock/MockApi';

let api: MockApi;
let id: string;

beforeEach(async () => {
  api = newApi();
  // A person with exactly one name, so the married name is new.
  const d = await findPerson(api, (p) => p.names.length === 1 && p.names[0].surnames.length > 0 && !!p.names[0].given);
  id = d.ref.id;
  await api.apply({ type: 'saveName', person: id, name: { id: d.names[0].id, kind: 'birth', preferred: true, private: false, given: 'Olena', surnames: ['Koval'] } });
  await settings.load(api);
});

async function openSurnames() {
  const user = userEvent.setup();
  renderWithApi(api, PersonEditor, { id });
  await user.click(await screen.findByRole('button', { name: 'Surnames…' }));
  await screen.findByRole('dialog', { name: 'Surname at birth and after marriage' });
  return user;
}

const BOX = 'Show the surname after marriage as the main name';
const names = async () => (await api.getPerson(id)).names;

describe('Surnames dialog', () => {
  it('adds_a_married_surname_as_the_main_name_by_default', async () => {
    const user = await openSurnames();
    expect(screen.getByRole('textbox', { name: 'Surname at birth' })).toHaveValue('Koval');
    expect(screen.getByRole('checkbox', { name: BOX })).toBeChecked();
    await user.type(screen.getByRole('textbox', { name: 'Surname after marriage' }), 'Shevchuk');
    await user.click(screen.getByRole('button', { name: 'Save surnames' }));
    await waitFor(async () => expect((await names()).length).toBe(2));
    const n = await names();
    const married = n.find((x) => x.kind === 'married')!;
    expect(married.surnames).toEqual(['Shevchuk']);
    expect(married.given).toBe('Olena');
    expect(married.preferred).toBe(true);
    expect(n.find((x) => x.kind === 'birth')!.preferred).toBe(false);
    expect((await api.getPerson(id)).ref.name).toMatch(/Shevchuk/);
    // The card shows both surnames.
    expect(await screen.findByText('Surname after marriage')).toBeInTheDocument();
  });

  it('unticked_keeps_the_birth_name_main', async () => {
    const user = await openSurnames();
    await user.type(screen.getByRole('textbox', { name: 'Surname after marriage' }), 'Shevchuk');
    await user.click(screen.getByRole('checkbox', { name: BOX }));
    await user.click(screen.getByRole('button', { name: 'Save surnames' }));
    await waitFor(async () => expect((await names()).length).toBe(2));
    const n = await names();
    expect(n.find((x) => x.kind === 'married')!.preferred).toBe(false);
    expect(n.find((x) => x.kind === 'birth')!.preferred).toBe(true);
    expect((await api.getPerson(id)).ref.name).toMatch(/Koval/);
  });

  it('an_existing_married_surname_shows_what_is_saved', async () => {
    await api.apply({ type: 'saveName', person: id, name: { kind: 'married', preferred: false, private: false, given: 'Olena', surnames: ['Shevchuk'] } });
    await openSurnames();
    expect(screen.getByRole('checkbox', { name: BOX })).not.toBeChecked();
    // Nothing changed, so there is nothing to save.
    expect((await api.getPerson(id)).ref.name).toMatch(/Koval/);
  });

  it('can_make_the_married_surname_the_main_name_and_back', async () => {
    let user = await openSurnames();
    await user.type(screen.getByRole('textbox', { name: 'Surname after marriage' }), 'Shevchuk');
    await user.click(screen.getByRole('button', { name: 'Save surnames' }));
    await waitFor(async () => expect((await api.getPerson(id)).ref.name).toMatch(/Shevchuk/));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());

    user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Surnames…' }));
    const box = await screen.findByRole('checkbox', { name: BOX });
    expect(box).toBeChecked();
    await user.click(box);
    await user.click(screen.getByRole('button', { name: 'Save surnames' }));
    await waitFor(async () => expect((await api.getPerson(id)).ref.name).toMatch(/Koval/));
  });

  it('fixes_the_birth_surname_and_emptying_the_married_one_removes_it', async () => {
    let user = await openSurnames();
    await user.type(screen.getByRole('textbox', { name: 'Surname after marriage' }), 'Shevchuk');
    await user.click(screen.getByRole('button', { name: 'Save surnames' }));
    await waitFor(async () => expect((await names()).length).toBe(2));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());

    user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Surnames…' }));
    const atBirth = await screen.findByRole('textbox', { name: 'Surname at birth' });
    await user.clear(atBirth);
    await user.type(atBirth, 'Kovalenko');
    await user.clear(screen.getByRole('textbox', { name: 'Surname after marriage' }));
    await user.click(screen.getByRole('button', { name: 'Save surnames' }));
    await waitFor(async () => expect((await names()).length).toBe(1));
    expect((await names())[0].surnames).toEqual(['Kovalenko']);
    expect((await names())[0].given).toBe('Olena');
  });

  it('edit_buttons_sit_on_the_name_card', async () => {
    const user = userEvent.setup();
    renderWithApi(api, PersonEditor, { id });
    const group = await screen.findByRole('group', { name: 'Edit this person' });
    for (const name of ['Edit name', 'Surnames…']) expect(group).toContainElement(screen.getByRole('button', { name }));
    await user.click(screen.getByRole('button', { name: 'Edit name' }));
    expect(await screen.findByRole('textbox', { name: 'Given name' })).toHaveValue('Olena');
  });

  it('offers_to_move_a_patronymic_out_of_the_married_surname', async () => {
    // The shape an older Древо Жизни import produced: the patronymic sat in the surname field next to a maiden name.
    const birth = (await names())[0];
    await api.apply({ type: 'saveName', person: id, name: { kind: 'married', preferred: true, private: false, given: 'Olena', surnames: ['Іванівна'] } });
    expect(birth.patronymic).toBeNull();
    const user = await openSurnames();
    expect(screen.getByRole('textbox', { name: 'Surname after marriage' })).toHaveValue('Іванівна');
    await user.click(screen.getByRole('button', { name: 'Move it to the patronymic' }));
    expect(screen.getByRole('textbox', { name: 'Surname after marriage' })).toHaveValue('');
    expect(screen.getByText('“Іванівна” will be saved as the patronymic.')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Save surnames' }));
    await waitFor(async () => expect((await names()).length).toBe(1));
    const [n] = await names();
    expect(n.kind).toBe('birth');
    expect(n.surnames).toEqual(['Koval']);
    expect(n.patronymic).toBe('Іванівна');
    expect(n.preferred).toBe(true);
  });

  it('does_not_offer_the_move_for_a_real_surname', async () => {
    const user = await openSurnames();
    await user.type(screen.getByRole('textbox', { name: 'Surname after marriage' }), 'Shevchuk');
    expect(screen.queryByRole('button', { name: 'Move it to the patronymic' })).not.toBeInTheDocument();
  });
});
