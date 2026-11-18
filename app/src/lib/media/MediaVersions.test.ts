import { describe, expect, it, vi } from 'vitest';
import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import MediaViewer from './MediaViewer.svelte';
import MediaGrid from './MediaGrid.svelte';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import { bestPrimary, collapseVersions, groupCommand, linkedMember, versionCount } from './versions';
import type { MediaDto, MediaLinkDto, TextRegionDto } from '../api/api';

// An invented register page kept in three files: a new scan, an older copy and a phone photo.
const TEXT: TextRegionDto[] = [{ id: 'r1', area: null, rotation: 0, text: 'Marta Hrytsenko, born 3 May 1901' }];

const file = (id: string, width: number, height: number, over: Partial<MediaDto> = {}): MediaDto => ({
  id,
  title: null,
  mime: 'image/jpeg',
  originalName: `${id}.jpg`,
  date: null,
  description: null,
  resolved: true,
  width,
  height,
  sha256: null,
  transcription: [],
  rotation: 0,
  ...over,
});

const scan = file('scan', 4392, 3496);
const older = file('older', 1232, 967, { transcription: TEXT });
const phone = file('phone', 800, 600);
/** The group as the backend sends it on every member: primary first, then the most pixels first. */
const GROUP = [scan, older, phone];
const member = (m: MediaDto): MediaDto => ({ ...m, versions: GROUP });
const link = (m: MediaDto, primary = false): MediaLinkDto => ({ media: m, primary });

describe('collapsing versions', () => {
  it('shows one tile per group, the primary, even when only a copy is linked', () => {
    const lone = file('letter', 300, 200);
    const shown = collapseVersions([link(lone), link(member(older))]);
    expect(shown.map((l) => l.media.id)).toEqual(['letter', 'scan']);
    expect(shown[1].media.versions?.map((v) => v.id)).toEqual(['scan', 'older', 'phone']);
    expect(shown[1].linked).toEqual(['older']);
    expect(versionCount(shown[1].media)).toBe(3);
    expect(versionCount(shown[0].media)).toBe(1);
  });

  it('keeps one tile when several versions are linked, primary when any of them is', () => {
    const shown = collapseVersions([link(member(phone)), link(member(older), true), link(file('x', 10, 10))]);
    expect(shown).toHaveLength(2);
    expect(shown[0].media.id).toBe('scan');
    expect(shown[0].primary).toBe(true);
    expect(shown[0].linked).toEqual(['phone', 'older']);
    // Remove and primary photo act on a linked file: the one on screen when it is linked.
    expect(linkedMember(shown[0], 'older')).toBe('older');
    expect(linkedMember(shown[0], 'scan')).toBe('phone');
  });

  it('chooses the file with the most pixels as the primary', () => {
    expect(bestPrimary([phone, older, scan])?.id).toBe('scan');
    expect(bestPrimary([file('a', 10, 10), file('b', 10, 10)])?.id).toBe('a');
    expect(groupCommand(phone, older)).toEqual({ type: 'groupMediaVersions', primary: 'older', others: ['phone'] });
    // A file picked from another group brings the group along; the largest of all leads.
    expect(groupCommand(file('letter', 300, 200), member(phone))).toEqual({
      type: 'groupMediaVersions',
      primary: 'scan',
      others: ['letter', 'phone', 'older'],
    });
    expect(groupCommand(phone, phone)).toBeNull();
  });
});

describe('gallery tile', () => {
  it('shows a badge with the number of versions', () => {
    renderWith(MediaGrid, { items: collapseVersions([link(member(older))]), onOpen: vi.fn() }, { api: new MockApi(fixtureTree(5)) });
    expect(screen.getAllByRole('listitem')).toHaveLength(1);
    expect(screen.getByTestId('versions-badge').textContent).toBe('3 versions');
    expect(document.querySelector('[data-media-id]')?.getAttribute('data-media-id')).toBe('scan');
  });
});

const OK = { label: 'Group versions', focus: null, touched: [], undo: { canUndo: true, canRedo: false, undoLabel: null, redoLabel: null } };

