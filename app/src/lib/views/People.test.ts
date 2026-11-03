import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import People from './People.svelte';
import ToastHost from '../ui/ToastHost.svelte';
import { apiContext } from '../api/context';
import type { PersonQuery } from '../api/api';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { observeApi } from '../state/observe';
import { nav } from '../state/nav.svelte';
import { project } from '../state/project.svelte';
import { toasts } from '../state/toast.svelte';
import { ui } from '../state/ui.svelte';

const BASE: PersonQuery = {
  sort: 'surname',
  desc: false,
  text: '',
  living: 'any',
  noSource: false,
  noBirthDate: false,
  privateOnly: false,
};

// Building 5000 people is slow; share one fixture and take a fresh MockApi (structuredClone) per test.
const fixture = fixtureTree(5000);

async function setup() {
  const raw = new MockApi(fixture);
  const api = observeApi(raw);
  project.open(await api.openProject('C:/Genealogy/Test'));
  nav.reset();
  nav.go({ name: 'people' });
  const user = userEvent.setup();
  const view = render(People, { context: apiContext(api, new MockPlatform()) });
  return { api, raw, user, view };
}

beforeEach(() => {
  toasts.clear();
  project.close();
});

const rows = () => {
  const box = screen.queryByRole('listbox', { name: 'People' });
  return box ? within(box).queryAllByRole('option') : [];
};

