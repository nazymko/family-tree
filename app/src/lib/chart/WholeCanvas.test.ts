import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import type { MockApi } from '../api/mock/MockApi';
import type { ChartWindow } from '../api/types';
import WholeCanvas from './WholeCanvas.svelte';
import Host from './testing/Host.svelte';
import { attachContext, fakeRaf, lastCamera, makeApi } from './testing/helpers';

const RECT = { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600, x: 0, y: 0, toJSON() {} };

async function setup(opts: { home?: boolean } = {}) {
  const mm = window.matchMedia;
  // Reduced motion: camera moves land at once, so the tests do not wait for tweens.
  window.matchMedia = ((q: string) => ({ ...mm(q), matches: q.includes('reduce') })) as typeof window.matchMedia;
  const raf = fakeRaf();
  const { api, fx } = makeApi(300);
  const winSpy = vi.spyOn(api, 'chartWindow');
  const openSpy = vi.spyOn(api, 'chartOpenWhole');
  const onOpenPerson = vi.fn();
  const onOpenInPedigree = vi.fn();
  render(Host, {
    props: { api, view: WholeCanvas, props: { homeId: opts.home ? fx.persons[0].id : null, onOpenPerson, onOpenInPedigree } },
  });
  const canvas = screen.getByRole('application', { name: 'Whole tree chart' }) as HTMLCanvasElement;
  const ctx = attachContext(canvas);
  canvas.getBoundingClientRect = () => RECT;
  const mini = (await screen.findByRole('application', { name: /Minimap/ })) as HTMLCanvasElement;
  mini.getBoundingClientRect = () => ({ ...RECT, width: 180, height: 120, right: 180, bottom: 120 });
  attachContext(mini);
  await waitFor(() => expect(openSpy).toHaveBeenCalled());
  await waitFor(() => expect(winSpy.mock.calls.length).toBeGreaterThanOrEqual(2));
  await new Promise((r) => setTimeout(r, 0));
  raf.pump(2);
  return { api: api as MockApi, fx, raf, canvas, mini, ctx, winSpy, openSpy, onOpenPerson, onOpenInPedigree };
}

type Setup = Awaited<ReturnType<typeof setup>>;

