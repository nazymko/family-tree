/**
 * "Build around these people" on the REAL app (debug build, tauri-driver, WebView2): two people are marked on the whole
 * tree (Space on the first, a real Shift+click on the second), the button counts them, the around screen names both in
 * its heading, draws both cards, lists both at the top of the «Outline» tab, and Back returns to the whole tree with the
 * two people still marked. The three kin choices (line only, brothers and sisters, all relatives) work for the two people:
 * the chart grows from one to the next and every chosen person's parents stay adjacent and over them. Screenshots in
 * English and Ukrainian.
 *
 * Synthetic data only.
 */
import { expect } from '@wdio/globals';
import { $, browser, importSynthetic, openTreeFromReport, ready, shot } from './helpers';

const SHIFT = '';

async function choose(label: string, value: string) {
  const select = await $(`//select[@id=//label[starts-with(normalize-space(.),"${label}")]/@for]`);
  await select.waitForDisplayed();
  await select.selectByAttribute('value', value);
}

async function toWholeTree(label: string, title: string) {
  const a = await $(`//nav//a[normalize-space(.)="${label}"]`);
  await a.waitForClickable();
  await a.click();
  await $(`h1=${title}`).waitForDisplayed({ timeout: 30000 });
}

async function flyTo(findLabel: string, name: string) {
  const box = await $(`//input[@role="combobox"][@id=//label[starts-with(normalize-space(.),"${findLabel}")]/@for]`);
  await box.waitForDisplayed();
  await box.setValue(name);
  const option = await $(`//ul[@role="listbox"][@aria-label="${findLabel}"]//*[@role="option"][contains(.,"${name}")]`);
  await option.waitForDisplayed({ timeout: 10000 });
  await option.click();
  await browser.pause(900);
}

async function focusCanvas() {
  await browser.execute(() => (document.querySelector('canvas[role="application"]') as HTMLElement).focus());
}

/** A click in the middle of the chart with the Shift key held. */
async function shiftClickCentre() {
  const canvas = await $('canvas[role="application"]');
  const { x, y } = await canvas.getLocation();
  const { width, height } = await canvas.getSize();
  const px = Math.round(x + width / 2);
  const py = Math.round(y + height / 2);
  await browser.performActions([
    {
      type: 'key',
      id: 'keyboard',
      actions: [{ type: 'keyDown', value: SHIFT }, { type: 'pause' }, { type: 'pause' }, { type: 'pause' }, { type: 'keyUp', value: SHIFT }],
    },
    {
      type: 'pointer',
      id: 'mouse',
      parameters: { pointerType: 'mouse' },
      actions: [
        { type: 'pause' },
        { type: 'pointerMove', duration: 0, x: px, y: py, origin: 'viewport' },
        { type: 'pointerDown', button: 0 },
        { type: 'pointerUp', button: 0 },
        { type: 'pause' },
      ],
    },
  ]);
  await browser.releaseActions();
}

/**
 * Two people of the synthetic tree who both have a father and a mother, standing near each other on the whole tree
 * (found in the backend's chart of everybody; names that appear once).
 */
