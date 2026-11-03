import { beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import DateField from './DateField.svelte';
import { newApi, renderWithApi } from './testing/helpers';
import type { DatePreview } from '../api/types';

let api: ReturnType<typeof newApi>;
beforeEach(() => {
  api = newApi(20);
});

function show(order: 'dmy' | 'mdy' = 'dmy') {
  const onChange = vi.fn();
  renderWithApi(api, DateField, { label: 'Date', value: '', onChange, order });
  return { onChange, box: screen.getByRole('textbox', { name: 'Date' }) };
}

describe('DateField', () => {
  it('shows_understood_as_live', async () => {
    const user = userEvent.setup();
    const { box } = show();
    await user.type(box, 'abt 1850');
    expect(await screen.findByText(/Understood as: about 1850/)).toBeInTheDocument();
  });

  it('dmy_vs_mdy_changes_preview', async () => {
    const user = userEvent.setup();
    let { box } = show('dmy');
    await user.type(box, '03/04/1901');
    expect(await screen.findByText(/Understood as: 3 Apr 1901/)).toBeInTheDocument();
    cleanup();
    ({ box } = show('mdy'));
    await user.type(box, '03/04/1901');
    expect(await screen.findByText(/Understood as: 4 Mar 1901/)).toBeInTheDocument();
  });

  it('dual_year_is_shown', async () => {
    const user = userEvent.setup();
    const { box } = show();
    await user.type(box, '12 Feb 1750/51');
    expect(await screen.findByText(/Understood as: 12 Feb 1750\/51/)).toBeInTheDocument();
  });

  it('unparsed_is_saved_verbatim_with_note', async () => {
    const user = userEvent.setup();
    const { box, onChange } = show();
    await user.type(box, 'sometime in the 1850s?');
    expect(await screen.findByText(/saved exactly as you typed it/)).toBeInTheDocument();
    expect(onChange).toHaveBeenLastCalledWith('sometime in the 1850s?');
  });

  it('phrase_in_parentheses', async () => {
    const user = userEvent.setup();
    const { box } = show();
    await user.type(box, '(the winter of the big flood)');
    expect(await screen.findByText(/Saved as a phrase: "\(the winter of the big flood\)"/)).toBeInTheDocument();
  });

  it('out_of_order_responses_ignored', async () => {
    const user = userEvent.setup();
    const resolvers: ((p: DatePreview) => void)[] = [];
    vi.spyOn(api, 'parseDate').mockImplementation(
      (raw) => new Promise((res) => resolvers.push((p) => res({ ...p, raw }))),
    );
    const { box } = show();
    const mk = (s: string): DatePreview => ({ raw: s, understoodAs: s, range: null, status: 'ok', warnings: [] });
    await user.type(box, 'a');
    await waitFor(() => expect(resolvers.length).toBeGreaterThanOrEqual(1));
    const before = resolvers.length;
    await user.type(box, 'b');
    await waitFor(() => expect(resolvers.length).toBe(before + 1));
    resolvers[resolvers.length - 1](mk('second'));
    await screen.findByText('Understood as: second');
    resolvers[0](mk('first'));
    await new Promise((r) => setTimeout(r, 30));
    expect(screen.getByRole('status')).toHaveTextContent('Understood as: second');
  });

  it('status_is_polite_live_region', () => {
    show();
    expect(screen.getByRole('status')).toHaveAttribute('aria-live', 'polite');
  });
});
