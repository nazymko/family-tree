import { describe, expect, it } from 'vitest';
import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Sources from './Sources.svelte';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import { joinKind, splitKind } from '../sources/sourceKind';

describe('source kind', () => {
  it('splits_and_joins_a_stored_kind', () => {
    expect(splitKind(null)).toEqual({ pick: '', label: '' });
    expect(splitKind('metric_book')).toEqual({ pick: 'metric_book', label: '' });
    expect(splitKind('other:Family letter')).toEqual({ pick: 'other', label: 'Family letter' });
    expect(joinKind('other', ' Family letter ')).toBe('other:Family letter');
    expect(joinKind('other', '')).toBe('other');
    expect(joinKind('photo', 'ignored')).toBe('photo');
  });

  it('the_picker_sets_the_kind_and_the_list_shows_it', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(80));
    renderWith(Sources, {}, { api });
    const list = await screen.findByRole('list', { name: 'Sources' });
    await waitFor(() => expect(within(list).getAllByRole('listitem').length).toBeGreaterThan(1));
    await user.selectOptions(screen.getByLabelText('Type of source', { selector: 'form select' }), 'civil_registry_extract');
    await user.type(screen.getByLabelText('Issued by'), 'CNAP of Lviv');
    await user.click(screen.getByRole('button', { name: 'Save source' }));
    await waitFor(() => expect(within(list).getAllByText('Registry extract (CNAP/civil registry)').length).toBe(1));
  });

  it('the_filter_keeps_only_the_chosen_kind', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(80));
    const all = await api.suggestSources('', 200);
    await api.apply({ type: 'saveSource', source: { id: all[1].id, kind: 'metric_book' } });
    renderWith(Sources, {}, { api });
    const list = await screen.findByRole('list', { name: 'Sources' });
    await waitFor(() => expect(within(list).getAllByRole('listitem').length).toBe(all.length));
    const filter = document.querySelector('.left select') as HTMLSelectElement;
    await user.selectOptions(filter, 'metric_book');
    await waitFor(() => expect(within(list).getAllByRole('listitem').length).toBe(1));
    expect(within(list).getByRole('listitem')).toHaveTextContent('Metric book (church record)');
    await user.selectOptions(filter, 'none');
    await waitFor(() => expect(within(list).getAllByRole('listitem').length).toBe(all.length - 1));
  });
});
