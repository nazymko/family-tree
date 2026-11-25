import { describe, expect, it, vi } from 'vitest';
import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Sources from './Sources.svelte';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import { project } from '../state/project.svelte';

describe('Sources', () => {
  it('shows_the_pictures_of_a_source_and_their_text', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(80));
    const real = api.suggestSources.bind(api);
    // An invented scanned letter on the first source.
    vi.spyOn(api, 'suggestSources').mockImplementation(async (q, n) => {
      const list = await real(q, n);
      list[0] = {
        ...list[0],
        media: [
          {
            primary: false,
            media: {
              id: 'scan1', title: 'Letter, page 1', mime: 'image/png', originalName: 'page1.png', date: null, description: null,
              resolved: true, width: 600, height: 800, sha256: null,
              transcription: [{ id: 't1', area: { x: 0.1, y: 0.1, w: 0.8, h: 0.1 }, rotation: 0, text: 'Dear cousin,' }],
              rotation: 0,
            },
          },
        ],
      };
      return list;
    });
    renderWith(Sources, {}, { api });
    const grid = await screen.findByRole('list', { name: 'Photos and documents' });
    await user.click(within(grid).getByRole('button', { name: /Letter, page 1/ }));
    const dlg = await screen.findByRole('dialog');
    expect(within(dlg).getByRole('button', { name: 'Text piece 1' })).toBeInTheDocument();
    expect(within(dlg).getByRole('region', { name: 'Full text' })).toHaveTextContent('Dear cousin,');
    expect(within(dlg).getByRole('button', { name: 'Remove from this source' })).toBeInTheDocument();
  });

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

  it('reloads_when_project_rev_changes', async () => {
    const api = new MockApi(fixtureTree(80));
    const first = (await api.suggestSources('', 1))[0];
    renderWith(Sources, {}, { api });
    const list = await screen.findByRole('list', { name: 'Sources' });
    await waitFor(() => expect(within(list).getAllByRole('listitem')[0]).toHaveTextContent(first.title!));
    // A change made outside this view (undo, redo) must show without reopening it.
    await api.apply({ type: 'saveSource', source: { id: first.id, title: 'Changed behind its back' } });
    project.rev++;
    await waitFor(() => expect(within(list).getAllByRole('listitem')[0]).toHaveTextContent('Changed behind its back'));
  });
});