describe('People', () => {
  it('initial_page_shows_first_surnames_sorted', async () => {
    const { raw } = await setup();
    const page = await raw.listPersons(BASE, 0, 5);
    await waitFor(() => expect(rows().length).toBeGreaterThan(5));
    for (let i = 0; i < 5; i++) expect(rows()[i]).toHaveTextContent(page.items[i].name);
    expect(await screen.findByText('5,000')).toBeInTheDocument();
  });

  it('filter_text_debounced_single_call', async () => {
    const { raw, user } = await setup();
    await waitFor(() => expect(rows().length).toBeGreaterThan(0));
    const spy = vi.spyOn(raw, 'listPersons');
    await user.type(screen.getByLabelText('Filter by name'), 'Weber');
    await waitFor(() => expect(spy).toHaveBeenCalled());
    await new Promise((r) => setTimeout(r, 300));
    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy.mock.calls[0][0].text).toBe('Weber');
    expect(spy.mock.calls[0][2]).toBe(200);
  });

  it('living_filter_only_living', async () => {
    const { user } = await setup();
    await waitFor(() => expect(rows().length).toBeGreaterThan(0));
    await user.click(screen.getByRole('button', { name: 'Living' }));
    await waitFor(() => {
      const r = rows();
      expect(r.length).toBeGreaterThan(0);
      for (const row of r) expect(within(row).getByText('Living')).toBeInTheDocument();
    });
  });

  it('letter_rail_jumps', async () => {
    const { raw, user } = await setup();
    const page = await raw.listPersons(BASE, 0, 1);
    const m = page.letters!.find((l) => l.letter === 'M')!;
    const expected = (await raw.listPersons(BASE, m.offset, 1)).items[0];
    await waitFor(() => expect(rows().length).toBeGreaterThan(0));
    await user.click(screen.getByRole('button', { name: 'M' }));
    await waitFor(() => {
      const first = rows().find((r) => r.getAttribute('aria-rowindex') === String(m.offset + 1));
      expect(first).toHaveTextContent(expected.name);
    });
    expect(expected.surname.charAt(0).toUpperCase()).toBe('M');
  });

  it('rev_change_reloads_in_place_without_reset', async () => {
    const { raw, user } = await setup();
    await waitFor(() => expect(rows().length).toBeGreaterThan(0));
    await user.click(screen.getByRole('button', { name: 'M' }));
    const box = screen.getByRole('listbox', { name: 'People' });
    await waitFor(() => expect(rows().some((r) => Number(r.getAttribute('aria-rowindex')) > 100)).toBe(true));
    const spy = vi.spyOn(raw, 'listPersons');
    project.rev++;
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(screen.getByRole('listbox', { name: 'People' })).toBe(box);
    await waitFor(() => expect(rows().some((r) => Number(r.getAttribute('aria-rowindex')) > 100)).toBe(true));
    expect(rows().some((r) => r.getAttribute('aria-rowindex') === '1')).toBe(false);
  });

  it('rtl_and_cjk_names_render_in_bdi_in_order', async () => {
    const { raw, user } = await setup();
    for (const text of ['יעקב', 'Иван', '王']) {
      const q = { ...BASE, text };
      const expected = (await raw.listPersons(q, 0, 200)).items;
      if (expected.length === 0) continue;
      const box = screen.getByLabelText('Filter by name');
      await user.clear(box);
      await user.type(box, text);
      await waitFor(() => expect(screen.getByText(`${expected.length}`)).toBeInTheDocument());
      const shown = rows().map((r) => r.querySelector('bdi[dir="auto"]')?.textContent);
      expect(shown.slice(0, 8)).toEqual(expected.slice(0, 8).map((e) => e.name));
    }
  });

  it('other_scripts_bucket_exists', async () => {
    await setup();
    await waitFor(() => expect(rows().length).toBeGreaterThan(0));
    const other = screen.getByRole('button', { name: 'Other scripts' });
    expect(other).toBeEnabled();
  });

  it('cyrillic_tree_rail_shows_its_own_letters', async () => {
    const raw = new MockApi(fixtureTree(80));
    const api = observeApi(raw);
    const real = api.listPersons.bind(api);
    api.listPersons = async (q, offset, limit) => ({
      ...(await real(q, offset, limit)),
      letters: [
        { letter: 'В', offset: 0 },
        { letter: 'Г', offset: 3 },
        { letter: 'К', offset: 9 },
        { letter: '?', offset: 40 },
      ],
    });
    project.open(await raw.openProject('C:/x'));
    nav.reset();
    nav.go({ name: 'people' });
    render(People, { context: apiContext(api, new MockPlatform()) });
    const bar = await screen.findByRole('navigation', { name: 'Jump to letter' });
    const labels = within(bar)
      .getAllByRole('button')
      .map((b) => b.textContent?.trim());
    expect(labels).toEqual(['В', 'Г', 'К', 'Other']);
    expect(within(bar).getByRole('button', { name: 'К' })).toBeEnabled();
  });

  it('empty_filter_result_text', async () => {
    const { user } = await setup();
    await user.type(screen.getByLabelText('Filter by name'), 'Zzzqxj');
    expect(await screen.findByText(/Nobody matches these filters\./)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Clear filters' }));
    await waitFor(() => expect(rows().length).toBeGreaterThan(0));
  });

  it('empty_tree_offers_add_first_person', async () => {
    const raw = new MockApi(fixtureTree(80));
    const api = observeApi(raw);
    api.listPersons = async () => ({ total: 0, offset: 0, items: [], letters: [] });
    project.open(await raw.openProject('C:/x'));
    const user = userEvent.setup();
    render(People, { context: apiContext(api, new MockPlatform()) });
    expect(await screen.findByText('This tree is empty. Add the first person.')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Add person' }));
    expect(ui.add.open).toBe(true);
    ui.closeAdd();
  });

  it('enter_opens_family_view', async () => {
    const { user } = await setup();
    await waitFor(() => expect(rows().length).toBeGreaterThan(0));
    screen.getByRole('listbox', { name: 'People' }).focus();
    await user.keyboard('{ArrowDown}{Enter}');
    expect(nav.route.name).toBe('family');
    expect(nav.focusPerson).toBeTruthy();
  });

  it('delete_asks_then_toasts_undo', async () => {
    const { user, raw } = await setup();
    render(ToastHost);
    await waitFor(() => expect(rows().length).toBeGreaterThan(0));
    const spy = vi.spyOn(raw, 'apply');
    await user.click(rows()[2]);
    screen.getByRole('listbox', { name: 'People' }).focus();
    await user.keyboard('{Delete}');
    const dlg = await screen.findByRole('dialog', { name: 'Delete person' });
    expect(spy).not.toHaveBeenCalled();
    await user.click(within(dlg).getByRole('button', { name: 'Delete person' }));
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
    expect(spy.mock.calls[0][0]).toMatchObject({ type: 'deletePerson' });
    expect(await screen.findByText('Person deleted')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Undo' })).toBeInTheDocument();
  });

  it('home_person_has_ribbon_with_label', async () => {
    const { raw } = await setup();
    const home = (await raw.getSettings()).homePerson!;
    const detail = await raw.getPerson(home);
    const q = { ...BASE, text: detail.ref.name };
    const user = userEvent.setup();
    await waitFor(() => expect(rows().length).toBeGreaterThan(0));
    await user.type(screen.getByLabelText('Filter by name'), detail.ref.name);
    await waitFor(() => expect(screen.getByRole('img', { name: 'Home person' })).toBeInTheDocument());
    void q;
  });
});
