import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Host from '../chart/testing/Host.svelte';
import { attachContext, fakeRaf, makeApi } from '../chart/testing/helpers';
import { nav } from '../state/nav.svelte';
import WholeTree from './WholeTree.svelte';

beforeEach(() => nav.reset());
afterEach(() => vi.unstubAllGlobals());

describe('WholeTree view', () => {
  it('shows_the_heading_and_a_people_count_and_opens_the_chart', async () => {
    fakeRaf();
    const { api, fx } = makeApi(120);
    render(Host, { props: { api, view: WholeTree } });
    expect(screen.getByRole('heading', { name: 'Whole tree' })).toBeInTheDocument();
    attachContext(await screen.findByRole('application', { name: 'Whole tree chart' }) as HTMLCanvasElement);
    expect(await screen.findByText(/people in \d+ groups?$/)).toBeInTheDocument();
    expect(screen.getByText(/people in/).textContent).toContain(String(fx.persons.length));
  });

  it('show_in_pedigree_focuses_the_person_and_goes_to_the_pedigree', async () => {
    const mm = window.matchMedia;
    window.matchMedia = ((q: string) => ({ ...mm(q), matches: q.includes('reduce') })) as typeof window.matchMedia;
    const raf = fakeRaf();
    const { api, fx } = makeApi(120);
    render(Host, { props: { api, view: WholeTree } });
    attachContext(await screen.findByRole('application', { name: 'Whole tree chart' }) as HTMLCanvasElement);
    const ref = (await api.getPerson(fx.persons[2].id)).ref;
    const box = await screen.findByRole('combobox', { name: 'Find a person…' });
    await fireEvent.input(box, { target: { value: ref.name } });
    const opts = await screen.findAllByRole('option', undefined, { timeout: 2000 });
    await fireEvent.click(opts.find((o) => o.textContent?.includes(ref.name))!);
    const btn = screen.getByRole('button', { name: 'Show in Pedigree' });
    await waitFor(() => {
      raf.pump(2);
      expect(btn).toBeEnabled();
    });
    await fireEvent.click(btn);
    expect(nav.route.name).toBe('pedigree');
    expect(nav.focusPerson).toBe(ref.id);
  });
});
