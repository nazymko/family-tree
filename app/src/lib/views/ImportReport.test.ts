import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import ImportReport from './ImportReport.svelte';
import { apiContext } from '../api/context';
import type { Api, ImportReportDto } from '../api/api';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { nav } from '../state/nav.svelte';
import { project } from '../state/project.svelte';
import { toasts } from '../state/toast.svelte';

async function setup(patch: Partial<ImportReportDto> = {}, apiPatch: Partial<Api> = {}) {
  const base = new MockApi(fixtureTree(80));
  const api = Object.assign(Object.create(base), apiPatch) as Api;
  const report = { ...(await base.importGedcom({ file: 'a.ged', projectDir: 'C:/Users/me/OneDrive/Trees/x', projectName: 'X', mediaRoots: [], dayOrder: 'dmy' })), ...patch };
  project.open(report.project, report);
  nav.reset();
  nav.go({ name: 'importReport' });
  const platform = new MockPlatform();
  const user = userEvent.setup();
  render(ImportReport, { context: apiContext(api, platform) });
  return { api, platform, user, base };
}

beforeEach(() => {
  project.close();
  toasts.clear();
});

describe('ImportReport', () => {
  it('heading_and_counts', async () => {
    await setup();
    expect(screen.getByRole('heading', { level: 1, name: 'Imported 80 people from Mock Genealogy 4.2' })).toBeInTheDocument();
    const table = screen.getByRole('table', { name: 'What was imported' });
    expect(within(table).getByRole('rowheader', { name: 'People' })).toBeInTheDocument();
  });

  it('report_sections_hidden_when_empty', async () => {
    await setup({ repairs: [], unmappedTags: {}, unparsedDates: [], missingMedia: [] });
    expect(screen.queryByText('Repairs')).toBeNull();
    expect(screen.queryByText('Dates we could not read')).toBeNull();
    expect(screen.queryByText('Tags we kept but did not understand')).toBeNull();
    expect(screen.queryByText('Photos and files we could not find')).toBeNull();
  });

  it('sections_open_when_non_empty', async () => {
    await setup();
    const repairs = screen.getByText('Repairs').closest('details')!;
    expect(repairs).toHaveAttribute('open');
    expect(within(repairs).getByText(/Line 1204/)).toBeInTheDocument();
    expect(screen.getByText('_FSFTID')).toBeInTheDocument();
  });

  it('report_shows_location_warning', async () => {
    await setup();
    expect(
      screen.getByText(
        'This folder is inside OneDrive. Syncing can corrupt the database. Keep the tree outside synced folders and use backups to move it.',
      ),
    ).toBeInTheDocument();
  });

  it('report_unparsed_dates_verbatim_rtl', async () => {
    await setup({ unparsedDates: [{ line: 12, text: 'ט״ו באב' }] });
    const el = screen.getByText('ט״ו באב');
    expect(el.tagName).toBe('BDI');
    expect(el).toHaveAttribute('dir', 'auto');
  });

  it('locate_media_updates_list', async () => {
    const { user } = await setup({}, { relinkMedia: async () => ({ relinked: 2, stillMissing: ['photos/farm.jpg'] }) });
    expect(screen.getByText('photos/wedding-1878.jpg')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Locate media folder' }));
    await waitFor(() => expect(screen.queryByText('photos/wedding-1878.jpg')).toBeNull());
    expect(screen.getByText('photos/farm.jpg')).toBeInTheDocument();
    expect(toasts.items.map((t) => t.text)).toContain('Linked 2 files; 1 still missing');
  });

  it('missing_media_grouped_by_reason', async () => {
    await setup();
    expect(screen.getByText("outside your tree's folders, so not copied for safety")).toBeInTheDocument();
    expect(screen.getByText('D:/Scans/census-1900.jpg')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Choose folder…' })).toBeInTheDocument();
    expect(screen.getByText('photos/anna/portrait.jpg')).toBeInTheDocument();
    expect(screen.getByText('photos/karl/portrait.jpg')).toBeInTheDocument();
    expect(screen.getAllByText('not found').length).toBe(2);
  });

  it('media_urls_listed_as_links', async () => {
    await setup();
    expect(screen.getByRole('heading', { name: 'Web links, kept as links' })).toBeInTheDocument();
    expect(screen.getByText('https://example.org/images/grandfather.jpg')).toBeInTheDocument();
  });

  it('undo_import_confirms', async () => {
    const discard = vi.fn(async () => {});
    const { user } = await setup({}, { discardImport: discard });
    await user.click(screen.getByRole('button', { name: 'Undo import' }));
    const dlg = screen.getByRole('dialog');
    expect(within(dlg).getByText('Delete the imported tree? Your GEDCOM file is not touched.')).toBeInTheDocument();
    await user.click(within(dlg).getByRole('button', { name: 'Cancel' }));
    expect(discard).not.toHaveBeenCalled();
    expect(nav.route.name).toBe('importReport');
    await user.click(screen.getByRole('button', { name: 'Undo import' }));
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Delete imported tree' }));
    await waitFor(() => expect(discard).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(nav.route.name).toBe('welcome'));
    expect(project.info).toBeNull();
  });

  it('save_report_uses_picked_path', async () => {
    const save = vi.fn(async () => {});
    const { user } = await setup({}, { saveImportReport: save });
    await user.click(screen.getByRole('button', { name: 'Save report…' }));
    await waitFor(() => expect(save).toHaveBeenCalledWith('C:/Mock/import-report.txt'));
  });

  it('open_the_tree_goes_to_family', async () => {
    const { user } = await setup();
    await user.click(screen.getByRole('button', { name: 'Open the tree' }));
    await waitFor(() => expect(nav.route.name).toBe('family'));
    expect(nav.focusPerson).toBeTruthy();
    expect(project.report).toBeNull();
  });
});
