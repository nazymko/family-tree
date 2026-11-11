import { describe, expect, it, vi } from 'vitest';
import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import MediaViewer from './MediaViewer.svelte';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import type { EditCommand, MediaDto, MediaLinkDto, TextRegionDto } from '../api/api';
import { areaBetween, fullText, moveArea, moved, resizeArea, toInput } from './transcription';

// Invented text on an invented letter; no real document.
const PIECES: TextRegionDto[] = [
  { id: 'r1', area: { x: 0.1, y: 0.1, w: 0.5, h: 0.1 }, rotation: 0, text: 'Dear Marta,' },
  { id: 'r2', area: { x: 0.1, y: 0.3, w: 0.6, h: 0.2 }, rotation: 0, text: 'the orchard is in bloom.' },
  { id: 'r3', area: { x: 0.85, y: 0.2, w: 0.1, h: 0.6 }, rotation: 90, text: 'Lviv, May' },
];

const media = (id: string, over: Partial<MediaDto> = {}): MediaDto => ({
  id,
  title: `Letter ${id}`,
  mime: 'image/jpeg',
  originalName: `${id}.jpg`,
  date: null,
  description: null,
  resolved: true,
  width: 800,
  height: 400,
  sha256: null,
  transcription: PIECES,
  rotation: 0,
  ...over,
});

const OK = { label: 'Edit transcription', focus: null, touched: [], undo: { canUndo: true, canRedo: false, undoLabel: null, redoLabel: null } };

function setup(over: Partial<MediaDto> = {}, count = 2) {
  const api = new MockApi(fixtureTree(5));
  const spy = vi.spyOn(api, 'apply').mockResolvedValue(OK);
  const items: MediaLinkDto[] = Array.from({ length: count }, (_, i) => ({ media: media(`m${i}`, i === 0 ? over : {}), primary: false }));
  const onChanged = vi.fn();
  const onClose = vi.fn();
  renderWith(MediaViewer, { items, index: 0, onClose, onChanged }, { api });
  const stage = screen.getByRole('group', { name: /^Preview/ });
  return { api, spy, stage, onChanged, onClose };
}

/** The pieces sent by the last save. */
function lastRegions(spy: ReturnType<typeof vi.spyOn>) {
  const cmd = spy.mock.calls.at(-1)?.[0] as Extract<EditCommand, { type: 'setMediaTranscription' }>;
  expect(cmd.type).toBe('setMediaTranscription');
  expect(cmd.id).toBe('m0');
  return cmd.regions;
}

const box = (i: number) => screen.getByRole('button', { name: `Text piece ${i}` });
const layer = () => document.querySelector<HTMLElement>('.layer')!;
function fakeSize(el: HTMLElement, width = 200, height = 100) {
  el.getBoundingClientRect = () => ({ left: 0, top: 0, right: width, bottom: height, width, height, x: 0, y: 0, toJSON: () => ({}) });
}
function pointer(el: Element, type: string, x: number, y: number) {
  el.dispatchEvent(new MouseEvent(type, { clientX: x, clientY: y, bubbles: true }));
}

describe('transcription geometry', () => {
  it('keeps areas on the picture and at least a hundredth wide', () => {
    expect(moveArea({ x: 0.9, y: 0.9, w: 0.2, h: 0.05 }, 0.5, 0.5)).toEqual({ x: 0.8, y: 0.95, w: 0.2, h: 0.05 });
    expect(resizeArea({ x: 0.2, y: 0.2, w: 0.1, h: 0.1 }, 'se', -0.5, 0)).toEqual({ x: 0.2, y: 0.2, w: 0.01, h: 0.1 });
    expect(resizeArea({ x: 0.2, y: 0.2, w: 0.1, h: 0.1 }, 'nw', -0.1, -0.1)).toEqual({ x: 0.1, y: 0.1, w: 0.2, h: 0.2 });
    expect(areaBetween(0.5, 0.5, 0.2, 0.1)).toEqual({ x: 0.2, y: 0.1, w: 0.3, h: 0.4 });
    expect(moved(['a', 'b', 'c'], 0, 2)).toEqual(['b', 'c', 'a']);
    expect(fullText([...PIECES, { id: 'x', area: null, rotation: 0, text: '  ' }])).toBe('Dear Marta,\n\nthe orchard is in bloom.\n\nLviv, May');
    expect(toInput([{ id: 'x', area: null, rotation: 0, text: 'a' }])).toEqual([{ id: 'x', rotation: 0, text: 'a' }]);
  });
});

