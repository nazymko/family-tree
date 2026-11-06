import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Host from '../chart/testing/Host.svelte';
import { attachContext, fakeRaf, makeApi, richestRoot } from '../chart/testing/helpers';
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
  fakeRaf();
  const made = makeApi();
  nav.go({ name: 'whole' });
  nav.go({ name: 'around', id });
  const spy = vi.spyOn(made.api, 'chartOpen');
  render(Host, { props: { api: made.api, view: Around } });
  const canvas = (await screen.findByRole('application')) as HTMLCanvasElement;
  attachContext(canvas);
  return { ...made, canvas, spy };
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
    expect(nav.route.name === 'around' && nav.route.id).toBe(nav.focusPerson);
    nav.back();
    expect(nav.route).toEqual({ name: 'whole' });
  });
});

describe('the screen built around a person', () => {
  it('asks_for_every_generation_names_the_person_and_has_a_back_button_to_the_whole_tree', async () => {
    const { api, fx } = makeApi();
    const root = await richestRoot(api, fx, 'relatives');
    const s = await mountAround(root);
    await waitFor(() => expect(s.spy).toHaveBeenCalledWith({ kind: 'around', root, generations: 255 }));
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
    nav.go({ name: 'around', id: second });
    await waitFor(() => expect(open).toHaveBeenCalledWith({ kind: 'around', root: second, generations: 255 }));
    // The label no longer promises the whole tree: the step back is the first person's screen.
    expect(await screen.findByRole('button', { name: 'Назад' })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Назад' }));
    expect(nav.route).toEqual({ name: 'around', id: root });
    nav.back();
    expect(nav.route).toEqual({ name: 'whole' });
    nav.forward();
    expect(nav.route).toEqual({ name: 'around', id: root });
  });

  it('a_person_who_is_gone_returns_to_the_whole_tree_with_a_message_and_no_history_trap', async () => {
    const s = await mountAround('00000000-0000-4000-8000-000000000000');
    await waitFor(() => expect(s.spy).toHaveBeenCalled());
    await waitFor(() => expect(nav.route).toEqual({ name: 'whole' }));
    expect(toasts.items.map((x) => x.text)).toContain('Цієї особи немає в родоводі.');
    expect(nav.history[nav.index - 1]?.name).not.toBe("whole");
  });
});
