import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import PlacePicker from './PlacePicker.svelte';
import { newApi, renderWithApi } from './testing/helpers';
import type { PlaceOption } from '../api/types';

let api: ReturnType<typeof newApi>;
let first: PlaceOption;

beforeEach(async () => {
  api = newApi(60);
  const all = await api.suggestPlaces('', 8);
  first = all.find((p) => p.full.includes(',')) ?? all[0];
});

function show() {
  const onChange = vi.fn();
  renderWithApi(api, PlacePicker, { label: 'Place', value: null, text: '', onChange });
  return { onChange, box: screen.getByRole('combobox', { name: 'Place' }) };
}

describe('PlacePicker', () => {
  it('typeahead_lists_matches_with_chain', async () => {
    const user = userEvent.setup();
    const { box } = show();
    await user.type(box, first.name.slice(0, 3));
    const rows = await screen.findAllByRole('option');
    expect(rows.length).toBeGreaterThan(0);
    expect(screen.getAllByText(first.full).length).toBeGreaterThan(0);
  });

  it('create_row_when_no_match', async () => {
    const user = userEvent.setup();
    const { box } = show();
    await user.type(box, 'Zzyzx, Nowhere');
    expect(await screen.findByRole('option', { name: 'Create "Zzyzx, Nowhere"' })).toBeInTheDocument();
  });

  it('create_returns_place_text', async () => {
    const user = userEvent.setup();
    const { box, onChange } = show();
    await user.type(box, 'Hamburg, Germany');
    await user.click(await screen.findByRole('option', { name: 'Create "Hamburg, Germany"' }));
    expect(onChange).toHaveBeenLastCalledWith({ placeText: 'Hamburg, Germany' });
  });

  it('keyboard_only_flow', async () => {
    const user = userEvent.setup();
    const { box, onChange } = show();
    await user.click(box);
    await user.keyboard(first.name.slice(0, 3));
    await screen.findAllByRole('option');
    await user.keyboard('{Enter}');
    expect(onChange).toHaveBeenLastCalledWith({ placeId: expect.any(String) });
  });
});
