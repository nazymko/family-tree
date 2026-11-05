import { beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Shell from './Shell.svelte';
import { apiContext } from '../api/context';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { observeApi } from '../state/observe';
import { nav } from '../state/nav.svelte';
import { project } from '../state/project.svelte';
import { toasts } from '../state/toast.svelte';
import { undoState } from '../state/undo.svelte';
import { agentChanges } from '../state/agentChanges.svelte';

async function mountShell() {
  const raw = new MockApi(fixtureTree(200));
  const api = observeApi(raw);
  const info = await api.openProject('C:/Genealogy/Test');
  project.open(info);
  nav.reset();
  nav.go({ name: 'family' });
  nav.focus(info.homePerson);
  const user = userEvent.setup();
  const platform = new MockPlatform();
  render(Shell, { context: apiContext(api, platform) });
  return { api, raw, user, platform };
}

beforeEach(() => {
  toasts.clear();
  undoState.reset();
  project.close();
});

describe('Shell', () => {
  it('sidebar_navigates_and_marks_current', async () => {
    const { user } = await mountShell();
    const navEl = screen.getByRole('navigation', { name: 'Main' });
    await user.click(within(navEl).getByRole('link', { name: 'People' }));
    expect(within(navEl).getByRole('link', { name: 'People' })).toHaveAttribute('aria-current', 'page');
    expect(within(navEl).getByRole('link', { name: 'Family' })).not.toHaveAttribute('aria-current');
    expect(nav.route.name).toBe('people');
    expect(screen.getByRole('heading', { level: 1, name: 'People' })).toBeInTheDocument();
  });

  it('change_log_is_in_the_navigation', async () => {
    const { user } = await mountShell();
    await user.click(within(screen.getByRole('navigation', { name: 'Main' })).getByRole('link', { name: 'Change log' }));
    expect(nav.route.name).toBe('log');
    expect(await screen.findByRole('heading', { level: 1, name: 'Change log' })).toBeInTheDocument();
  });

  it('ctrl_z_refused_by_the_backend_shows_why', async () => {
    const { api, raw, user } = await mountShell();
    await api.apply({ type: 'createPerson', draft: { given: 'A', surnames: ['T'], sex: 'unknown' } }, { silent: true });
    toasts.clear();
    vi.spyOn(raw, 'undo').mockRejectedValue({ code: 'conflict', message: 'An agent changed this since. Undo it from the change log first.' });
    await user.keyboard('{Control>}z{/Control}');
    expect(await screen.findByText('An agent changed this since. Undo it from the change log first.')).toBeInTheDocument();
  });

  it('an_agent_event_refreshes_the_open_view', async () => {
    const { raw, platform } = await mountShell();
    const person = (await raw.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 1)).items[0];
    const rev = project.rev;
    const badges = vi.spyOn(raw, 'unreviewedPeople');
    platform.simulateAgentBatch(raw, { changes: [{ person: person.id, note: 'Live' }] });
    expect(project.rev).toBe(rev + 1);
    expect(badges).toHaveBeenCalledOnce();
    await waitFor(() => expect(agentChanges.unreviewed.has(person.id)).toBe(true));
  });

  it('closing_the_shell_stops_listening_and_forgets_the_badges', async () => {
    const { raw, platform } = await mountShell();
    const person = (await raw.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 1)).items[0];
    platform.simulateAgentBatch(raw, { changes: [{ person: person.id, note: 'Live' }] });
    await waitFor(() => expect(agentChanges.unreviewed.size).toBe(1));
    cleanup();
    expect(agentChanges.unreviewed.size).toBe(0);
    const rev = project.rev;
    platform.simulateAgentBatch(raw, { changes: [{ person: person.id, note: 'Late' }] });
    expect(project.rev).toBe(rev);
  });

  it('close_tree_returns_to_welcome', async () => {
    const { api, user } = await mountShell();
    const spy = vi.spyOn(api, 'closeProject');
    await user.click(screen.getByRole('button', { name: 'Close tree' }));
    await waitFor(() => expect(project.info).toBeNull());
    expect(spy).toHaveBeenCalledOnce();
    expect(nav.route.name).toBe('welcome');
  });

  it('footer_shows_project_and_saved', async () => {
    await mountShell();
    expect(screen.getByText('Test')).toBeInTheDocument();
    expect(screen.getByText('200 people')).toBeInTheDocument();
    expect(screen.getByRole('status', { name: 'Save status' })).toHaveTextContent('Saved');
  });

  it('ctrl_k_opens_palette_and_enter_opens_person', async () => {
    const { user } = await mountShell();
    nav.go({ name: 'people' });
    await user.keyboard('{Control>}k{/Control}');
    const box = screen.getByRole('combobox', { name: 'Search the tree' });
    await user.type(box, 'Schmidt');
    const results = await screen.findByRole('listbox', { name: 'Search results' });
    const options = await within(results).findAllByRole('option');
    expect(options.length).toBeGreaterThan(0);
    await user.keyboard('{ArrowDown}{Enter}');
    expect(nav.route.name).toBe('family');
    expect(nav.focusPerson).toBeTruthy();
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('palette_debounced_and_limited', async () => {
    const { user, raw } = await mountShell();
    const spy = vi.spyOn(raw, 'search');
    await user.keyboard('{Control>}k{/Control}');
    await user.type(screen.getByRole('combobox', { name: 'Search the tree' }), 'Weber');
    await waitFor(() => expect(spy).toHaveBeenCalled());
    await new Promise((r) => setTimeout(r, 250));
    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy).toHaveBeenCalledWith('Weber', 20);
  });

  it('palette_empty_state_text', async () => {
    const { user } = await mountShell();
    await user.keyboard('{Control>}k{/Control}');
    await user.type(screen.getByRole('combobox', { name: 'Search the tree' }), 'Zzzqx');
    expect(
      await screen.findByText('No matches. Try a different spelling; search also finds similar-sounding names.'),
    ).toBeInTheDocument();
  });

  it('palette_search_failure_shows_hint', async () => {
    const { user, raw } = await mountShell();
    vi.spyOn(raw, 'search').mockRejectedValue(new Error('boom'));
    await user.keyboard('{Control>}k{/Control}');
    await user.type(screen.getByRole('combobox', { name: 'Search the tree' }), 'Weber');
    expect(await screen.findByRole('alert')).toHaveTextContent('Search did not work');
  });

  it('slash_and_mod_digit_ignored_while_modal_open', async () => {
    const { user } = await mountShell();
    const modal = document.createElement('div');
    modal.setAttribute('aria-modal', 'true');
    document.body.append(modal);
    await user.keyboard('{Control>}2{/Control}');
    expect(nav.route.name).toBe('family');
    await user.keyboard('/');
    expect(screen.queryByRole('combobox', { name: 'Search the tree' })).toBeNull();
    modal.remove();
    await user.keyboard('{Control>}2{/Control}');
    expect(nav.route.name).not.toBe('family');
  });

  it('undo_via_keyboard_and_toast', async () => {
    const { api, raw, user } = await mountShell();
    const spy = vi.spyOn(raw, 'undo');
    await api.apply({ type: 'createPerson', draft: { given: 'Zoe', surnames: ['Test'], sex: 'female' } });
    const toast = await screen.findByText('Person added');
    expect(toast).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Undo' })).toBeInTheDocument();
    await waitFor(() => expect(undoState.canUndo).toBe(true));
    await user.keyboard('{Control>}z{/Control}');
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
    expect(screen.getByRole('status', { name: 'Save status' })).toHaveTextContent('Saved');
  });

  it('toast_undo_button_calls_undo', async () => {
    const { api, raw, user } = await mountShell();
    const spy = vi.spyOn(raw, 'undo');
    await api.apply({ type: 'createPerson', draft: { given: 'Zoe', surnames: ['Test'], sex: 'female' } });
    await user.click(await screen.findByRole('button', { name: 'Undo' }));
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
  });

  it('shortcut_sheet_lists_all', async () => {
    const { user } = await mountShell();
    await user.keyboard('?');
    const dlg = await screen.findByRole('dialog', { name: 'Keyboard shortcuts' });
    for (const label of [
      'Search the tree',
      'Focus the list filter',
      'Undo',
      'Redo',
      'New person',
      'Edit the focus person',
      'Go to the home person',
      'Back',
      'Forward',
      'Family',
      'People',
      'Pedigree',
      'Descendants',
      'Relatives',
      'Sources',
      'Settings',
      'Save',
      'Keyboard shortcuts',
    ]) {
      expect(within(dlg).getAllByText(label).length).toBeGreaterThan(0);
    }
  });

  it('alt_arrow_goes_back', async () => {
    const { user } = await mountShell();
    nav.go({ name: 'people' });
    await user.keyboard('{Alt>}{ArrowLeft}{/Alt}');
    expect(nav.route.name).toBe('family');
  });

  it('mod_s_toasts_only_with_open_project_and_no_pending_write', async () => {
    const { user } = await mountShell();
    await user.keyboard('{Control>}s{/Control}');
    expect(toasts.items.map((t) => t.text)).toEqual(['All changes are saved']);
    toasts.clear();
    project.save = 'saving';
    await user.keyboard('{Control>}s{/Control}');
    expect(toasts.items).toEqual([]);
    project.save = 'failed';
    await user.keyboard('{Control>}s{/Control}');
    expect(toasts.items).toEqual([]);
    project.save = 'saved';
    project.info = null;
    await user.keyboard('{Control>}s{/Control}');
    expect(toasts.items).toEqual([]);
  });
});

describe('Shell tree name', () => {
  it('opens_the_folder_of_the_tree', async () => {
    const { user, platform } = await mountShell();
    await user.click(screen.getByRole('button', { name: project.info!.name }));
    expect(platform.calls).toContainEqual({ method: 'reveal', args: ['C:/Genealogy/Test/tree.db'] });
  });

  it('audio_linked_to_a_person_shows_as_a_voice_row_not_a_broken_photo', async () => {
    const { raw } = await mountShell();
    const r = await raw.apply({ type: 'createPerson', draft: { given: 'Olena', surnames: ['Koval'], sex: 'female' } });
    const db = (raw as unknown as { db: { media: Map<string, unknown>; persons: Map<string, { media: unknown[] }> } }).db;
    const m = { id: 'imp-1', title: null, mime: null, originalName: 'greeting.mp3', date: null, description: null, resolved: true, width: null, height: null, sha256: null };
    db.media.set(m.id, m);
    db.persons.get(r.focus!)!.media.push({ media: m, primary: false });
    nav.go({ name: 'person', id: r.focus! });
    await waitFor(() => expect(screen.getAllByTestId('voice-sample')).toHaveLength(1));
    expect(document.querySelector('img[src*="thumb"]')).toBeNull();
  });
});
