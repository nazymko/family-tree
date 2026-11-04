import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { makeFakeContext } from '../../test-setup';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import type { ChartKind, ChartNode } from '../api/types';
import { notices } from '../ui/notices.svelte';
import ChartCanvas from './ChartCanvas.svelte';
import WholeCanvas from './WholeCanvas.svelte';
import Host from './testing/Host.svelte';
import { allNodes, attachContext, fakeRaf, lastCamera, makeApi, richestRoot } from './testing/helpers';

const RECT = { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600, x: 0, y: 0, toJSON() {} };
const PNG_HEAD = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

/** Offscreen canvases the picture code makes: every one that is not in the page. */
function stubOffscreen() {
  const made: { canvas: HTMLCanvasElement; ctx: ReturnType<typeof makeFakeContext> }[] = [];
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(function (this: HTMLCanvasElement) {
    const ctx = makeFakeContext();
    made.push({ canvas: this, ctx });
    return ctx as unknown as CanvasRenderingContext2D;
  } as never);
  vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(function (cb: BlobCallback) {
    const bytes = new Uint8Array([...PNG_HEAD, 1, 2, 3]);
    // jsdom's Blob has no arrayBuffer().
    cb(Object.assign(new Blob([bytes], { type: 'image/png' }), { arrayBuffer: async () => bytes.buffer }));
  } as never);
  return made;
}

/** Boxes painted into a picture: the rectangles added to a path, minus the one clip rectangle. */
const boxes = (ctx: ReturnType<typeof makeFakeContext>) => ctx.calls.filter((c) => c.op === 'rect').length - 1;

async function setupChart(kind: ChartKind = 'pedigree') {
  const raf = fakeRaf();
  const { api, fx } = makeApi();
  const root = await richestRoot(api, fx, kind);
  const platform = new MockPlatform();
  const openSpy = vi.spyOn(api, 'chartOpen');
  const winSpy = vi.spyOn(api, 'chartWindow');
  const saveSpy = vi.spyOn(api, 'savePicture');
  render(Host, {
    props: { api, view: ChartCanvas, props: { kind, root, generations: 6, onRecentre: vi.fn(), onOpenPerson: vi.fn() }, platform },
  });
  const canvas = screen.getByRole('application') as HTMLCanvasElement;
  const ctx = attachContext(canvas);
  canvas.getBoundingClientRect = () => RECT;
  await waitFor(() => expect(openSpy).toHaveBeenCalled());
  await waitFor(() => expect(winSpy.mock.calls.length).toBeGreaterThanOrEqual(2));
  await new Promise((r) => setTimeout(r, 0));
  raf.pump(2);
  return { api, platform, raf, canvas, ctx, openSpy, winSpy, saveSpy, root };
}

const selectedLabel = () => screen.getByRole('option').textContent;
// The menu is named for the pictures alone, or for the person and the pictures when it starts with the add items.
const menu = () => screen.getByRole('menu', { name: /^(Save as picture|Person and picture options)$/ });
async function rightClick(el: Element, at = { clientX: 400, clientY: 300 }) {
  await fireEvent.contextMenu(el, { button: 2, ...at });
  // Opening on a person first reads that person's parents.
  await waitFor(() => expect(menu()).toBeTruthy());
}

