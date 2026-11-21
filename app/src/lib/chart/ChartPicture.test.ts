import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { makeFakeContext } from '../../test-setup';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import type { ChartKind, ChartNode } from '../api/types';
import { setLanguage } from '../i18n';
import type { PrintLayout } from './sheets';
import { bigAround, capturePrints, servePrintCharts } from './testing/syntheticAround';
import { notices } from '../ui/notices.svelte';
import ChartCanvas from './ChartCanvas.svelte';
import WholeCanvas from './WholeCanvas.svelte';
import Host from './testing/Host.svelte';
import type { ChartDirection } from './direction';
import { A3_BACKGROUND_KEY, A3_DIRECTION_KEY, A3_TITLE_KEY, a3Background, a3Direction, a3Title } from './a3Pref.svelte';
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

async function setupChart(kind: ChartKind = 'pedigree', direction?: ChartDirection) {
  const raf = fakeRaf();
  const { api, fx } = makeApi();
  const root = await richestRoot(api, fx, kind);
  const platform = new MockPlatform();
  const openSpy = vi.spyOn(api, 'chartOpen');
  const winSpy = vi.spyOn(api, 'chartWindow');
  const saveSpy = vi.spyOn(api, 'savePicture');
  render(Host, {
    props: { api, view: ChartCanvas, props: { kind, root, generations: 6, onRecentre: vi.fn(), onOpenPerson: vi.fn(), ...(direction ? { direction } : {}) }, platform },
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

/** An A3 save lays the chart out up to four times; under a loaded machine that takes longer than waitFor's 1 s. */
const A3_WAIT = { timeout: 15000 };

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
  // Every case starts with the default A3 direction (as on screen).
  localStorage.removeItem(A3_DIRECTION_KEY);
  a3Direction.reload();
  localStorage.removeItem(A3_TITLE_KEY);
  localStorage.removeItem(A3_BACKGROUND_KEY);
  a3Title.reload();
  a3Background.reload();
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('picture menu on a chart', () => {
  it('right_click_on_empty_space_opens_a_menu_with_only_the_save_items', async () => {
    const s = await setupChart();
    expect(screen.queryByRole('menu')).toBeNull();
    await rightClick(s.canvas, { clientX: 5, clientY: 5 });
    const items = within(menu()).getAllByRole('menuitem');
    expect(items.map((i) => i.textContent?.trim())).toEqual([
      'Save the whole chart as a picture…',
      "Save the selected person's branch as a picture…",
      'Save what is on the screen as a picture…',
      'Save for A3 print…',
      'Save for A3 print, landscape…',
      'Save for A3 print, portrait…',
      'Print on A3…',
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
    expect(within(menu()).getAllByRole('menuitem')).toHaveLength(7);
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
    // The last stop is the "names as recorded" choice, a checkable item.
    const choices = within(m).getAllByRole('menuitemradio');
    const end = choices[choices.length - 1];
    expect(items[0]).toHaveFocus();
    await fireEvent.keyDown(m, { key: 'ArrowDown' });
    expect(items[1]).toHaveFocus();
    await fireEvent.keyDown(m, { key: 'End' });
    expect(end).toHaveFocus();
    await fireEvent.keyDown(m, { key: 'ArrowDown' });
    expect(items[0]).toHaveFocus();
    await fireEvent.keyDown(m, { key: 'ArrowUp' });
    expect(end).toHaveFocus();
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
    // The world is drawn at 100% zoom with three times the pixels (a sharp picture), starting 40 px before the first box.
    const all = await allNodes(s.api as MockApi, m.chartId);
    const x0 = Math.min(...all.map((n) => n.x));
    const y0 = Math.min(...all.map((n) => n.y));
    expect(tr.some((a) => a[0] === 3 && a[3] === 3 && Math.abs(a[4] + (x0 - 40) * 3) < 1e-6 && Math.abs(a[5] + (y0 - 40) * 3) < 1e-6)).toBe(true);
    expect(shot.ctx.calls.some((c) => c.op === 'fillText' && /Pedigree/.test(String(c.args[0])))).toBe(true); // the caption
    await waitFor(() => expect(notices.current?.message).toBe('Picture saved'));
    expect((s.platform.calls.find((c) => c.method === 'pickSavePath')!.args[0] as { extensions: string[] }).extensions).toEqual(['png', 'svg']);
  });

  it('the_menu_offers_the_a3_direction_as_on_screen_by_default_and_remembers_best_fit', async () => {
    const s = await setupChart();
    await rightClick(s.canvas);
    const radios = within(menu()).getAllByRole('menuitemradio').slice(0, 2);
    expect(radios.map((r) => r.textContent?.trim())).toEqual(['Direction: as on screen', 'Direction: best fit']);
    expect(radios.map((r) => r.getAttribute('aria-checked'))).toEqual(['true', 'false']);
    // Choosing one ticks it and leaves the menu open for the save.
    await fireEvent.click(radios[1]);
    expect(menu()).toBeTruthy();
    const after = within(menu()).getAllByRole('menuitemradio').slice(0, 2);
    expect(after.map((r) => r.getAttribute('aria-checked'))).toEqual(['false', 'true']);
    expect(localStorage.getItem(A3_DIRECTION_KEY)).toBe('best');
    // The arrow keys reach the choices too.
    after[1].focus();
    await fireEvent.keyDown(menu(), { key: 'ArrowUp' });
    expect(after[0]).toHaveFocus();
    await fireEvent.keyDown(menu(), { key: 'Escape' });
    expect(screen.queryByRole('menu')).toBeNull();
    // Kept for the next menu.
    await rightClick(s.canvas);
    expect(within(menu()).getByRole('menuitemradio', { name: 'Direction: best fit' }).getAttribute('aria-checked')).toBe('true');
  });

  it('the_menu_has_the_title_block_and_background_choices_and_remembers_them', async () => {
    const s = await setupChart();
    await rightClick(s.canvas);
    const title = within(menu()).getByRole('menuitemradio', { name: 'Title block' });
    expect(title.getAttribute('aria-checked')).toBe('true');
    expect(within(menu()).getByRole('menuitemradio', { name: 'Background: paper' }).getAttribute('aria-checked')).toBe('true');
    await fireEvent.click(title);
    await fireEvent.click(within(menu()).getByRole('menuitemradio', { name: 'Background: transparent' }));
    expect(localStorage.getItem(A3_TITLE_KEY)).toBe('0');
    expect(localStorage.getItem(A3_BACKGROUND_KEY)).toBe('transparent');
    expect(within(menu()).getByRole('menuitemradio', { name: 'Title block' }).getAttribute('aria-checked')).toBe('false');
    expect(within(menu()).getByRole('menuitemradio', { name: 'Background: transparent' }).getAttribute('aria-checked')).toBe('true');
  });

  it('an_a3_save_without_title_block_on_a_transparent_page_paints_no_background_and_no_title', async () => {
    const made = stubOffscreen();
    a3Title.set(false);
    a3Background.set('transparent');
    const s = await setupChart();
    await rightClick(s.canvas);
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /^Save for A3 print…/ }));
    await waitFor(() => expect(s.saveSpy, JSON.stringify(notices.current)).toHaveBeenCalled(), A3_WAIT);
    const page = made[made.length - 1];
    expect(page.canvas.width).toBeGreaterThan(4000);
    const calls = page.ctx.calls;
    expect(calls.some((c) => c.op === 'fillRect' && (c.args[2] as number) === page.canvas.width)).toBe(false);
    // No title block: no 18 pt serif chart name and no rule under it.
    expect(calls.some((c) => c.op === 'set:font' && /600 [\d.]+px/.test(String(c.args[0])) && Number(/([\d.]+)px/.exec(String(c.args[0]))![1]) > 23)).toBe(false);
  });

  it('a_default_a3_save_lays_the_chart_out_once_in_the_direction_on_screen', async () => {
    stubOffscreen();
    // Ancestors on the left: not what this chart has by default, and not what best fit has to pick.
    const s = await setupChart('around', 'left');
    const closeSpy = vi.spyOn(s.api, 'chartClose');
    const before = s.openSpy.mock.calls.length;
    await rightClick(s.canvas);
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /^Save for A3 print…/ }));
    await waitFor(() => expect(s.saveSpy, JSON.stringify(notices.current)).toHaveBeenCalled(), A3_WAIT);
    const probes = s.openSpy.mock.calls.slice(before).map((c) => c[0]);
    expect(probes).toHaveLength(1);
    expect(probes[0].print).toBe(true);
    expect(probes[0].direction).toBe('left');
    expect(closeSpy.mock.calls.length).toBe(1);
  });

  it('a_print_under_compact_asks_for_a_folded_layout_and_under_as_today_does_not', async () => {
    stubOffscreen();
    for (const [layout, fold] of [['compact', true], ['today', undefined]] as const) {
      const raf = fakeRaf();
      const { api, fx } = makeApi();
      const root = await richestRoot(api, fx, 'around');
      const openSpy = vi.spyOn(api, 'chartOpen');
      const saveSpy = vi.spyOn(api, 'savePicture');
      const { unmount } = render(Host, { props: { api, view: ChartCanvas, props: { kind: 'around', root, generations: 6, printLayout: layout, onRecentre: vi.fn(), onOpenPerson: vi.fn() }, platform: new MockPlatform() } });
      const canvas = screen.getByRole('application') as HTMLCanvasElement;
      attachContext(canvas);
      canvas.getBoundingClientRect = () => RECT;
      await waitFor(() => expect(openSpy).toHaveBeenCalled());
      await new Promise((r) => setTimeout(r, 0));
      raf.pump(2);
      const before = openSpy.mock.calls.length;
      await rightClick(canvas);
      await fireEvent.click(within(menu()).getByRole('menuitem', { name: /^Save for A3 print…/ }));
      await waitFor(() => expect(saveSpy).toHaveBeenCalled(), A3_WAIT);
      const probes = openSpy.mock.calls.slice(before).map((c) => c[0]);
      expect(probes.length).toBeGreaterThan(0);
      expect(probes.every((r) => r.print === true && r.fold === fold)).toBe(true);
      unmount();
    }
  });

  it('an_a3_save_lays_out_print_charts_only_and_closes_them_without_touching_the_open_chart', async () => {
    stubOffscreen();
    a3Direction.set('best');
    const s = await setupChart('around');
    const closeSpy = vi.spyOn(s.api, 'chartClose');
    const screenChart = (await s.openSpy.mock.results[0].value).chartId;
    const before = s.openSpy.mock.calls.length;
    await rightClick(s.canvas);
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /^Save for A3 print, portrait/ }));
    await waitFor(() => expect(s.saveSpy, JSON.stringify(notices.current)).toHaveBeenCalled(), A3_WAIT);
    const probes = s.openSpy.mock.calls.slice(before).map((c) => c[0]);
    // Best fit: one layout per direction; every one a print chart that sets its own direction.
    expect(probes.length).toBe(4);
    expect(probes.every((r) => r.print === true && r.direction !== undefined)).toBe(true);
    // Each was closed, and the chart on screen was not.
    expect(closeSpy.mock.calls.length).toBe(4);
    expect(closeSpy.mock.calls.some((c) => c[0] === screenChart)).toBe(false);
    expect(s.saveSpy.mock.calls[0][0]).toMatch(/\(A3\)\.png$/);
  });

  it('an_error_in_the_middle_of_the_probes_still_closes_every_print_chart', async () => {
    stubOffscreen();
    a3Direction.set('best');
    const s = await setupChart('around');
    const closeSpy = vi.spyOn(s.api, 'chartClose');
    const before = s.openSpy.mock.calls.length;
    // The window read of the third print chart fails.
    let reads = 0;
    s.winSpy.mockImplementation(async (id, rect, zoom) => {
      const printCharts = s.openSpy.mock.calls.length - before;
      if (printCharts === 3) {
        reads++;
        throw new Error('window failed');
      }
      return MockApi.prototype.chartWindow.call(s.api, id, rect, zoom);
    });
    await rightClick(s.canvas);
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /^Save for A3 print, landscape/ }));
    await waitFor(() => expect(reads).toBeGreaterThan(0), A3_WAIT);
    await waitFor(() => expect(closeSpy.mock.calls.length).toBe(s.openSpy.mock.calls.length - before), A3_WAIT);
    expect(s.saveSpy).not.toHaveBeenCalled();
  });

  it('a_close_that_fails_does_not_stop_the_a3_save', async () => {
    stubOffscreen();
    const s = await setupChart('around');
    vi.spyOn(s.api, 'chartClose').mockRejectedValue(new Error('gone'));
    await rightClick(s.canvas);
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /^Save for A3 print, portrait/ }));
    await waitFor(() => expect(s.saveSpy, JSON.stringify(notices.current)).toHaveBeenCalled(), A3_WAIT);
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