function openViewer(items: MediaLinkDto[]) {
  const api = new MockApi(fixtureTree(5));
  const spy = vi.spyOn(api, 'apply').mockResolvedValue(OK);
  const onChanged = vi.fn();
  renderWith(MediaViewer, { items, index: 0, onClose: vi.fn(), onChanged, target: { kind: 'person', id: 'p1' } }, { api });
  return { api, spy, onChanged };
}

describe('version switcher', () => {
  it('opens the primary and switches to another version with its own text', async () => {
    const user = userEvent.setup();
    openViewer(collapseVersions([link(member(older))]));
    const img = () => document.querySelector<HTMLImageElement>('.frame img')!;
    expect(img().alt).toBe('scan.jpg');
    const switcher = screen.getByTestId('version-switcher');
    const buttons = within(switcher).getAllByRole('button');
    expect(buttons.map((b) => b.textContent?.trim())).toEqual(['primary 4392×3496', 'copy 1232×967', 'copy 800×600']);
    expect(buttons[0].getAttribute('aria-pressed')).toBe('true');
    // The primary has no text, a copy has: the hint says so and shows it.
    expect(screen.getByTestId('version-text-hint')).toBeTruthy();
    await user.click(within(screen.getByTestId('version-text-hint')).getByRole('button'));
    expect(img().alt).toBe('older.jpg');
    expect(within(screen.getByTestId('version-switcher')).getAllByRole('button')[1].getAttribute('aria-pressed')).toBe('true');
    expect(screen.queryByTestId('version-text-hint')).toBeNull();
    expect(screen.getByText('Marta Hrytsenko, born 3 May 1901')).toBeTruthy();
  });

  it('makes the shown version primary and ungroups it', async () => {
    const user = userEvent.setup();
    const { spy } = openViewer(collapseVersions([link(member(older))]));
    await user.click(within(screen.getByTestId('version-switcher')).getByRole('button', { name: 'copy 800×600' }));
    await user.click(screen.getByRole('button', { name: 'Make primary' }));
    expect(spy).toHaveBeenLastCalledWith({ type: 'groupMediaVersions', primary: 'phone', others: [] });
    await user.click(screen.getByRole('button', { name: 'Ungroup' }));
    expect(spy).toHaveBeenLastCalledWith({ type: 'ungroupMediaVersion', media: 'phone' });
  });

  it('ungrouping the primary hands the group to the largest of the rest', async () => {
    const user = userEvent.setup();
    const { spy } = openViewer(collapseVersions([link(member(phone))]));
    expect(screen.queryByRole('button', { name: 'Make primary' })).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Ungroup' }));
    expect(spy).toHaveBeenLastCalledWith({ type: 'ungroupMediaVersion', media: 'scan', newPrimary: 'older' });
  });

  it('a file with no other versions has no switcher, and is grouped with a picked one', async () => {
    const user = userEvent.setup();
    const { api, spy } = openViewer([link(file('letter', 300, 200))]);
    expect(screen.queryByTestId('version-switcher')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Ungroup' })).toBeNull();
    const big = { ...file('letter-scan', 3000, 2000), title: 'Letter, new scan' };
    vi.spyOn(api, 'allPictures').mockResolvedValue([file('letter', 300, 200), big]);
    await user.click(screen.getByRole('button', { name: 'Group with another version…' }));
    const list = await screen.findByTestId('version-candidates');
    // The file on screen is not offered again.
    await waitFor(() => expect(within(list).getAllByRole('button')).toHaveLength(1));
    await user.click(within(list).getByRole('button', { name: 'Group with Letter, new scan' }));
    expect(spy).toHaveBeenLastCalledWith({ type: 'groupMediaVersions', primary: 'letter-scan', others: ['letter'] });
  });
});

describe('MockApi versions', () => {
  it('groups, shows the group on every link, and undoes', async () => {
    const api = new MockApi(fixtureTree(60));
    const pictures = await api.allPictures();
    expect(pictures.length).toBeGreaterThan(1);
    const [a, b] = pictures;
    await api.apply({ type: 'groupMediaVersions', primary: a.id, others: [b.id] });
    const after = await api.allPictures();
    expect(after.find((m) => m.id === b.id)).toBeUndefined();
    expect(after.find((m) => m.id === a.id)?.versions?.map((v) => v.id)).toEqual([a.id, b.id]);
    await api.undo();
    expect((await api.allPictures()).find((m) => m.id === b.id)).toBeDefined();
  });
});
