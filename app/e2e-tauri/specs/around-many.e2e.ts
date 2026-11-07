/**
 * "Build around these people" on the REAL app (debug build, tauri-driver, WebView2): two people are marked on the whole
 * tree (Space on the first, a real Shift+click on the second), the button counts them, the around screen names both in
 * its heading, draws both cards, lists both at the top of the «Outline» tab, and Back returns to the whole tree with the
 * two people still marked. Screenshots in English and Ukrainian.
 *
 * Synthetic data only.
 */
import { expect } from '@wdio/globals';
import { $, browser, importSynthetic, meta, openTreeFromReport, ready, shot } from './helpers';

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

function names(): { first: string; second: string } {
  const first = meta().home.name;
  const other = Object.values(meta().scripts).find((p) => `${p.given} ${p.surname}` !== first);
  if (!other) throw new Error('the synthetic tree has no second named person');
  return { first, second: `${other.given} ${other.surname}` };
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
async function cardNames(first: string, second: string): Promise<{ chosen: number; names: string[] }> {
  await browser.execute((a: string, b: string) => {
    delete (window as unknown as { __gtMany?: unknown }).__gtMany;
    type Inv = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
    const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: Inv } }).__TAURI_INTERNALS__.invoke;
    const big = { x: -1e7, y: -1e7, w: 2e7, h: 2e7 };
    void (async () => {
      const whole = (await invoke('chart_open_whole')) as { meta: { chartId: number } };
      const all = (await invoke('chart_window', { chartId: whole.meta.chartId, rect: big, zoom: 1 })) as { nodes: { person?: { id: string; name: string } }[] };
      const id = (n: string) => all.nodes.find((x) => x.person?.name === n)!.person!.id;
      const roots = [id(a), id(b)];
      const m = (await invoke('chart_open', { req: { kind: 'around', root: roots[0], generations: 255, kin: 'line', roots } })) as { chartId: number; chosenNodes?: number[] };
      const w = (await invoke('chart_window', { chartId: m.chartId, rect: big, zoom: 1 })) as { nodes: { person?: { name: string } }[] };
      (window as unknown as { __gtMany?: unknown }).__gtMany = { chosen: m.chosenNodes?.length ?? 0, names: w.nodes.filter((n) => n.person).map((n) => n.person!.name) };
    })();
  }, first, second);
  await browser.waitUntil(() => browser.execute(() => !!(window as unknown as { __gtMany?: unknown }).__gtMany), { timeout: 30000 });
  return browser.execute(() => (window as unknown as { __gtMany: { chosen: number; names: string[] } }).__gtMany);
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
    const { first, second } = names();
    await importSynthetic('Around many');
    await openTreeFromReport();
    await toWholeTree('Whole tree', 'Whole tree');
    await markBoth('Find a person…', first, second);
    await expect($('[data-testid="marked-count"]')).toHaveText('2 people selected');
    await $('//button[normalize-space(.)="Build around these people (2)"]').waitForDisplayed();

    await $('//button[normalize-space(.)="Build around these people (2)"]').click();
    await $(`//h1[contains(.,"Built around:")][contains(.,"${first}")][contains(.,"${second}")]`).waitForDisplayed({ timeout: 30000 });
    await $('canvas[role="application"]').waitForDisplayed();
    await browser.pause(1200);

    // Both cards are in the chart, and the wider kin choices are shown but cannot be picked.
    const cards = await cardNames(first, second);
    expect(cards.chosen).toBe(2);
    expect(cards.names).toContain(first);
    expect(cards.names).toContain(second);
    await expect($('//label[normalize-space(.)="Line only"]//input')).toBeSelected();
    await expect($('//label[normalize-space(.)="All relatives"]//input')).toBeDisabled();
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
    const { first, second } = names();
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
    await $('//*[@role="tab"][normalize-space(.)="Список"]').click();
    await expectTopRows(first, second);
    await $('//*[@role="tab"][normalize-space(.)="Схема"]').click();
    await $('//button[normalize-space(.)="Назад до всього родоводу"]').click();
    await $('h1=Увесь родовід').waitForDisplayed({ timeout: 30000 });
  });
});
