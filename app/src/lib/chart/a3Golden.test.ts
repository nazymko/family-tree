import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { writeFileSync } from 'node:fs';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { makeFakeContext } from '../../test-setup';
import { MockPlatform } from '../api/mock/MockPlatform';
import { notices } from '../ui/notices.svelte';
import ChartCanvas from './ChartCanvas.svelte';
import Host from './testing/Host.svelte';
import { A3_BACKGROUND_KEY, A3_DIRECTION_KEY, A3_TITLE_KEY, a3Background, a3Direction, a3Title } from './a3Pref.svelte';
import { attachContext, fakeRaf, makeApi, richestRoot } from './testing/helpers';
import { bigAround, capturePrints, servePrintCharts } from './testing/syntheticAround';

/**
 * The A3 print of the "built around" chart under "as today" and "compact", compared with a fixed document recorded from
 * the code before the print layouts (commit 4bb016c, where both took the single-page path). The fixture was made by
 * running this same file in a checkout of 4bb016c with GT_GOLDEN_OUT set to the fixture's path.
 */
/** The recorded document; line ends do not count (a checkout may turn them into CRLF). */
const golden = async (): Promise<string> => lf((await import('./testing/a3-today-print.golden.html?raw')).default);
const lf = (s: string) => s.split(String.fromCharCode(13, 10)).join(String.fromCharCode(10));
const RECT = { left: 0, top: 0, right: 800, bottom: 600, width: 800, height: 600, x: 0, y: 0, toJSON() {} };
const A3_WAIT = { timeout: 15000 };

beforeEach(() => {
  notices.dismiss();
  for (const k of [A3_DIRECTION_KEY, A3_TITLE_KEY, A3_BACKGROUND_KEY]) localStorage.removeItem(k);
  a3Direction.reload();
  a3Title.reload();
  a3Background.reload();
  // The title block carries today's date: a fixed day keeps the document fixed.
  vi.useFakeTimers({ toFake: ['Date'] });
  vi.setSystemTime(new Date(2026, 10, 4, 12, 0, 0));
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

async function printAround(layout: 'today' | 'compact'): Promise<string> {
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation((() => makeFakeContext()) as never);
  const printed = capturePrints();
  const raf = fakeRaf();
  const { api, fx } = makeApi();
  const root = await richestRoot(api, fx, 'around');
  const openSpy = vi.spyOn(api, 'chartOpen');
  servePrintCharts(api, openSpy, bigAround());
  render(Host, { props: { api, view: ChartCanvas, props: { kind: 'around', root, generations: 6, printLayout: layout, onRecentre: vi.fn(), onOpenPerson: vi.fn() }, platform: new MockPlatform() } });
  const canvas = screen.getByRole('application') as HTMLCanvasElement;
  attachContext(canvas);
  canvas.getBoundingClientRect = () => RECT;
  await waitFor(() => expect(openSpy).toHaveBeenCalled());
  await new Promise((r) => setTimeout(r, 0));
  raf.pump(2);
  await fireEvent.contextMenu(canvas, { button: 2, clientX: 400, clientY: 300 });
  const menu = await screen.findByRole('menu', { name: /^(Save as picture|Person and picture options)$/ });
  await fireEvent.click(within(menu).getByRole('menuitem', { name: /^Print on A3/ }));
  await waitFor(() => expect(printed).toHaveLength(1), A3_WAIT);
  return printed[0];
}

describe('A3 print, as today and compact', () => {
  it('prints_the_document_recorded_before_the_print_layouts', async () => {
    const html = await printAround('today');
    const out = process.env.GT_GOLDEN_OUT;
    if (out) {
      writeFileSync(out, html, 'utf8');
      return;
    }
    expect(html.length).toBeGreaterThan(10_000);
    expect(lf(html)).toBe(await golden());
  }, 30000);

  it('compact_takes_the_same_single_page_path', async () => {
    // The mock lays the chart out the same with or without folding, so the document is the recorded one too.
    if (process.env.GT_GOLDEN_OUT) return;
    expect(lf(await printAround('compact'))).toBe(await golden());
  }, 30000);
});
