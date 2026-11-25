import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import Host from '../chart/testing/Host.svelte';
import { attachContext, fakeRaf, makeApi, richestRoot } from '../chart/testing/helpers';
import { nav } from '../state/nav.svelte';
import { toasts } from '../state/toast.svelte';
import Around from './Around.svelte';

beforeEach(() => {
  nav.reset();
  toasts.clear();
  try {
    localStorage.removeItem('gt.chartView');
  } catch {
    // ignore
  }
});
afterEach(() => vi.unstubAllGlobals());

async function mountAround(id: string) {
  fakeRaf();
  const made = makeApi();
  nav.go({ name: 'whole' });
  nav.go({ name: 'around', ids: [id] });
  const spy = vi.spyOn(made.api, 'chartOpen');
  render(Host, { props: { api: made.api, view: Around } });
  const canvas = (await screen.findByRole('application')) as HTMLCanvasElement;
  attachContext(canvas);
  return { ...made, canvas, spy };
}

describe('opening side branches on the chart built around a person', () => {
  it('offers_the_controls_for_the_selected_card_sends_the_openings_and_clears_them', async () => {
    const { api, fx } = makeApi();
    const id = await richestRoot(api, fx, 'relatives');
    const s = await mountAround(id);
    await waitFor(() => expect(s.spy).toHaveBeenCalled());
    // Nothing opened: the request is the old one, without `open`.
    expect(s.spy.mock.lastCall?.[0]).toEqual({ kind: 'around', root: id, generations: 255, kin: 'line' });
    expect(screen.queryByRole('button', { name: 'Clear all openings' })).toBeNull();

    const group = await screen.findByRole('radiogroup', { name: 'Brothers and sisters' });
    await fireEvent.click(within(group).getByRole('radio', { name: 'With children' }));
    await waitFor(() => expect(s.spy.mock.lastCall?.[0].open).toEqual([{ person: id, siblings: 'children' }]));
    await fireEvent.click(within(await screen.findByRole('radiogroup', { name: 'Brothers and sisters' })).getByRole('radio', { name: 'All descendants' }));
    await waitFor(() => expect(s.spy.mock.lastCall?.[0].open).toEqual([{ person: id, siblings: 'all' }]));

    await fireEvent.click(await screen.findByRole('button', { name: 'Clear all openings' }));
    await waitFor(() => expect(s.spy.mock.lastCall?.[0]).toEqual({ kind: 'around', root: id, generations: 255, kin: 'line' }));
    expect(screen.queryByRole('button', { name: 'Clear all openings' })).toBeNull();
  });

  it('are_cleared_when_the_chart_is_built_around_other_people', async () => {
    const { api, fx } = makeApi();
    const id = await richestRoot(api, fx, 'relatives');
    const s = await mountAround(id);
    const group = await screen.findByRole('radiogroup', { name: 'Brothers and sisters' });
    await fireEvent.click(within(group).getByRole('radio', { name: 'With children' }));
    await waitFor(() => expect(s.spy.mock.lastCall?.[0].open).toBeDefined());
    const other = fx.persons.find((p) => p.id !== id)!.id;
    nav.go({ name: 'around', ids: [other] });
    await waitFor(() => expect(s.spy.mock.lastCall?.[0].root).toBe(other));
    expect(s.spy.mock.lastCall?.[0].open).toBeUndefined();
  });

  it('are_not_offered_when_the_chart_is_built_around_several_people', async () => {
    const { api, fx } = makeApi();
    const id = await richestRoot(api, fx, 'relatives');
    const other = fx.persons.find((p) => p.id !== id)!.id;
    fakeRaf();
    const made = makeApi();
    nav.go({ name: 'whole' });
    nav.go({ name: 'around', ids: [id, other] });
    render(Host, { props: { api: made.api, view: Around } });
    await screen.findByRole('application');
    expect(screen.queryByRole('radiogroup', { name: 'Brothers and sisters' })).toBeNull();
  });
});
