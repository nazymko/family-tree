import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import type { ChartKind, ChartNode } from '../api/types';
import { MockApi } from '../api/mock/MockApi';
import ChartCanvas from './ChartCanvas.svelte';
import Host from './testing/Host.svelte';
import { BIG, allNodes, attachContext, fakeRaf, lastCamera, makeApi, richestRoot } from './testing/helpers';

type Setup = Awaited<ReturnType<typeof setup>>;

async function setup(opts: { kind?: ChartKind; root?: string; reducedMotion?: boolean } = {}) {
  const raf = fakeRaf();
  const { api, fx } = makeApi();
  const kind = opts.kind ?? 'pedigree';
  const root = opts.root ?? (await richestRoot(api, fx, kind));
  const onRecentre = vi.fn();
  const onOpenPerson = vi.fn();
  const openSpy = vi.spyOn(api, 'chartOpen');
  const winSpy = vi.spyOn(api, 'chartWindow');
  if (opts.reducedMotion) {
    const mm = window.matchMedia;
    window.matchMedia = ((q: string) => ({ ...mm(q), matches: q.includes('reduce') })) as typeof window.matchMedia;
  }
  render(Host, { props: { api, view: ChartCanvas, props: { kind, root, generations: 6, onRecentre, onOpenPerson } } });
  const canvas = screen.getByRole('application') as HTMLCanvasElement;
  const ctx = attachContext(canvas);
  canvas.getBoundingClientRect = () => ({ left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600, x: 0, y: 0, toJSON() {} });
  await waitFor(() => expect(openSpy).toHaveBeenCalled());
  await waitFor(() => expect(winSpy.mock.calls.length).toBeGreaterThanOrEqual(2));
  await new Promise((r) => setTimeout(r, 0));
  raf.pump(2);
  return { api, raf, canvas, ctx, root, onRecentre, onOpenPerson, openSpy, winSpy };
}

const selectedLabel = () => screen.getByRole('option').textContent;

