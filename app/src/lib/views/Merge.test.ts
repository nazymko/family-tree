import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import SendCopyDialog from './SendCopyDialog.svelte';
import MergeDialog from './MergeDialog.svelte';
import UndoDialog from './UndoDialog.svelte';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { notices } from '../ui/notices.svelte';
import { openReceivedCopy } from '../state/openReceived';
import { setLanguage } from '../i18n';
import { editLabel } from '../i18n/backend';

beforeEach(() => {
  notices.dismiss();
  setLanguage('en');
});

const mergeProps = () => ({ onClose: vi.fn(), onShowChanges: vi.fn(), onUndo: vi.fn(), onOpenPerson: vi.fn() });

async function toPreview(user: ReturnType<typeof userEvent.setup>, platform: MockPlatform) {
  platform.next.file = 'C:/Mock/Olena Koval.zip';
  await user.click(screen.getByRole('button', { name: 'Choose a zip…' }));
  await user.click(screen.getByRole('button', { name: 'Look at it' }));
  await screen.findByText('Nothing has been changed yet.', { exact: false });
}

describe('SendCopyDialog', () => {
  it('names_what_the_copy_contains_and_its_size_before_anything_is_made', async () => {
    const api = new MockApi(fixtureTree(30));
    api.copyBytes = 3_500_000;
    renderWith(SendCopyDialog, { onClose: vi.fn() }, { api });
    expect(screen.getByText('Living people, with their names and details.')).toBeInTheDocument();
    expect(screen.getByText('Records you marked private.')).toBeInTheDocument();
    expect(screen.getByText(/edit history, including old text and text you have since removed, reports from assistants, and the names of relatives/)).toBeInTheDocument();
    await screen.findByText(/Size of the file: about 3.3 MB/);
    expect(api.sentZips).toEqual([]);
  });

  it('makes_the_zip_and_offers_to_show_it', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(30));
    const onClose = vi.fn();
    const { platform } = renderWith(SendCopyDialog, { onClose }, { api });
    platform.next.save = 'C:/Mock/Out/copy.zip';
    const make = screen.getByRole('button', { name: 'Make the zip…' });
    await waitFor(() => expect(make).toBeEnabled());
    await user.click(make);
    await waitFor(() => expect(api.sentZips).toEqual(['C:/Mock/Out/copy.zip']));
    expect(onClose).toHaveBeenCalled();
    expect(notices.current?.message).toContain('copy.zip');
  });

  it('says_it_in_ukrainian_and_never_calls_a_family_tree_a_derevo', async () => {
    setLanguage('uk');
    renderWith(SendCopyDialog, { onClose: vi.fn() }, { api: new MockApi(fixtureTree(30)) });
    expect(screen.getByText('Живі люди з іменами та відомостями про них.')).toBeInTheDocument();
    expect(screen.getByText(/Історія змін, разом зі старим текстом.*звіти помічників та імена родичів/)).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/дерев/i);
  });
});

