import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Shell from './Shell.svelte';
import Welcome from './Welcome.svelte';
import ImportWizard from './ImportWizard.svelte';
import SendCopyDialog from './SendCopyDialog.svelte';
import ExportDialog from './ExportDialog.svelte';
import { apiContext } from '../api/context';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { observeApi } from '../state/observe';
import { nav } from '../state/nav.svelte';
import { project } from '../state/project.svelte';
import { toasts } from '../state/toast.svelte';
import { undoState } from '../state/undo.svelte';
import { folderNameFor } from '../ui/mobile';
import { openReceivedCopy } from '../state/openReceived';

function phone(): MockPlatform {
  const p = new MockPlatform();
  p.isMobile = true;
  return p;
}

const realMatchMedia = window.matchMedia;
/** A window of this width for the media queries the layout asks about. */
function widthIs(px: number) {
  window.matchMedia = ((q: string) => {
    const max = /max-width:\s*(\d+)px/.exec(q);
    return { matches: max ? px <= Number(max[1]) : false, media: q, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {}, dispatchEvent: () => false } as unknown as MediaQueryList;
  }) as typeof window.matchMedia;
}

beforeEach(() => {
  localStorage.removeItem('gt.hiddenRecents');
  nav.reset();
  project.close();
  toasts.clear();
  undoState.reset();
});
afterEach(() => {
  window.matchMedia = realMatchMedia;
});

async function mountShell(platform: MockPlatform) {
  const api = observeApi(new MockApi(fixtureTree(100)));
  const info = await api.openProject('/data/trees/x');
  project.open(info);
  nav.reset();
  nav.go({ name: 'family' });
  nav.focus(info.homePerson);
  const user = userEvent.setup();
  render(Shell, { context: apiContext(api, platform) });
  return { user, api };
}

describe('the sidebar as a drawer on a narrow screen', () => {
  it('on a phone the menu button opens the sidebar, a choice closes it, Escape closes it', async () => {
    widthIs(400);
    const { user } = await mountShell(phone());
    const menu = await screen.findByRole('button', { name: 'Menu' });
    expect(menu).toHaveAttribute('aria-expanded', 'false');
    // Closed: its links cannot be reached.
    expect(screen.queryByRole('navigation', { name: 'Main' })).toBeNull();
    await user.click(menu);
    const navEl = await screen.findByRole('navigation', { name: 'Main' });
    expect(menu).toHaveAttribute('aria-expanded', 'true');
    await user.click(within(navEl).getByRole('link', { name: 'People' }));
    expect(nav.route.name).toBe('people');
    await waitFor(() => expect(screen.queryByRole('navigation', { name: 'Main' })).toBeNull());
    await user.click(screen.getByRole('button', { name: 'Menu' }));
    expect(await screen.findByRole('navigation', { name: 'Main' })).toBeInTheDocument();
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('navigation', { name: 'Main' })).toBeNull());
  });

  it('a desktop-width window keeps the sidebar and shows no menu button', async () => {
    widthIs(1280);
    await mountShell(new MockPlatform());
    expect(screen.queryByRole('button', { name: 'Menu' })).toBeNull();
    expect(screen.getByRole('navigation', { name: 'Main' })).toBeInTheDocument();
  });

  it('a tablet (800 px) gets the drawer, a window at the desktop minimum (960 px) does not', async () => {
    widthIs(800);
    await mountShell(phone());
    expect(await screen.findByRole('button', { name: 'Menu' })).toBeInTheDocument();
  });

  it('the desktop minimum width is wider than the drawer breakpoint', async () => {
    widthIs(960);
    await mountShell(new MockPlatform());
    expect(screen.queryByRole('button', { name: 'Menu' })).toBeNull();
  });
});

