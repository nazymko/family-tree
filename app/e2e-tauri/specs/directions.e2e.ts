/**
 * The side the ancestors are drawn on, on the REAL app (debug build, tauri-driver, WebView2): the pedigree chart is
 * switched through the four choices of the direction control (top, bottom, left, right). After each click the backend's
 * layout puts the father's card above, below, left or right of the home person's card, with the card sizes unchanged.
 * A screenshot per direction.
 *
 * Synthetic data only.
 */
import { expect } from '@wdio/globals';
import { $, browser, goTo, importSynthetic, meta, openTreeFromReport, ready, shot } from './helpers';

type Box = { x: number; y: number; w: number; h: number };
type Placed = { root: Box; father: Box };

/** The home person's pedigree as the backend lays it out under the current settings. */
async function pedigreeNow(name: string): Promise<Placed> {
  await browser.execute((n: string) => {
    delete (window as unknown as { __gtDir?: unknown }).__gtDir;
    type Inv = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
    const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: Inv } }).__TAURI_INTERNALS__.invoke;
    const big = { x: -1e7, y: -1e7, w: 2e7, h: 2e7 };
    void (async () => {
      const whole = (await invoke('chart_open_whole')) as { meta: { chartId: number } };
      const all = (await invoke('chart_window', { chartId: whole.meta.chartId, rect: big, zoom: 1 })) as { nodes: { person?: { id: string; name: string } }[] };
      const id = all.nodes.find((x) => x.person?.name === n)!.person!.id;
      const m = (await invoke('chart_open', { req: { kind: 'pedigree', root: id, generations: 3 } })) as { chartId: number; rootNode: number };
      const w = (await invoke('chart_window', { chartId: m.chartId, rect: big, zoom: 1 })) as {
        nodes: { id: number; x: number; y: number; w: number; h: number }[];
        edges: { from: number; to: number; kind: string }[];
      };
      const box = (i: number) => {
        const q = w.nodes.find((k) => k.id === i)!;
        return { x: q.x, y: q.y, w: q.w, h: q.h };
      };
      const parents = w.edges.filter((e) => e.kind === 'parent' && e.to === m.rootNode).map((e) => e.from);
      (window as unknown as { __gtDir?: unknown }).__gtDir = { root: box(m.rootNode), father: box(parents[0]) };
    })();
  }, name);
  await browser.waitUntil(() => browser.execute(() => !!(window as unknown as { __gtDir?: unknown }).__gtDir), { timeout: 30000 });
  return browser.execute(() => (window as unknown as { __gtDir: Placed }).__gtDir);
}

const cx = (b: Box) => b.x + b.w / 2;
const cy = (b: Box) => b.y + b.h / 2;

describe('Direction of the charts', () => {
  before(async () => {
    await ready();
    await importSynthetic('Directions');
    await openTreeFromReport();
  });

  it('puts_the_ancestors_on_each_of_the_four_sides', async () => {
    await goTo('Pedigree');
    await $('canvas[aria-label="Pedigree chart"]').waitForDisplayed({ timeout: 30000 });
    const group = await $('[role="radiogroup"][aria-label="Where the ancestors are"]');
    await group.waitForDisplayed();
    const cases = [
      { label: 'Ancestors on top', key: 'top' },
      { label: 'Ancestors at the bottom', key: 'bottom' },
      { label: 'Ancestors on the left', key: 'left' },
      { label: 'Ancestors on the right', key: 'right' },
    ] as const;
    for (const c of cases) {
      const radio = await group.$(`input[aria-label="${c.label}"]`);
      await radio.click();
      await browser.waitUntil(() => radio.isSelected(), { timeout: 10000 });
      await browser.pause(700);
      const p = await pedigreeNow(meta().home.name);
      const holds = {
        top: cy(p.father) < cy(p.root),
        bottom: cy(p.father) > cy(p.root),
        left: cx(p.father) < cx(p.root),
        right: cx(p.father) > cx(p.root),
      }[c.key];
      expect(holds).toBe(true);
      // Cards stay upright: the same size in every direction.
      expect([p.father.w, p.father.h]).toEqual([p.root.w, p.root.h]);
      await shot(`directions-pedigree-${c.key}.png`);
    }
  });
});
