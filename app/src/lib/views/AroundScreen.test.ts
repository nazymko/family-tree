import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import Host from '../chart/testing/Host.svelte';
import { attachContext, fakeRaf, lastCamera, makeApi, richestRoot } from '../chart/testing/helpers';
import { setLanguage } from '../i18n';
import { nav } from '../state/nav.svelte';
import { toasts } from '../state/toast.svelte';
import Around from './Around.svelte';
import WholeTree from './WholeTree.svelte';

// The test setup forces English before each test; these tests use Ukrainian, the owner's language.
beforeEach(() => {
  nav.reset();
  toasts.clear();
  setLanguage('uk');
  try {
    localStorage.removeItem('gt.chartView');
  } catch {
    // ignore
  }
});
afterEach(() => {
  vi.unstubAllGlobals();
  setLanguage('en');
});

async function mountWhole() {
  fakeRaf();
  const made = makeApi(120);
  nav.go({ name: 'whole' });
  render(Host, { props: { api: made.api, view: WholeTree } });
  const canvas = (await waitFor(() => {
    const c = document.querySelector('canvas[role="application"]');
    expect(c).not.toBeNull();
    return c;
  })) as HTMLCanvasElement;
  attachContext(canvas);
  return { ...made, canvas };
}

async function mountAround(id: string) {
  const raf = fakeRaf();
  const made = makeApi();
  nav.go({ name: 'whole' });
  nav.go({ name: 'around', ids: [id] });
  const spy = vi.spyOn(made.api, 'chartOpen');
  render(Host, { props: { api: made.api, view: Around } });
  const canvas = (await screen.findByRole('application')) as HTMLCanvasElement;
  const ctx = attachContext(canvas);
  return { ...made, canvas, spy, ctx, raf };
}

describe('the button on the whole tree', () => {
  it('opens_the_new_screen_for_the_selected_person_and_leaves_the_chart_as_it_is', async () => {
    const s = await mountWhole();
    const btn = screen.getByRole('button', { name: 'Побудувати навколо цієї особи' });
    expect(btn).toBeDisabled();
    await fireEvent.keyDown(s.canvas, { key: 'Home' });
    await waitFor(() => expect(btn).toBeEnabled());
    await fireEvent.click(btn);
    expect(nav.route.name).toBe('around');
    // The chart was not rearranged or reopened, and the old header text and button are gone.
    expect(screen.queryByText(/Побудовано навколо/)).toBeNull();
    expect(screen.queryByRole('button', { name: 'Звичайний порядок' })).toBeNull();
    // The person the shell is centred on is the one it was built around, so Back brings them back.
    expect(nav.route.name === 'around' && nav.route.ids[0]).toBe(nav.focusPerson);
    nav.back();
    expect(nav.route).toEqual({ name: 'whole' });
  });
});

describe('the relatives choice', () => {
  it('has_three_options_starts_on_the_line_only_and_asks_for_the_others_on_this_screen', async () => {
    const { api, fx } = makeApi();
    const id = await richestRoot(api, fx, 'relatives');
    const s = await mountAround(id);
    const group = screen.getByRole('radiogroup', { name: 'Рідня' });
    const options = within(group).getAllByRole('radio');
    expect(options.map((o) => (o as HTMLInputElement).labels?.[0]?.textContent?.trim())).toEqual(['Лише лінія', 'Брати й сестри', 'Уся рідня']);
    const line = within(group).getByRole('radio', { name: 'Лише лінія' });
    const sibs = within(group).getByRole('radio', { name: 'Брати й сестри' });
    const all = within(group).getByRole('radio', { name: 'Уся рідня' });
    expect(line).toBeChecked();
    await waitFor(() => expect(s.spy).toHaveBeenCalled());
    expect(s.spy.mock.lastCall?.[0].kin).toBe('line');
    await fireEvent.click(sibs);
    await waitFor(() => expect(s.spy.mock.lastCall?.[0].kin).toBe('siblings'));
    expect(sibs).toBeChecked();
    await fireEvent.click(all);
    await waitFor(() => expect(s.spy.mock.lastCall?.[0].kin).toBe('all'));
    expect(all).toBeChecked();
    await fireEvent.click(line);
    await waitFor(() => expect(s.spy.mock.lastCall?.[0].kin).toBe('line'));
  });

  it('the_all_relatives_chart_opens_with_more_people_than_the_line', async () => {
    const { api, fx } = makeApi();
    const id = await richestRoot(api, fx, 'relatives');
    const s = await mountAround(id);
    await waitFor(() => expect(s.spy).toHaveBeenCalled());
    const first = await s.spy.mock.results[s.spy.mock.results.length - 1].value;
    await fireEvent.click(screen.getByRole('radio', { name: 'Уся рідня' }));
    await waitFor(() => expect(s.spy.mock.lastCall?.[0].kin).toBe('all'));
    const last = await s.spy.mock.results[s.spy.mock.results.length - 1].value;
    expect(last.nodeCount).toBeGreaterThan(first.nodeCount);
    // The choice is remembered for the session: put it back for the other tests.
    await fireEvent.click(screen.getByRole('radio', { name: 'Лише лінія' }));
    await waitFor(() => expect(s.spy.mock.lastCall?.[0].kin).toBe('line'));
  });
});