describe('storage on a phone', () => {
  it('the welcome page offers no folder to open and says where trees are kept', async () => {
    render(Welcome, { context: apiContext(new MockApi(fixtureTree(50)), phone()) });
    expect(screen.queryByRole('button', { name: 'Open a tree' })).toBeNull();
    expect(screen.getByTestId('mobile-storage-note')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Import a family tree file' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Create a new tree' })).toBeInTheDocument();
    // The list shows names, not folders.
    const link = await screen.findByRole('link', { name: /Schmidt family/ });
    expect(link.textContent).not.toContain('Genealogy');
  });

  it('creating a tree needs only a name and puts it in the app storage', async () => {
    const platform = phone();
    const api = new MockApi(fixtureTree(10));
    const create = vi.spyOn(api, 'createProject');
    const user = userEvent.setup();
    render(ImportWizard, { props: { mode: 'create', oncancel: () => {} }, context: apiContext(api, platform) });
    expect(screen.queryByRole('button', { name: /Choose folder/i })).toBeNull();
    expect(screen.getByTestId('mobile-storage-note')).toBeInTheDocument();
    await user.type(screen.getByRole('textbox', { name: /Tree name/ }), 'Grandma: the Koval/line');
    const start = screen.getByRole('button', { name: 'Create tree' });
    await waitFor(() => expect(start).toBeEnabled());
    await user.click(start);
    await waitFor(() => expect(create).toHaveBeenCalled());
    expect(create).toHaveBeenCalledWith('/data/app/trees/Grandma_ the Koval_line', 'Grandma: the Koval/line');
    expect(platform.calls.some((c) => c.method === 'pickFolder')).toBe(false);
  });

  it('importing a file asks for the file and never for a folder or photo folders', async () => {
    const platform = phone();
    platform.next = { file: '/cache/picked/family.ged' };
    const api = new MockApi(fixtureTree(10));
    const user = userEvent.setup();
    render(ImportWizard, { props: { mode: 'import', oncancel: () => {} }, context: apiContext(api, platform) });
    await user.click(screen.getByRole('button', { name: /Choose file/i }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Start import' })).toBeEnabled());
    expect(screen.queryByRole('button', { name: /photo folder/i })).toBeNull();
    expect(platform.calls.filter((c) => c.method === 'pickFolder')).toHaveLength(0);
  });

  it('a received project zip unpacks beside the other trees, not into a picked folder', async () => {
    const platform = phone();
    platform.next = { file: '/cache/picked/Koval copy.zip' };
    const api = new MockApi(fixtureTree(10));
    const spy = vi.spyOn(api, 'openReceivedCopy');
    await openReceivedCopy(api, platform);
    expect(spy).toHaveBeenCalled();
    const [zip, dest] = spy.mock.calls[0];
    expect(zip).toBe('/cache/picked/Koval copy.zip');
    expect(dest).toMatch(/^\/data\/app\/trees\/Koval copy-[a-z0-9]+$/);
    expect(platform.calls.some((c) => c.method === 'pickFolder')).toBe(false);
  });

  it('folder names lose characters a folder cannot hold', () => {
    expect(folderNameFor('  A/B\\C:D ')).toBe('A_B_C_D');
    expect(folderNameFor('...')).toBe('_');
    expect(folderNameFor('   ')).toBe('tree');
  });
});

describe('export on a phone', () => {
  it('the zip is made, then handed over; the notice has no show-in-folder action', async () => {
    const platform = phone();
    platform.next = { save: '/cache/export/family.zip' };
    const api = new MockApi(fixtureTree(30));
    await api.openProject('/data/trees/x');
    const user = userEvent.setup();
    render(SendCopyDialog, { props: { onClose: () => {} }, context: apiContext(api, platform) });
    const go = await screen.findByRole('button', { name: /Make the zip|Make a copy|Create/i }).catch(() => null);
    expect(go).toBeTruthy();
    await waitFor(() => expect(go).toBeEnabled());
    await user.click(go!);
    await waitFor(() => expect(platform.calls.some((c) => c.method === 'deliver')).toBe(true));
    expect(platform.calls.some((c) => c.method === 'reveal')).toBe(false);
  });

  it('GEDCOM export is handed over too, and the folder copy is replaced by a reason', async () => {
    const platform = phone();
    platform.next = { save: '/cache/export/tree.ged' };
    const api = new MockApi(fixtureTree(30));
    await api.openProject('/data/trees/x');
    const user = userEvent.setup();
    render(ExportDialog, { props: { onClose: () => {} }, context: apiContext(api, platform) });
    expect(screen.queryByRole('button', { name: /Save a copy/i })).toBeNull();
    expect(screen.getByTestId('mobile-no-folder-copy')).toBeInTheDocument();
    const exportBtn = screen.getByRole('button', { name: /^Export/ });
    await waitFor(() => expect(exportBtn).toBeEnabled());
    await user.click(exportBtn);
    await waitFor(() => expect(platform.calls.some((c) => c.method === 'deliver')).toBe(true));
  });

  it('on a computer nothing is handed over and the folder action stays', async () => {
    const platform = new MockPlatform();
    const api = new MockApi(fixtureTree(30));
    await api.openProject('/data/trees/x');
    const user = userEvent.setup();
    render(ExportDialog, { props: { onClose: () => {} }, context: apiContext(api, platform) });
    expect(screen.getByRole('button', { name: /Save a copy/i })).toBeInTheDocument();
    const exportBtn = screen.getByRole('button', { name: /^Export/ });
    await waitFor(() => expect(exportBtn).toBeEnabled());
    await user.click(exportBtn);
    await waitFor(() => expect(platform.calls.some((c) => c.method === 'pickSavePath')).toBe(true));
    expect(platform.calls.some((c) => c.method === 'deliver')).toBe(false);
  });
});