async function settle(s: Setup, rounds = 4): Promise<void> {
  for (let i = 0; i < rounds; i++) {
    await new Promise((r) => setTimeout(r, 0));
    s.raf.pump(2);
  }
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('WholeCanvas', () => {
  it('opens_the_whole_tree_and_draws_block_headings', async () => {
    const s = await setup();
    expect(s.openSpy).toHaveBeenCalledTimes(1);
    const texts = s.ctx.calls.filter((c) => c.op === 'fillText').map((c) => String(c.args[0]));
    expect(texts.some((t) => /family \(\d+ people\)|Unconnected people \(\d+\)/.test(t))).toBe(true);
  });

  it('search_moves_the_camera_to_the_person_and_selects_it', async () => {
    const s = await setup();
    const target = s.fx.persons[7];
    const ref = (await s.api.getPerson(target.id)).ref;
    const before = lastCamera(s.ctx);
    const box = screen.getByRole('combobox', { name: 'Find a person…' });
    await fireEvent.input(box, { target: { value: ref.name } });
    const option = await screen.findByRole('option', { name: new RegExp(ref.name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')) }, { timeout: 2000 });
    await fireEvent.click(option);
    await settle(s);
    await waitFor(() => expect(screen.getByRole('listbox', { name: 'Selected person' })).toHaveTextContent(ref.name));
    const loc = (await s.api.chartLocate((await s.openSpy.mock.results[0].value).meta.chartId, ref.id))!;
    const cam = lastCamera(s.ctx)!;
    expect(cam.zoom).toBeGreaterThanOrEqual(0.8);
    // The person is in the middle of the 800x600 view.
    expect(cam.x + 400 / cam.zoom).toBeCloseTo(loc.rect.x + loc.rect.w / 2, 0);
    expect(cam.y + 300 / cam.zoom).toBeCloseTo(loc.rect.y + loc.rect.h / 2, 0);
    expect(before).not.toEqual(cam);
  });

  it('minimap_click_moves_the_view', async () => {
    const s = await setup();
    // Zoom in first so the view is smaller than the chart and moving it is visible.
    for (let i = 0; i < 6; i++) await fireEvent.wheel(s.canvas, { deltaY: -300, clientX: 400, clientY: 300 });
    await settle(s);
    const before = lastCamera(s.ctx)!;
    // jsdom's PointerEvent drops the coordinates, so send a mouse event under the pointer event's name.
    await fireEvent(s.mini, new MouseEvent('pointerdown', { clientX: 150, clientY: 100, bubbles: true }));
    await fireEvent(s.mini, new MouseEvent('pointerup', { bubbles: true }));
    await settle(s);
    const after = lastCamera(s.ctx)!;
    expect(after.zoom).toBeCloseTo(before.zoom, 5);
    expect(after.x !== before.x || after.y !== before.y).toBe(true);
    const b = (await s.openSpy.mock.results[0].value).meta.bounds;
    const scale = Math.min(180 / b.w, 120 / b.h);
    expect(after.x + 400 / after.zoom).toBeCloseTo(Math.min(b.w, 150 / scale), 0);
    expect(after.y + 300 / after.zoom).toBeCloseTo(Math.min(b.h, 100 / scale), 0);
  });

  it('double_click_and_enter_open_the_person', async () => {
    const s = await setup();
    const id = (await s.openSpy.mock.results[0].value).meta.chartId;
    const ref = (await s.api.getPerson(s.fx.persons[3].id)).ref;
    const loc = (await s.api.chartLocate(id, ref.id))!;
    const box = screen.getByRole('combobox', { name: 'Find a person…' });
    await fireEvent.input(box, { target: { value: ref.name } });
    await fireEvent.click(await screen.findByRole('option', { name: new RegExp(ref.name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')) }, { timeout: 2000 }));
    await settle(s);
    expect(loc).toBeTruthy();
    await fireEvent.dblClick(s.canvas, { clientX: 400, clientY: 300 });
    expect(s.onOpenPerson).toHaveBeenCalledWith(ref.id);
    s.onOpenPerson.mockClear();
    await fireEvent.keyDown(s.canvas, { key: 'Enter' });
    await waitFor(() => expect(s.onOpenPerson).toHaveBeenCalledWith(ref.id));
    await fireEvent.keyDown(s.canvas, { key: 'p' });
    expect(s.onOpenInPedigree).toHaveBeenCalledWith(ref.id);
  });

  it('far_out_asks_for_surnames_only_and_near_asks_for_full_cards', async () => {
    const s = await setup();
    for (let i = 0; i < 30; i++) await fireEvent.wheel(s.canvas, { deltaY: 400, clientX: 400, clientY: 300 });
    await settle(s, 6);
    const far = s.winSpy.mock.calls.filter((c) => (c[2] as number) < 0.25).at(-1);
    expect(far).toBeTruthy();
    const out: ChartWindow = await s.winSpy.mock.results[s.winSpy.mock.calls.indexOf(far!)].value;
    expect(out.nodes.length).toBeGreaterThan(0);
    expect(out.nodes.every((n) => n.label === '' && n.person!.lifespan === '')).toBe(true);
    const chartLod = lastCamera(s.ctx)!.zoom;
    expect(chartLod).toBeLessThan(0.25);
    for (let i = 0; i < 40; i++) await fireEvent.wheel(s.canvas, { deltaY: -400, clientX: 400, clientY: 300 });
    await settle(s, 6);
    const near = s.winSpy.mock.calls.filter((c) => (c[2] as number) >= 0.55).at(-1);
    expect(near).toBeTruthy();
    expect(s.ctx.calls.filter((c) => c.op === 'fillText').length).toBeGreaterThan(0);
  });

  it('highlight_toggle_asks_for_the_family_line_of_the_selection', async () => {
    const s = await setup();
    const spy = vi.spyOn(s.api, 'chartLineage');
    const ref = (await s.api.getPerson(s.fx.persons[5].id)).ref;
    const box = screen.getByRole('combobox', { name: 'Find a person…' });
    await fireEvent.input(box, { target: { value: ref.name } });
    await fireEvent.click(await screen.findByRole('option', { name: new RegExp(ref.name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')) }, { timeout: 2000 }));
    await settle(s);
    const toggle = screen.getByRole('button', { name: 'Highlight family line' });
    expect(toggle).toHaveAttribute('aria-pressed', 'false');
    await fireEvent.click(toggle);
    expect(toggle).toHaveAttribute('aria-pressed', 'true');
    await waitFor(() => expect(spy).toHaveBeenCalled());
  });
});