describe('transcription overlay', () => {
  it('draws a box per marked piece and follows zoom and pan', async () => {
    const user = userEvent.setup();
    const { stage } = setup();
    expect(box(1).getAttribute('style')).toContain('left: 10%');
    expect(box(3).getAttribute('style')).toContain('height: 60%');
    expect(layer().getAttribute('dir')).toBe('ltr');
    stage.focus();
    await user.keyboard('+');
    const img = screen.getByRole('img');
    expect(img.getAttribute('style')).toContain('scale(2)');
    expect(layer().getAttribute('style')).toBe(img.getAttribute('style'));
    pointer(stage, 'pointerdown', 10, 10);
    pointer(stage, 'pointermove', 30, 20);
    pointer(stage, 'pointerup', 30, 20);
    await waitFor(() => expect(layer().getAttribute('style')).toContain('translate(20px, 10px)'));
    expect(layer().getAttribute('style')).toBe(img.getAttribute('style'));
  });

  it('show text hides and brings back the boxes', async () => {
    const user = userEvent.setup();
    setup();
    const toggle = screen.getByRole('button', { name: 'Show text' });
    expect(toggle).toHaveAttribute('aria-pressed', 'true');
    await user.click(toggle);
    expect(screen.queryByRole('button', { name: 'Text piece 1' })).toBeNull();
    expect(toggle).toHaveAttribute('aria-pressed', 'false');
    await user.click(toggle);
    expect(box(1)).toBeInTheDocument();
  });

  it('hover and keyboard focus show the text', async () => {
    const user = userEvent.setup();
    setup();
    await user.hover(box(2));
    const tip = screen.getByRole('tooltip');
    expect(tip.textContent).toContain('the orchard is in bloom.');
    expect(box(2).getAttribute('aria-describedby')).toContain(tip.id);
    expect(tip.getAttribute('dir')).toBe('auto');
    await user.unhover(box(2));
    expect(screen.queryByRole('tooltip')).toBeNull();
    box(3).focus();
    await waitFor(() => expect(screen.getByRole('tooltip').textContent).toContain('Lviv, May'));
    expect(screen.getByRole('tooltip').textContent).toContain('Turned 90°');
  });

  it('a box and its paragraph light up together', async () => {
    const user = userEvent.setup();
    setup();
    const panel = screen.getByRole('region', { name: 'Full text' });
    const paras = within(panel).getAllByRole('button');
    expect(paras.map((p) => p.textContent)).toEqual(['Dear Marta,', 'the orchard is in bloom.', 'Lviv, May']);
    await user.click(paras[1]);
    expect(box(2)).toHaveAttribute('aria-pressed', 'true');
    expect(box(1)).toHaveAttribute('aria-pressed', 'false');
    await user.click(box(3));
    expect(paras[2]).toHaveAttribute('aria-pressed', 'true');
    expect(paras[1]).toHaveAttribute('aria-pressed', 'false');
  });

  it('a picture with no transcription has no text controls but edit', () => {
    setup({ transcription: [] });
    expect(screen.queryByRole('button', { name: 'Show text' })).toBeNull();
    expect(screen.queryByRole('region', { name: 'Full text' })).toBeNull();
    expect(screen.getByRole('button', { name: 'Edit text' })).toBeInTheDocument();
  });
});