describe('A3 print layouts on the around chart', () => {
  async function renderAround(layout: PrintLayout, synthetic: boolean) {
    const raf = fakeRaf();
    const { api, fx } = makeApi();
    const root = await richestRoot(api, fx, 'around');
    const openSpy = vi.spyOn(api, 'chartOpen');
    const saveSpy = vi.spyOn(api, 'savePicture');
    const platform = new MockPlatform();
    // Print charts get the synthetic chart; the chart on screen stays the mock's.
    if (synthetic) servePrintCharts(api, openSpy, bigAround());
    render(Host, { props: { api, view: ChartCanvas, props: { kind: 'around', root, generations: 6, printLayout: layout, onRecentre: vi.fn(), onOpenPerson: vi.fn() }, platform } });
    const canvas = screen.getByRole('application') as HTMLCanvasElement;
    attachContext(canvas);
    canvas.getBoundingClientRect = () => RECT;
    await waitFor(() => expect(openSpy).toHaveBeenCalled());
    await new Promise((r) => setTimeout(r, 0));
    raf.pump(2);
    return { api, platform, canvas, openSpy, saveSpy };
  }

  /** The text of every page of a print document, in order. */
  const pages = (html: string) =>
    html
      .split('<div class="sheet')
      .slice(1)
      .map((b) => [...b.matchAll(/>([^<]+)<\/text>/g)].map((m) => m[1]).join(' | '));

  it('sheets_print_as_one_document_with_a_page_per_sheet', async () => {
    stubOffscreen();
    const printed = capturePrints();
    const s = await renderAround('sheets', true);
    await rightClick(s.canvas);
    const item = within(menu()).getByRole('menuitem', { name: /^Print on A3/ });
    // The sheets are titled in the language of the moment of printing.
    setLanguage('uk');
    await fireEvent.click(item);
    await waitFor(() => expect(printed).toHaveLength(1), A3_WAIT);
    // The print chart was folded, as for "compact".
    expect(s.openSpy.mock.calls.filter((c) => c[0].print).every((c) => c[0].fold === true)).toBe(true);
    const html = printed[0];
    const texts = pages(html);
    const m = texts.length;
    // The cover and one sheet per grandparent line.
    expect(m).toBe(5);
    // One picture per page (each SVG nests its chart area in an inner one).
    expect(html.match(/<svg xmlns=/g)).toHaveLength(m);
    expect(html.match(/break-after:page/g)).toHaveLength(m - 1);
    expect(texts[0]).not.toContain('Гілка:');
    texts.slice(1).forEach((text, i) => {
      expect(text).toContain(`аркуш ${i + 2} з ${m}`);
      expect(text).toContain('→ арк. 1');
    });
    expect(texts.slice(1).map((t) => /Гілка: (\S+)/.exec(t)?.[1])).toEqual(['Bondar', 'Hrytsenko', 'Melnyk', 'Tkach']);
  }, 30000);

  it('sheets_save_one_file_per_sheet_after_one_question', async () => {
    stubOffscreen();
    const s = await renderAround('sheets', true);
    s.platform.next.save = 'C:/Mock/Around.svg';
    await rightClick(s.canvas);
    await fireEvent.click(within(menu()).getByRole('menuitem', { name: /^Save for A3 print…/ }));
    await waitFor(() => expect(s.saveSpy.mock.calls.length).toBe(5), A3_WAIT);
    expect(s.platform.calls.filter((c) => c.method === 'pickSavePath')).toHaveLength(1);
    // Every sheet in a new folder named after the chosen file; the folder is shown once all are written.
    expect(s.saveSpy.mock.calls.map((c) => c[0])).toEqual([1, 2, 3, 4, 5].map((n) => `C:/Mock/Around — sheets/Around — sheet ${n}.svg`));
    await waitFor(() => expect(s.platform.calls.filter((c) => c.method === 'reveal').map((c) => c.args[0])).toEqual(['C:/Mock/Around — sheets/Around — sheet 1.svg']));
    const second = new TextDecoder().decode(s.saveSpy.mock.calls[1][1]);
    expect(second).toContain('Branch: Bondar');
    expect(second).toContain('Sheet 2 of 5');
    expect(second).toContain('→ sheet 1');
  }, 30000);
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
