import { describe, expect, it, vi } from 'vitest';
import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Sources from './Sources.svelte';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';

describe('Sources', () => {
  it('lists_with_counts', async () => {
    const api = new MockApi(fixtureTree(80));
    const all = await api.suggestSources('', 200);
    renderWith(Sources, {}, { api });
    const list = await screen.findByRole('list', { name: 'Sources' });
    await waitFor(() => expect(within(list).getAllByRole('listitem').length).toBe(all.length));
    const first = all[0];
    const row = within(list).getAllByRole('listitem')[0];
    expect(row).toHaveTextContent(first.title!);
    expect(row).toHaveTextContent(`${first.citationCount} ${first.citationCount === 1 ? 'citation' : 'citations'}`);
  });

  it('filter_narrows', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(80));
    const all = await api.suggestSources('', 200);
    renderWith(Sources, {}, { api });
    const list = await screen.findByRole('list', { name: 'Sources' });
    const word = all[0].title!.split(' ')[0].toLowerCase();
    const expected = (await api.suggestSources(word, 200)).length;
    await user.type(screen.getByRole('searchbox', { name: 'Filter sources' }), word);
    await waitFor(() => expect(within(list).getAllByRole('listitem').length).toBe(expected));
    expect(expected).toBeLessThanOrEqual(all.length);
  });

  it('edit_source_saves', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(80));
    const spy = vi.spyOn(api, 'apply');
    const first = (await api.suggestSources('', 1))[0];
    renderWith(Sources, {}, { api });
    const list = await screen.findByRole('list', { name: 'Sources' });
    await user.click((await within(list).findAllByRole('button'))[0]);
    const title = await screen.findByRole('textbox', { name: 'Title' });
    await user.clear(title);
    await user.type(title, 'Renamed register');
    await user.click(screen.getByRole('button', { name: 'Save source' }));
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(spy.mock.calls[0][0]).toMatchObject({ type: 'saveSource', source: { id: first.id, title: 'Renamed register' } });
    await waitFor(() => expect(within(list).getAllByRole('listitem')[0]).toHaveTextContent('Renamed register'));
  });
});