async function nodesOf(s: Setup): Promise<ChartNode[]> {
  const id = (await s.openSpy.mock.results[0].value).chartId;
  return allNodes(s.api as MockApi, id);
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('ChartCanvas', () => {
  it('opens_chart_and_draws_once_per_change', async () => {
    const s = await setup();
    expect(s.openSpy).toHaveBeenCalledWith({ kind: 'pedigree', root: s.root, generations: 6 });
    expect(s.ctx.calls.filter((c) => c.op === 'fillText').length).toBeGreaterThan(0);
    const before = s.ctx.calls.length;
    s.raf.pump(10);
    expect(s.ctx.calls.length).toBe(before);
    await fireEvent.wheel(s.canvas, { deltaY: -100, clientX: 400, clientY: 300 });
    s.raf.pump(10);
    expect(s.ctx.calls.filter((c) => c.op === 'setTransform').length).toBeGreaterThan(0);
    const after = s.ctx.calls.length;
    s.raf.pump(10);
    expect(s.ctx.calls.length).toBe(after);
  });

  it('wheel_zoom_calls_window_only_after_leaving_cache', async () => {
    const s = await setup();
    const base = s.winSpy.mock.calls.length;
    await fireEvent.wheel(s.canvas, { deltaY: -100, clientX: 400, clientY: 300 });
    await fireEvent.wheel(s.canvas, { deltaY: 100, clientX: 400, clientY: 300 });
    expect(s.winSpy.mock.calls.length).toBe(base);
    for (let i = 0; i < 12; i++) await fireEvent.wheel(s.canvas, { deltaY: 200, clientX: 400, clientY: 300 });
    expect(s.winSpy.mock.calls.length).toBeGreaterThan(base);
  });

  it('arrow_keys_move_selection_via_nav', async () => {
    const s = await setup();
    const nodes = await nodesOf(s);
    const byId = new Map(nodes.map((n) => [n.id, n]));
    const root = nodes.find((n) => n.ahnen === 1)!;
    expect(selectedLabel()).toBe(root.label);
    // Pedigree: Right goes towards the ancestors (nav.child), Left back to the root.
    await fireEvent.keyDown(s.canvas, { key: 'ArrowRight' });
    const first = byId.get(root.nav.child!)!;
    expect(selectedLabel()).toBe(first.label);
    // Up and Down move between siblings (father and mother).
    await fireEvent.keyDown(s.canvas, { key: 'ArrowDown' });
    expect(selectedLabel()).toBe(byId.get(first.nav.next!)!.label);
    await fireEvent.keyDown(s.canvas, { key: 'ArrowUp' });
    expect(selectedLabel()).toBe(first.label);
    await fireEvent.keyDown(s.canvas, { key: 'ArrowLeft' });
    expect(selectedLabel()).toBe(root.label);
    await fireEvent.keyDown(s.canvas, { key: 'ArrowRight' });
    await fireEvent.keyDown(s.canvas, { key: 'Home' });
    expect(selectedLabel()).toBe(root.label);
  });

  it('descendants_arrows_follow_the_top_down_layout', async () => {
    const { api, fx } = makeApi();
    const root = await richestRoot(api, fx, 'descendants');
    const raf = fakeRaf();
    render(Host, { props: { api, view: ChartCanvas, props: { kind: 'descendants', root, generations: 4, onRecentre: vi.fn(), onOpenPerson: vi.fn() } } });
    const canvas = screen.getByRole('application');
    attachContext(canvas as HTMLCanvasElement);
    await waitFor(() => expect(screen.queryByRole('option')).not.toBeNull());
    await new Promise((r) => setTimeout(r, 0));
    raf.pump(2);
    const rootLabel = selectedLabel();
    await fireEvent.keyDown(canvas, { key: 'ArrowDown' });
    // The first child can lie outside the cached window: the camera scrolls there, then the window loads.
    await waitFor(() => {
      raf.pump(5);
      expect(selectedLabel()).not.toBe(rootLabel);
    });
    await fireEvent.keyDown(canvas, { key: 'ArrowUp' });
    await waitFor(() => {
      raf.pump(5);
      expect(selectedLabel()).toBe(rootLabel);
    });
  });

  it('enter_recentres_and_tweens', async () => {
    let now = 1000;
    vi.spyOn(performance, 'now').mockImplementation(() => now);
    const s = await setup();
    const rootNode = (await nodesOf(s)).find((n) => n.ahnen === 1)!;
    const before = lastCamera(s.ctx)!;
    await fireEvent.keyDown(s.canvas, { key: 'Enter' });
    expect(s.onRecentre).toHaveBeenCalledWith(s.root);
    const target = { x: rootNode.x + rootNode.w / 2 - 400 / before.zoom, y: rootNode.y + rootNode.h / 2 - 300 / before.zoom };
    now = 1140;
    s.raf.pump(2);
    const mid = lastCamera(s.ctx)!;
    expect(mid.x).not.toBeCloseTo(before.x, 3);
    expect(mid.x).not.toBeCloseTo(target.x, 3);
    now = 1000 + 280;
    s.raf.pump(3);
    const end = lastCamera(s.ctx)!;
    expect(end.x).toBeCloseTo(target.x, 3);
    expect(end.y).toBeCloseTo(target.y, 3);
  });

  it('enter_with_reduced_motion_jumps', async () => {
    const mm = window.matchMedia;
    const s = await setup({ reducedMotion: true });
    const rootNode = (await nodesOf(s)).find((n) => n.ahnen === 1)!;
    const before = lastCamera(s.ctx)!;
    await fireEvent.keyDown(s.canvas, { key: 'Enter' });
    s.raf.pump(2);
    const end = lastCamera(s.ctx)!;
    expect(end.x).toBeCloseTo(rootNode.x + rootNode.w / 2 - 400 / before.zoom, 3);
    window.matchMedia = mm;
  });

  it('e_opens_the_editor', async () => {
    const s = await setup();
    await fireEvent.keyDown(s.canvas, { key: 'e' });
    expect(s.onOpenPerson).toHaveBeenCalledWith(s.root);
  });

  it('live_region_announces_selection', async () => {
    const s = await setup();
    await fireEvent.keyDown(s.canvas, { key: 'ArrowRight' });
    const text = screen.getByRole('status').textContent ?? '';
    expect(text).toBe(`${selectedLabel()}, 1 of 2 in this generation`);
  });

  it('activedescendant_label_matches', async () => {
    const s = await setup();
    const opt = screen.getByRole('option');
    expect(s.canvas.getAttribute('aria-activedescendant')).toBe(opt.id);
    expect(s.canvas.getAttribute('aria-label')).toBe('Pedigree chart');
    expect(s.canvas).toHaveAttribute('tabindex', '0');
  });

  it('dup_node_click_jumps_to_original', async () => {
    const { api, fx } = makeApi();
    const raf = fakeRaf();
    render(Host, {
      props: {
        api,
        view: ChartCanvas,
        props: { kind: 'pedigree', root: fx.specials.cycleA!, generations: 8, onRecentre: vi.fn(), onOpenPerson: vi.fn() },
      },
    });
    const canvas = screen.getByRole('application') as HTMLCanvasElement;
    const ctx = attachContext(canvas);
    canvas.getBoundingClientRect = () => ({ left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600, x: 0, y: 0, toJSON() {} });
    await waitFor(() => expect(screen.queryByRole('option')).not.toBeNull());
    await new Promise((r) => setTimeout(r, 0));
    raf.pump(2);
    const opened = await api.chartOpen({ kind: 'pedigree', root: fx.specials.cycleA!, generations: 8 });
    const nodes = await allNodes(api, opened.chartId);
    const dup = nodes.find((n) => n.dupOf !== null)!;
    expect(dup).toBeTruthy();
    const orig = nodes.find((n) => n.id === dup.dupOf)!;
    const cam = lastCamera(ctx)!;
    const x = (dup.x - cam.x + 20) * cam.zoom;
    const y = (dup.y - cam.y + 20) * cam.zoom;
    await fireEvent.pointerDown(canvas, { clientX: x, clientY: y, pointerId: 1 });
    await fireEvent.pointerUp(canvas, { clientX: x, clientY: y, pointerId: 1 });
    expect(selectedLabel()).toBe(orig.label);
  });

  it('descendants_couple_centred_with_children_row_visible', async () => {
    const s = await setup({ kind: 'descendants' });
    const cam = lastCamera(s.ctx)!;
    const id = (await s.openSpy.mock.results[0].value).chartId;
    const nodes = await allNodes(s.api as MockApi, id);
    const root = nodes.find((n) => n.id === 0) ?? nodes[0];
    const row = nodes.filter((n) => n.y === root.y);
    const cx = (Math.min(...row.map((n) => n.x)) + Math.max(...row.map((n) => n.x + n.w))) / 2;
    expect((cx - cam.x) * cam.zoom).toBeCloseTo(400, -1);
    const childY = Math.min(...nodes.filter((n) => n.y > root.y).map((n) => n.y), Infinity);
    if (Number.isFinite(childY)) expect((childY + 56 - cam.y) * cam.zoom).toBeLessThan(600);
    expect((root.y - cam.y) * cam.zoom).toBeGreaterThanOrEqual(0);
  });

  it('wide_descendants_initial_camera_has_focus_and_a_child_or_hint', async () => {
    const s = await setup({ kind: 'descendants' });
    const cam = lastCamera(s.ctx)!;
    const nodes = await allNodes(s.api as MockApi, (await s.openSpy.mock.results[0].value).chartId);
    const root = nodes.find((n) => n.id === 0) ?? nodes[0];
    const inView = (n: ChartNode) => {
      const x = (n.x - cam.x) * cam.zoom;
      const y = (n.y - cam.y) * cam.zoom;
      return x + n.w * cam.zoom > 0 && x < 800 && y + n.h * cam.zoom > 0 && y < 600;
    };
    expect(cam.zoom).toBeGreaterThanOrEqual(0.55 - 1e-9);
    expect(inView(root)).toBe(true);
    const childY = Math.min(...nodes.filter((n) => n.y > root.y).map((n) => n.y));
    const kids = nodes.filter((n) => n.y === childY);
    expect(kids.some(inView) || document.querySelector('.hint') !== null).toBe(true);
  });

  it('frames_after_the_observer_reports_a_size_following_open', async () => {
    const s = await setup();
    const cam = lastCamera(s.ctx)!;
    // The default 800x600 box is only known after open() resolved; the root must sit inside it.
    const nodes = await allNodes(s.api as MockApi, (await s.openSpy.mock.results[0].value).chartId);
    const r = nodes[0];
    const sx = (r.x - cam.x) * cam.zoom;
    expect(sx).toBeGreaterThanOrEqual(-1);
    expect(sx).toBeLessThan(800);
  });

  it('relatives_first_view_shows_the_person_with_parents_and_children_uncut', async () => {
    const { api, fx } = makeApi();
    // A person with both parents and children, so all three rows have to be in view.
    let root: string | null = null;
    for (const p of fx.persons.slice(0, 300)) {
      const m = await api.chartOpen({ kind: 'relatives', root: p.id, generations: 3 });
      const w = await api.chartWindow(m.chartId, BIG, 1);
      await api.chartClose(m.chartId);
      const hasParents = w.edges.some((e) => e.kind === 'parent' && e.to === m.rootNode);
      const hasKids = w.edges.some((e) => e.kind === 'parent' && e.from === m.rootNode);
      if (!hasParents || !hasKids) continue;
      // Pick one whose three rows fit a window of this size at a readable zoom.
      const ids = new Set<number>([m.rootNode]);
      for (const e of w.edges) {
        if (e.kind === 'parent' && (e.to === m.rootNode || e.from === m.rootNode)) {
          ids.add(e.from);
          ids.add(e.to);
        }
        if (e.kind === 'partner' && e.from === m.rootNode) ids.add(e.to);
      }
      const ns = w.nodes.filter((n) => ids.has(n.id));
      const width = Math.max(...ns.map((n) => n.x + n.w)) - Math.min(...ns.map((n) => n.x));
      if (width <= 1000) {
        root = p.id;
        break;
      }
    }
    expect(root).not.toBeNull();
    const s = await setup({ kind: 'relatives', root: root! });
    const win = await (s.api as MockApi).chartWindow((await s.openSpy.mock.results[0].value).chartId, BIG, 1);
    const rootId = (await s.openSpy.mock.results[0].value).rootNode;
    const byId = new Map(win.nodes.map((n) => [n.id, n]));
    const ids = new Set<number>([rootId]);
    for (const e of win.edges) {
      if (e.kind === 'parent' && (e.to === rootId || e.from === rootId)) {
        ids.add(e.from);
        ids.add(e.to);
      }
      if (e.kind === 'partner' && e.from === rootId) ids.add(e.to);
    }
    const cam = lastCamera(s.ctx)!;
    expect(cam.zoom).toBeGreaterThanOrEqual(0.55);
    expect(cam.zoom).toBeLessThanOrEqual(1);
    for (const id of ids) {
      const n = byId.get(id)!;
      expect((n.x - cam.x) * cam.zoom).toBeGreaterThanOrEqual(0);
      expect((n.y - cam.y) * cam.zoom).toBeGreaterThanOrEqual(0);
      expect((n.x + n.w - cam.x) * cam.zoom).toBeLessThanOrEqual(800);
      expect((n.y + n.h - cam.y) * cam.zoom).toBeLessThanOrEqual(600);
    }
  });

  it('resize_updates_backing_store_size', async () => {
    let cb: ResizeObserverCallback = () => {};
    vi.stubGlobal(
      'ResizeObserver',
      class {
        constructor(c: ResizeObserverCallback) {
          cb = c;
        }
        observe() {
          queueMicrotask(() => cb([{ contentRect: { width: 800, height: 600 } } as ResizeObserverEntry], {} as ResizeObserver));
        }
        disconnect() {}
        unobserve() {}
      },
    );
    Object.defineProperty(window, 'devicePixelRatio', { value: 2, configurable: true });
    const s = await setup();
    cb([{ contentRect: { width: 500, height: 300 } } as ResizeObserverEntry], {} as ResizeObserver);
    expect(s.canvas.width).toBe(1000);
    expect(s.canvas.height).toBe(600);
    Object.defineProperty(window, 'devicePixelRatio', { value: 1, configurable: true });
  });

  it('stale_error_triggers_reopen', async () => {
    const s = await setup();
    const n = s.openSpy.mock.calls.length;
    await (s.api as MockApi).apply({ type: 'createPerson', draft: { given: 'New', surnames: ['Person'], sex: 'unknown' } } as never);
    for (let i = 0; i < 12; i++) await fireEvent.wheel(s.canvas, { deltaY: 200, clientX: 400, clientY: 300 });
    await waitFor(() => expect(s.openSpy.mock.calls.length).toBeGreaterThan(n));
  });
});
