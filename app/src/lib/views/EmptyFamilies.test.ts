import { describe, expect, it } from 'vitest';
import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import EmptyFamilies from './EmptyFamilies.svelte';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import { nav } from '../state/nav.svelte';
import type { Id } from '../api/types';

async function family(api: MockApi, partner1: Id | null = null): Promise<Id> {
  const r = await api.apply({ type: 'saveFamily', partner1, partner2: null, kind: 'unknown' });
  return r.touched[0];
}

const link = (api: MockApi, f: Id, child: Id) => api.apply({ type: 'linkChild', family: f, child, rel: ['birth', 'birth'], preferred: false });

async function mount(api: MockApi) {
  renderWith(EmptyFamilies, {}, { api });
}

describe('EmptyFamilies', () => {
  it('says_so_when_there_are_none', async () => {
    await mount(new MockApi(fixtureTree(30)));
    expect(await screen.findByTestId('empty-families-none')).toHaveTextContent('No empty families.');
  });

  it('lists_each_kind_and_says_what_blocks_a_family', async () => {
    const api = new MockApi(fixtureTree(30));
    const people = (await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 20)).items;
    const [alone, child] = people;
    await family(api); // nobody in it
    await family(api, alone.id); // one partner, no children
    const orphans = await family(api);
    await link(api, orphans, child.id); // a parentless family: this child has no other family
    const kinds = (await api.listEmptyFamilies()).map((f) => f.kind);
    expect(kinds.sort()).toEqual(['empty', 'parentless', 'singlePartner']);

    await mount(api);
    const rows = await screen.findAllByTestId('empty-family-row');
    expect(rows).toHaveLength(3);
    const text = rows.map((r) => r.textContent ?? '');
    expect(text.some((x) => x.includes('Nobody in it'))).toBe(true);
    expect(text.some((x) => x.includes('One partner, no children'))).toBe(true);
    const blocked = rows.find((r) => r.textContent?.includes('No parents listed'))!;
    expect(within(blocked).getByTestId('empty-family-blocked')).toHaveTextContent('would be left with no parent family');
    expect(within(blocked).getByRole('button', { name: 'Delete' })).toBeDisabled();
    expect(within(blocked).getByRole('button', { name: 'Open' })).toBeEnabled();
  });

  it('delete_asks_first_then_removes_and_can_be_undone', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(30));
    const made = await family(api);
    await mount(api);
    const row = await screen.findByTestId('empty-family-row');
    await user.click(within(row).getByRole('button', { name: 'Delete' }));
    const dialog = await screen.findByRole('dialog');
    expect(dialog).toHaveTextContent('Delete this family?');
    // Cancel changes nothing.
    await user.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    expect((await api.listEmptyFamilies()).map((f) => f.id)).toEqual([made]);

    await user.click(within(row).getByRole('button', { name: 'Delete' }));
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Delete' }));
    await waitFor(() => expect(screen.getByTestId('empty-families-none')).toBeInTheDocument());
    expect(await api.listEmptyFamilies()).toEqual([]);

    await api.undo();
    expect((await api.listEmptyFamilies()).map((f) => f.id)).toEqual([made]);
  });

  it('open_goes_to_the_family_page', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(30));
    const made = await family(api);
    await mount(api);
    await user.click(within(await screen.findByTestId('empty-family-row')).getByRole('button', { name: 'Open' }));
    expect(nav.route).toEqual({ name: 'familyPage', id: made });
  });

  it('a_family_holding_a_note_gives_it_to_its_partner', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(30));
    const [alone] = (await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 5)).items;
    const f = await family(api, alone.id);
    await api.apply({ type: 'saveNote', target: { kind: 'family', id: f }, note: { kind: 'research', text: 'He had a first wife, name unknown.' } });
    const notesBefore = (await api.getPerson(alone.id)).notes.length;
    await mount(api);
    const row = await screen.findByTestId('empty-family-row');
    expect(row).toHaveTextContent('Holds 1 notes');
    await user.click(within(row).getByRole('button', { name: 'Delete' }));
    expect(await screen.findByRole('dialog')).toHaveTextContent(`move to ${alone.name}`);
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Delete' }));
    await waitFor(() => expect(screen.getByTestId('empty-families-none')).toBeInTheDocument());
    expect((await api.getPerson(alone.id)).notes.length).toBe(notesBefore + 1);
  });
});
