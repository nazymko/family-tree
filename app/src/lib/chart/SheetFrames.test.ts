import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import LayoutHost from './testing/LayoutHost.svelte';
import { within } from '@testing-library/svelte';
import { asPrintChart, bigAround, capturePrints, servePrintCharts } from './testing/syntheticAround';
import { attachContext, fakeRaf, makeApi, richestRoot } from './testing/helpers';
import { makeFakeContext } from '../../test-setup';
import { screenFrames } from './sheets';
import type { SheetPlan } from './sheetPlan';
import { a3Orientation } from './a3Pref.svelte';

// The plans the frames on screen are drawn from.
vi.mock('./sheets', async (orig) => {
  const m = await orig<typeof import('./sheets')>();
  return { ...m, screenFrames: vi.fn(m.screenFrames) };
});

const RECT = { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600, x: 0, y: 0, toJSON() {} };
const WAIT = { timeout: 10000 };

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  a3Orientation.set('auto');
});

/**
 * The around chart on screen is the synthetic chart; every print chart is the same people as a print lays them out
 * (other ids, smaller cards, other places), so the screen's frames can only be right if they follow the print's cut.
 */
async function setup() {
  const raf = fakeRaf();
  const { api, fx } = makeApi();
  const root = await richestRoot(api, fx, 'around');
  const openSpy = vi.spyOn(api, 'chartOpen');
  const big = bigAround();
  servePrintCharts(api, openSpy, asPrintChart(big), big);
  render(LayoutHost, { props: { api, root, kind: 'around', person: root } });
  const canvas = screen.getByRole('application') as HTMLCanvasElement;
  const ctx = attachContext(canvas);
  canvas.getBoundingClientRect = () => RECT;
  await waitFor(() => expect(openSpy).toHaveBeenCalled());
  const prints = () => openSpy.mock.calls.filter((c) => c[0].print).length;
  /** The sheet labels of the last frame drawn (after drawing what is due). */
  const labels = () => {
    raf.pump(2);
    const from = ctx.calls.map((c) => c.op).lastIndexOf('clearRect');
    return ctx.calls
      .slice(Math.max(0, from))
      .filter((c) => c.op === 'fillText' && /^sheet \d+$/.test(String(c.args[0])))
      .map((c) => String(c.args[0]));
  };
  /** How many frames were drawn. */
  const draws = () => ctx.calls.filter((c) => c.op === 'clearRect').length;
  return { openSpy, prints, labels, draws };
}