async function pickPair(): Promise<{ first: string; second: string }> {
  await browser.execute(() => {
    delete (window as unknown as { __gtPair?: unknown }).__gtPair;
    type Inv = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
    const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: Inv } }).__TAURI_INTERNALS__.invoke;
    const big = { x: -1e7, y: -1e7, w: 2e7, h: 2e7 };
    const fail = (e: unknown) => ((window as unknown as { __gtPair?: unknown }).__gtPair = { error: String(e) });
    void (async () => {
      const whole = (await invoke('chart_open_whole')) as { meta: { chartId: number } };
      const w = (await invoke('chart_window', { chartId: whole.meta.chartId, rect: big, zoom: 1 })) as {
        nodes: { id: number; x: number; y: number; person?: { id: string; name: string } }[];
      };
      const count = new Map<string, number>();
      for (const n of w.nodes) if (n.person) count.set(n.person.name, (count.get(n.person.name) ?? 0) + 1);
      const people = w.nodes.filter((n) => n.person && count.get(n.person.name) === 1);
      const found: { name: string; dad: string }[] = [];
      for (const n of people.slice(Math.floor(people.length / 3))) {
        if (found.length === 2) break;
        const m = (await invoke('chart_open', { req: { kind: 'around', root: n.person!.id, generations: 2, kin: 'line' } })) as { chartId: number; rootNode: number };
        const c = (await invoke('chart_window', { chartId: m.chartId, rect: big, zoom: 1 })) as {
          nodes: { id: number; person?: { name: string } }[];
          edges: { from: number; to: number; kind: string }[];
        };
        const ps = c.edges.filter((e) => e.kind === 'parent' && e.to === m.rootNode).map((e) => c.nodes.find((x) => x.id === e.from)?.person?.name ?? '');
        if (ps.length === 2 && found.every((f) => f.dad !== ps[0])) found.push({ name: n.person!.name, dad: ps[0] });
      }
      (window as unknown as { __gtPair?: unknown }).__gtPair = { first: found[0].name, second: found[1].name };
    })().catch(fail);
  });
  await browser.waitUntil(() => browser.execute(() => !!(window as unknown as { __gtPair?: unknown }).__gtPair), { timeout: 30000 });
  const got = await browser.execute(() => (window as unknown as { __gtPair: { first: string; second: string; error?: string } }).__gtPair);
  if (got.error) throw new Error(got.error);
  return got;
}

/** Marks the two people on the whole tree: Space on the first, Shift+click on the second. */
async function markBoth(findLabel: string, first: string, second: string) {
  await flyTo(findLabel, first);
  const marked = $('[data-testid="marked-count"]');
  // Marks left from before (the whole tree remembers them): Escape clears them.
  if (await marked.isDisplayed()) {
    await focusCanvas();
    await browser.keys('Escape');
    await marked.waitForDisplayed({ reverse: true, timeout: 10000 });
    // Clearing leaves nothing selected: find the person again.
    await flyTo(findLabel, first);
  }
  // Space marks the selected card.
  for (let i = 0; i < 3 && !(await marked.isDisplayed()); i++) {
    await focusCanvas();
    await browser.keys(' ');
    await browser.pause(700);
  }
  await marked.waitForDisplayed();
  await flyTo(findLabel, second);
  await shiftClickCentre();
}

/** The names of the cards the around chart draws, read from the backend for the same two people. */
type Layout = {
  chosen: number[];
  nodes: { id: number; name: string; x: number; y: number; w: number }[];
  parents: { from: number; to: number }[];
};

async function cardNames(first: string, second: string, kin: 'line' | 'siblings' | 'all' = 'line'): Promise<Layout> {
  await browser.execute((a: string, b: string, k: string) => {
    delete (window as unknown as { __gtMany?: unknown }).__gtMany;
    type Inv = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
    const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: Inv } }).__TAURI_INTERNALS__.invoke;
    const big = { x: -1e7, y: -1e7, w: 2e7, h: 2e7 };
    void (async () => {
      const whole = (await invoke('chart_open_whole')) as { meta: { chartId: number } };
      const all = (await invoke('chart_window', { chartId: whole.meta.chartId, rect: big, zoom: 1 })) as { nodes: { person?: { id: string; name: string } }[] };
      const id = (n: string) => all.nodes.find((x) => x.person?.name === n)!.person!.id;
      const roots = [id(a), id(b)];
      const m = (await invoke('chart_open', { req: { kind: 'around', root: roots[0], generations: 255, kin: k, roots } })) as { chartId: number; chosenNodes?: number[] };
      const w = (await invoke('chart_window', { chartId: m.chartId, rect: big, zoom: 1 })) as {
        nodes: { id: number; x: number; y: number; w: number; person?: { name: string } }[];
        edges: { from: number; to: number; kind: string }[];
      };
      (window as unknown as { __gtMany?: unknown }).__gtMany = {
        chosen: m.chosenNodes ?? [],
        nodes: w.nodes.filter((n) => n.person).map((n) => ({ id: n.id, name: n.person!.name, x: n.x, y: n.y, w: n.w })),
        parents: w.edges.filter((e) => e.kind === 'parent').map((e) => ({ from: e.from, to: e.to })),
      };
    })();
  }, first, second, kin);
  await browser.waitUntil(() => browser.execute(() => !!(window as unknown as { __gtMany?: unknown }).__gtMany), { timeout: 30000 });
  return browser.execute(() => (window as unknown as { __gtMany: Layout }).__gtMany);
}

