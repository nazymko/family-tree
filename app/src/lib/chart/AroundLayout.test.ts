import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import type { ChartKind } from '../api/types';
import LayoutHost from './testing/LayoutHost.svelte';
import { fakeRaf, makeApi, richestRoot } from './testing/helpers';

async function setup(kind: ChartKind = 'around', withControl = true) {
  fakeRaf();
  const { api, fx } = makeApi();
  const root = await richestRoot(api, fx, 'around');
  const openSpy = vi.spyOn(api, 'chartOpen');
  render(LayoutHost, { props: { api, root, kind: kind as 'around' | 'pedigree', withControl, person: root } });
  await waitFor(() => expect(openSpy).toHaveBeenCalled());
  return { openSpy };
}

const last = (s: { openSpy: ReturnType<typeof vi.spyOn> }) => s.openSpy.mock.calls.at(-1)![0] as { fold?: boolean };

afterEach(() => vi.unstubAllGlobals());

describe('the layout choice on the around chart', () => {
  it('shows_the_three_choices_on_the_around_chart_only', async () => {
    await setup('around');
    const group = screen.getByRole('radiogroup', { name: 'Layout' });
    expect(group).toBeInTheDocument();
    for (const n of ['As today', 'Compact', 'Sheets by branch']) expect(screen.getByRole('radio', { name: n })).toBeInTheDocument();
  });

  it('is_not_shown_on_other_charts', async () => {
    await setup('pedigree');
    expect(screen.queryByRole('radiogroup', { name: 'Layout' })).toBeNull();
  });

  it('as_today_sends_no_fold', async () => {
    const s = await setup();
    expect(last(s).fold).toBeUndefined();
  });

  it('compact_sends_fold_true_and_going_back_sends_none', async () => {
    const s = await setup();
    const before = s.openSpy.mock.calls.length;
    await fireEvent.click(screen.getByRole('radio', { name: 'Compact' }));
    await waitFor(() => expect(s.openSpy.mock.calls.length).toBeGreaterThan(before));
    expect(last(s).fold).toBe(true);
    const mid = s.openSpy.mock.calls.length;
    await fireEvent.click(screen.getByRole('radio', { name: 'As today' }));
    await waitFor(() => expect(s.openSpy.mock.calls.length).toBeGreaterThan(mid));
    expect(last(s).fold).toBeUndefined();
  });

  it('sheets_also_sends_fold_true', async () => {
    const s = await setup();
    const before = s.openSpy.mock.calls.length;
    await fireEvent.click(screen.getByRole('radio', { name: 'Sheets by branch' }));
    await waitFor(() => expect(s.openSpy.mock.calls.length).toBeGreaterThan(before));
    expect(last(s).fold).toBe(true);
  });

  it('the_choice_survives_opening_a_branch', async () => {
    const s = await setup();
    await fireEvent.click(screen.getByRole('radio', { name: 'Compact' }));
    await waitFor(() => expect(last(s).fold).toBe(true));
    const before = s.openSpy.mock.calls.length;
    await fireEvent.click(screen.getByRole('button', { name: 'open-branch' }));
    await waitFor(() => expect(s.openSpy.mock.calls.length).toBeGreaterThan(before));
    const req = last(s) as { fold?: boolean; open?: unknown[] };
    expect(req.open).toHaveLength(1);
    expect(req.fold).toBe(true);
    expect(screen.getByRole('radio', { name: 'Compact' })).toBeChecked();
  });
});
