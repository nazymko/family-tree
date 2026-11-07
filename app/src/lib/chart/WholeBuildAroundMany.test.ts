import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import type { MockApi } from '../api/mock/MockApi';
import type { ChartNode } from '../api/types';
import { setLanguage } from '../i18n';
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
  const onBuildAround = vi.fn();
  render(Host, { props: { api, view: WholeCanvas, props: { homeId: null, onOpenPerson: vi.fn(), onOpenInPedigree: vi.fn(), onShowSelection: vi.fn(), onBuildAround } } });
  const canvas = screen.getByRole('application', { name: /Whole tree chart|Діаграма всього родоводу/ }) as HTMLCanvasElement;
  const ctx = attachContext(canvas);
  canvas.getBoundingClientRect = () => RECT;
  const mini = (await screen.findByRole('application', { name: /Minimap|Мінікарта/ })) as HTMLCanvasElement;
  mini.getBoundingClientRect = () => ({ ...RECT, width: 220, height: mini.height, right: 220, bottom: mini.height });
  attachContext(mini);
  await waitFor(() => expect(openSpy).toHaveBeenCalled());
  await waitFor(() => expect(winSpy.mock.calls.length).toBeGreaterThanOrEqual(2));
  await new Promise((r) => setTimeout(r, 0));
  raf.pump(2);
  for (let i = 0; i < 5; i++) {
    await fireEvent.keyDown(canvas, { key: '-' });
    for (let k = 0; k < 3; k++) {
      await new Promise((r) => setTimeout(r, 0));
      raf.pump(2);
    }
  }
  return { api: api as MockApi, fx, raf, canvas, ctx, openSpy, onBuildAround };
}
type Setup = Awaited<ReturnType<typeof setup>>;

async function settle(s: Setup): Promise<void> {
  for (let i = 0; i < 4; i++) {
    await new Promise((r) => setTimeout(r, 0));
    s.raf.pump(2);
  }
}

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

afterEach(() => {
  vi.unstubAllGlobals();
  setLanguage('en');
});

describe('the build-around button on the whole tree with several people marked', () => {
  it('reads_for_one_person_until_two_are_marked_then_counts_them_in_english', async () => {
    const s = await setup();
    const [a, b, c] = await cardsInView(s);
    await click(s, a);
    expect(screen.getByRole('button', { name: 'Build around this person' })).toBeEnabled();
    await click(s, b, { shift: true });
    expect(screen.getByRole('button', { name: 'Build around these people (2)' })).toBeEnabled();
    await click(s, c, { shift: true });
    expect(screen.getByRole('button', { name: 'Build around these people (3)' })).toBeEnabled();
    await click(s, c, { shift: true });
    await click(s, b, { shift: true });
    expect(screen.getByRole('button', { name: 'Build around this person' })).toBeInTheDocument();
  });

  it('reads_in_ukrainian_and_hands_all_marked_people_over_in_the_order_marked', async () => {
    setLanguage('uk');
    const s = await setup();
    const [a, b, c] = await cardsInView(s);
    await click(s, a);
    await click(s, c, { shift: true });
    await click(s, b, { shift: true });
    const btn = screen.getByRole('button', { name: 'Побудувати навколо цих осіб (3)' });
    await fireEvent.click(btn);
    expect(s.onBuildAround).toHaveBeenCalledWith([a.person!.id, c.person!.id, b.person!.id]);
  });

  it('with_one_selected_person_it_builds_around_that_person_as_before', async () => {
    const s = await setup();
    const [a] = await cardsInView(s);
    await click(s, a);
    await fireEvent.click(screen.getByRole('button', { name: 'Build around this person' }));
    expect(s.onBuildAround).toHaveBeenCalledWith([a.person!.id]);
  });
});