describe('the screen built around a person', () => {
  it('asks_for_every_generation_names_the_person_and_has_a_back_button_to_the_whole_tree', async () => {
    const { api, fx } = makeApi();
    const root = await richestRoot(api, fx, 'relatives');
    const s = await mountAround(root);
    await waitFor(() => expect(s.spy).toHaveBeenCalledWith({ kind: 'around', root, generations: 255, kin: 'line' }));
    const name = (await s.api.getPerson(root)).ref.name;
    expect(await screen.findByRole('heading', { name: `Побудовано навколо: ${name}` })).toBeInTheDocument();
    expect(screen.getByRole('application', { name: 'Діаграма навколо особи' })).toBeInTheDocument();
    expect(screen.getByRole('toolbar', { name: 'Інструменти діаграми навколо особи' })).toBeInTheDocument();
    for (const label of ['Вписати', 'Відкрити особу', 'Побудувати навколо цієї особи']) {
      expect(screen.getByRole('button', { name: new RegExp(label) })).toBeInTheDocument();
    }
    await fireEvent.click(screen.getByRole('button', { name: 'Назад до всього родоводу' }));
    expect(nav.route).toEqual({ name: 'whole' });
  });

  it('the_shell_arrows_work_and_building_around_another_person_makes_back_go_one_step', async () => {
    const { api, fx } = makeApi();
    const root = await richestRoot(api, fx, 'relatives');
    const s = await mountAround(root);
    // Somebody else: building around another person is one more step in the history.
    const second = fx.persons.find((p) => p.id !== root)!.id;
    const open = vi.spyOn(s.api, 'chartOpen');
    nav.go({ name: 'around', ids: [second] });
    await waitFor(() => expect(open).toHaveBeenCalledWith({ kind: 'around', root: second, generations: 255, kin: 'line' }));
    // The label no longer promises the whole tree: the step back is the first person's screen.
    expect(await screen.findByRole('button', { name: 'Назад' })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Назад' }));
    expect(nav.route).toEqual({ name: 'around', ids: [root] });
    nav.back();
    expect(nav.route).toEqual({ name: 'whole' });
    nav.forward();
    expect(nav.route).toEqual({ name: 'around', ids: [root] });
  });

  it('a_person_who_is_gone_returns_to_the_whole_tree_with_a_message_and_no_history_trap', async () => {
    const s = await mountAround('00000000-0000-4000-8000-000000000000');
    await waitFor(() => expect(s.spy).toHaveBeenCalled());
    await waitFor(() => expect(nav.route).toEqual({ name: 'whole' }));
    expect(toasts.items.map((x) => x.text)).toContain('Цієї особи немає в родоводі.');
    expect(nav.history[nav.index - 1]?.name).not.toBe("whole");
  });
});

async function mountAroundMany(ids: string[]) {
  fakeRaf();
  const made = makeApi();
  nav.go({ name: 'whole' });
  nav.go({ name: 'around', ids });
  const spy = vi.spyOn(made.api, 'chartOpen');
  render(Host, { props: { api: made.api, view: Around } });
  const canvas = (await screen.findByRole('application')) as HTMLCanvasElement;
  attachContext(canvas);
  return { ...made, canvas, spy };
}

describe('the screen built around several people', () => {
  const pick = (fx: ReturnType<typeof makeApi>['fx'], n: number) => fx.persons.filter((p) => !p.siblingMarker).slice(0, n).map((p) => p.id);

  it('names_one_two_and_five_people_in_the_heading_without_declining_the_names', async () => {
    for (const count of [1, 2, 5]) {
      nav.reset();
      nav.go({ name: 'whole' });
      fakeRaf();
      const { fx, api } = makeApi();
      const ids = pick(fx, count);
      const names = await Promise.all(ids.map(async (id) => (await api.getPerson(id)).ref.name));
      const want =
        count === 1 ? `Побудовано навколо: ${names[0]}` : count === 2 ? `Побудовано навколо: ${names[0]} і ${names[1]}` : `Побудовано навколо: ${names[0]}, ${names[1]} і ще ${count - 2}`;
      nav.go({ name: 'around', ids });
      const { unmount } = render(Host, { props: { api, view: Around } });
      expect(await screen.findByRole('heading', { name: want })).toBeInTheDocument();
      unmount();
    }
  });

  it('opens_one_chart_for_all_of_them_with_the_three_kin_choices_and_back_returns_to_the_whole_tree', async () => {
    const probe = makeApi();
    const ids = pick(probe.fx, 3);
    const s = await mountAroundMany(ids);
    await waitFor(() => expect(s.spy).toHaveBeenCalledWith({ kind: 'around', root: ids[0], generations: 255, kin: 'line', roots: ids }));
    // The three choices work for several people too: switching asks for the same people with the other kin.
    const group = screen.getByRole('radiogroup', { name: 'Рідня' });
    const line = within(group).getByRole('radio', { name: 'Лише лінія' });
    const sibs = within(group).getByRole('radio', { name: 'Брати й сестри' });
    const all = within(group).getByRole('radio', { name: 'Уся рідня' });
    expect(line).toBeChecked();
    expect(sibs).toBeEnabled();
    expect(all).toBeEnabled();
    expect(screen.queryByText(/Для кількох осіб показано лише лінію/)).toBeNull();
    await fireEvent.click(sibs);
    await waitFor(() => expect(s.spy).toHaveBeenCalledWith({ kind: 'around', root: ids[0], generations: 255, kin: 'siblings', roots: ids }));
    await fireEvent.click(all);
    await waitFor(() => expect(s.spy).toHaveBeenCalledWith({ kind: 'around', root: ids[0], generations: 255, kin: 'all', roots: ids }));
    await fireEvent.click(line);
    await waitFor(() => expect(s.spy.mock.lastCall?.[0].kin).toBe('line'));
    // The outline lists every chosen person.
    const meta = await s.spy.mock.results[s.spy.mock.results.length - 1].value;
    expect(meta.chosenNodes).toHaveLength(3);
    const top = await s.api.chartOutline(meta.chartId, null, 100);
    expect(top.map((i) => i.person?.id)).toEqual(ids);
    await fireEvent.click(screen.getByRole('button', { name: 'Назад до всього родоводу' }));
    expect(nav.route).toEqual({ name: 'whole' });
  });

  it('shift_click_on_the_screen_marks_people_and_the_button_counts_them_escape_clears', async () => {
    const { api, fx } = makeApi();
    const root = await richestRoot(api, fx, 'relatives');
    const s = await mountAround(root);
    const RECT = { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600, x: 0, y: 0, toJSON() {} };
    s.canvas.getBoundingClientRect = () => RECT;
    const meta = await s.spy.mock.results[s.spy.mock.results.length - 1].value;
    const win = await s.api.chartWindow(meta.chartId, { x: 0, y: 0, w: meta.bounds.w, h: meta.bounds.h }, 1);
    const others = win.nodes.filter((n) => n.person && n.id !== meta.rootNode && n.dupOf === null).slice(0, 2);
    expect(others.length).toBe(2);
    // Mark by Shift+click at the card's place on screen: the view is centred on the person, so ask where the camera is.
    await waitFor(() => {
      s.raf.pump(2);
      expect(lastCamera(s.ctx)).not.toBeNull();
    });
    const cam = lastCamera(s.ctx)!;
    const shiftClick = async (n: (typeof others)[number]) => {
      const at = { clientX: (n.x + n.w / 2 - cam.x) * cam.zoom, clientY: (n.y + n.h / 2 - cam.y) * cam.zoom, shiftKey: true, bubbles: true };
      await fireEvent(s.canvas, new MouseEvent('pointerdown', at));
      await fireEvent(s.canvas, new MouseEvent('pointerup', at));
    };
    expect(screen.getByRole('button', { name: 'Побудувати навколо цієї особи' })).toBeInTheDocument();
    await shiftClick(others[0]);
    await shiftClick(others[1]);
    expect(await screen.findByRole('button', { name: 'Побудувати навколо цих осіб (2)' })).toBeInTheDocument();
    expect(screen.getByTestId('marked-count')).toHaveTextContent('2');
    await fireEvent.keyDown(s.canvas, { key: 'Escape' });
    expect(await screen.findByRole('button', { name: 'Побудувати навколо цієї особи' })).toBeInTheDocument();
    expect(screen.queryByTestId('marked-count')).toBeNull();
  });

  it('routes_with_the_same_people_are_one_step_and_other_people_are_another', () => {
    nav.go({ name: 'around', ids: ['a', 'b'] });
    const n = nav.history.length;
    nav.go({ name: 'around', ids: ['a', 'b'] });
    expect(nav.history.length).toBe(n);
    nav.go({ name: 'around', ids: ['a'] });
    expect(nav.history.length).toBe(n + 1);
  });
});