describe('editing the transcription', () => {
  it('keys move, resize, turn and delete a box without changing the picture', async () => {
    const user = userEvent.setup();
    const { spy, onChanged } = setup();
    await user.click(screen.getByRole('button', { name: 'Edit text' }));
    box(1).focus();
    expect(box(1).getAttribute('aria-describedby')).toContain('box-help');
    await user.keyboard('{ArrowRight}');
    expect(lastRegions(spy)[0]).toEqual({ id: 'r1', area: { x: 0.11, y: 0.1, w: 0.5, h: 0.1 }, rotation: 0, text: 'Dear Marta,' });
    expect(screen.getByText('1 of 2')).toBeInTheDocument();
    expect(onChanged).toHaveBeenCalled();
    await user.keyboard('{Shift>}{ArrowDown}{/Shift}');
    expect(lastRegions(spy)[0].area).toEqual({ x: 0.11, y: 0.1, w: 0.5, h: 0.11 });
    await user.keyboard('r');
    expect(lastRegions(spy)[0].rotation).toBe(90);
    await user.keyboard('{Delete}');
    expect(lastRegions(spy).map((r) => r.id)).toEqual(['r2', 'r3']);
    // The box went: focus is on the next one, still inside the dialog.
    await waitFor(() => expect(document.activeElement?.getAttribute('aria-label')).toBe('Text piece 1'));
    expect(screen.getByRole('dialog').contains(document.activeElement)).toBe(true);
  });

  it('dragging moves a box and a handle resizes it', async () => {
    const user = userEvent.setup();
    const { spy } = setup();
    await user.click(screen.getByRole('button', { name: 'Edit text' }));
    fakeSize(layer());
    pointer(box(2), 'pointerdown', 50, 50);
    pointer(layer(), 'pointermove', 70, 60);
    pointer(layer(), 'pointerup', 70, 60);
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(lastRegions(spy)[1].area).toEqual({ x: 0.2, y: 0.4, w: 0.6, h: 0.2 });
    const se = box(2).querySelector('.handle.se')!;
    pointer(se, 'pointerdown', 160, 60);
    pointer(layer(), 'pointermove', 180, 70);
    pointer(layer(), 'pointerup', 180, 70);
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(2));
    expect(lastRegions(spy)[1].area).toEqual({ x: 0.2, y: 0.4, w: 0.7, h: 0.3 });
  });

  it('marking a piece adds it and puts the cursor in its text', async () => {
    const user = userEvent.setup();
    const { spy } = setup();
    await user.click(screen.getByRole('button', { name: 'Edit text' }));
    await user.click(screen.getByRole('button', { name: 'Mark a piece' }));
    expect(screen.getByText('Drag across the picture where the text is written.')).toBeInTheDocument();
    fakeSize(layer());
    pointer(layer(), 'pointerdown', 20, 80);
    pointer(layer(), 'pointermove', 120, 95);
    pointer(layer(), 'pointerup', 120, 95);
    await waitFor(() => expect(spy).toHaveBeenCalled());
    const sent = lastRegions(spy);
    expect(sent).toHaveLength(4);
    expect(sent[3]).toMatchObject({ area: { x: 0.1, y: 0.8, w: 0.5, h: 0.15 }, rotation: 0, text: '' });
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole('textbox', { name: /Text of piece 4/ })));
    expect(screen.getByRole('button', { name: 'Mark a piece' })).toHaveAttribute('aria-pressed', 'false');
    await user.type(document.activeElement as HTMLElement, 'Your loving sister');
    await user.tab();
    await waitFor(() => expect(lastRegions(spy)[3]).toMatchObject({ id: sent[3].id, text: 'Your loving sister' }));
  });

  it('the full text panel edits, reorders, turns and deletes pieces', async () => {
    const user = userEvent.setup();
    const { spy } = setup();
    await user.click(screen.getByRole('button', { name: 'Edit text' }));
    const first = screen.getByRole('textbox', { name: /Text of piece 1/ });
    expect(first.getAttribute('dir')).toBe('auto');
    await user.clear(first);
    await user.type(first, 'Dear Marta and Ivan,');
    await user.tab();
    await waitFor(() => expect(lastRegions(spy)[0].text).toBe('Dear Marta and Ivan,'));
    // Focus on a paragraph selects its box.
    await user.click(screen.getByRole('textbox', { name: /Text of piece 2/ }));
    expect(box(2)).toHaveAttribute('aria-pressed', 'true');
    await user.click(screen.getByRole('button', { name: 'Move up' }));
    expect(lastRegions(spy).map((r) => r.id)).toEqual(['r2', 'r1', 'r3']);
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Move down' })));
    await user.click(screen.getByRole('button', { name: 'Turn' }));
    expect(lastRegions(spy)[0]).toMatchObject({ id: 'r2', rotation: 90 });
    await user.click(screen.getByRole('button', { name: 'Delete piece' }));
    expect(lastRegions(spy).map((r) => r.id)).toEqual(['r1', 'r3']);
    await waitFor(() => expect(screen.getByRole('dialog').contains(document.activeElement)).toBe(true));
    expect(document.activeElement?.tagName).toBe('TEXTAREA');
  });

  it('adds text with no place, and a PDF gets only that', async () => {
    const user = userEvent.setup();
    const { spy } = setup({ mime: 'application/pdf', transcription: [] });
    expect(screen.queryByRole('button', { name: 'Show text' })).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Edit text' }));
    expect(screen.queryByRole('button', { name: 'Mark a piece' })).toBeNull();
    expect(screen.getByText('Nothing is transcribed on this picture yet.')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Add text without a place' }));
    const ta = screen.getByRole('textbox', { name: /Text of piece 1/ });
    await waitFor(() => expect(document.activeElement).toBe(ta));
    expect(screen.getByText(/not marked on the picture/)).toBeInTheDocument();
    await user.type(ta, 'Page one, typed.');
    await user.tab();
    await waitFor(() => expect(lastRegions(spy).at(-1)).toMatchObject({ text: 'Page one, typed.' }));
    expect(lastRegions(spy).at(-1)).not.toHaveProperty('area');
  });

  it('a refused save shows why and puts the old pieces back', async () => {
    const user = userEvent.setup();
    const { spy } = setup();
    spy.mockRejectedValueOnce(new Error('the tree file is read-only'));
    await user.click(screen.getByRole('button', { name: 'Edit text' }));
    box(1).focus();
    await user.keyboard('{ArrowRight}');
    expect(await screen.findByText('the tree file is read-only')).toBeInTheDocument();
    expect(box(1).getAttribute('style')).toContain('left: 10%');
  });

  it('a click gives the box the keyboard, so arrows move it and not the picture', async () => {
    const user = userEvent.setup();
    const { spy } = setup();
    await user.click(screen.getByRole('button', { name: 'Edit text' }));
    await user.click(box(2));
    expect(document.activeElement).toBe(box(2));
    await user.keyboard('{ArrowRight}');
    await waitFor(() => expect(lastRegions(spy)[1].area).toEqual({ x: 0.11, y: 0.3, w: 0.6, h: 0.2 }));
    expect(screen.getByText('1 of 2')).toBeInTheDocument();
  });

  it('a cancelled drag is dropped, not saved', async () => {
    const user = userEvent.setup();
    const { spy } = setup();
    await user.click(screen.getByRole('button', { name: 'Edit text' }));
    fakeSize(layer());
    pointer(box(2), 'pointerdown', 50, 50);
    pointer(layer(), 'pointermove', 90, 70);
    pointer(layer(), 'pointercancel', 90, 70);
    pointer(layer(), 'pointerup', 90, 70);
    await new Promise((r) => setTimeout(r, 20));
    expect(spy).not.toHaveBeenCalled();
    expect(box(2).getAttribute('style')).toContain('left: 10%');
  });

  it('a failed save puts back what was last saved, and the edit built on it is not sent', async () => {
    const user = userEvent.setup();
    const { spy } = setup();
    let fail: (e: Error) => void = () => {};
    spy.mockImplementationOnce(() => new Promise((_, reject) => (fail = reject)));
    await user.click(screen.getByRole('button', { name: 'Edit text' }));
    box(1).focus();
    await user.keyboard('{ArrowRight}{ArrowRight}');
    expect(box(1).getAttribute('style')).toContain('left: 12%');
    fail(new Error('the disk is full'));
    expect(await screen.findByText('the disk is full')).toBeInTheDocument();
    await waitFor(() => expect(box(1).getAttribute('style')).toContain('left: 10%'));
    await new Promise((r) => setTimeout(r, 20));
    expect(spy).toHaveBeenCalledTimes(1);
  });

  it('text typed but not yet saved is saved when the viewer closes', async () => {
    const user = userEvent.setup();
    const { spy, onClose } = setup();
    await user.click(screen.getByRole('button', { name: 'Edit text' }));
    const first = screen.getByRole('textbox', { name: /Text of piece 1/ });
    await user.click(first);
    await user.type(first, ' and Ivan');
    await user.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalled();
    await waitFor(() => expect(lastRegions(spy)[0].text).toBe('Dear Marta, and Ivan'));
  });
});