describe('MergeDialog', () => {
  it('shows_the_preview_and_changes_nothing_until_merge_is_pressed', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(30));
    const { platform } = renderWith(MergeDialog, mergeProps(), { api });
    await toPreview(user, platform);
    expect(api.mergeApplied).toEqual([]);
    expect(api.mergeSources).toEqual(['C:/Mock/Olena Koval.zip']);
    for (const h of [/^New in the copy/, /^Changed by your relative/, /^Needs deciding/, /^Removed in the copy/, /^Cannot come across/]) {
      expect(screen.getByRole('heading', { name: h })).toBeInTheDocument();
    }
    expect(screen.getByText('Both of you changed this.')).toBeInTheDocument();
    expect(screen.getByText('Not applied. A removal is never taken automatically.')).toBeInTheDocument();
    expect(screen.getByText('Might be someone already in your tree:')).toBeInTheDocument();
  });

  it('suggests_the_name_from_the_file_and_needs_one', async () => {
    const user = userEvent.setup();
    const { platform } = renderWith(MergeDialog, mergeProps(), { api: new MockApi(fixtureTree(30)) });
    expect(screen.getByRole('button', { name: 'Look at it' })).toBeDisabled();
    platform.next.file = 'C:/Mock/Olena Koval.zip';
    await user.click(screen.getByRole('button', { name: 'Choose a zip…' }));
    expect(screen.getByRole('textbox', { name: /Whose copy is this/ })).toHaveValue('Olena Koval');
  });

  it('applies_then_offers_undo_and_the_change_log', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(30));
    const props = mergeProps();
    const { platform } = renderWith(MergeDialog, props, { api });
    await toPreview(user, platform);
    await user.click(screen.getByRole('button', { name: 'Merge 5' }));
    await screen.findByText(/A backup of your tree was made first/);
    expect(api.mergeApplied).toHaveLength(1);
    expect(screen.getByText(/still waiting for you/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Undo this merge' }));
    expect(props.onUndo).toHaveBeenCalledWith(expect.any(String), 'Olena Koval');
    await user.click(screen.getByRole('button', { name: 'Show the changes' }));
    expect(props.onShowChanges).toHaveBeenCalled();
  });

  it('reads_the_copy_again_when_the_tree_changed_after_the_preview', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(30));
    const { platform } = renderWith(MergeDialog, mergeProps(), { api });
    await toPreview(user, platform);
    api.mergeStaleOnce = true;
    await user.click(screen.getByRole('button', { name: 'Merge 5' }));
    await screen.findByText(/Your tree changed after the copy was looked at/);
    expect(api.mergeApplied).toEqual([]);
    expect(api.mergeSources).toHaveLength(2);
    await user.click(screen.getByRole('button', { name: 'Merge 5' }));
    await screen.findByText(/A backup of your tree was made first/);
  });

  it('refuses_a_broken_copy_with_a_plain_message_and_stays_on_the_first_step', async () => {
    const user = userEvent.setup();
    const { platform } = renderWith(MergeDialog, mergeProps(), { api: new MockApi(fixtureTree(30)) });
    platform.next.file = 'C:/Mock/broken.zip';
    await user.click(screen.getByRole('button', { name: 'Choose a zip…' }));
    await user.click(screen.getByRole('button', { name: 'Look at it' }));
    await screen.findByRole('alert');
    expect(screen.getByRole('button', { name: 'Look at it' })).toBeInTheDocument();
  });

  it('cancelling_a_preview_tells_the_backend_to_drop_it', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(30));
    const spy = vi.spyOn(api, 'mergeCancel');
    const props = mergeProps();
    const { platform } = renderWith(MergeDialog, props, { api });
    await toPreview(user, platform);
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(spy).toHaveBeenCalled();
    expect(props.onClose).toHaveBeenCalled();
  });

  it('has_nothing_to_merge_when_the_copy_adds_nothing', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(30));
    const { platform } = renderWith(MergeDialog, mergeProps(), { api });
    await toPreview(user, platform);
    api.mergeFixture = null;
    // A fixture with nothing in it.
    const empty = { ...(await api.mergePreview('x', 'y')), added: [], changed: [], undecided: [], removed: [], blocked: [], toApply: 0, nothingNew: true };
    api.mergeFixture = empty;
    const view = renderWith(MergeDialog, mergeProps(), { api, platform });
    const dialogs = screen.getAllByRole('dialog');
    const mine = within(dialogs[dialogs.length - 1]);
    platform.next.file = 'C:/Mock/Empty.zip';
    await user.click(mine.getByRole('button', { name: 'Choose a zip…' }));
    await user.click(mine.getByRole('button', { name: 'Look at it' }));
    await mine.findByText('There is nothing new in this copy.');
    expect(mine.getByRole('button', { name: 'Nothing to merge' })).toBeDisabled();
    view.unmount();
  });

  it('speaks_ukrainian_in_the_preview', async () => {
    setLanguage('uk');
    const user = userEvent.setup();
    const { platform } = renderWith(MergeDialog, mergeProps(), { api: new MockApi(fixtureTree(30)) });
    platform.next.file = 'C:/Mock/Олена.zip';
    await user.click(screen.getByRole('button', { name: 'Вибрати zip…' }));
    await user.click(screen.getByRole('button', { name: 'Переглянути' }));
    await screen.findByText(/Поки що нічого не змінено/);
    expect(screen.getByRole('heading', { name: /^Потребує рішення/ })).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/дерев/i);
  });
});

describe('UndoDialog for a merge', () => {
  it('undoes_through_undoMerge_and_reports', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(30));
    const { platform } = renderWith(MergeDialog, mergeProps(), { api });
    await toPreview(user, platform);
    await user.click(screen.getByRole('button', { name: 'Merge 5' }));
    await screen.findByText(/A backup of your tree was made first/);
    const batch = api.mergeApplied.length ? [...(api as unknown as { merges: Map<string, number> }).merges.keys()][0] : '';
    const spy = vi.spyOn(api, 'undoMerge');
    renderWith(UndoDialog, { pending: { kind: 'merge', id: batch, title: 'Olena Koval' }, onclose: vi.fn() }, { api });
    const dialogs = screen.getAllByRole('dialog');
    const mine = within(dialogs[dialogs.length - 1]);
    expect(mine.getByText(/Everything that came from Olena Koval is taken back/)).toBeInTheDocument();
    await user.click(mine.getByRole('button', { name: 'Undo' }));
    await waitFor(() => expect(spy).toHaveBeenCalledWith(batch));
  });
});

describe('openReceivedCopy', () => {
  it('picks_a_zip_then_a_folder_and_opens_it', async () => {
    const api = new MockApi(fixtureTree(30));
    const platform = new MockPlatform();
    platform.next.file = 'C:/Mock/from-olena.zip';
    platform.next.folder = 'C:/Mock/Olena copy';
    expect(await openReceivedCopy(api, platform)).toBeNull();
    expect(api.receivedCopies).toEqual([{ zip: 'C:/Mock/from-olena.zip', dest: 'C:/Mock/Olena copy' }]);
  });

  it('does_nothing_when_the_person_backs_out', async () => {
    const api = new MockApi(fixtureTree(30));
    const platform = new MockPlatform();
    platform.next.file = null;
    expect(await openReceivedCopy(api, platform)).toBeNull();
    platform.next.file = 'C:/Mock/a.zip';
    platform.next.folder = null;
    expect(await openReceivedCopy(api, platform)).toBeNull();
    expect(api.receivedCopies).toEqual([]);
  });

  it('returns_a_plain_message_for_a_file_that_is_not_a_copy', async () => {
    const api = new MockApi(fixtureTree(30));
    const platform = new MockPlatform();
    platform.next.file = 'C:/Mock/notes.txt';
    platform.next.folder = 'C:/Mock/x';
    expect(await openReceivedCopy(api, platform)).toMatch(/could not be opened as a copy/);
  });
});

describe('the change log words', () => {
  it('words_the_merge_label_in_both_languages', () => {
    expect(editLabel('Merged copy: Olena Koval')).toBe('Merged copy: Olena Koval');
    setLanguage('uk');
    expect(editLabel('Merged copy: Olena Koval')).toBe('Об\'єднано копію: Olena Koval');
  });
});
