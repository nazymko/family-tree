import { describe, expect, it } from 'vitest';
import { screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import AboutSection from './AboutSection.svelte';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import { setLanguage } from '../i18n';
import { shortDate, versionLine } from './about';

function mount(setup?: (api: MockApi) => void) {
  const api = new MockApi(fixtureTree(10));
  setup?.(api);
  renderWith(AboutSection, {}, { api });
  return api;
}

describe('About section', () => {
  it('shows_a_local_build_with_commit_and_date', async () => {
    mount();
    const line = await screen.findByTestId('version-line');
    expect(line.textContent).toContain('Version 0.3.0+12 (a1b2c3d, ');
    expect(line.textContent).toContain('2026');
    expect(screen.getByText('This build is newer than release 0.3.0.')).toBeTruthy();
  });

  it('shows_a_release_as_a_bare_version', async () => {
    mount((api) => (api.about = { version: '0.3.0', base: '0.3.0', release: true, commit: 'a1b2c3d', date: '2026-11-01' }));
    expect((await screen.findByTestId('version-line')).textContent).toBe('Version 0.3.0');
    expect(screen.queryByText(/newer than release/)).toBeNull();
    expect(screen.getByText(/^Built /)).toBeTruthy();
  });

  it('asks_nothing_until_the_button_is_pressed', async () => {
    const api = mount();
    await screen.findByTestId('version-line');
    expect(api.updateChecks).toBe(0);
    expect(screen.queryByRole('status')).toBeNull();
  });

  it('says_you_have_the_latest', async () => {
    const user = userEvent.setup();
    const api = mount();
    await user.click(await screen.findByRole('button', { name: 'Check for a newer version' }));
    expect(await screen.findByText('You have the latest version (0.3.0).')).toBeTruthy();
    expect(api.updateChecks).toBe(1);
  });

  it('offers_the_download_of_a_newer_version', async () => {
    const user = userEvent.setup();
    const url = 'https://github.com/nazymko/family-tree/releases/tag/v0.4.0';
    const api = mount((a) => (a.updateAnswer = { kind: 'newer', version: '0.4.0', url, published: '2026-12-01T06:00:00Z' }));
    await user.click(await screen.findByRole('button', { name: 'Check for a newer version' }));
    expect(await screen.findByText(/Version 0\.4\.0 is available\./)).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Download' }));
    await waitFor(() => expect(api.openedLinks).toEqual([url]));
  });

  it.each([
    ['offline', 'Could not reach GitHub'],
    ['server', 'GitHub could not answer just now'],
    ['badReply', 'was not understood'],
  ] as const)('tells_a_%s_failure_in_words', async (reason, text) => {
    const user = userEvent.setup();
    mount((a) => (a.updateAnswer = { kind: 'failed', reason }));
    await user.click(await screen.findByRole('button', { name: 'Check for a newer version' }));
    expect(await screen.findByText(new RegExp(text))).toBeTruthy();
  });

  it('opens_the_releases_and_the_problem_pages', async () => {
    const user = userEvent.setup();
    const api = mount();
    await user.click(await screen.findByRole('link', { name: 'Releases and downloads' }));
    await user.click(screen.getByRole('link', { name: 'Report a problem' }));
    expect(api.openedLinks).toEqual([
      'https://github.com/nazymko/family-tree/releases',
      'https://github.com/nazymko/family-tree/issues/new/choose',
    ]);
  });

  it('speaks_ukrainian', async () => {
    const user = userEvent.setup();
    setLanguage('uk');
    try {
      mount();
      expect((await screen.findByTestId('version-line')).textContent).toContain('Версія 0.3.0+12');
      await user.click(screen.getByRole('button', { name: 'Перевірити наявність новішої версії' }));
      expect(await screen.findByText('У Вас остання версія (0.3.0).')).toBeTruthy();
    } finally {
      setLanguage('en');
    }
  });
});

describe('version text', () => {
  it('formats_dates_and_leaves_odd_text', () => {
    expect(shortDate('2026-11-08')).toMatch(/2026/);
    expect(shortDate('soon')).toBe('soon');
  });
  it('leaves_out_what_is_unknown', () => {
    const base = { version: '0.1.0', base: '0.1.0', release: false, commit: null, date: null };
    expect(versionLine(base)).toBe('Version 0.1.0');
    expect(versionLine({ ...base, commit: 'abc1234' })).toBe('Version 0.1.0 (abc1234)');
  });
});