describe('saving over a list that changed meanwhile', () => {
  it('keeps a piece an agent added after the viewer opened', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(30));
    const page = await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 30);
    let link: MediaLinkDto | undefined;
    for (const row of page.items) {
      link = (await api.getPerson(row.id)).media[0];
      if (link) break;
    }
    if (!link) throw new Error('the fixture tree has a picture');
    const id = link.media.id;
    await api.apply({ type: 'setMediaTranscription', id, regions: toInput(PIECES) });
    const opened = { ...link, media: { ...link.media, transcription: structuredClone(PIECES) } };
    renderWith(MediaViewer, { items: [opened], index: 0, onClose: vi.fn() }, { api });
    // An agent adds a piece; the open viewer does not know it.
    await api.apply({ type: 'setMediaTranscription', id, regions: [...toInput(PIECES), { text: 'Added by an agent' }] });
    await user.click(screen.getByRole('button', { name: 'Edit text' }));
    box(1).focus();
    await user.keyboard('{ArrowRight}');
    await waitFor(async () => {
      const m = await findMedia(api, id);
      expect(m?.transcription.map((r) => r.text)).toEqual(['Dear Marta,', 'the orchard is in bloom.', 'Lviv, May', 'Added by an agent']);
      expect(m?.transcription[0].area?.x).toBeCloseTo(0.11);
    });
  });
});

async function findMedia(api: MockApi, id: string): Promise<MediaDto | undefined> {
  const page = await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 30);
  for (const row of page.items) {
    const hit = (await api.getPerson(row.id)).media.find((l) => l.media.id === id);
    if (hit) return hit.media;
  }
  return undefined;
}
