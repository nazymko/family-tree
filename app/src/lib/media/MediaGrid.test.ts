import { describe, expect, it, vi } from 'vitest';
import { screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import MediaGrid from './MediaGrid.svelte';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import { notices } from '../ui/notices.svelte';
import type { MediaDto, MediaLinkDto } from '../api/api';

const media = (id: string, over: Partial<MediaDto> = {}): MediaDto => ({
  id,
  title: null,
  mime: 'image/jpeg',
  originalName: `${id}.jpg`,
  date: null,
  description: null,
  resolved: true,
  width: 800,
  height: 600,
  sha256: null,
  ...over,
});
const link = (m: MediaDto, primary = false): MediaLinkDto => ({ media: m, primary });

describe('MediaGrid', () => {
  it('grid_alt_text_and_primary_ribbon', () => {
    const api = new MockApi(fixtureTree(30));
    const items = [link(media('m1', { title: 'Wedding day' }), true), link(media('m2'))];
    renderWith(MediaGrid, { items, onOpen: vi.fn() }, { api });
    const a = screen.getByAltText('Wedding day');
    expect(a).toHaveAttribute('loading', 'lazy');
    expect(a).toHaveAttribute('src', api.mediaUrl('m1', 'thumb'));
    expect(screen.getByAltText('m2.jpg')).toBeInTheDocument();
    expect(screen.getAllByTitle('Primary photo')).toHaveLength(1);
    const tiles = screen.getAllByRole('listitem');
    expect(within(tiles[0]).getByTitle('Primary photo')).toBeInTheDocument();
  });

  it('pdf_tile_has_document_icon_and_open_button', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(30));
    const items = [link(media('d1', { mime: 'application/pdf', originalName: 'will.pdf' }))];
    renderWith(MediaGrid, { items, onOpen: vi.fn() }, { api });
    expect(screen.queryByRole('img')).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Open will.pdf' }));
    expect(api.openedMedia).toEqual(['d1']);
  });

  it('refused_open_shows_a_plain_message', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(30));
    vi.spyOn(api, 'openMediaExternally').mockRejectedValue(new Error('Family Tree will only open photos and PDF documents. This file is some other kind of file, so it was not opened.'));
    const items = [link(media('d2', { mime: 'application/pdf', originalName: 'baptism.pdf' }))];
    renderWith(MediaGrid, { items, onOpen: vi.fn() }, { api });
    await user.click(screen.getByRole('button', { name: 'Open baptism.pdf' }));
    await vi.waitFor(() => expect(notices.current?.message).toMatch(/only open photos and PDF documents/));
  });

  it('missing_file_tile_text', () => {
    const items = [link(media('x1', { resolved: false }))];
    renderWith(MediaGrid, { items, onOpen: vi.fn() });
    expect(screen.getByText('File missing')).toBeInTheDocument();
    expect(document.querySelector('img')).toBeNull();
  });

  it('click_opens_viewer_index', async () => {
    const user = userEvent.setup();
    const onOpen = vi.fn();
    renderWith(MediaGrid, { items: [link(media('a')), link(media('b'))], onOpen });
    await user.click(screen.getByRole('button', { name: 'View b.jpg' }));
    expect(onOpen).toHaveBeenCalledWith(1);
  });
});