/**
 * Each chosen person's father and mother stand in the row above, side by side (a card and the partner gap apart), over
 * the person: exactly centred, or moved sideways by at most what clearing the neighbouring couple takes.
 */
function expectParentsAbove(l: Layout) {
  const card = 176;
  const pstep = card + 8;
  const row = 114;
  const slack = (2 * card + 8 + 40 - card) / 2 + 1;
  let couples = 0;
  for (const id of l.chosen) {
    const me = l.nodes.find((n) => n.id === id)!;
    const ps = l.parents.filter((e) => e.to === id).map((e) => l.nodes.find((n) => n.id === e.from)!);
    if (ps.length < 2) continue;
    couples++;
    const [a, b] = ps.sort((p, q) => p.x - q.x);
    expect(Math.abs(a.y - (me.y - row))).toBeLessThan(1);
    expect(Math.abs(b.y - a.y)).toBeLessThan(1);
    expect(Math.abs(b.x - a.x - pstep)).toBeLessThan(1);
    expect(Math.abs((a.x + b.x + card) / 2 - (me.x + card / 2))).toBeLessThan(slack);
  }
  expect(couples).toBeGreaterThan(0);
}

/** Both names are rows at the top level of the outline. */
async function expectTopRows(first: string, second: string) {
  const rows = await browser.waitUntil(
    async () => {
      const r = await browser.execute(() =>
        [...document.querySelectorAll('[role="treeitem"][aria-level="1"] > .row')].map((e) => (e.textContent ?? '').trim()),
      );
      return r.length >= 2 ? r : false;
    },
    { timeout: 20000, interval: 600, timeoutMsg: 'the outline has no top level rows' },
  );
  expect(rows.some((r) => r.includes(first))).toBe(true);
  expect(rows.some((r) => r.includes(second))).toBe(true);
}

let pair = { first: '', second: '' };

