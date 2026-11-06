import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Host from '../chart/testing/Host.svelte';
import { attachContext, fakeRaf, makeApi } from '../chart/testing/helpers';
import { setLanguage } from '../i18n';
import { nav } from '../state/nav.svelte';
import WholeTree from './WholeTree.svelte';

beforeEach(() => nav.reset());
afterEach(() => {
  vi.unstubAllGlobals();
  setLanguage('en');
});

async function open(people: string[]) {
  fakeRaf();
  const made = makeApi(120);
  const selSpy = vi.spyOn(made.api, 'chartOpenSelection');
  const allSpy = vi.spyOn(made.api, 'chartOpenWhole');
  nav.go({ name: 'whole' });
  if (people.length) nav.go({ name: 'whole', people });
  render(Host, { props: { api: made.api, view: WholeTree } });
  await waitFor(() => expect(document.querySelector('canvas[role="application"]')).not.toBeNull());
  attachContext(document.querySelector('canvas[role="application"]') as HTMLCanvasElement);
  return { ...made, selSpy, allSpy };
}

/** Two people who share nothing: the first and the last of the fixture. */
const pair = () => {
  const { fx } = makeApi(120);
  return [fx.persons[0].id, fx.persons[fx.persons.length - 1].id];
};

describe('WholeTree with chosen people', () => {
  it('shows_the_header_with_counts_the_names_and_a_way_back', async () => {
    const people = pair();
    const s = await open(people);
    expect(screen.getByRole('heading', { name: 'Tree of selected people' })).toBeInTheDocument();
    await waitFor(() => expect(s.selSpy).toHaveBeenCalledWith(people, false));
    expect(s.allSpy).not.toHaveBeenCalled();
    const first = (await s.api.getPerson(people[0])).ref.name;
    await waitFor(() => expect(screen.getByTestId('chosen')).toHaveTextContent(`2 people selected: ${first}`));
    expect(await screen.findByText(/\d+ people? in \d+ groups?$/)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Back to the whole tree' }));
    expect(nav.route).toEqual({ name: 'whole' });
    expect(await screen.findByRole('heading', { name: 'Whole tree' })).toBeInTheDocument();
    expect(screen.queryByRole('checkbox', { name: 'Show all descendants' })).toBeNull();
  });

  it('the_descendants_box_is_off_at_first_and_lays_the_chart_out_again_when_ticked', async () => {
    const people = pair();
    const s = await open(people);
    const box = await screen.findByRole('checkbox', { name: 'Show all descendants' });
    expect(box).not.toBeChecked();
    await waitFor(() => expect(s.selSpy).toHaveBeenCalledWith(people, false));
    await fireEvent.click(box);
    await waitFor(() => expect(s.selSpy).toHaveBeenLastCalledWith(people, true));
    await fireEvent.click(box);
    await waitFor(() => expect(s.selSpy).toHaveBeenLastCalledWith(people, false));
  });

  it('speaks_ukrainian_with_родовід_and_the_right_plural', async () => {
    setLanguage('uk');
    const s = await open(pair());
    expect(screen.getByRole('heading', { name: 'Родовід вибраних осіб' })).toBeInTheDocument();
    await waitFor(() => expect(s.selSpy).toHaveBeenCalled());
    expect(screen.getByTestId('chosen')).toHaveTextContent('2 вибрані особи');
    expect(screen.getByRole('checkbox', { name: 'Показати всіх нащадків' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Назад до всього родоводу' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Показати в родоводі' })).toBeInTheDocument();
  });

  it('back_returns_to_the_whole_tree_and_forward_to_the_chosen_people', async () => {
    const people = pair();
    await open(people);
    expect(nav.route).toEqual({ name: 'whole', people });
    nav.back();
    expect(nav.route).toEqual({ name: 'whole' });
    expect(await screen.findByRole('heading', { name: 'Whole tree' })).toBeInTheDocument();
    nav.forward();
    expect(nav.route).toEqual({ name: 'whole', people });
    expect(await screen.findByRole('heading', { name: 'Tree of selected people' })).toBeInTheDocument();
    // The same people again are not a new history entry; other people are.
    const n = nav.history.length;
    nav.go({ name: 'whole', people: [...people] });
    expect(nav.history.length).toBe(n);
    nav.go({ name: 'whole', people: [people[0]] });
    expect(nav.history.length).toBe(n + 1);
  });

  it('when_everyone_chosen_is_gone_it_returns_to_the_whole_tree_without_a_history_trap', async () => {
    const s = await open(['00000000-0000-4000-8000-000000000000']);
    await waitFor(() => expect(s.selSpy).toHaveBeenCalled());
    await waitFor(() => expect(nav.route).toEqual({ name: 'whole' }));
    expect(await screen.findByRole('heading', { name: 'Whole tree' })).toBeInTheDocument();
    expect(nav.history.at(-1)).toEqual({ name: 'whole' });
    // Back goes to where the user came from, not into the broken chart again.
    expect(nav.history.filter((h) => h.name === 'whole' && h.people).length).toBe(0);
  });
});
