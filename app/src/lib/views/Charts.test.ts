import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { MockApi } from '../api/mock/MockApi';
import { nav } from '../state/nav.svelte';
import Host from '../chart/testing/Host.svelte';
import { makeApi, richestRoot } from '../chart/testing/helpers';
import Pedigree from './Pedigree.svelte';
import Descendants from './Descendants.svelte';

function mount(api: MockApi, view: typeof Pedigree | typeof Descendants) {
  return render(Host, { props: { api, view } });
}

beforeEach(() => {
  try {
    localStorage.removeItem('gt.chartView');
  } catch {
    // ignore
  }
  nav.reset();
});
afterEach(() => vi.restoreAllMocks());

describe('chart views', () => {
  it('view_toggle_chart_outline_persists_choice', async () => {
    const { api, fx } = makeApi();
    nav.focus(await richestRoot(api, fx, 'pedigree'));
    mount(api, Pedigree);
    const chartTab = await screen.findByRole('tab', { name: 'Chart' });
    expect(chartTab).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('application')).toBeVisible();
    await fireEvent.click(screen.getByRole('tab', { name: 'Outline' }));
    expect(await screen.findByRole('tree')).toBeInTheDocument();
    expect(localStorage.getItem('gt.chartView')).toBe('outline');
    cleanup();
    mount(api, Pedigree);
    expect(await screen.findByRole('tab', { name: 'Outline' })).toHaveAttribute('aria-selected', 'true');
    expect(await screen.findByRole('tree')).toBeInTheDocument();
  });

  it('reduced_motion_defaults_to_outline', async () => {
    const { api, fx } = makeApi();
    nav.focus(await richestRoot(api, fx, 'pedigree'));
    const mm = window.matchMedia;
    window.matchMedia = ((q: string) => ({ ...mm(q), matches: q.includes('reduce') })) as typeof window.matchMedia;
    mount(api, Pedigree);
    expect(await screen.findByRole('tab', { name: 'Outline' })).toHaveAttribute('aria-selected', 'true');
    window.matchMedia = mm;
  });

  it('empty_root_without_parents_shows_helper', async () => {
    const { api, fx } = makeApi();
    nav.focus(fx.specials.undated!);
    mount(api, Pedigree);
    const text = await screen.findByText(/^No parents recorded for .+\. Add a father or mother\.$/);
    expect(text).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add a parent' })).toBeInTheDocument();
  });

  it('cycle_fixture_opens_without_hanging', { timeout: 2000 }, async () => {
    const { api, fx } = makeApi();
    nav.focus(fx.specials.cycleA!);
    const spy = vi.spyOn(api, 'chartOpen');
    mount(api, Pedigree);
    await waitFor(() => expect(spy).toHaveBeenCalled());
    const meta = await spy.mock.results[0].value;
    expect(Number.isFinite(meta.nodeCount)).toBe(true);
    expect(meta.nodeCount).toBeLessThan(4001);
    await screen.findByRole('option', { name: /.+/, hidden: true, selected: true });
  });

  it('generations_select_reopens_chart', async () => {
    const { api, fx } = makeApi();
    nav.focus(await richestRoot(api, fx, 'descendants'));
    const spy = vi.spyOn(api, 'chartOpen');
    mount(api, Descendants);
    await waitFor(() => expect(spy).toHaveBeenCalledWith(expect.objectContaining({ kind: 'descendants', generations: 3 })));
    const select = await screen.findByLabelText('Generations');
    await fireEvent.change(select, { target: { value: '6' } });
    await waitFor(() => expect(spy).toHaveBeenCalledWith(expect.objectContaining({ kind: 'descendants', generations: 6 })));
  });

  it('recentre_updates_the_focus_person', async () => {
    const { api, fx } = makeApi();
    const root = await richestRoot(api, fx, 'pedigree');
    nav.focus(root);
    mount(api, Pedigree);
    const canvas = await screen.findByRole('application');
    await screen.findByRole('option', { name: /.+/, hidden: true, selected: true });
    await waitFor(() => expect(screen.getByRole('status').textContent).not.toBe(''));
    const before = screen.getByRole('status').textContent;
    await fireEvent.keyDown(canvas, { key: 'ArrowRight' });
    await waitFor(() => expect(screen.getByRole('status').textContent).not.toBe(before));
    await fireEvent.keyDown(canvas, { key: 'Enter' });
    await waitFor(() => expect(nav.focusPerson).not.toBe(root));
  });
});