describe('The screen built around several people', () => {
  before(async () => {
    await ready();
  });

  after(async () => {
    await $('//nav//a[normalize-space(.)="Налаштування"]').click();
    await choose('Мова', 'en');
    await $('h1=Settings').waitForDisplayed({ timeout: 20000 });
  });

  it('builds one chart around two marked people, lists both in the outline and goes back', async () => {
    await importSynthetic('Around many');
    await openTreeFromReport();
    await toWholeTree('Whole tree', 'Whole tree');
    pair = await pickPair();
    const { first, second } = pair;
    await markBoth('Find a person…', first, second);
    await expect($('[data-testid="marked-count"]')).toHaveText('2 people selected');
    await $('//button[normalize-space(.)="Build around these people (2)"]').waitForDisplayed();

    await $('//button[normalize-space(.)="Build around these people (2)"]').click();
    await $(`//h1[contains(.,"Built around:")][contains(.,"${first}")][contains(.,"${second}")]`).waitForDisplayed({ timeout: 30000 });
    await $('canvas[role="application"]').waitForDisplayed();
    await browser.pause(1200);

    // Both cards are in the chart; the three kin choices can be picked.
    const cards = await cardNames(first, second);
    expect(cards.chosen.length).toBe(2);
    const onChart = cards.nodes.map((n) => n.name);
    expect(onChart).toContain(first);
    expect(onChart).toContain(second);
    expectParentsAbove(cards);
    await expect($('//label[normalize-space(.)="Line only"]//input')).toBeSelected();
    await expect($('//label[normalize-space(.)="All relatives"]//input')).toBeEnabled();
    await expect($('//label[normalize-space(.)="Brothers and sisters"]//input')).toBeEnabled();
    await shot('around-many-en.png');

    // The outline lists both chosen people at the top.
    await $('//*[@role="tab"][normalize-space(.)="Outline"]').click();
    await expectTopRows(first, second);
    await $('//*[@role="tab"][normalize-space(.)="Chart"]').click();

    // Back: the whole tree, the two people still marked.
    await $('//button[normalize-space(.)="Back to the whole tree"]').click();
    await $('h1=Whole tree').waitForDisplayed({ timeout: 30000 });
    await expect($('[data-testid="marked-count"]')).toHaveText('2 people selected');
  });

  it('shows the same screen in Ukrainian', async () => {
    const { first, second } = pair;
    await $('//nav//a[normalize-space(.)="Settings"]').click();
    await choose('Language', 'uk');
    await $('h1=Налаштування').waitForDisplayed({ timeout: 20000 });
    await toWholeTree('Увесь родовід', 'Увесь родовід');
    await markBoth('Знайти особу…', first, second);
    await $('//button[normalize-space(.)="Побудувати навколо цих осіб (2)"]').waitForDisplayed();
    await $('//button[normalize-space(.)="Побудувати навколо цих осіб (2)"]').click();
    await $(`//h1[contains(.,"Побудовано навколо:")][contains(.,"${first}")][contains(.,"${second}")]`).waitForDisplayed({ timeout: 30000 });
    await $('canvas[role="application"]').waitForDisplayed();
    await browser.pause(1200);
    await shot('around-many-uk.png');

    // Brothers and sisters, then all relatives: the chart grows and the parents stay over the chosen people.
    const line = await cardNames(first, second, 'line');
    await $('//label[normalize-space(.)="Брати й сестри"]//input').waitForEnabled();
    await $('//label[normalize-space(.)="Брати й сестри"]').click();
    await browser.pause(2000);
    await shot('around-many-siblings-uk.png');
    const sibs = await cardNames(first, second, 'siblings');
    expect(sibs.nodes.length).toBeGreaterThan(line.nodes.length);
    expect(sibs.nodes.map((n) => n.name)).toContain(first);
    expectParentsAbove(sibs);
    await $('//label[normalize-space(.)="Уся рідня"]').click();
    await browser.pause(2000);
    await shot('around-many-all-uk.png');
    const all = await cardNames(first, second, 'all');
    expect(all.nodes.length).toBeGreaterThan(sibs.nodes.length);
    expectParentsAbove(all);
    await $('//label[normalize-space(.)="Лише лінія"]').click();
    await browser.pause(1500);

    await $('//*[@role="tab"][normalize-space(.)="Список"]').click();
    await expectTopRows(first, second);
    await $('//*[@role="tab"][normalize-space(.)="Схема"]').click();
    await $('//button[normalize-space(.)="Назад до всього родоводу"]').click();
    await $('h1=Увесь родовід').waitForDisplayed({ timeout: 30000 });
  });
});

/**
 * A married couple of the synthetic tree whose husband (the partner of the first) has brothers or sisters of his own,
 * found in the backend: the wife first, then the husband and the names of his brothers and sisters.
 */
