import { afterEach, describe, expect, it } from 'vitest';
import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Settings from './Settings.svelte';
import { renderWith, ALL_PERSONS } from '../testing/renderWith';
import { settings } from '../state/settings.svelte';
import { currentLocale, setLanguage } from '../i18n';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';

afterEach(() => document.documentElement.removeAttribute('data-theme'));

async function mount() {
  const api = new MockApi(fixtureTree(80));
  await settings.load(api);
  renderWith(Settings, {}, { api });
  return api;
}

describe('Settings', () => {
  it('theme_change_applies_immediately', async () => {
    const user = userEvent.setup();
    const api = await mount();
    await user.selectOptions(screen.getByRole('combobox', { name: 'Theme' }), 'Dark');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    await user.selectOptions(screen.getByRole('combobox', { name: 'Theme' }), 'System');
    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
    await user.selectOptions(screen.getByRole('combobox', { name: 'Theme' }), 'Light');
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
    expect((await api.getSettings()).theme).toBe('light');
  });

  it('living_threshold_clamped', async () => {
    const user = userEvent.setup();
    const api = await mount();
    const field = screen.getByRole('spinbutton', { name: 'Living threshold' });
    await user.clear(field);
    await user.type(field, '0');
    await user.tab();
    await waitFor(() => expect(field).toHaveValue(1));
    expect((await api.getSettings()).livingMaxAge).toBe(1);
    await user.clear(field);
    await user.type(field, '500');
    await user.tab();
    await waitFor(() => expect(field).toHaveValue(150));
    expect((await api.getSettings()).livingMaxAge).toBe(150);
    expect(screen.getByText('People born more than 150 years ago with no other information are treated as deceased')).toBeInTheDocument();
  });

  it('home_person_saved', async () => {
    const user = userEvent.setup();
    const api = await mount();
    const other = (await api.listPersons(ALL_PERSONS, 0, 200)).items.find((p) => p.id !== settings.value.homePerson && /^[A-Za-z ]+$/.test(p.name))!;
    const box = screen.getByRole('combobox', { name: 'Home person' });
    await user.clear(box);
    await user.type(box, other.name);
    await user.click((await screen.findAllByRole('option', { name: new RegExp(other.name) }))[0]);
    await waitFor(async () => {
      const home = (await api.getSettings()).homePerson;
      expect(home).not.toBe(null);
      expect((await api.getPerson(home!)).ref.name).toBe(other.name);
    });
  });

  it('date_order_persists', async () => {
    const user = userEvent.setup();
    const api = await mount();
    await user.selectOptions(screen.getByRole('combobox', { name: 'Date order' }), 'Month, day, year (March 4, 1901)');
    await waitFor(async () => expect((await api.getSettings()).dayOrder).toBe('mdy'));
    expect(settings.value.dayOrder).toBe('mdy');
    expect(screen.getByText('How 03/04/1901 is read when you type it')).toBeInTheDocument();
  });

  it('language_change_switches_visible_text_at_once_and_is_saved', async () => {
    const user = userEvent.setup();
    const api = await mount();
    expect(screen.getByRole('heading', { name: 'Settings' })).toBeInTheDocument();
    await user.selectOptions(screen.getByRole('combobox', { name: 'Language' }), 'Українська');
    expect(await screen.findByRole('heading', { name: 'Налаштування' })).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: 'Мова' })).toBeInTheDocument();
    expect(document.documentElement.lang).toBe('uk');
    expect((await api.getSettings()).language).toBe('uk');
  });

  it('language_survives_a_reload', async () => {
    const api = new MockApi(fixtureTree(80));
    await settings.load(api);
    await settings.update(api, { language: 'uk' });
    // A reload: a fresh start reads the saved settings again and picks the language up.
    setLanguage('en');
    await settings.load(api);
    expect(currentLocale()).toBe('uk');
    renderWith(Settings, {}, { api });
    expect(screen.getByRole('heading', { name: 'Налаштування' })).toBeInTheDocument();
  });

  it('language_options_show_each_language_by_its_own_name', async () => {
    await mount();
    const box = screen.getByRole('combobox', { name: 'Language' });
    expect(within(box).getAllByRole('option').map((o) => o.textContent)).toEqual(['Same as the system', 'English', 'Українська']);
  });
});
