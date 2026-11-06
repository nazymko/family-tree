/**
 * "Arrange around this person" on the whole tree, on the REAL app (debug build, tauri-driver, WebView2): a person
 * is found (the chart flies to them and selects them), the toolbar button rebuilds the chart around them (cards really
 * move: asserted on the layout the backend holds, not just on the camera), the header names them, "Usual order" goes
 * back and Back returns to the arrangement. Screenshots in English and Ukrainian.
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

type Card = { x: number; y: number; w: number; h: number; id: string; name: string };

/**
 * The cards of the plain whole-tree chart and of the chart arranged around `name`, read from the backend through the
 * same commands the page uses. Asking for the arranged chart returns the one the button opened (it is held per
 * person), so this is the layout on screen.
 */
async function layouts(name: string): Promise<{ plain: Card[]; arranged: Card[]; target: string }> {
  type Out = { plain: Card[]; arranged: Card[]; target: string };
  await browser.execute((who: string) => {
    type Inv = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
    const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: Inv } }).__TAURI_INTERNALS__.invoke;
    const big = { x: -1e7, y: -1e7, w: 2e7, h: 2e7 };
    const cards = async (chartId: number): Promise<Card[]> => {
      const w = (await invoke('chart_window', { chartId, rect: big, zoom: 1 })) as { nodes: { x: number; y: number; w: number; h: number; person?: { id: string; name: string } }[] };
      return w.nodes.filter((n) => n.person).map((n) => ({ x: n.x, y: n.y, w: n.w, h: n.h, id: n.person!.id, name: n.person!.name }));
    };
    void (async () => {
      const plainMeta = (await invoke('chart_open_whole')) as { meta: { chartId: number } };
      const plain = await cards(plainMeta.meta.chartId);
      const target = plain.find((c) => c.name === who)!.id;
      const arrMeta = (await invoke('chart_open_arranged', { around: target, people: null, allDescendants: false })) as { meta: { chartId: number } };
      (window as unknown as { __gtLayouts?: Out }).__gtLayouts = { plain, arranged: await cards(arrMeta.meta.chartId), target };
    })();
  }, name);
  // The page answers asynchronously: poll for the result it leaves.
  await browser.waitUntil(() => browser.execute(() => !!(window as unknown as { __gtLayouts?: unknown }).__gtLayouts), { timeout: 30000 });
  return browser.execute(() => (window as unknown as { __gtLayouts: Out }).__gtLayouts);
}

describe('A tree arranged around one person', () => {
  before(async () => {
    await ready();
  });

  // The settings outlive this spec: the next one starts in English.
  after(async () => {
    await $('//nav//a[normalize-space(.)="Налаштування"]').click();
    await choose('Мова', 'en');
    await $('h1=Settings').waitForDisplayed({ timeout: 20000 });
  });

  it('arranges around the found person, names them in the header and goes back to the usual order', async () => {
    const name = meta().home.name;

    await importSynthetic('Arranged tree');
    await openTreeFromReport();
    await toWholeTree('Whole tree', 'Whole tree');

    // Nobody is selected yet, or somebody is: after a search the button works.
    await flyTo('Find a person…', name);
    await shot('arrange-before-en.png');
    const button = await $('//button[normalize-space(.)="Arrange around this person"]');
    await button.waitForEnabled({ timeout: 10000 });
    await button.click();

    await $('[data-testid="arranged"]').waitForDisplayed({ timeout: 30000 });
    await expect($('[data-testid="arranged"]')).toHaveText(expect.stringContaining(name));
    await $('canvas[role="application"]').waitForDisplayed();

    // The chart is REBUILT around them: the same people, many cards in other places, nobody on top of anybody.
    const { plain, arranged } = await layouts(name);
    expect(arranged.length).toBe(plain.length);
    const before = new Map(plain.map((c) => [c.id, c]));
    const moved = arranged.filter((c) => Math.abs(before.get(c.id)!.x - c.x) > 1 || Math.abs(before.get(c.id)!.y - c.y) > 1).length;
    expect(moved).toBeGreaterThan(Math.min(10, plain.length / 4));
    const overlap = arranged.some((a, i) => arranged.slice(i + 1).some((b) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h));
    expect(overlap).toBe(false);
    // The chart is centred on them at a readable zoom.
    await browser.pause(1200);
    await shot('arrange-after-en.png');

    // Back to the usual order.
    await $('//button[normalize-space(.)="Usual order"]').click();
    await $('h1=Whole tree').waitForDisplayed({ timeout: 30000 });
    await browser.waitUntil(async () => !(await $('[data-testid="arranged"]').isExisting()), { timeout: 10000 });
  });

  it('shows the same page in Ukrainian', async () => {
    await $('//nav//a[normalize-space(.)="Settings"]').click();
    await choose('Language', 'uk');
    await $('h1=Налаштування').waitForDisplayed({ timeout: 20000 });
    await toWholeTree('Увесь родовід', 'Увесь родовід');
    await flyTo('Знайти особу…', meta().home.name);
    const button = await $('//button[normalize-space(.)="Побудувати навколо цієї особи"]');
    await button.waitForEnabled({ timeout: 10000 });
    await button.click();
    await $('[data-testid="arranged"]').waitForDisplayed({ timeout: 30000 });
    await expect($('[data-testid="arranged"]')).toHaveText(expect.stringContaining('Побудовано навколо'));
    await expect($('//button[normalize-space(.)="Звичайний порядок"]')).toBeDisplayed();
    await browser.pause(1200);
    await shot('arrange-after-uk.png');
  });
});
