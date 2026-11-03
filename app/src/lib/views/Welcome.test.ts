import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Welcome from './Welcome.svelte';
import { apiContext } from '../api/context';
import type { Api, ImportProgress, ImportReportDto, ImportRequest } from '../api/api';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { nav } from '../state/nav.svelte';
import { project } from '../state/project.svelte';
import { toasts } from '../state/toast.svelte';

function setup(api: Api = new MockApi(fixtureTree(80))) {
  const platform = new MockPlatform();
  const user = userEvent.setup();
  render(Welcome, { context: apiContext(api, platform) });
  return { api, platform, user };
}

beforeEach(() => {
  nav.reset();
  project.close();
  toasts.clear();
});

describe('Welcome', () => {
  it('welcome_lists_recent_and_actions', async () => {
    setup();
    for (const name of ['Import a GEDCOM or Древо Жизни file', 'Create a new tree', 'Open a tree']) {
      expect(screen.getByRole('button', { name })).toBeInTheDocument();
    }
    const link = await screen.findByRole('link', { name: /Schmidt family.*C:\/Genealogy\/Schmidt family/ });
    expect(link).toBeInTheDocument();
  });

  it('opening_a_tree_that_is_open_elsewhere_shows_the_conflict_message_plainly', async () => {
    const api = new MockApi(fixtureTree(80));
    const msg = 'This tree is open in another Gene Tree window.';
    vi.spyOn(api, 'openProject').mockRejectedValue(Object.assign(new Error(msg), { code: 'conflict' }));
    const { platform, user } = setup(api);
    platform.next = { folder: 'C:/Trees/Busy' };
    await user.click(screen.getByRole('button', { name: /Open a tree/ }));
    expect(await screen.findByText(msg)).toBeInTheDocument();
    expect(project.info).toBeNull();
  });

  it('welcome_empty_state', async () => {
    const api = new MockApi(fixtureTree(80));
    api.recentProjects = async () => [];
    setup(api);
    expect(
      await screen.findByText('No trees yet. Import a GEDCOM or Древо Жизни file to start, or create an empty tree.'),
    ).toBeInTheDocument();
  });

  it('missing_recent_shows_remove', async () => {
    const { user } = setup();
    expect(await screen.findByText('This folder is not there any more')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /Remove Weber tree/ }));
    expect(screen.queryByText('This folder is not there any more')).toBeNull();
    expect(screen.queryByText('Weber tree')).toBeNull();
  });

  it('open_recent_goes_to_family', async () => {
    const { user } = setup();
    await user.click(await screen.findByRole('link', { name: /Schmidt family/ }));
    await waitFor(() => expect(nav.route.name).toBe('family'));
    expect(project.info?.name).toBe('Schmidt family');
    expect(nav.focusPerson).toBeTruthy();
    expect(nav.canBack).toBe(false);
  });

  it('import_flow_reports_progress', async () => {
    const base = new MockApi(fixtureTree(80));
    let advance: () => void = () => {};
    const gate = () => new Promise<void>((r) => (advance = r));
    const api = Object.create(base) as Api;
    api.importGedcom = async (req: ImportRequest, onProgress?: (p: ImportProgress) => void): Promise<ImportReportDto> => {
      onProgress?.({ phase: 'reading', done: 50, total: 100 });
      await gate();
      onProgress?.({ phase: 'parsing', done: 100, total: 100 });
      await gate();
      onProgress?.({ phase: 'saving', done: 0, total: 100 });
      await gate();
      return base.importGedcom(req);
    };
    const { user } = setup(api);
    await user.click(screen.getByRole('button', { name: 'Import a GEDCOM or Древо Жизни file' }));
    await user.click(screen.getByRole('button', { name: 'Choose file…' }));
    expect(screen.getByRole('textbox', { name: 'Tree name' })).toHaveValue('Family tree');
    await user.click(screen.getByRole('button', { name: 'Choose folder…' }));
    await user.click(screen.getByRole('button', { name: 'Start import' }));
    const bar = await screen.findByRole('progressbar');
    await waitFor(() => expect(bar).toHaveAttribute('aria-valuenow', '10'));
    advance();
    await waitFor(() => expect(bar).toHaveAttribute('aria-valuenow', '40'));
    advance();
    await waitFor(() => expect(bar).toHaveAttribute('aria-valuenow', '60'));
    advance();
    await waitFor(() => expect(nav.route.name).toBe('importReport'));
    expect(project.report?.producer).toBe('Mock Genealogy 4.2');
  });

  it('file_picker_offers_gedcom_and_atdb_files', async () => {
    const { user, platform } = setup();
    await user.click(screen.getByRole('button', { name: 'Import a GEDCOM or Древо Жизни file' }));
    expect(screen.getByRole('heading', { name: 'Import a GEDCOM or Древо Жизни file' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Choose file…' }));
    const call = platform.calls.find((c) => c.method === 'pickFile');
    const opts = call?.args[0] as { title: string; extensions: string[] };
    expect(opts.title).toBe('Choose a GEDCOM or Древо Жизни file');
    expect(opts.extensions).toEqual(expect.arrayContaining(['ged', 'gedcom', 'atdb']));
  });

  it('date_order_is_passed_to_import', async () => {
    const base = new MockApi(fixtureTree(80));
    let seen: ImportRequest | null = null;
    const api = Object.create(base) as Api;
    api.importGedcom = async (req, p) => {
      seen = req;
      return base.importGedcom(req, p);
    };
    const { user } = setup(api);
    await user.click(screen.getByRole('button', { name: 'Import a GEDCOM or Древо Жизни file' }));
    await user.click(screen.getByRole('button', { name: 'Choose file…' }));
    await user.click(screen.getByRole('button', { name: 'Choose folder…' }));
    expect(screen.getByText('Used only for dates like 03/04/1901')).toBeInTheDocument();
    await user.click(screen.getByRole('radio', { name: /Month, day, year/ }));
    await user.click(screen.getByRole('button', { name: 'Start import' }));
    await waitFor(() => expect(seen).not.toBeNull());
    expect((seen as unknown as ImportRequest).dayOrder).toBe('mdy');
  });

  it('empty_tree_opens_to_add_first_person', async () => {
    const base = new MockApi(fixtureTree(80));
    const api = Object.create(base) as Api;
    api.openProject = async (dir) => ({ ...(await base.openProject(dir)), personCount: 0, homePerson: null });
    const { user, platform } = setup(api);
    platform.next.folder = 'C:/Mock/Empty';
    await user.click(screen.getByRole('button', { name: 'Open a tree' }));
    await waitFor(() => expect(nav.route.name).toBe('people'));
  });

  it('create_new_tree_goes_to_people', async () => {
    const { user } = setup();
    await user.click(screen.getByRole('button', { name: 'Create a new tree' }));
    await user.type(screen.getByRole('textbox', { name: 'Tree name' }), 'Kowalski');
    await user.click(screen.getByRole('button', { name: 'Choose folder…' }));
    const dlgBtn = screen.getByRole('button', { name: 'Create tree' });
    await user.click(dlgBtn);
    await waitFor(() => expect(project.info?.name).toBe('Kowalski'));
    expect(within(document.body).queryByRole('progressbar')).toBeNull();
  });
});
