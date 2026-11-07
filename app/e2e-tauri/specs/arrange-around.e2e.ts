/**
 * "Build around this person" on the whole tree, on the REAL app (debug build, tauri-driver, WebView2): a person is
 * found (the chart flies to them and selects them), the toolbar button opens a NEW screen built around them (their
 * name in the heading, a Back button), and the screen shows only their own line: no brothers or sisters, their
 * ancestors above and their descendants below (asserted on the layout the backend holds, not just on the camera).
 * Back returns to the whole tree. Screenshots in English and Ukrainian.
 *
 * Synthetic data only.
 */
import { expect } from '@wdio/globals';
import { $, browser, importSynthetic, meta, openTreeFromReport, ready, shot } from './helpers';

/** The select that belongs to a label, whatever language the label is in. */
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

/** Finds a person in the whole-tree search: the chart flies to them and selects them, their card in the middle of the canvas. */
async function flyTo(findLabel: string, name: string) {
  const box = await $(`//input[@role="combobox"][@id=//label[starts-with(normalize-space(.),"${findLabel}")]/@for]`);
  await box.waitForDisplayed();
  await box.setValue(name);
  // By name: for a moment the list still holds the results of the search before.
  const option = await $(`//ul[@role="listbox"][@aria-label="${findLabel}"]//*[@role="option"][contains(.,"${name}")]`);
  await option.waitForDisplayed({ timeout: 10000 });
  await option.click();
  // The camera flies for a third of a second.
  await browser.pause(900);
}

type Card = { y: number; relation: string; name: string; me: boolean };

/** The cards of the chart built around `name`, read from the backend through the same commands the page uses. */
async function aroundCards(name: string, siblings = false): Promise<Card[]> {
  await browser.execute((who: string, sibs: boolean) => {
    delete (window as unknown as { __gtAround?: unknown }).__gtAround;
    type Inv = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
    const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: Inv } }).__TAURI_INTERNALS__.invoke;
    const big = { x: -1e7, y: -1e7, w: 2e7, h: 2e7 };
    void (async () => {
      const whole = (await invoke('chart_open_whole')) as { meta: { chartId: number } };
      const all = (await invoke('chart_window', { chartId: whole.meta.chartId, rect: big, zoom: 1 })) as { nodes: { person?: { id: string; name: string } }[] };
      const root = all.nodes.find((n) => n.person?.name === who)!.person!.id;
      const m = (await invoke('chart_open', { req: { kind: 'around', root, generations: 255, siblings: sibs } })) as { chartId: number; rootNode: number };
      const w = (await invoke('chart_window', { chartId: m.chartId, rect: big, zoom: 1 })) as { nodes: { id: number; y: number; relation?: string; person?: { name: string } }[] };
      (window as unknown as { __gtAround?: Card[] }).__gtAround = w.nodes
        .filter((n) => n.person)
        .map((n) => ({ y: n.y, relation: n.relation ?? '', name: n.person!.name, me: n.id === m.rootNode }));
    })();
  }, name, siblings);
  await browser.waitUntil(() => browser.execute(() => !!(window as unknown as { __gtAround?: unknown }).__gtAround), { timeout: 30000 });
  return browser.execute(() => (window as unknown as { __gtAround: Card[] }).__gtAround);
}

describe('A tree built around one person', () => {
  before(async () => {
    await ready();
  });

  it('opens a new screen with only their own line and goes back to the whole tree', async () => {
    const name = meta().home.name;

    await importSynthetic('Around tree');
    await openTreeFromReport();
    await toWholeTree('Whole tree', 'Whole tree');

    await flyTo('Find a person…', name);
    await shot('arrange-before-en.png');
    const button = await $('//button[normalize-space(.)="Build around this person"]');
    await button.waitForEnabled({ timeout: 10000 });
    await button.click();

    // The new screen: named after them, with a Back button, and a chart.
    await $(`h1=Built around ${name}`).waitForDisplayed({ timeout: 30000 });
    await $('//button[normalize-space(.)="Back to the whole tree"]').waitForDisplayed();
    await $('canvas[role="application"]').waitForDisplayed();

    // Only their own line: nobody who is a brother, sister, aunt, uncle, cousin, nephew or niece; ancestors above, descendants below.
    const cards = await aroundCards(name);
    const me = cards.find((c) => c.me)!;
    expect(me.name).toBe(name);
    expect(cards.some((c) => /brother|sister|sibling|aunt|uncle|cousin|nephew|niece/.test(c.relation))).toBe(false);
    const above = cards.filter((c) => /father|mother|parent/.test(c.relation));
    const below = cards.filter((c) => /\b(son|daughter|child|grandson|granddaughter|grandchild)\b/.test(c.relation));
    expect(above.length + below.length).toBeGreaterThan(0);
    for (const c of above) expect(c.y).toBeLessThan(me.y);
    for (const c of below) expect(c.y).toBeGreaterThan(me.y);
    await browser.pause(1200);
    await shot('arrange-after-en.png');

    // The checkbox brings brothers and sisters in, and still no aunts, uncles or cousins.
    const withSiblings = await aroundCards(name, true);
    expect(withSiblings.some((c) => /brother|sister/.test(c.relation))).toBe(true);
    expect(withSiblings.some((c) => /aunt|uncle|cousin/.test(c.relation))).toBe(false);
    await $('//label[contains(normalize-space(.),"Brothers and sisters")]').click();
    await browser.pause(1500);
    await shot('arrange-siblings-en.png');
    await $('//label[contains(normalize-space(.),"Brothers and sisters")]').click();
    await browser.pause(800);

    // Back to the whole tree.
    await $('//button[normalize-space(.)="Back to the whole tree"]').click();
    await $('h1=Whole tree').waitForDisplayed({ timeout: 30000 });
    await expect($('h1*=Built around')).not.toExist();
  });

  it('shows the same screen in Ukrainian', async () => {
    await $('//nav//a[normalize-space(.)="Settings"]').click();
    await choose('Language', 'uk');
    await $('h1=Налаштування').waitForDisplayed({ timeout: 20000 });
    await toWholeTree('Увесь родовід', 'Увесь родовід');
    await flyTo('Знайти особу…', meta().home.name);
    const button = await $('//button[normalize-space(.)="Побудувати навколо цієї особи"]');
    await button.waitForEnabled({ timeout: 10000 });
    await button.click();
    await $(`h1=Побудовано навколо: ${meta().home.name}`).waitForDisplayed({ timeout: 30000 });
    await expect($('//button[normalize-space(.)="Назад до всього родоводу"]')).toBeDisplayed();
    await browser.pause(1200);
    await shot('arrange-after-uk.png');
    await $('//button[normalize-space(.)="Назад до всього родоводу"]').click();
    await $('h1=Увесь родовід').waitForDisplayed({ timeout: 30000 });
  });
});