beforeEach(() => {
  notices.dismiss();
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('picture menu on a chart', () => {
  it('right_click_on_empty_space_opens_a_menu_with_only_the_three_save_items', async () => {
    const s = await setupChart();
    expect(screen.queryByRole('menu')).toBeNull();
    await rightClick(s.canvas, { clientX: 5, clientY: 5 });
    const items = within(menu()).getAllByRole('menuitem');
    expect(items.map((i) => i.textContent?.trim())).toEqual([
      'Save the whole chart as a picture…',
      "Save the selected person's branch as a picture…",
      'Save what is on the screen as a picture…',
    ]);
    expect(items.every((i) => i.getAttribute('aria-disabled') !== 'true')).toBe(true);
  });

  it('right_click_on_a_box_selects_that_person_first', async () => {
    const s = await setupChart();
    const first = selectedLabel();
    const cam = lastCamera(s.ctx)!;
    const nodes: ChartNode[] = await allNodes(s.api as MockApi, (await s.openSpy.mock.results[0].value).chartId);
    // A box that is on screen and is not the selected one.
    const other = nodes.find((n) => n.label !== first && n.dupOf === null && n.x >= cam.x && (n.x + n.w - cam.x) * cam.zoom < 780 && (n.y - cam.y) * cam.zoom > 0 && (n.y + n.h - cam.y) * cam.zoom < 590)!;
    expect(other).toBeTruthy();
    await rightClick(s.canvas, { clientX: (other.x + other.w / 2 - cam.x) * cam.zoom, clientY: (other.y + other.h / 2 - cam.y) * cam.zoom });
    await waitFor(() => expect(selectedLabel()).toBe(other.label));
    expect(menu()).toBeTruthy();
  });

  it('the_toolbar_button_opens_the_same_menu', async () => {
    const s = await setupChart();
    const button = screen.getByRole('button', { name: 'Save as picture' });
    await fireEvent.click(button);
    expect(within(menu()).getAllByRole('menuitem')).toHaveLength(3);
    // The button also closes it.
    await fireEvent.mouseDown(button);
    await fireEvent.click(button);
    expect(screen.queryByRole('menu')).toBeNull();
    expect(s.canvas).toBeTruthy();
  });

  it('shift_f10_and_the_context_menu_key_open_it_over_the_chart_and_escape_gives_focus_back', async () => {
    const s = await setupChart();
    s.canvas.focus();
    await fireEvent.keyDown(s.canvas, { key: 'F10', shiftKey: true });
    await waitFor(() => expect(within(menu()).getAllByRole('menuitem').length).toBeGreaterThan(3));
    const m = menu();
    const items = within(m).getAllByRole('menuitem');
    const last = items.length - 1;
    expect(items[0]).toHaveFocus();
    await fireEvent.keyDown(m, { key: 'ArrowDown' });
    expect(items[1]).toHaveFocus();
    await fireEvent.keyDown(m, { key: 'End' });
    expect(items[last]).toHaveFocus();
    await fireEvent.keyDown(m, { key: 'ArrowDown' });
    expect(items[0]).toHaveFocus();
    await fireEvent.keyDown(m, { key: 'ArrowUp' });
    expect(items[last]).toHaveFocus();
    await fireEvent.keyDown(m, { key: 'Escape' });
    expect(screen.queryByRole('menu')).toBeNull();
    expect(s.canvas).toHaveFocus();

    await fireEvent.keyDown(s.canvas, { key: 'ContextMenu' });
    await waitFor(() => expect(menu()).toBeTruthy());
    await fireEvent.keyDown(menu(), { key: 'Escape' });
    expect(s.canvas).toHaveFocus();
  });

  it('a_right_click_does_not_pan_or_recentre', async () => {
    const s = await setupChart();
    const before = lastCamera(s.ctx);
    await fireEvent.pointerDown(s.canvas, { button: 2, pointerId: 1, clientX: 400, clientY: 300 });
    await fireEvent.pointerUp(s.canvas, { button: 2, pointerId: 1, clientX: 400, clientY: 300 });
    s.raf.pump(2);
    expect(lastCamera(s.ctx)).toEqual(before);
  });

  it('whole_chart_saves_every_box_at_100_percent_with_a_suggested_name', async () => {
    const made = stubOffscreen();
    const s = await setupChart();
    const m = await s.openSpy.mock.results[0].value;
    await rightClick(s.canvas);
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /whole chart/ }));
    await waitFor(() => expect(s.saveSpy, JSON.stringify(notices.current)).toHaveBeenCalled());
    const [dest, bytes] = s.saveSpy.mock.calls[0];
    expect(dest).toMatch(/^C:\/Mock\/Pedigree - .+\.png$/);
    expect([...bytes.slice(0, 8)]).toEqual(PNG_HEAD);
    const shot = made.find((x) => !x.canvas.isConnected)!;
    expect(boxes(shot.ctx)).toBe(m.nodeCount);
    const tr = shot.ctx.calls.filter((c) => c.op === 'setTransform').map((c) => c.args as number[]);
    // The world is drawn at 100% zoom with twice the pixels (a sharp picture), starting 40 px before the first box.
    const all = await allNodes(s.api as MockApi, m.chartId);
    const x0 = Math.min(...all.map((n) => n.x));
    const y0 = Math.min(...all.map((n) => n.y));
    expect(tr.some((a) => a[0] === 2 && a[3] === 2 && Math.abs(a[4] + (x0 - 40) * 2) < 1e-6 && Math.abs(a[5] + (y0 - 40) * 2) < 1e-6)).toBe(true);
    expect(shot.ctx.calls.some((c) => c.op === 'fillText' && /Pedigree/.test(String(c.args[0])))).toBe(true); // the caption
    await waitFor(() => expect(notices.current?.message).toBe('Picture saved'));
    expect((s.platform.calls.find((c) => c.method === 'pickSavePath')!.args[0] as { extensions: string[] }).extensions).toEqual(['png', 'svg']);
  });

  it('the_branch_item_saves_the_selected_person_and_the_people_behind_them_only', async () => {
    const made = stubOffscreen();
    const s = await setupChart();
    const m = await s.openSpy.mock.results[0].value;
    // One step towards the ancestors, then save that person's branch.
    await fireEvent.keyDown(s.canvas, { key: 'ArrowRight' });
    await rightClick(s.canvas, { clientX: 5, clientY: 5 });
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /branch/ }));
    await waitFor(() => expect(s.saveSpy, JSON.stringify(notices.current)).toHaveBeenCalled());
    const shot = made.find((x) => !x.canvas.isConnected)!;
    expect(boxes(shot.ctx)).toBeGreaterThan(0);
    expect(boxes(shot.ctx)).toBeLessThan(m.nodeCount);
    expect(s.saveSpy.mock.calls[0][0]).toMatch(/ \(branch\)\.png$/);
  });

  it('the_screen_item_saves_exactly_the_viewport_at_the_current_zoom', async () => {
    const made = stubOffscreen();
    const s = await setupChart();
    const cam = lastCamera(s.ctx)!;
    await rightClick(s.canvas);
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /on the screen/ }));
    await waitFor(() => expect(s.saveSpy, JSON.stringify(notices.current)).toHaveBeenCalled());
    const shot = made.find((x) => !x.canvas.isConnected)!;
    expect(shot.canvas.width).toBe(800);
    expect(shot.canvas.height).toBe(600 + 30);
    const tr = shot.ctx.calls.filter((c) => c.op === 'setTransform').map((c) => c.args as number[]);
    const last = tr.find((a) => Math.abs(a[0] - cam.zoom) < 1e-9)!; // the world transform
    expect(last).toBeTruthy();
    expect(last[0]).toBeCloseTo(cam.zoom);
    expect(last[4]).toBeCloseTo(-cam.x * cam.zoom);
    expect(last[5]).toBeCloseTo(-cam.y * cam.zoom);
    expect(s.saveSpy.mock.calls[0][0]).toMatch(/ \(screen\)\.png$/);
  });

  it('the_screen_item_fetches_a_fresh_window_for_the_viewport_instead_of_trusting_the_cache', async () => {
    const made = stubOffscreen();
    const s = await setupChart();
    const m = await s.openSpy.mock.results[0].value;
    const wins = await Promise.all(s.winSpy.mock.results.map((r) => r.value));
    const cached = wins.reduce((a, b) => (b.nodes.length > a.nodes.length ? b : a));
    const cam = lastCamera(s.ctx)!;
    const fresh = { ...cached, nodes: cached.nodes.filter((n: ChartNode) => n.dupOf === null).slice(0, 2), edges: [] };
    expect(cached.nodes.length).toBeGreaterThan(3);
    s.winSpy.mockClear();
    s.winSpy.mockResolvedValue(fresh);
    await rightClick(s.canvas);
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /on the screen/ }));
    await waitFor(() => expect(s.saveSpy, JSON.stringify(notices.current)).toHaveBeenCalled());
    expect(s.winSpy).toHaveBeenCalledTimes(1);
    const [id, rect, zoom] = s.winSpy.mock.calls[0];
    expect(id).toBe(m.chartId);
    expect(zoom).toBeCloseTo(cam.zoom);
    expect(rect.x).toBeCloseTo(cam.x);
    expect(rect.y).toBeCloseTo(cam.y);
    expect(rect.w).toBeCloseTo(800 / cam.zoom);
    expect(rect.h).toBeCloseTo(600 / cam.zoom);
    // What was painted is the freshly fetched window (2 boxes, plus the selection frame), not the cached one.
    const shot = made.find((x) => !x.canvas.isConnected)!;
    expect(boxes(shot.ctx)).toBeGreaterThanOrEqual(2);
    expect(boxes(shot.ctx)).toBeLessThanOrEqual(3);
  });

  it('an_svg_name_saves_a_vector_picture_without_making_a_canvas_of_the_chart_size', async () => {
    stubOffscreen();
    const s = await setupChart();
    s.platform.next.save = 'C:/Mock/Pedigree.svg';
    await rightClick(s.canvas);
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /whole chart/ }));
    await waitFor(() => expect(s.saveSpy, JSON.stringify(notices.current)).toHaveBeenCalled());
    const [dest, bytes] = s.saveSpy.mock.calls[0];
    expect(dest).toBe('C:/Mock/Pedigree.svg');
    const text = new TextDecoder().decode(bytes);
    expect(text).toMatch(/^<\?xml/);
    expect(text).toContain('<path d="M');
    expect(text).toMatch(/<text [^>]*>Pedigree/);
    expect(HTMLCanvasElement.prototype.toBlob).not.toHaveBeenCalled(); // no PNG was made
    await waitFor(() => expect(notices.current?.message).toBe('Picture saved'));
  });

  it('a_context_menu_event_without_a_pointer_type_and_with_no_position_counts_as_the_keyboard', async () => {
    const s = await setupChart();
    s.canvas.focus();
    const ev = new MouseEvent('contextmenu', { bubbles: true, cancelable: true, button: 0, detail: 0, clientX: 0, clientY: 0 });
    s.canvas.dispatchEvent(ev);
    await waitFor(() => expect(menu()).toBeTruthy());
    expect(ev.defaultPrevented).toBe(true);
    expect(within(menu()).getAllByRole('menuitem')[0]).toHaveFocus();
  });

  it('shows_saving_while_it_works_and_a_plain_error_when_it_fails', async () => {
    stubOffscreen();
    const s = await setupChart();
    let fail!: (e: unknown) => void;
    s.saveSpy.mockImplementation(() => new Promise((_, reject) => (fail = reject)));
    await rightClick(s.canvas);
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /whole chart/ }));
    expect(await screen.findByText('Saving the picture…')).toBeInTheDocument();
    await waitFor(() => expect(fail).toBeTypeOf('function'));
    // The button is off while it works.
    expect(screen.getByRole('button', { name: 'Save as picture' })).toBeDisabled();
    fail({ code: 'io', message: 'Could not save the picture: the disk is full. Choose another place and try again.' });
    await waitFor(() => expect(screen.queryByText('Saving the picture…')).toBeNull());
    expect(notices.current?.message).toBe('Could not save the picture: the disk is full. Choose another place and try again.');
    expect(screen.getByRole('button', { name: 'Save as picture' })).toBeEnabled();
  });

  it('cancelling_the_save_dialog_writes_nothing', async () => {
    stubOffscreen();
    const s = await setupChart();
    s.platform.next.save = null;
    await rightClick(s.canvas);
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /whole chart/ }));
    await new Promise((r) => setTimeout(r, 20));
    expect(s.saveSpy).not.toHaveBeenCalled();
    expect(notices.current).toBeNull();
  });
});

