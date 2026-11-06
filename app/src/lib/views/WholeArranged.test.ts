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

async function open(route: Parameters<typeof nav.go>[0] = { name: 'whole' }) {
  fakeRaf();
  const made = makeApi(120);
  const arrSpy = vi.spyOn(made.api, 'chartOpenArranged');
  const allSpy = vi.spyOn(made.api, 'chartOpenWhole');
  const selSpy = vi.spyOn(made.api, 'chartOpenSelection');
  nav.go({ name: 'whole' });
  if (route.name === 'whole' && (route.around || route.people)) nav.go(route);
  render(Host, { props: { api: made.api, view: WholeTree } });
  await waitFor(() => expect(document.querySelector('canvas[role="application"]')).not.toBeNull());
  attachContext(document.querySelector('canvas[role="application"]') as HTMLCanvasElement);
  return { ...made, arrSpy, allSpy, selSpy };
}

const somebody = () => makeApi(120).fx.persons.find((p) => !p.siblingMarker)!;

describe('arranging the whole tree around a person', () => {
  it('the_button_is_off_until_a_person_is_selected_and_the_route_remembers_the_choice', async () => {
    const who = somebody();
    const s = await open({ name: 'whole', around: who.id });
    await waitFor(() => expect(s.arrSpy).toHaveBeenCalledWith(who.id, undefined, false));
    expect(s.allSpy).not.toHaveBeenCalled();
    const name = (await s.api.getPerson(who.id)).ref.name;
    await waitFor(() => expect(screen.getByTestId("arranged")).toHaveTextContent(`Arranged around ${name}`));
    // The chart starts on that person, selected, so the button works.
    const btn = screen.getByRole('button', { name: 'Arrange around this person' });
    await waitFor(() => expect(btn).toBeEnabled());
    expect(nav.route).toEqual({ name: 'whole', around: who.id });
  });

  it('pressing_it_on_the_selected_person_arranges_around_them_and_back_returns_to_the_usual_order', async () => {
    const s = await open();
    const btn = screen.getByRole('button', { name: 'Arrange around this person' });
    expect(btn).toBeDisabled();
    // Select somebody with the keyboard: Home goes to the home person.
    const canvas = document.querySelector('canvas[role="application"]') as HTMLCanvasElement;
    await fireEvent.keyDown(canvas, { key: 'Home' });
    await waitFor(() => expect(btn).toBeEnabled());
    await fireEvent.click(btn);
    await waitFor(() => expect(nav.route.name === 'whole' && nav.route.around).toBeTruthy());
    await waitFor(() => expect(s.arrSpy).toHaveBeenCalled());
    expect(await screen.findByTestId('arranged')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Usual order' }));
    expect(nav.route).toEqual({ name: 'whole' });
    await waitFor(() => expect(screen.queryByTestId('arranged')).toBeNull());
    nav.back();
    expect(nav.route.name === 'whole' && nav.route.around).toBeTruthy();
    expect(await screen.findByTestId('arranged')).toBeInTheDocument();
  });

  it('works_on_the_chart_of_chosen_people_and_keeps_them', async () => {
    const who = somebody();
    const s = await open({ name: 'whole', people: [who.id], around: who.id });
    await waitFor(() => expect(s.arrSpy).toHaveBeenCalledWith(who.id, [who.id], false));
    expect(s.selSpy).not.toHaveBeenCalled();
    expect(screen.getByRole('heading', { name: 'Tree of selected people' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Back to the whole tree' })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Usual order' }));
    expect(nav.route).toEqual({ name: 'whole', people: [who.id] });
    await waitFor(() => expect(s.selSpy).toHaveBeenCalledWith([who.id], false));
  });

  it('a_person_who_is_gone_returns_to_the_usual_order_without_a_history_trap', async () => {
    const s = await open({ name: 'whole', around: '00000000-0000-4000-8000-000000000000' });
    await waitFor(() => expect(s.arrSpy).toHaveBeenCalled());
    await waitFor(() => expect(nav.route).toEqual({ name: 'whole' }));
    await waitFor(() => expect(s.allSpy).toHaveBeenCalled());
    expect(nav.history.filter((h) => h.name === 'whole' && h.around).length).toBe(0);
  });

  it('is_a_different_route_for_another_person', () => {
    nav.go({ name: 'whole', around: 'a' });
    const n = nav.history.length;
    nav.go({ name: 'whole', around: 'a' });
    expect(nav.history.length).toBe(n);
    nav.go({ name: 'whole', around: 'b' });
    expect(nav.history.length).toBe(n + 1);
  });

  it('speaks_ukrainian', async () => {
    setLanguage('uk');
    const who = somebody();
    const s = await open({ name: 'whole', around: who.id });
    await waitFor(() => expect(s.arrSpy).toHaveBeenCalled());
    expect(screen.getByRole('button', { name: 'Побудувати навколо цієї особи' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Звичайний порядок' })).toBeInTheDocument();
    const name = (await s.api.getPerson(who.id)).ref.name;
    await waitFor(() => expect(screen.getByTestId("arranged")).toHaveTextContent(`Побудовано навколо: ${name}`));
  });
});