async function pickCouple(): Promise<{ wife: string; husband: string; siblings: string[] }> {
  await browser.execute(() => {
    delete (window as unknown as { __gtCouple?: unknown }).__gtCouple;
    type Inv = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
    const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: Inv } }).__TAURI_INTERNALS__.invoke;
    const big = { x: -1e7, y: -1e7, w: 2e7, h: 2e7 };
    type Win = {
      nodes: { id: number; relation?: string; person?: { id: string; name: string } }[];
      edges: { from: number; to: number; kind: string }[];
    };
    const open = async (root: string, kin: string) => {
      const m = (await invoke('chart_open', { req: { kind: 'around', root, generations: 2, kin } })) as { chartId: number; rootNode: number };
      const w = (await invoke('chart_window', { chartId: m.chartId, rect: big, zoom: 1 })) as Win;
      return { w, root: m.rootNode };
    };
    void (async () => {
      const whole = (await invoke('chart_open_whole')) as { meta: { chartId: number } };
      const all = (await invoke('chart_window', { chartId: whole.meta.chartId, rect: big, zoom: 1 })) as Win;
      const count = new Map<string, number>();
      for (const n of all.nodes) if (n.person) count.set(n.person.name, (count.get(n.person.name) ?? 0) + 1);
      const once = (name: string) => count.get(name) === 1;
      for (const n of all.nodes.filter((x) => x.person && once(x.person.name))) {
        const a = await open(n.person!.id, 'line');
        const edge = a.w.edges.find((e) => e.kind === 'partner' && e.from === a.root);
        const partner = edge && a.w.nodes.find((x) => x.id === edge.to);
        if (!partner?.person || !once(partner.person.name)) continue;
        const b = await open(partner.person.id, 'siblings');
        const sibs = b.w.nodes.filter((x) => x.person && /brother|sister|sibling/.test(x.relation ?? '') && once(x.person.name));
        if (sibs.length === 0) continue;
        (window as unknown as { __gtCouple?: unknown }).__gtCouple = {
          wife: n.person!.name,
          husband: partner.person.name,
          siblings: sibs.map((x) => x.person!.name),
        };
        return;
      }
      (window as unknown as { __gtCouple?: unknown }).__gtCouple = { error: 'no couple with a sibling' };
    })().catch((e) => ((window as unknown as { __gtCouple?: unknown }).__gtCouple = { error: String(e) }));
  });
  await browser.waitUntil(() => browser.execute(() => !!(window as unknown as { __gtCouple?: unknown }).__gtCouple), { timeout: 60000 });
  const got = await browser.execute(
    () => (window as unknown as { __gtCouple: { wife: string; husband: string; siblings: string[]; error?: string } }).__gtCouple,
  );
  if (got.error) throw new Error(got.error);
  return got;
}

describe('The screen built around a married couple', () => {
  before(async () => {
    await ready();
  });

  after(async () => {
    await $('//nav//a[normalize-space(.)="Налаштування"]').click();
    await choose('Мова', 'en');
    await $('h1=Settings').waitForDisplayed({ timeout: 20000 });
  });

  it("keeps the husband's brothers and sisters when the wife is chosen first", async () => {
    // The synthetic tree of the first part is still open.
    await $('//nav//a[normalize-space(.)="Settings"]').click();
    await choose('Language', 'uk');
    await $('h1=Налаштування').waitForDisplayed({ timeout: 20000 });
    await toWholeTree('Увесь родовід', 'Увесь родовід');
    const { wife, husband, siblings } = await pickCouple();
    await markBoth('Знайти особу…', wife, husband);
    await $('//button[normalize-space(.)="Побудувати навколо цих осіб (2)"]').waitForDisplayed();
    await $('//button[normalize-space(.)="Побудувати навколо цих осіб (2)"]').click();
    await $(`//h1[contains(.,"Побудовано навколо:")][contains(.,"${wife}")][contains(.,"${husband}")]`).waitForDisplayed({ timeout: 30000 });
    await $('canvas[role="application"]').waitForDisplayed();
    await $('//label[normalize-space(.)="Уся рідня"]').click();
    await browser.pause(2500);
    await shot('around-many-couple-all-uk.png');

    // Brothers and sisters mode: every sibling of the husband is on the chart, on the far side of the pair.
    const sibs = await cardNames(wife, husband, 'siblings');
    const names = sibs.nodes.map((n) => n.name);
    for (const s of siblings) expect(names).toContain(s);
    const man = sibs.nodes.find((n) => n.name === husband)!;
    const woman = sibs.nodes.find((n) => n.name === wife)!;
    expect(Math.abs(man.y - woman.y)).toBeLessThan(1);
    const sign = man.x > woman.x ? 1 : -1;
    for (const s of siblings) {
      const c = sibs.nodes.find((n) => n.name === s)!;
      if (Math.abs(c.y - man.y) < 1) expect((c.x - man.x) * sign).toBeGreaterThan(0);
    }
    const all = await cardNames(wife, husband, 'all');
    expect(all.nodes.length).toBeGreaterThanOrEqual(sibs.nodes.length);
    for (const s of siblings) expect(all.nodes.map((n) => n.name)).toContain(s);
    await $('//button[normalize-space(.)="Назад до всього родоводу"]').click();
    await $('h1=Увесь родовід').waitForDisplayed({ timeout: 30000 });
  });
});