describe('sheet frames on the around chart', () => {
  it('one_frame_per_piece_with_its_page', async () => {
    const s = await setup();
    // As today: no frames, and no print chart laid out for them.
    await new Promise((r) => setTimeout(r, 400));
    expect(s.labels()).toEqual([]);
    expect(s.prints()).toBe(0);

    await fireEvent.click(screen.getByRole('radio', { name: 'Sheets by branch' }));
    await waitFor(() => expect(s.labels().length).toBeGreaterThan(0), WAIT);
    // One frame per packed piece, the cover included, each labelled with the page it prints on: the pages are numbered
    // from 1 with none skipped, and page 1 (the cover's) is framed.
    const nos = s.labels().map((l) => Number(l.slice('sheet '.length)));
    const pagesSeen = [...new Set(nos)].sort((a, b) => a - b);
    expect(pagesSeen.length).toBeGreaterThan(1);
    expect(pagesSeen).toEqual(Array.from({ length: pagesSeen.length }, (_, i) => i + 1));
    expect(nos[0]).toBe(1);
    // The print planned every direction, whatever the A3 direction setting.
    expect(s.openSpy.mock.calls.filter((c) => c[0].print).map((c) => c[0].direction)).toEqual(['top', 'bottom', 'left', 'right']);

    const before = s.draws();
    await fireEvent.click(screen.getByRole('radio', { name: 'Compact' }));
    await waitFor(() => {
      expect(s.labels()).toEqual([]);
      expect(s.draws()).toBeGreaterThan(before);
    }, WAIT);
    // No late result from the sheets chart brings them back.
    await new Promise((r) => setTimeout(r, 400));
    expect(s.labels()).toEqual([]);
  }, 30000);

  it('frames_show_the_pages_the_print_gives_in_each_orientation', async () => {
    // Offscreen canvases (the pages) record what they are given; the chart on screen keeps its own context.
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation((() => makeFakeContext()) as never);
    vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(function (cb: BlobCallback) {
      const bytes = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
      // jsdom's Blob has no arrayBuffer().
      cb(Object.assign(new Blob([bytes], { type: 'image/png' }), { arrayBuffer: async () => bytes.buffer }));
    } as never);
    const printed = capturePrints();
    const s = await setup();
    const shown = vi.mocked(screenFrames);
    shown.mockClear();
    await fireEvent.click(screen.getByRole('radio', { name: 'Sheets by branch' }));
    await waitFor(() => expect(s.labels().length).toBeGreaterThan(0), WAIT);
    const menu = () => screen.getByRole('menu', { name: /^(Save as picture|Person and picture options)$/ });
    const openMenu = async () => {
      // The menu opens again once the last print or save is done.
      await waitFor(async () => {
        if (!screen.queryByRole('menu')) await fireEvent.contextMenu(screen.getByRole('application'), { button: 2, clientX: 400, clientY: 300 });
        expect(menu()).toBeTruthy();
      }, WAIT);
      return within(menu());
    };
    const items = { auto: /^Save for A3 print…$/, landscape: /^Save for A3 print, landscape…$/, portrait: /^Save for A3 print, portrait…$/ } as const;
    for (const choice of ['landscape', 'portrait', 'auto'] as const) {
      // Saving in an orientation makes it the one remembered: the frames show its pages.
      const framed = shown.mock.calls.length;
      await fireEvent.click((await openMenu()).getByRole('menuitem', { name: items[choice] }));
      await waitFor(() => expect(a3Orientation.value).toBe(choice));
      await waitFor(() => expect(screen.queryByText(/^Saving the picture/)).toBeNull(), WAIT);
      await waitFor(() => {
        expect(shown.mock.calls.length).toBeGreaterThan(framed);
        const last = shown.mock.calls.at(-1)?.[0] as SheetPlan | undefined;
        expect(last && last.pages.every((pg) => choice === 'auto' || pg.orientation === choice)).toBe(true);
        expect(s.labels().length).toBeGreaterThan(0);
      }, WAIT);
      const plan = shown.mock.calls.at(-1)![0] as SheetPlan;
      const opened = s.prints();
      // Printing uses that same plan: no print chart is laid out again, and each person is on the page their frame says.
      const before = printed.length;
      await fireEvent.click((await openMenu()).getByRole('menuitem', { name: /^Print on A3/ }));
      await waitFor(() => expect(printed.length).toBe(before + 1), WAIT);
      expect(s.prints(), choice).toBe(opened);
      const pages = printed
        .at(-1)!
        .split('<div class="sheet')
        .slice(1)
        .map((b) => new Set([...b.matchAll(/>([^<]+)<\/text>/g)].flatMap((m) => m[1].split(' '))));
      expect(pages.length, choice).toBe(plan.pages.length);
      let people = 0;
      for (const pg of plan.pages)
        for (const pc of pg.pieces)
          for (const n of pc.view.scene.nodes) {
            if (n.dupOf !== null || pc.view.refs.has(n.id) || !n.person) continue;
            const word = n.person.name.split(' ')[0];
            expect(pages[pg.no - 1].has(word), `${choice}: ${n.person.name} on sheet ${pg.no}`).toBe(true);
            people++;
          }
      expect(people).toBeGreaterThan(30);
      expect(new Set(s.labels()).size, choice).toBe(plan.pages.length);
    }
  }, 60000);

  it('a_quick_change_of_layout_lays_out_no_print_chart', async () => {
    const s = await setup();
    await fireEvent.click(screen.getByRole('radio', { name: 'Sheets by branch' }));
    await fireEvent.click(screen.getByRole('radio', { name: 'As today' }));
    await new Promise((r) => setTimeout(r, 500));
    expect(s.prints()).toBe(0);
    expect(s.labels()).toEqual([]);
  }, 30000);
});
