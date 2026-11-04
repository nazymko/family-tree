import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { nav } from '../state/nav.svelte';
import Host from '../chart/testing/Host.svelte';
import { fakeRaf, attachContext, makeApi, richestRoot } from '../chart/testing/helpers';
import { handleSpots } from '../chart/hit';
import Relatives from './Relatives.svelte';

beforeEach(() => {
  try {
    localStorage.removeItem('gt.chartView');
  } catch {
    // ignore
  }
  nav.reset();
});
afterEach(() => vi.restoreAllMocks());

async function mountRelatives() {
  const { api, fx } = makeApi();
  const root = await richestRoot(api, fx, 'relatives');
  nav.focus(root);
  const raf = fakeRaf();
  render(Host, { props: { api, view: Relatives } });
  const canvas = (await screen.findByRole('application')) as HTMLCanvasElement;
  attachContext(canvas);
  await screen.findByRole('option', { name: /.+/, hidden: true, selected: true });
  await new Promise((r) => setTimeout(r, 0));
  raf.pump(2);
  return { api, fx, root, canvas, raf };
}

describe('Relatives view', () => {
  it('renders_the_chart_and_asks_for_a_relatives_chart', async () => {
    const { api, fx } = makeApi();
    nav.focus(await richestRoot(api, fx, 'relatives'));
    const spy = vi.spyOn(api, 'chartOpen');
    render(Host, { props: { api, view: Relatives } });
    await screen.findByRole('application');
    await waitFor(() => expect(spy).toHaveBeenCalledWith(expect.objectContaining({ kind: 'relatives', generations: 3, auntsUncles: true })));
    expect(screen.getByRole('heading', { name: 'Relatives' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Centre on me' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Fit' })).toBeInTheDocument();
  });

  it('aunts_checkbox_reopens_the_chart', async () => {
    const { api, fx } = makeApi();
    nav.focus(await richestRoot(api, fx, 'relatives'));
    const spy = vi.spyOn(api, 'chartOpen');
    render(Host, { props: { api, view: Relatives } });
    const box = await screen.findByRole('checkbox', { name: 'Show aunts and uncles' });
    expect(box).toBeChecked();
    await fireEvent.click(box);
    await waitFor(() => expect(spy).toHaveBeenCalledWith(expect.objectContaining({ kind: 'relatives', auntsUncles: false })));
  });

  it('outline_tab_groups_relatives', async () => {
    const { api, fx } = makeApi();
    nav.focus(await richestRoot(api, fx, 'relatives'));
    render(Host, { props: { api, view: Relatives } });
    await fireEvent.click(await screen.findByRole('tab', { name: 'Outline' }));
    const tree = await screen.findByRole('tree');
    await fireEvent.click(within(tree).getByText('Starting person').closest('.row')!.querySelector('.twist')!);
    expect(await within(tree).findByText('Parents')).toBeInTheDocument();
    expect(within(tree).getByText('Siblings')).toBeInTheDocument();
  });

  it('arrows_walk_parent_child_and_the_focus_row', async () => {
    const { canvas, raf } = await mountRelatives();
    const text = () => screen.getByRole('status').textContent;
    await waitFor(() => expect(text()).not.toBe(''));
    const start = text();
    await fireEvent.keyDown(canvas, { key: 'ArrowUp' });
    raf.pump(3);
    await waitFor(() => expect(text()).not.toBe(start));
    await fireEvent.keyDown(canvas, { key: 'ArrowDown' });
    raf.pump(3);
    await waitFor(() => expect(text()).toBe(start));
    // Right and Left walk the focus row (siblings and partners) and come back.
    let moved = false;
    for (const [out, back] of [['ArrowRight', 'ArrowLeft'], ['ArrowLeft', 'ArrowRight']]) {
      await fireEvent.keyDown(canvas, { key: out });
      raf.pump(3);
      if (text() !== start) {
        moved = true;
        await fireEvent.keyDown(canvas, { key: back });
        raf.pump(3);
        await waitFor(() => expect(text()).toBe(start));
        break;
      }
    }
    expect(moved).toBe(true);
  });

  it('enter_recentres_and_pushes_history_then_back_returns', async () => {
    const { root, canvas, raf } = await mountRelatives();
    expect(nav.focusPerson).toBe(root);
    // Move to a different box, then Enter re-centres on it.
    await fireEvent.keyDown(canvas, { key: 'ArrowUp' });
    raf.pump(3);
    await waitFor(() => expect(screen.getByRole('status').textContent).not.toBe(''));
    await fireEvent.keyDown(canvas, { key: 'Enter' });
    await waitFor(() => expect(nav.focusPerson).not.toBe(root));
    const now = nav.focusPerson;
    nav.back();
    expect(nav.focusPerson).toBe(root);
    nav.forward();
    expect(nav.focusPerson).toBe(now);
  });

  it('breadcrumb_click_recentres_on_that_person', async () => {
    const { root, canvas, raf } = await mountRelatives();
    const trail = await screen.findByRole('navigation', { name: 'People you have visited' });
    await waitFor(() => expect(within(trail).getAllByRole('listitem')).toHaveLength(1));
    await fireEvent.keyDown(canvas, { key: 'ArrowUp' });
    raf.pump(3);
    await fireEvent.keyDown(canvas, { key: 'Enter' });
    await waitFor(() => expect(within(trail).getAllByRole('listitem')).toHaveLength(2));
    const crumbs = within(trail).getAllByRole('listitem');
    expect(crumbs[1].textContent).toMatch(/\((father|mother|parent)\)/);
    expect(within(crumbs[1]).getByText(/./).getAttribute('aria-current')).toBe('location');
    await fireEvent.click(within(crumbs[0]).getByRole('button'));
    await waitFor(() => expect(nav.focusPerson).toBe(root));
    // The path stays; only the current crumb moves back.
    await waitFor(() => expect(within(trail).getAllByRole('listitem')[0].querySelector('[aria-current]')).not.toBeNull());
    expect(within(trail).getAllByRole('listitem')).toHaveLength(2);
  });

  it('back_and_forward_keep_the_whole_trail', async () => {
    const { root, canvas, raf } = await mountRelatives();
    const trail = await screen.findByRole('navigation', { name: 'People you have visited' });
    await waitFor(() => expect(within(trail).getAllByRole('listitem')).toHaveLength(1));
    await fireEvent.keyDown(canvas, { key: 'ArrowUp' });
    raf.pump(3);
    await fireEvent.keyDown(canvas, { key: 'Enter' });
    await waitFor(() => expect(within(trail).getAllByRole('listitem')).toHaveLength(2));
    const names = () => within(trail).getAllByRole('listitem').map((li) => li.textContent);
    const both = names();
    nav.back();
    await waitFor(() => expect(nav.focusPerson).toBe(root));
    await waitFor(() => expect(within(trail).getAllByRole('listitem')[0].querySelector('[aria-current]')).not.toBeNull());
    expect(names()).toEqual(both);
    nav.forward();
    await waitFor(() => expect(within(trail).getAllByRole('listitem')[1].querySelector('[aria-current]')).not.toBeNull());
    expect(names()).toEqual(both);
  });

  it('picking_in_the_outline_extends_the_trail', async () => {
    const { api, fx } = makeApi();
    nav.focus(await richestRoot(api, fx, 'relatives'));
    render(Host, { props: { api, view: Relatives } });
    const trail = await screen.findByRole('navigation', { name: 'People you have visited' });
    await waitFor(() => expect(within(trail).getAllByRole('listitem')).toHaveLength(1));
    await fireEvent.click(await screen.findByRole('tab', { name: 'Outline' }));
    const tree = await screen.findByRole('tree');
    await fireEvent.click(within(tree).getByText('Starting person').closest('li')!.querySelector('.twist')!);
    const parentsRow = (await within(tree).findByText('Parents')).closest('li')!;
    await fireEvent.click(parentsRow.querySelector('.twist')!);
    await waitFor(() => expect(within(parentsRow).getAllByRole('treeitem').length).toBeGreaterThan(0));
    const dad = within(parentsRow).getAllByRole('treeitem')[0];
    await fireEvent.keyDown(dad, { key: 'Enter' });
    await waitFor(() => expect(within(trail).getAllByRole('listitem')).toHaveLength(2));
    expect(within(trail).getAllByRole('listitem')[1].textContent).toMatch(/\((father|mother|parent)\)/);
  });

  it('centre_on_me_extends_the_trail', async () => {
    const { api, fx } = makeApi();
    const home = (await api.getSettings()).homePerson;
    const other = fx.persons.find((p) => p.id !== home && p.id !== fx.specials.undated)!.id;
    nav.focus(other);
    render(Host, { props: { api, view: Relatives } });
    const trail = await screen.findByRole('navigation', { name: 'People you have visited' });
    await waitFor(() => expect(within(trail).getAllByRole('listitem')).toHaveLength(1));
    const btn = await screen.findByRole('button', { name: 'Centre on me' });
    await waitFor(() => expect(btn).toBeEnabled());
    await fireEvent.click(btn);
    await waitFor(() => expect(nav.focusPerson).toBe(home));
    await waitFor(() => expect(within(trail).getAllByRole('listitem')).toHaveLength(2));
  });

  it('outline_expands_everything_without_overflowing', async () => {
    const { api, fx } = makeApi();
    nav.focus(await richestRoot(api, fx, 'relatives'));
    render(Host, { props: { api, view: Relatives } });
    await fireEvent.click(await screen.findByRole('tab', { name: 'Outline' }));
    const tree = await screen.findByRole('tree');
    // Open every row that can be opened, round after round; it must come to an end.
    let rounds = 0;
    for (; rounds < 12; rounds++) {
      const closed = Array.from(tree.querySelectorAll('li[aria-expanded="false"]'));
      if (closed.length === 0) break;
      for (const li of closed) await fireEvent.click(li.querySelector(':scope > .row .twist')!);
      await new Promise((r) => setTimeout(r, 20));
    }
    expect(rounds).toBeLessThan(12);
    const items = Array.from(tree.querySelectorAll('[role=treeitem]'));
    expect(items.length).toBeGreaterThan(3);
    const ids = items.map((e) => e.id);
    expect(new Set(ids).size).toBe(ids.length);
    // A person listed again is only a reference: it cannot be opened.
    const refs = items.filter((e) => /also shown above/.test(e.querySelector(':scope > .row')!.textContent ?? ''));
    expect(refs.length).toBeGreaterThan(0);
    for (const r of refs) expect(r.hasAttribute('aria-expanded')).toBe(false);
  });
});

describe('relatives handles', () => {
  it('handle_spots_sit_at_top_and_bottom_centre', () => {
    const n = { x: 100, y: 50, w: 80, h: 40, moreUp: true, moreDown: true } as Parameters<typeof handleSpots>[0];
    const spots = handleSpots(n, 'relatives');
    expect(spots).toHaveLength(2);
    expect(spots[0].x).toBe(140);
    expect(spots[0].y).toBeLessThanOrEqual(50);
    expect(spots[1].x).toBe(140);
    expect(spots[1].y).toBeGreaterThanOrEqual(90);
  });
});
