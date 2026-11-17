import { describe, expect, it, vi } from 'vitest';
import { screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import MediaViewer from './MediaViewer.svelte';
import MediaGrid from './MediaGrid.svelte';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import type { EditCommand, MediaDto, MediaLinkDto, TextRegionDto } from '../api/api';
import { cornerShownAt, moveOnPicture, pointOnPicture, shownTurn, turnedBy, unturn, type Rotation } from './transcription';

// Invented pieces on an invented sideways letter.
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
  width: 200,
  height: 100,
  sha256: null,
  transcription: PIECES,
  rotation: 0,
  ...over,
});

const OK = { label: 'Turn picture', focus: null, touched: [], undo: { canUndo: true, canRedo: false, undoLabel: null, redoLabel: null } };
const TURNS: Rotation[] = [0, 90, 180, 270];

function setup(over: Partial<MediaDto> = {}) {
  const api = new MockApi(fixtureTree(5));
  const spy = vi.spyOn(api, 'apply').mockResolvedValue(OK);
  const items: MediaLinkDto[] = [{ media: media('m0', over), primary: false }, { media: media('m1'), primary: false }];
  const onChanged = vi.fn();
  renderWith(MediaViewer, { items, index: 0, onClose: vi.fn(), onChanged }, { api });
  const stage = screen.getByRole('group', { name: /^Preview/ });
  return { spy, stage, onChanged };
}

const box = (i: number) => screen.getByRole('button', { name: `Text piece ${i}` });
const layer = () => document.querySelector<HTMLElement>('.layer')!;
const frame = () => document.querySelector<HTMLElement>('.frame')!;
/** The turned picture's box on screen: a 200 by 100 picture, standing up across a quarter turn. */
function fakeSize(turn: Rotation) {
  const [width, height] = turn % 180 === 0 ? [200, 100] : [100, 200];
  layer().getBoundingClientRect = () => ({ left: 0, top: 0, right: width, bottom: height, width, height, x: 0, y: 0, toJSON: () => ({}) });
}
function pointer(el: Element, type: string, x: number, y: number) {
  el.dispatchEvent(new MouseEvent(type, { clientX: x, clientY: y, bubbles: true }));
}
function lastRegions(spy: ReturnType<typeof vi.spyOn>) {
  const cmd = spy.mock.calls.at(-1)?.[0] as Extract<EditCommand, { type: 'setMediaTranscription' }>;
  expect(cmd.type).toBe('setMediaTranscription');
  return cmd.regions;
}
const close = (a: number, b: number) => Math.abs(a - b) < 1e-9;
function expectArea(got: { x: number; y: number; w: number; h: number } | undefined, want: { x: number; y: number; w: number; h: number }) {
  expect(got).toBeDefined();
  for (const k of ['x', 'y', 'w', 'h'] as const) expect(close(got![k], want[k]), `${k}: ${got![k]} vs ${want[k]}`).toBe(true);
}

describe('turn geometry', () => {
  it('a piece shows the turn left after the picture is turned', () => {
    // Writing running down the page (90) reads upright once the picture is turned left once (270).
    expect(shownTurn(90, 270)).toBe(0);
    expect(shownTurn(90, 90)).toBe(180);
    expect(shownTurn(270, 90)).toBe(0);
    expect(shownTurn(0, 90)).toBe(90);
    expect(shownTurn(180, 180)).toBe(0);
    expect(turnedBy(0, -90)).toBe(270);
    expect(turnedBy(270, 90)).toBe(0);
  });

  it('screen points and moves map back onto the picture at every turn', () => {
    const box = (t: Rotation) => (t % 180 === 0 ? { left: 0, top: 0, w: 200, h: 100 } : { left: 0, top: 0, w: 100, h: 200 });
    // Where the picture's own top left corner shows on screen.
    const topLeft: Record<Rotation, [number, number]> = { 0: [0, 0], 90: [100, 0], 180: [200, 100], 270: [0, 200] };
    for (const t of TURNS) {
      expect(pointOnPicture(t, box(t), ...topLeft[t])).toEqual({ x: 0, y: 0 });
      const mid = pointOnPicture(t, box(t), box(t).w / 2, box(t).h / 2);
      expect(mid).toEqual({ x: 0.5, y: 0.5 });
    }
    expect(moveOnPicture(90, box(90), 20, 10)).toEqual({ dx: 0.05, dy: -0.2 });
    expect(moveOnPicture(270, box(270), 20, 10)).toEqual({ dx: -0.05, dy: 0.2 });
    expect(unturn(180, 1, 0)).toEqual([-1, 0]);
    expect(TURNS.map((t) => cornerShownAt(t, 'se'))).toEqual(['se', 'ne', 'nw', 'sw']);
    expect(TURNS.map((t) => cornerShownAt(t, 'ne'))).toEqual(['ne', 'nw', 'sw', 'se']);
  });
});

