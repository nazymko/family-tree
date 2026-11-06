import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import type { MockApi } from '../api/mock/MockApi';
import type { ChartNode } from '../api/types';
import WholeCanvas from './WholeCanvas.svelte';
import Host from './testing/Host.svelte';
import { attachContext, fakeRaf, lastCamera, makeApi } from './testing/helpers';

const RECT = { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600, x: 0, y: 0, toJSON() {} };

async function setup() {
  const mm = window.matchMedia;
  window.matchMedia = ((q: string) => ({ ...mm(q), matches: q.includes('reduce') })) as typeof window.matchMedia;
  const raf = fakeRaf();
  const { api, fx } = makeApi(300);
  const winSpy = vi.spyOn(api, 'chartWindow');
  const openSpy = vi.spyOn(api, 'chartOpenWhole');
  const onOpenPerson = vi.fn();
  const onOpenInPedigree = vi.fn();
  const onShowSelection = vi.fn();
  render(Host, { props: { api, view: WholeCanvas, props: { homeId: null, onOpenPerson, onOpenInPedigree, onShowSelection } } });
  const canvas = screen.getByRole('application', { name: 'Whole tree chart' }) as HTMLCanvasElement;
  const ctx = attachContext(canvas);
  canvas.getBoundingClientRect = () => RECT;
  const mini = (await screen.findByRole('application', { name: /Minimap/ })) as HTMLCanvasElement;
  mini.getBoundingClientRect = () => ({ ...RECT, width: 220, height: mini.height, right: 220, bottom: mini.height });
  attachContext(mini);
  await waitFor(() => expect(openSpy).toHaveBeenCalled());
  await waitFor(() => expect(winSpy.mock.calls.length).toBeGreaterThanOrEqual(2));
  await new Promise((r) => setTimeout(r, 0));
  raf.pump(2);
  // Zoom out a little so several cards are on screen.
  const out = async () => {
    await fireEvent.keyDown(canvas, { key: '-' });
    for (let i = 0; i < 3; i++) {
      await new Promise((r) => setTimeout(r, 0));
      raf.pump(2);
    }
  };
  for (let i = 0; i < 5; i++) await out();
  return { api: api as MockApi, fx, raf, canvas, ctx, openSpy, onOpenPerson, onOpenInPedigree, onShowSelection };
}
type Setup = Awaited<ReturnType<typeof setup>>;

async function settle(s: Setup): Promise<void> {
  for (let i = 0; i < 4; i++) {
    await new Promise((r) => setTimeout(r, 0));
    s.raf.pump(2);
  }
}

/** Real people whose whole card is on screen, left to right. */
async function cardsInView(s: Setup): Promise<ChartNode[]> {
  const id = (await s.openSpy.mock.results[0].value).meta.chartId;
  const cam = lastCamera(s.ctx)!;
  const win = await s.api.chartWindow(id, { x: cam.x, y: cam.y, w: 800 / cam.zoom, h: 600 / cam.zoom }, 1);
  return win.nodes
    .filter((n) => n.person && !n.person.siblingMarker)
    .filter((n) => {
      const cx = (n.x + n.w / 2 - cam.x) * cam.zoom;
      const cy = (n.y + n.h / 2 - cam.y) * cam.zoom;
      return cx > 20 && cx < 780 && cy > 20 && cy < 580;
    })
    .sort((a, b) => a.x - b.x || a.y - b.y);
}

async function click(s: Setup, n: ChartNode, opts: { shift?: boolean } = {}): Promise<void> {
  const cam = lastCamera(s.ctx)!;
  const at = { clientX: (n.x + n.w / 2 - cam.x) * cam.zoom, clientY: (n.y + n.h / 2 - cam.y) * cam.zoom, shiftKey: !!opts.shift, bubbles: true };
  // jsdom's PointerEvent drops the coordinates and modifier keys, so send mouse events under the pointer events' names.
  await fireEvent(s.canvas, new MouseEvent('pointerdown', at));
  await fireEvent(s.canvas, new MouseEvent('pointerup', at));
  await settle(s);
}

const chip = () => screen.queryByTestId('marked-count');

afterEach(() => vi.unstubAllGlobals());

