import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import ExportDialog from './ExportDialog.svelte';
import { renderWith, ALL_PERSONS } from '../testing/renderWith';
import { PrivacyMockApi } from '../api/mock/mockPrivacy';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { notices } from '../ui/notices.svelte';
import type { ExportOptions, PrivacyPreview } from '../api/api';

beforeEach(() => notices.dismiss());

const WARN = "Living people's details will be written to the file.";
const exportBtn = () => screen.getByRole('button', { name: 'Export…' });
const stat = (label: string) => {
  const region = screen.getByRole('region', { name: 'Privacy preview' });
  return Number(within(region).getByText(label).parentElement!.querySelector('dd')!.textContent);
};

/** previewExport calls wait until `release()`; each call is answered by the real mock when released. */
function gated(api: PrivacyMockApi) {
  const real = api.previewExport.bind(api);
  const pending: { opts: ExportOptions; go: () => void }[] = [];
  vi.spyOn(api, 'previewExport').mockImplementation(
    (opts) =>
      new Promise<PrivacyPreview>((resolve, reject) => {
        pending.push({ opts, go: () => real(opts).then(resolve, reject) });
      }),
  );
  return { pending };
}

describe('ExportDialog', () => {
  it('default_options_are_safest', async () => {
    renderWith(ExportDialog, { onClose: vi.fn() }, { api: new PrivacyMockApi(fixtureTree(80)) });
    expect(screen.getByRole('radio', { name: 'Show as Living (default)' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'Include records marked private' })).not.toBeChecked();
    expect(screen.getByText('Photos are not included in a GEDCOM file.')).toBeInTheDocument();
  });

  it('export_disabled_until_preview_matches_options', async () => {
    const user = userEvent.setup();
    const api = new PrivacyMockApi(fixtureTree(80));
    const { pending } = gated(api);
    const spy = vi.spyOn(api, 'exportGedcom');
    renderWith(ExportDialog, { onClose: vi.fn() }, { api });
    expect(exportBtn()).toBeDisabled();
    pending.shift()!.go();
    await waitFor(() => expect(exportBtn()).toBeEnabled());

    await user.click(screen.getByRole('radio', { name: 'Leave out' }));
    expect(exportBtn()).toBeDisabled();
    const second = pending.shift()!;
    expect(second.opts.living).toBe('exclude');
    second.go();
    await waitFor(() => expect(exportBtn()).toBeEnabled());

    await user.click(screen.getByRole('checkbox', { name: 'Include records marked private' }));
    expect(exportBtn()).toBeDisabled();
    const third = pending.shift()!;
    const expected = await new PrivacyMockApi(fixtureTree(80)).previewExport(third.opts);
    third.go();
    await waitFor(() => expect(exportBtn()).toBeEnabled());

    await user.click(exportBtn());
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(spy.mock.calls[0][0]).toEqual({ version: '7.0', living: 'exclude', includePrivate: true });
    expect(spy.mock.calls[0][2]).toBe(expected.token);
    await waitFor(() => expect(notices.current?.message).toMatch(/^Exported \d+ KB to Family tree\.ged$/));
    expect(notices.current?.actionLabel).toBe('Reveal');
  });

  it('stale_preview_response_ignored', async () => {
    const user = userEvent.setup();
    const api = new PrivacyMockApi(fixtureTree(80));
    const { pending } = gated(api);
    renderWith(ExportDialog, { onClose: vi.fn() }, { api });
    const first = pending.shift()!; // placeholder
    await user.click(screen.getByRole('radio', { name: 'Leave out' }));
    const second = pending.shift()!; // exclude
    second.go();
    await waitFor(() => expect(exportBtn()).toBeEnabled());
    const shownAsLiving = stat("Shown as 'Living'");
    const leftOut = stat('Left out');
    expect(shownAsLiving).toBe(0);
    expect(leftOut).toBeGreaterThan(0);
    first.go(); // the older answer arrives late
    await new Promise((r) => setTimeout(r, 20));
    expect(stat("Shown as 'Living'")).toBe(0);
    expect(stat('Left out')).toBe(leftOut);
    expect(exportBtn()).toBeEnabled();
  });

  it('undated_person_counted_as_placeholder', async () => {
    const api = new PrivacyMockApi(fixtureTree(80));
    const p = await api.previewExport({ version: '7.0', living: 'placeholder', includePrivate: false });
    expect(p.placeholderPersons).toBeGreaterThanOrEqual(1);
    renderWith(ExportDialog, { onClose: vi.fn() }, { api });
    await waitFor(() => expect(exportBtn()).toBeEnabled());
    expect(stat("Shown as 'Living'")).toBe(p.placeholderPersons);
    expect(stat('Named')).toBe(p.exportedPersons);
  });

  it('include_all_shows_warning', async () => {
    const user = userEvent.setup();
    renderWith(ExportDialog, { onClose: vi.fn() }, { api: new PrivacyMockApi(fixtureTree(80)) });
    expect(screen.queryByText(WARN)).toBeNull();
    await user.click(screen.getByRole('radio', { name: 'Include everyone' }));
    expect(screen.getByText(WARN)).toBeInTheDocument();
    await user.click(screen.getByRole('radio', { name: 'Show as Living (default)' }));
    expect(screen.queryByText(WARN)).toBeNull();
    await user.click(screen.getByRole('checkbox', { name: 'Include records marked private' }));
    expect(screen.getByText(WARN)).toBeInTheDocument();
  });

  it('version_defaults_from_source', async () => {
    const api = new PrivacyMockApi(fixtureTree(80), { defaultVersion: '5.5.1' });
    renderWith(ExportDialog, { onClose: vi.fn() }, { api });
    await waitFor(() => expect(screen.getByRole('radio', { name: 'GEDCOM 5.5.1' })).toBeChecked());
    expect(screen.getByRole('radio', { name: 'GEDCOM 7.0' })).not.toBeChecked();
    expect(screen.getByText('Defaults to the version your file came in, so your other programs can read it.')).toBeInTheDocument();
  });

  it('lossless_note_only_for_551', async () => {
    const user = userEvent.setup();
    const api = new PrivacyMockApi(fixtureTree(80), { defaultVersion: '5.5.1' });
    renderWith(ExportDialog, { onClose: vi.fn() }, { api });
    const note = "Roles, 'did not happen' records, shared notes and external IDs are written as extension tags; other programs may drop them.";
    expect(await screen.findByText(note)).toBeInTheDocument();
    await user.click(screen.getByRole('radio', { name: 'GEDCOM 7.0' }));
    await waitFor(() => expect(exportBtn()).toBeEnabled());
    expect(screen.queryByText(note)).toBeNull();
  });

  it('mention_warnings_listed_with_best_effort_text', async () => {
    const api = new PrivacyMockApi(fixtureTree(80));
    // The fixture's undated person is living under the fail-safe rule.
    const dead = (await api.listPersons({ ...ALL_PERSONS, living: 'deceased' }, 0, 1)).items[0];
    const name = 'Uma Undated';
    await api.apply({ type: 'saveNote', target: { kind: 'person', id: dead.id }, note: { kind: 'general', text: `Looked after by ${name} until 1950.` } });
    renderWith(ExportDialog, { onClose: vi.fn() }, { api });
    await screen.findByRole('heading', { name: 'Notes that may name hidden people' });
    const item = screen.getByText(/Looked after by .* mentions/);
    expect(item).toHaveTextContent(`mentions ${name}`);
    expect(screen.getByText(/This check is best effort: nicknames and initials are not caught\./)).toBeInTheDocument();
  });

  it('save_copy_has_unfiltered_warning', async () => {
    const user = userEvent.setup();
    const api = new PrivacyMockApi(fixtureTree(80));
    const spy = vi.spyOn(api, 'saveCopy');
    const platform = new MockPlatform();
    platform.next = { folder: 'D:/Backups/Tree copy' };
    renderWith(ExportDialog, { onClose: vi.fn() }, { api, platform });
    expect(screen.getByText('Save a copy of the whole tree')).toBeInTheDocument();
    expect(screen.getByText('This copy is not filtered. It includes living people, private records and history; keep it to yourself.')).toBeInTheDocument();
    expect(screen.getByText(/The copy is a tree folder, not a zip file/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Save a copy to a folder…' }));
    await waitFor(() => expect(spy).toHaveBeenCalledWith('D:/Backups/Tree copy'));
    expect(platform.calls.some((c) => c.method === 'pickFolder')).toBe(true);
    expect(platform.calls.some((c) => c.method === 'pickSavePath')).toBe(false);
  });

  it('export_failure_shows_error_and_keeps_dialog', async () => {
    const user = userEvent.setup();
    const api = new PrivacyMockApi(fixtureTree(80));
    vi.spyOn(api, 'exportGedcom').mockRejectedValue(new Error('The disk is full. Free some space and export again.'));
    const onClose = vi.fn();
    renderWith(ExportDialog, { onClose }, { api });
    await waitFor(() => expect(exportBtn()).toBeEnabled());
    await user.click(exportBtn());
    expect(await screen.findByRole('alert')).toHaveTextContent('The disk is full. Free some space and export again.');
    expect(screen.getByRole('dialog', { name: 'Export' })).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });
});
