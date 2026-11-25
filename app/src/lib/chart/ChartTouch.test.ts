import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { MockPlatform } from '../api/mock/MockPlatform';
import type { ChartKind, ChartNode } from '../api/types';
import type { MockApi } from '../api/mock/MockApi';
import ChartCanvas from './ChartCanvas.svelte';
import WholeCanvas from './WholeCanvas.svelte';
import Host from './testing/Host.svelte';
import { allNodes, attachContext, fakeRaf, lastCamera, makeApi, richestRoot } from './testing/helpers';

const RECT = { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600, x: 0, y: 0, toJSON() {} };

async function setupChart(kind: ChartKind = 'pedigree') {
  const raf = fakeRaf();
  const { api, fx } = makeApi();
  const root = await richestRoot(api, fx, kind);
  const onRecentre = vi.fn();
  const openSpy = vi.spyOn(api, 'chartOpen');
  const winSpy = vi.spyOn(api, 'chartWindow');
  render(Host, {
    props: { api, view: ChartCanvas, props: { kind, root, generations: 6, onRecentre, onOpenPerson: vi.fn() }, platform: new MockPlatform() },
  });
  const canvas = screen.getByRole('application') as HTMLCanvasElement;
  const ctx = attachContext(canvas);
  canvas.getBoundingClientRect = () => RECT;
  await waitFor(() => expect(openSpy).toHaveBeenCalled());
  await waitFor(() => expect(winSpy.mock.calls.length).toBeGreaterThanOrEqual(2));
  await new Promise((r) => setTimeout(r, 0));
  raf.pump(2);
  return { api, canvas, ctx, raf, openSpy, onRecentre };
}

const menuOpen = () => screen.queryByRole('menu');

function finger(el: Element, type: string, at: { clientX: number; clientY: number }, id = 1) {
  const e = new MouseEvent(type, { bubbles: true, cancelable: true, ...at });
  for (const [k, v] of Object.entries({ pointerType: 'touch', pointerId: id })) Object.defineProperty(e, k, { value: v });
  el.dispatchEvent(e);
  return e;
}

/** A box that is on screen and is not the selected one: where a finger can land. */
async function aBox(s: Awaited<ReturnType<typeof setupChart>>) {
  const cam = lastCamera(s.ctx)!;
  const first = screen.getByRole('option').textContent;
  const nodes: ChartNode[] = await allNodes(s.api as MockApi, (await s.openSpy.mock.results[0].value).chartId);
  const n = nodes.find((x) => x.label !== first && x.dupOf === null && x.person && x.x >= cam.x && (x.x + x.w - cam.x) * cam.zoom < 780 && (x.y - cam.y) * cam.zoom > 0 && (x.y + x.h - cam.y) * cam.zoom < 590)!;
  return { n, at: { clientX: (n.x + n.w / 2 - cam.x) * cam.zoom, clientY: (n.y + n.h / 2 - cam.y) * cam.zoom } };
}

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('touch on a chart', () => {
  it('a finger held on a box selects it and opens the menu once; the native menu event adds nothing', async () => {
    const s = await setupChart();
    const { n, at } = await aBox(s);
    vi.useFakeTimers({ shouldAdvanceTime: true });
    finger(s.canvas, 'pointerdown', at);
    await vi.advanceTimersByTimeAsync(300);
    expect(menuOpen()).toBeNull();
    await vi.advanceTimersByTimeAsync(300);
    await waitFor(() => expect(screen.getByRole('menu')).toBeTruthy());
    await waitFor(() => expect(screen.getByRole('option').textContent).toBe(n.label));
    const native = new MouseEvent('contextmenu', { bubbles: true, cancelable: true, ...at });
    Object.defineProperty(native, 'pointerType', { value: 'touch' });
    s.canvas.dispatchEvent(native);
    finger(s.canvas, 'pointerup', at);
    expect(screen.getAllByRole('menu')).toHaveLength(1);
    // The lift after a held press is not a tap: nothing was recentred.
    expect(s.onRecentre).not.toHaveBeenCalled();
  });

  it('moving the finger pans and cancels the hold', async () => {
    const s = await setupChart();
    const { at } = await aBox(s);
    vi.useFakeTimers({ shouldAdvanceTime: true });
    finger(s.canvas, 'pointerdown', at);
    finger(s.canvas, 'pointermove', { clientX: at.clientX + 40, clientY: at.clientY + 40 });
    await vi.advanceTimersByTimeAsync(800);
    expect(menuOpen()).toBeNull();
  });

  it('two fingers (a pinch) open no menu', async () => {
    const s = await setupChart();
    const { at } = await aBox(s);
    vi.useFakeTimers({ shouldAdvanceTime: true });
    finger(s.canvas, 'pointerdown', at, 1);
    finger(s.canvas, 'pointerdown', { clientX: at.clientX + 100, clientY: at.clientY }, 2);
    await vi.advanceTimersByTimeAsync(800);
    expect(menuOpen()).toBeNull();
  });

  it('a right-click with the mouse still opens the menu at once', async () => {
    const s = await setupChart();
    await fireEvent.contextMenu(s.canvas, { button: 2, clientX: 5, clientY: 5 });
    await waitFor(() => expect(screen.getByRole('menu')).toBeTruthy());
  });

  it('two quick taps on a box recentre on it, like a double-click; one tap only selects', async () => {
    const s = await setupChart();
    const { n, at } = await aBox(s);
    finger(s.canvas, 'pointerdown', at);
    finger(s.canvas, 'pointerup', at);
    await waitFor(() => expect(screen.getByRole('option').textContent).toBe(n.label));
    expect(s.onRecentre).not.toHaveBeenCalled();
    finger(s.canvas, 'pointerdown', at);
    finger(s.canvas, 'pointerup', at);
    expect(s.raf).toBeTruthy();
    // The native double-click that follows a double tap must not recentre a second time.
    const before = s.onRecentre.mock.calls.length;
    await fireEvent.dblClick(s.canvas, at);
    expect(s.onRecentre.mock.calls.length).toBe(before);
  });
});

describe('touch on the whole-tree chart', () => {
  it('a finger held on empty space opens the menu', async () => {
    fakeRaf();
    const { api } = makeApi();
    render(Host, { props: { api, view: WholeCanvas, props: { onOpenPerson: vi.fn(), onOpenInPedigree: vi.fn() }, platform: new MockPlatform() } });
    const canvas = (await screen.findByRole('application')) as HTMLCanvasElement;
    attachContext(canvas);
    canvas.getBoundingClientRect = () => RECT;
    await new Promise((r) => setTimeout(r, 0));
    vi.useFakeTimers({ shouldAdvanceTime: true });
    finger(canvas, 'pointerdown', { clientX: 4, clientY: 4 });
    await vi.advanceTimersByTimeAsync(700);
    await waitFor(() => expect(screen.getByRole('menu')).toBeTruthy());
  });
});