describe('WholeCanvas multi-selection', () => {
  it('shift_click_adds_and_removes_people_and_the_first_selected_person_counts', async () => {
    const s = await setup();
    const [a, b, c] = await cardsInView(s);
    expect(c).toBeTruthy();
    expect(chip()).toBeNull();
    await click(s, a);
    expect(chip()).toBeNull();
    await click(s, b, { shift: true });
    expect(chip()).toHaveTextContent('2 people selected');
    await click(s, c, { shift: true });
    expect(chip()).toHaveTextContent('3 people selected');
    expect(screen.getByRole('status')).toHaveTextContent('3 people selected');
    await click(s, c, { shift: true });
    expect(chip()).toHaveTextContent('2 people selected');
    await click(s, b, { shift: true });
    expect(chip()).toHaveTextContent('1 person selected');
    await click(s, a, { shift: true });
    expect(chip()).toBeNull();
  });

  it('marked_cards_get_a_check_badge_besides_the_colour', async () => {
    const s = await setup();
    const [a, b] = await cardsInView(s);
    await click(s, a);
    const arcs = () => s.ctx.calls.filter((x) => x.op === 'arc').length;
    const before = arcs();
    await click(s, b, { shift: true });
    s.raf.pump(3);
    // The badges are circles: one per marked card in every frame drawn since.
    expect(arcs()).toBeGreaterThan(before);
  });

  it('a_plain_click_clears_the_multi_selection_and_selects_one_person', async () => {
    const s = await setup();
    const [a, b, c] = await cardsInView(s);
    await click(s, a);
    await click(s, b, { shift: true });
    expect(chip()).not.toBeNull();
    await click(s, c);
    expect(chip()).toBeNull();
    expect(screen.getByRole('listbox', { name: 'Selected person' }).textContent).toContain(c.person!.name);
  });

  it('escape_and_the_clear_button_drop_the_selection', async () => {
    const s = await setup();
    const [a, b] = await cardsInView(s);
    await click(s, a);
    await click(s, b, { shift: true });
    await fireEvent.click(screen.getByRole('button', { name: 'Clear selection' }));
    expect(chip()).toBeNull();
    expect(screen.getByRole('status')).toHaveTextContent('No people selected.');
    await click(s, a);
    await click(s, b, { shift: true });
    await fireEvent.keyDown(s.canvas, { key: 'Escape' });
    expect(chip()).toBeNull();
    expect(screen.getByRole('listbox', { name: 'Selected person' }).textContent).toBe('');
  });

  it('space_toggles_the_selected_person_for_keyboard_users', async () => {
    const s = await setup();
    const [a, b] = await cardsInView(s);
    await click(s, a);
    await fireEvent.keyDown(s.canvas, { key: ' ' });
    expect(chip()).toHaveTextContent('1 person selected');
    await click(s, b, { shift: true });
    expect(chip()).toHaveTextContent('2 people selected');
    await fireEvent.keyDown(s.canvas, { key: ' ' });
    expect(chip()).toHaveTextContent('1 person selected');
  });

  it('the_button_and_P_show_one_person_in_the_pedigree_and_several_in_one_chart', async () => {
    const s = await setup();
    const [a, b, c] = await cardsInView(s);
    const button = screen.getByRole('button', { name: 'Show in Pedigree' });
    await click(s, a);
    await fireEvent.click(button);
    expect(s.onOpenInPedigree).toHaveBeenCalledWith(a.person!.id);
    expect(s.onShowSelection).not.toHaveBeenCalled();
    s.onOpenInPedigree.mockClear();
    await fireEvent.keyDown(s.canvas, { key: 'p' });
    expect(s.onOpenInPedigree).toHaveBeenCalledWith(a.person!.id);
    s.onOpenInPedigree.mockClear();

    await click(s, b, { shift: true });
    await click(s, c, { shift: true });
    expect(button).toBeEnabled();
    await fireEvent.click(button);
    expect(s.onShowSelection).toHaveBeenLastCalledWith([a.person!.id, b.person!.id, c.person!.id]);
    await fireEvent.keyDown(s.canvas, { key: 'P' });
    expect(s.onShowSelection).toHaveBeenCalledTimes(2);
    expect(s.onOpenInPedigree).not.toHaveBeenCalled();
  });

  it('shift_arrows_keep_panning_and_leave_the_selection_alone', async () => {
    const s = await setup();
    const [a, b] = await cardsInView(s);
    await click(s, a);
    await click(s, b, { shift: true });
    const before = lastCamera(s.ctx)!;
    await fireEvent.keyDown(s.canvas, { key: 'ArrowRight', shiftKey: true });
    s.raf.pump(2);
    expect(lastCamera(s.ctx)!.x).toBeGreaterThan(before.x);
    expect(chip()).toHaveTextContent('2 people selected');
  });
});
