import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import type { MockApi } from '../api/mock/MockApi';
import type { ChartWindow } from '../api/types';
import { agentChanges } from '../state/agentChanges.svelte';
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
  mini.getBoundingClientRect = () => ({ ...RECT, width: 220, height: mini.height, right: 220, bottom: mini.height });
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

  it('selection_says_when_an_agent_changed_the_person', async () => {
    const s = await setup();
    const target = s.fx.persons[7];
    const ref = (await s.api.getPerson(target.id)).ref;
    agentChanges.unreviewed = new Set([target.id]);
    try {
      const box = screen.getByRole('combobox', { name: 'Find a person…' });
      await fireEvent.input(box, { target: { value: ref.name } });
      const option = await screen.findByRole('option', { name: new RegExp(ref.name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')) }, { timeout: 2000 });
      await fireEvent.click(option);
      await settle(s);
      await waitFor(() => expect(screen.getByRole('listbox', { name: 'Selected person' })).toHaveTextContent('Changed by an agent'));
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Changed by an agent'));
    } finally {
      agentChanges.reset();
    }
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
    await fireEvent(s.mini, new MouseEvent('pointerdown', { clientX: 150, clientY: s.mini.height / 2, bubbles: true }));
    await fireEvent(s.mini, new MouseEvent('pointerup', { bubbles: true }));
    await settle(s);
    const after = lastCamera(s.ctx)!;
    expect(after.zoom).toBeCloseTo(before.zoom, 5);
    expect(after.x !== before.x || after.y !== before.y).toBe(true);
    const b = (await s.openSpy.mock.results[0].value).meta.bounds;
    const scale = Math.min((220 - 8) / b.w, (s.mini.height - 8) / b.h);
    const ox = (220 - b.w * scale) / 2;
    const oy = (s.mini.height - b.h * scale) / 2;
    expect(after.x + 400 / after.zoom).toBeCloseTo(Math.min(b.w, Math.max(0, (150 - ox) / scale)), 0);
    expect(after.y + 300 / after.zoom).toBeCloseTo(Math.min(b.h, Math.max(0, (s.mini.height / 2 - oy) / scale)), 0);
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

  it('far_out_draws_bars_with_surnames_and_near_draws_full_names', async () => {
    const s = await setup();
    for (let i = 0; i < 30; i++) await fireEvent.wheel(s.canvas, { deltaY: 400, clientX: 400, clientY: 300 });
    await settle(s, 6);
    expect(lastCamera(s.ctx)!.zoom).toBeLessThan(0.25);
    const far = s.winSpy.mock.calls.filter((c) => (c[2] as number) < 0.25).at(-1);
    expect(far).toBeTruthy();
    const out: ChartWindow = await s.winSpy.mock.results[s.winSpy.mock.calls.indexOf(far!)].value;
    expect(out.nodes.length).toBeGreaterThan(0);
    expect(out.nodes.every((n) => n.label === '' && n.person!.lifespan === '')).toBe(true);
    const surnames = new Set(out.nodes.map((n) => n.person!.name));
    // A fresh frame at the far zoom: coloured bars, no person names or dates, only surnames and headings.
    s.ctx.calls.length = 0;
    await fireEvent.wheel(s.canvas, { deltaY: 10, clientX: 400, clientY: 300 });
    s.raf.pump(3);
    const farTexts = s.ctx.calls.filter((c) => c.op === 'fillText').map((c) => String(c.args[0]));
    expect(s.ctx.calls.filter((c) => c.op === 'fill').length).toBeGreaterThan(0);
    for (const t of farTexts) expect(surnames.has(t) || /family \(\d+ people\)|Unconnected people/.test(t)).toBe(true);

    for (let i = 0; i < 40; i++) await fireEvent.wheel(s.canvas, { deltaY: -400, clientX: 400, clientY: 300 });
    await settle(s, 6);
    const near = s.winSpy.mock.calls.filter((c) => (c[2] as number) >= 0.55).at(-1);
    expect(near).toBeTruthy();
    const full: ChartWindow = await s.winSpy.mock.results[s.winSpy.mock.calls.indexOf(near!)].value;
    const names = new Set(full.nodes.map((n) => n.person!.name));
    s.ctx.calls.length = 0;
    await fireEvent.wheel(s.canvas, { deltaY: -10, clientX: 400, clientY: 300 });
    s.raf.pump(3);
    const nearTexts = s.ctx.calls.filter((c) => c.op === 'fillText').map((c) => String(c.args[0]));
    expect(nearTexts.some((t) => names.has(t) || [...names].some((n) => n.startsWith(t.replace('…', ''))))).toBe(true);
  });

  async function pickPerson(s: Setup, index: number) {
    const ref = (await s.api.getPerson(s.fx.persons[index].id)).ref;
    const box = screen.getByRole('combobox', { name: 'Find a person…' });
    await fireEvent.input(box, { target: { value: ref.name } });
    const list = await screen.findByRole('listbox', { name: 'Find a person…' });
    const opts = await within(list).findAllByRole('option', undefined, { timeout: 2000 });
    await fireEvent.click(opts.find((o) => o.textContent?.includes(ref.name))!);
    await settle(s);
    return ref;
  }

  it('highlight_is_on_by_default_asks_for_the_line_and_the_toggle_turns_it_off', async () => {
    const s = await setup();
    const spy = vi.spyOn(s.api, 'chartLineage');
    await pickPerson(s, 5);
    const toggle = screen.getByRole('button', { name: 'Highlight family line' });
    expect(toggle).toHaveAttribute('aria-pressed', 'true');
    await waitFor(() => expect(spy).toHaveBeenCalled());
    spy.mockClear();
    await fireEvent.click(toggle);
    expect(toggle).toHaveAttribute('aria-pressed', 'false');
    await settle(s);
    expect(spy).not.toHaveBeenCalled();
  });

  it('arrows_pan_when_nothing_is_selected_and_shift_arrows_pan_when_something_is', async () => {
    const s = await setup();
    for (let i = 0; i < 6; i++) await fireEvent.wheel(s.canvas, { deltaY: -300, clientX: 400, clientY: 300 });
    s.raf.pump(2);
    // The first view selects a person; Escape clears it so the arrows pan.
    await fireEvent.keyDown(s.canvas, { key: 'Escape' });
    const c0 = lastCamera(s.ctx)!;
    await fireEvent.keyDown(s.canvas, { key: 'ArrowLeft' });
    s.raf.pump(2);
    const c1 = lastCamera(s.ctx)!;
    expect(c1.x).toBeLessThan(c0.x);
    await fireEvent.keyDown(s.canvas, { key: 'ArrowDown' });
    s.raf.pump(2);
    expect(lastCamera(s.ctx)!.y).toBeGreaterThanOrEqual(c1.y);
    // With a person selected, Shift+arrow still pans and does not change the selection.
    await pickPerson(s, 4);
    const sel = screen.getByRole('listbox', { name: 'Selected person' }).textContent;
    expect(sel).toBeTruthy();
    const c2 = lastCamera(s.ctx)!;
    await fireEvent.keyDown(s.canvas, { key: 'ArrowRight', shiftKey: true });
    s.raf.pump(2);
    expect(lastCamera(s.ctx)!.x).toBeGreaterThan(c2.x);
    expect(screen.getByRole('listbox', { name: 'Selected person' }).textContent).toBe(sel);
  });

  it('arrows_move_the_selection_when_a_person_is_selected_and_escape_clears_it', async () => {
    const s = await setup();
    // A person with a parent in the tree: the fixture's later persons are children.
    const id = (await s.openSpy.mock.results[0].value).meta.chartId;
    const all = (await s.api.chartWindow(id, { x: 0, y: 0, w: 1e7, h: 1e7 }, 1)).nodes;
    const child = all.find((n) => n.nav.parent !== null && n.person)!;
    await pickPerson(s, s.fx.persons.findIndex((p) => p.id === child.person!.id));
    const before = screen.getByRole('listbox', { name: 'Selected person' }).textContent;
    const cam = lastCamera(s.ctx)!;
    await fireEvent.keyDown(s.canvas, { key: 'ArrowUp' });
    await settle(s);
    const after = screen.getByRole('listbox', { name: 'Selected person' }).textContent;
    expect(after).not.toBe(before);
    expect(lastCamera(s.ctx)!.zoom).toBeCloseTo(cam.zoom, 5);
    await fireEvent.keyDown(s.canvas, { key: 'Escape' });
    expect(screen.getByRole('listbox', { name: 'Selected person' }).textContent).toBe('');
    expect(screen.getByRole('status')).toHaveTextContent('Selection cleared.');
  });

  it('the_keyboard_help_names_panning_and_escape', async () => {
    await setup();
    const help = document.querySelector('.sr-only[id$="-help"]')!;
    expect(help.textContent).toMatch(/Arrow keys move the view/);
    expect(help.textContent).toMatch(/Shift/);
    expect(help.textContent).toMatch(/Escape/);
  });

  async function nodesInView(s: Setup): Promise<number> {
    const id = (await s.openSpy.mock.results[0].value).meta.chartId;
    const cam = lastCamera(s.ctx)!;
    const win = await s.api.chartWindow(id, { x: cam.x, y: cam.y, w: 800 / cam.zoom, h: 600 / cam.zoom }, 1);
    return win.nodes.length;
  }

  it('first_view_shows_people_without_a_home_person', async () => {
    const s = await setup();
    expect(lastCamera(s.ctx)!.zoom).toBeGreaterThanOrEqual(0.6);
    expect(await nodesInView(s)).toBeGreaterThan(0);
    expect(screen.getByRole('listbox', { name: 'Selected person' }).textContent).not.toBe('');
  });

  it('first_view_shows_the_home_person', async () => {
    const s = await setup({ home: true });
    expect(lastCamera(s.ctx)!.zoom).toBeGreaterThanOrEqual(0.6);
    expect(await nodesInView(s)).toBeGreaterThan(0);
    const ref = (await s.api.getPerson(s.fx.persons[0].id)).ref;
    await waitFor(() => expect(screen.getByRole('listbox', { name: 'Selected person' })).toHaveTextContent(ref.name));
  });

  it('fit_all_never_goes_below_a_readable_zoom', async () => {
    const s = await setup();
    await fireEvent.keyDown(s.canvas, { key: 'f' });
    await settle(s);
    expect(lastCamera(s.ctx)!.zoom).toBeGreaterThanOrEqual(0.06 - 1e-9);
  });
});
