import { describe, expect, it, vi } from 'vitest';
import { screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import MediaViewer from './MediaViewer.svelte';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import type { MediaDto, MediaLinkDto, OwnerRef } from '../api/api';

const media = (id: string): MediaDto => ({
  id,
  title: `Photo ${id}`,
  mime: 'image/jpeg',
  originalName: `${id}.jpg`,
  date: null,
  description: null,
  resolved: true,
  width: 800,
  height: 600,
  sha256: null,
});

async function setup(count = 3, index = 0) {
  const api = new MockApi(fixtureTree(30));
  const personId = (await api.search('a', 1))[0]?.person?.id ?? (await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 1)).items[0].id;
  const items: MediaLinkDto[] = Array.from({ length: count }, (_, i) => ({ media: media(`m${i}`), primary: false }));
  const target: OwnerRef = { kind: 'person', id: personId };
  const onClose = vi.fn();
  const platform = new MockPlatform();
  renderWith(MediaViewer, { items, index, target, onClose }, { api, platform });
  const stage = screen.getByRole('group', { name: /^Preview/ });
  return { api, items, target, onClose, platform, stage, personId };
}

describe('MediaViewer', () => {
  it('viewer_zoom_pan_keys', async () => {
    const user = userEvent.setup();
    const { stage } = await setup(3, 0);
    stage.focus();
    expect(screen.getByText('Zoom 100%')).toBeInTheDocument();
    await user.keyboard('+');
    expect(screen.getByText('Zoom 200%')).toBeInTheDocument();
    await user.keyboard('+++');
    expect(screen.getByText('Zoom 800%')).toBeInTheDocument();
    await user.keyboard('+');
    expect(screen.getByText('Zoom 800%')).toBeInTheDocument();
    await user.keyboard('0');
    expect(screen.getByText('Zoom 100%')).toBeInTheDocument();
    // arrows move, never wrap
    await user.keyboard('{ArrowLeft}');
    expect(screen.getByText('1 of 3')).toBeInTheDocument();
    await user.keyboard('{ArrowRight}{ArrowRight}{ArrowRight}');
    expect(screen.getByText('3 of 3')).toBeInTheDocument();
    expect(screen.getByRole('img')).toHaveAttribute('alt', 'Photo m2');
  });

  it('drag_pans_when_zoomed', async () => {
    const user = userEvent.setup();
    const { stage } = await setup();
    stage.focus();
    await user.keyboard('+');
    const img = screen.getByRole('img');
    stage.dispatchEvent(new MouseEvent('pointerdown', { clientX: 10, clientY: 10, bubbles: true }));
    stage.dispatchEvent(new MouseEvent('pointermove', { clientX: 40, clientY: 25, bubbles: true }));
    stage.dispatchEvent(new MouseEvent('pointerup', { bubbles: true }));
    await waitFor(() => expect(img.getAttribute('style')).toContain('translate(30px, 15px)'));
  });

  it('viewer_p_sets_primary', async () => {
    const user = userEvent.setup();
    const { api, stage, personId } = await setup(2, 1);
    const spy = vi.spyOn(api, 'apply').mockResolvedValue({ label: 'Set primary media', focus: null, touched: [], undo: { canUndo: true, canRedo: false, undoLabel: null, redoLabel: null } });
    stage.focus();
    await user.keyboard('p');
    await waitFor(() => expect(spy).toHaveBeenCalledWith({ type: 'setPrimaryMedia', person: personId, media: 'm1' }));
  });

  it('viewer_focus_trapped_and_esc_closes', async () => {
    const user = userEvent.setup();
    const { onClose } = await setup();
    const dlg = screen.getByRole('dialog');
    for (let i = 0; i < 12; i++) {
      await user.tab();
      expect(dlg.contains(document.activeElement)).toBe(true);
    }
    await user.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('open_original_and_inline_title_edit', async () => {
    const user = userEvent.setup();
    const { api, platform } = await setup();
    const spy = vi.spyOn(api, 'apply').mockResolvedValue({ label: 'Edit media', focus: null, touched: [], undo: { canUndo: true, canRedo: false, undoLabel: null, redoLabel: null } });
    await user.click(screen.getByRole('button', { name: 'Open original' }));
    expect(platform.calls[0].method).toBe('openExternally');
    const title = screen.getByRole('textbox', { name: 'Title' });
    await user.clear(title);
    await user.type(title, 'Garden party');
    await user.tab();
    await waitFor(() => expect(spy).toHaveBeenCalledWith({ type: 'updateMedia', id: 'm0', title: 'Garden party' }));
  });
});
