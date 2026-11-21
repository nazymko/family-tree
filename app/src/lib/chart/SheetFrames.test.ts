import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import LayoutHost from './testing/LayoutHost.svelte';
import { asPrintChart, bigAround, servePrintCharts } from './testing/syntheticAround';
import { attachContext, fakeRaf, makeApi, richestRoot } from './testing/helpers';

const RECT = { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600, x: 0, y: 0, toJSON() {} };
const WAIT = { timeout: 10000 };

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
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

  it('a_quick_change_of_layout_lays_out_no_print_chart', async () => {
    const s = await setup();
    await fireEvent.click(screen.getByRole('radio', { name: 'Sheets by branch' }));
    await fireEvent.click(screen.getByRole('radio', { name: 'As today' }));
    await new Promise((r) => setTimeout(r, 500));
    expect(s.prints()).toBe(0);
    expect(s.labels()).toEqual([]);
  }, 30000);
});