describe('turning the picture', () => {
  it('turn right saves a quarter turn, turns picture and boxes together and fits it again', async () => {
    const user = userEvent.setup();
    const { spy, stage, onChanged } = setup();
    stage.focus();
    await user.keyboard('+');
    expect(screen.getByText('Zoom 200%')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Turn right' }));
    expect(spy).toHaveBeenCalledWith({ type: 'setMediaRotation', id: 'm0', rotation: 90 });
    await waitFor(() => expect(onChanged).toHaveBeenCalled());
    expect(screen.getByText('Zoom 100%')).toBeInTheDocument();
    expect(frame().dataset.turn).toBe('90');
    const img = screen.getByRole('img');
    expect(img.getAttribute('style')).toContain('rotate(90deg)');
    expect(layer().getAttribute('style')).toBe(img.getAttribute('style'));
    // The 200 by 100 picture stands up: the frame is 1 by 2, the picture inside it as wide as the frame is tall.
    expect(frame().getAttribute('style')).toContain('--ratio: 0.5');
    expect(img.getAttribute('style')).toContain('width: 200%');
    expect(img.getAttribute('style')).toContain('height: 50%');
    // The boxes keep their stored places.
    expect(box(1).getAttribute('style')).toContain('left: 10%');
  });

  it('the bracket keys turn left and right, and R and the arrows keep their jobs', async () => {
    const user = userEvent.setup();
    const { spy, stage } = setup();
    stage.focus();
    await user.keyboard('[[');
    expect(spy).toHaveBeenLastCalledWith({ type: 'setMediaRotation', id: 'm0', rotation: 270 });
    await user.keyboard('[[');
    expect(spy).toHaveBeenLastCalledWith({ type: 'setMediaRotation', id: 'm0', rotation: 180 });
    await user.keyboard(']');
    expect(spy).toHaveBeenLastCalledWith({ type: 'setMediaRotation', id: 'm0', rotation: 270 });
    expect(screen.getByRole('button', { name: 'Turn left' })).toHaveAttribute('aria-keyshortcuts', '[');
    await user.keyboard('{ArrowRight}');
    expect(screen.getByText('2 of 2')).toBeInTheDocument();
  });

  it('a refused turn shows why and goes back', async () => {
    const user = userEvent.setup();
    const { spy } = setup({ rotation: 90 });
    spy.mockRejectedValueOnce(new Error('the tree file is read-only'));
    await user.click(screen.getByRole('button', { name: 'Turn right' }));
    expect(await screen.findByText('the tree file is read-only')).toBeInTheDocument();
    await waitFor(() => expect(frame().dataset.turn).toBe('90'));
  });

  it('a document or a missing file has no turn buttons', () => {
    setup({ mime: 'application/pdf', transcription: [] });
    expect(screen.queryByRole('button', { name: 'Turn right' })).toBeNull();
  });

  it("a piece's badge shows the turn left on screen; none once its writing reads upright", async () => {
    const user = userEvent.setup();
    setup({ rotation: 270 });
    expect(box(3).querySelector('.turn')).toBeNull();
    box(3).focus();
    await waitFor(() => expect(screen.getByRole('tooltip').textContent).toContain('Lviv, May'));
    expect(screen.getByRole('tooltip').textContent).not.toContain('Turned');
    await user.click(screen.getByRole('button', { name: 'Turn right' }));
    await waitFor(() => expect(box(3).querySelector('.turn')?.textContent).toBe('↻90°'));
    await user.click(screen.getByRole('button', { name: 'Turn right' }));
    await waitFor(() => expect(box(3).querySelector('.turn')?.textContent).toBe('↻180°'));
    // The badge itself reads upright: turned back by the picture's turn.
    expect(box(3).querySelector('.turn')?.getAttribute('style')).toContain('rotate(-90deg)');
    // The full text says the same as the badges.
    await user.click(screen.getByRole('button', { name: 'Edit text' }));
    expect(screen.getByText(/Text of piece 3/).textContent).toContain('Turned 180°');
    expect(screen.getByText(/Text of piece 1/).textContent).toContain('Turned 90°');
  });
});

describe('editing boxes on a turned picture', () => {
  for (const turn of TURNS) {
    it(`dragging moves and resizes a box the way the pointer goes at ${turn}°`, async () => {
      const user = userEvent.setup();
      const { spy } = setup({ rotation: turn });
      await user.click(screen.getByRole('button', { name: 'Edit text' }));
      fakeSize(turn);
      pointer(box(2), 'pointerdown', 50, 50);
      pointer(layer(), 'pointermove', 70, 60);
      pointer(layer(), 'pointerup', 70, 60);
      await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
      const { dx, dy } = moveOnPicture(turn, turn % 180 === 0 ? { left: 0, top: 0, w: 200, h: 100 } : { left: 0, top: 0, w: 100, h: 200 }, 20, 10);
      const moved = { x: Math.min(Math.max(0.1 + dx, 0), 0.4), y: Math.min(Math.max(0.3 + dy, 0), 0.8), w: 0.6, h: 0.2 };
      expectArea(lastRegions(spy)[1].area ?? undefined, moved);
      // The stored south-east corner, wherever it shows, follows the pointer.
      const se = box(2).querySelector('.handle.se')!;
      pointer(se, 'pointerdown', 100, 100);
      pointer(layer(), 'pointermove', 110, 90);
      pointer(layer(), 'pointerup', 110, 90);
      await waitFor(() => expect(spy).toHaveBeenCalledTimes(2));
      const r = moveOnPicture(turn, turn % 180 === 0 ? { left: 0, top: 0, w: 200, h: 100 } : { left: 0, top: 0, w: 100, h: 200 }, 10, -10);
      expectArea(lastRegions(spy)[1].area ?? undefined, { x: moved.x, y: moved.y, w: 0.6 + r.dx, h: 0.2 + r.dy });
    });

    it(`marking a piece at ${turn}° stores it on the upright picture`, async () => {
      const user = userEvent.setup();
      const { spy } = setup({ rotation: turn });
      await user.click(screen.getByRole('button', { name: 'Edit text' }));
      await user.click(screen.getByRole('button', { name: 'Mark text' }));
      fakeSize(turn);
      // From where the picture's own top left corner shows, to the middle.
      const from: Record<Rotation, [number, number]> = { 0: [0, 0], 90: [100, 0], 180: [200, 100], 270: [0, 200] };
      const mid: [number, number] = turn % 180 === 0 ? [100, 50] : [50, 100];
      pointer(layer(), 'pointerdown', ...from[turn]);
      pointer(layer(), 'pointermove', ...mid);
      pointer(layer(), 'pointerup', ...mid);
      await waitFor(() => expect(spy).toHaveBeenCalled());
      expectArea(lastRegions(spy)[3].area ?? undefined, { x: 0, y: 0, w: 0.5, h: 0.5 });
    });
  }

  it('arrow keys move a box the way they point on screen', async () => {
    const user = userEvent.setup();
    const { spy } = setup({ rotation: 90 });
    await user.click(screen.getByRole('button', { name: 'Edit text' }));
    box(1).focus();
    // Right on screen is up the picture once it is turned right.
    await user.keyboard('{ArrowRight}');
    expectArea(lastRegions(spy)[0].area ?? undefined, { x: 0.1, y: 0.09, w: 0.5, h: 0.1 });
    // Shift moves the corner that shows bottom right: here the stored top right.
    await user.keyboard('{Shift>}{ArrowRight}{/Shift}');
    expectArea(lastRegions(spy)[0].area ?? undefined, { x: 0.1, y: 0.08, w: 0.5, h: 0.11 });
  });
});

describe('thumbnails', () => {
  it('show the saved turn', async () => {
    const api = new MockApi(fixtureTree(5));
    const items: MediaLinkDto[] = [{ media: media('m0', { rotation: 90 }), primary: false }, { media: media('m1'), primary: false }];
    renderWith(MediaGrid, { items, onOpen: vi.fn() }, { api });
    const [turned, plain] = screen.getAllByRole('img');
    expect(turned.getAttribute('style')).toContain('rotate(90deg)');
    expect(plain.getAttribute('style') ?? '').not.toContain('rotate');
  });
});