describe('picture menu on the whole tree', () => {
  async function setupWhole() {
    const mm = window.matchMedia;
    window.matchMedia = ((q: string) => ({ ...mm(q), matches: q.includes('reduce') })) as typeof window.matchMedia;
    const raf = fakeRaf();
    const { api } = makeApi(300);
    const platform = new MockPlatform();
    const winSpy = vi.spyOn(api, 'chartWindow');
    const saveSpy = vi.spyOn(api, 'savePicture');
    render(Host, { props: { api, view: WholeCanvas, props: { homeId: null, onOpenPerson: vi.fn(), onOpenInPedigree: vi.fn() }, platform } });
    const canvas = screen.getByRole('application', { name: 'Whole tree chart' }) as HTMLCanvasElement;
    const ctx = attachContext(canvas);
    canvas.getBoundingClientRect = () => RECT;
    const mini = (await screen.findByRole('application', { name: /Minimap/ })) as HTMLCanvasElement;
    mini.getBoundingClientRect = () => ({ ...RECT, width: 220, height: mini.height, right: 220, bottom: mini.height });
    attachContext(mini);
    await waitFor(() => expect(winSpy.mock.calls.length).toBeGreaterThanOrEqual(2));
    await new Promise((r) => setTimeout(r, 0));
    raf.pump(2);
    return { api, platform, canvas, ctx, saveSpy, winSpy };
  }

  it('the_branch_item_is_disabled_with_a_hint_when_nothing_is_selected', async () => {
    const s = await setupWhole();
    // The tree opens on a person; Escape clears the selection.
    await fireEvent.keyDown(s.canvas, { key: 'Escape' });
    await rightClick(s.canvas, { clientX: 5, clientY: 5 });
    const branch = within(menu()).getByRole('menuitem', { name: /branch/ });
    expect(branch).toHaveAttribute('aria-disabled', 'true');
    expect(within(menu()).getByText('Select a person first')).toBeInTheDocument();
    await fireEvent.click(branch);
    expect(menu()).toBeTruthy(); // a disabled item does not close the menu or save
    expect(s.saveSpy).not.toHaveBeenCalled();
  });

  it('a_branch_picture_closes_the_empty_stretches_between_its_people', async () => {
    const made = stubOffscreen();
    const s = await setupWhole();
    await rightClick(s.canvas, { clientX: 5, clientY: 5 });
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /branch/ }));
    await waitFor(() => expect(s.saveSpy, JSON.stringify(notices.current)).toHaveBeenCalledTimes(1));
    const shot = made.filter((x) => !x.canvas.isConnected).at(-1)!;
    // Every rectangle but the first (the clip of the chart area) is a person; no two neighbours are further apart than 40.
    const spans = shot.ctx.calls
      .filter((c) => c.op === 'rect')
      .slice(1)
      .map((c) => [c.args[0] as number, (c.args[0] as number) + (c.args[2] as number)] as [number, number])
      .sort((a, b) => a[0] - b[0]);
    expect(spans.length).toBeGreaterThan(0);
    let reach = spans[0][1];
    for (const [lo, hi] of spans) {
      expect(lo - reach).toBeLessThanOrEqual(40 + 1e-6);
      reach = Math.max(reach, hi);
    }
  });

  it('saves_the_whole_tree_with_its_headings_and_the_branch_without_them', async () => {
    const made = stubOffscreen();
    const s = await setupWhole();
    const people = (await s.winSpy.mock.results[0].value).nodes.length;
    expect(people).toBeGreaterThan(0);
    await rightClick(s.canvas, { clientX: 5, clientY: 5 });
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /whole chart/ }));
    await waitFor(() => expect(s.saveSpy).toHaveBeenCalledTimes(1));
    expect(s.saveSpy.mock.calls[0][0]).toBe('C:/Mock/Whole tree.png');
    const whole = made.filter((x) => !x.canvas.isConnected).at(-1)!;
    expect(whole.ctx.calls.some((c) => c.op === 'fillText' && /family \(\d+ people\)|Unconnected people/.test(String(c.args[0])))).toBe(true);
    const wholeBoxes = boxes(whole.ctx);

    await rightClick(s.canvas, { clientX: 5, clientY: 5 });
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /branch/ }));
    await waitFor(() => expect(s.saveSpy).toHaveBeenCalledTimes(2));
    expect(s.saveSpy.mock.calls[1][0]).toMatch(/^C:\/Mock\/Whole tree - .+ \(branch\)\.png$/);
    const branch = made.filter((x) => !x.canvas.isConnected).at(-1)!;
    expect(boxes(branch.ctx)).toBeGreaterThan(0);
    expect(boxes(branch.ctx)).toBeLessThan(wholeBoxes);
    expect(branch.ctx.calls.some((c) => c.op === 'fillText' && /family \(\d+ people\)/.test(String(c.args[0])))).toBe(false);
  });
});
