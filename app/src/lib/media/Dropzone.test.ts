import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Dropzone from './Dropzone.svelte';
import MediaView from '../views/MediaView.svelte';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { notices } from '../ui/notices.svelte';
import type { OwnerRef } from '../api/api';

beforeEach(() => notices.dismiss());

async function emptyPerson() {
  const api = new MockApi(fixtureTree(80));
  const r = await api.apply({ type: 'createPerson', draft: { given: 'Zofia', surnames: ['Nowak'], sex: 'female' } });
  const id = r.focus!;
  const target: OwnerRef = { kind: 'person', id };
  return { api, id, target };
}

describe('Dropzone', () => {
  it('drop_attaches_all_paths_in_one_call', async () => {
    const { api, target } = await emptyPerson();
    const spy = vi.spyOn(api, 'attachFiles');
    const onAttached = vi.fn();
    const { platform } = renderWith(Dropzone, { target, targetName: 'Zofia Nowak', onAttached }, { api });
    const paths = ['C:/p/a.jpg', 'C:/p/b.png', 'C:/p/c.pdf'];
    await act(() => platform.simulateDrop(paths));
    await waitFor(() => expect(onAttached).toHaveBeenCalled());
    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy).toHaveBeenCalledWith(target, paths, { silent: true });
    expect(notices.current?.message).toBe('Attached 3 files');
    expect(notices.current?.actionLabel).toBe('Undo');
  });

  it('non_ascii_windows_path_passthrough', async () => {
    const { api, target } = await emptyPerson();
    const spy = vi.spyOn(api, 'attachFiles');
    const { platform } = renderWith(Dropzone, { target, targetName: 'Zofia', onAttached: vi.fn() }, { api });
    const path = 'C:\\Users\\Zoë\\Фото\\свадьба 1901.jpg';
    await act(() => platform.simulateDrop([path]));
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(spy.mock.calls[0][1]).toEqual([path]);
  });

  it('drag_overlay_toggles_with_hover', async () => {
    const { target, api } = await emptyPerson();
    const { platform } = renderWith(Dropzone, { target, targetName: 'Zofia Nowak', onAttached: vi.fn() }, { api });
    expect(screen.queryByText(/Drop photos and documents/)).toBeNull();
    await act(() => platform.simulateHover(true));
    expect(screen.getByText('Drop photos and documents to attach them to Zofia Nowak')).toBeInTheDocument();
    await act(() => platform.simulateHover(false));
    expect(screen.queryByText(/Drop photos and documents/)).toBeNull();
  });

  it('add_files_button_uses_media_extensions', async () => {
    const user = userEvent.setup();
    const { target, api } = await emptyPerson();
    const platform = new MockPlatform();
    platform.next = { files: ['C:/Mock/x.jpg'] };
    const onAttached = vi.fn();
    renderWith(Dropzone, { target, targetName: 'Zofia', onAttached }, { api, platform });
    await user.click(screen.getByRole('button', { name: 'Add files…' }));
    await waitFor(() => expect(onAttached).toHaveBeenCalled());
    expect(platform.calls[0].args[0]).toMatchObject({ extensions: ['jpg', 'jpeg', 'png', 'webp', 'tif', 'tiff', 'gif', 'pdf'] });
  });

  it('exif_banner_offers_not_applies', async () => {
    const user = userEvent.setup();
    const { target, api } = await emptyPerson();
    vi.spyOn(api, 'exifSuggestion').mockResolvedValue({ date: '3 Jun 1901', lat: null, lon: null });
    const apply = vi.spyOn(api, 'apply');
    const onSuggestEvent = vi.fn();
    const { platform } = renderWith(Dropzone, { target, targetName: 'Zofia', onAttached: vi.fn(), onSuggestEvent }, { api });
    await act(() => platform.simulateDrop(['C:/p/wedding.jpg']));
    await screen.findByText("Use the photo's date (3 Jun 1901) for a new event?");
    expect(apply).not.toHaveBeenCalled();
    expect(onSuggestEvent).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Create event' }));
    expect(onSuggestEvent).toHaveBeenCalledWith('3 Jun 1901', target);
    expect(apply).not.toHaveBeenCalled();
  });
});

describe('Undo toast', () => {
  it('undo_only_its_own_command', async () => {
    const { api, target } = await emptyPerson();
    const { platform } = renderWith(Dropzone, { target, targetName: 'Zofia', onAttached: vi.fn() }, { api });
    await act(() => platform.simulateDrop(['C:/p/a.jpg']));
    await waitFor(() => expect(notices.current?.actionLabel).toBe('Undo'));
    const n = notices.current!;
    expect(n.actionValid?.()).toBe(true);
    await api.apply({ type: 'updatePerson', id: target.id, sex: 'male' });
    const undo = vi.spyOn(api, 'undo');
    n.onaction!();
    await waitFor(() => expect(notices.current?.message).toMatch(/no longer the latest/));
    expect(undo).not.toHaveBeenCalled();
  });
});

describe('MediaView attach flow', () => {
  it('partial_failure_reported_per_file', async () => {
    const { api, id } = await emptyPerson();
    const { platform } = renderWith(MediaView, { personId: id }, { api });
    await screen.findByText(/Drop files here to attach them/);
    await act(() => platform.simulateDrop(['C:\\Users\\Zoë\\scan.exe', 'C:\\Users\\Zoë\\one.jpg', 'C:\\Users\\Zoë\\two.png']));
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('scan.exe: unsupported type; we attach JPEG, PNG, WebP, TIFF, GIF and PDF files');
    const grid = await screen.findByRole('list', { name: 'Photos and documents' });
    await waitFor(() => expect(within(grid).getAllByRole('listitem')).toHaveLength(2));
    expect(within(grid).getByAltText('one')).toBeInTheDocument();
    expect(within(grid).getByAltText('two')).toBeInTheDocument();
  });

  it('duplicate_file_attached_once', async () => {
    const { api, id } = await emptyPerson();
    const { platform } = renderWith(MediaView, { personId: id }, { api });
    await screen.findByText(/Drop files here to attach them/);
    await act(() => platform.simulateDrop(['C:/x/a.jpg']));
    const grid = await screen.findByRole('list', { name: 'Photos and documents' });
    await act(() => platform.simulateDrop(['C:/y/a.jpg']));
    await screen.findByText('a.jpg: Already attached');
    expect(within(grid).getAllByRole('listitem')).toHaveLength(1);
  });

  it('empty_state_names_the_person', async () => {
    const { api, id } = await emptyPerson();
    renderWith(MediaView, { personId: id }, { api });
    expect(await screen.findByText(/No photos or documents for/)).toHaveTextContent('No photos or documents for Zofia Nowak. Drop files here to attach them.');
  });
});
