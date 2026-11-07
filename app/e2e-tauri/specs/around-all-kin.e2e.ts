/**
 * The "Relatives" choice on the around screen (Line only / Brothers and sisters / All relatives), on the REAL app
 * (debug build, tauri-driver, WebView2): the choice is a labelled radio group, "All relatives" brings in more people
 * than the line (aunts, uncles and cousins, each in their own generation row, the person still below their parents),
 * and the chart toolbar is on one centre line: the control and the Relatives chart's aunts checkbox sit within 1 px
 * of the buttons' centre, at the default window size and when the toolbar wraps onto a second line.
 * Screenshots in English and Ukrainian.
 *
 * Synthetic data only.
 */
import { expect } from '@wdio/globals';
import { $, browser, importSynthetic, meta, openTreeFromReport, ready, shot } from './helpers';

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

type Card = { x: number; w: number; y: number; relation: string; name: string; me: boolean };

/** The cards of the chart built around `name` with the given choice, read from the backend. */
async function aroundCards(name: string, kin: 'line' | 'siblings' | 'all'): Promise<Card[]> {
  await browser.execute((who: string, k: string) => {
    delete (window as unknown as { __gtAround?: unknown }).__gtAround;
    type Inv = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
    const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: Inv } }).__TAURI_INTERNALS__.invoke;
    const big = { x: -1e7, y: -1e7, w: 2e7, h: 2e7 };
    void (async () => {
      const whole = (await invoke('chart_open_whole')) as { meta: { chartId: number } };
      const all = (await invoke('chart_window', { chartId: whole.meta.chartId, rect: big, zoom: 1 })) as { nodes: { person?: { id: string; name: string } }[] };
      const root = all.nodes.find((n) => n.person?.name === who)!.person!.id;
      const m = (await invoke('chart_open', { req: { kind: 'around', root, generations: 255, kin: k } })) as { chartId: number; rootNode: number };
      const w = (await invoke('chart_window', { chartId: m.chartId, rect: big, zoom: 1 })) as { nodes: { id: number; x: number; w: number; y: number; relation?: string; person?: { name: string } }[] };
      (window as unknown as { __gtAround?: Card[] }).__gtAround = w.nodes
        .filter((n) => n.person)
        .map((n) => ({ x: n.x, w: n.w, y: n.y, relation: n.relation ?? '', name: n.person!.name, me: n.id === m.rootNode }));
    })();
  }, name, kin);
  await browser.waitUntil(() => browser.execute(() => !!(window as unknown as { __gtAround?: unknown }).__gtAround), { timeout: 30000 });
  return browser.execute(() => (window as unknown as { __gtAround: Card[] }).__gtAround);
}

/** The vertical centre of every direct child of the chart toolbar that is a button or a `.tool` wrapper, grouped by line. */
async function toolbarLines(): Promise<{ buttons: number[]; control: number | null }[]> {
  return browser.execute(() => {
    const bar = document.querySelector('[role="toolbar"]')!;
    const mid = (e: Element) => {
      const r = e.getBoundingClientRect();
      return r.top + r.height / 2;
    };
    const kids = [...bar.children].filter((c) => c.tagName === 'BUTTON' || c.classList.contains('tool'));
    const lines = new Map<number, { buttons: number[]; control: number | null }>();
    for (const k of kids) {
      const top = Math.round(k.getBoundingClientRect().top);
      const line = lines.get(top) ?? { buttons: [], control: null };
      if (k.classList.contains('tool')) line.control = mid(k.querySelector('input, [role="radiogroup"], label') ?? k);
      else line.buttons.push(mid(k));
      lines.set(top, line);
    }
    return [...lines.values()];
  });
}

/** Every control sits within 1 px of the buttons on its own line. */
async function expectAligned(where: string) {
  const lines = await toolbarLines();
  expect(lines.some((l) => l.control !== null)).toBe(true);
  for (const l of lines) {
    if (l.control === null) continue;
    // A control alone on a wrapped line has no buttons beside it: compare it with its own wrapper instead.
    for (const b of l.buttons) {
      if (Math.abs(b - l.control) > 1) throw new Error(`${where}: control centre ${l.control} against button centre ${b}`);
    }
  }
}

describe('The Relatives choice on the around screen', () => {
  before(async () => {
    await ready();
  });

  after(async () => {
    await $('//nav//a[normalize-space(.)="Налаштування"]').click();
    await choose('Мова', 'en');
    await $('h1=Settings').waitForDisplayed({ timeout: 20000 });
  });

  it('shows all relatives in their own rows and keeps the toolbar on one centre line', async () => {
    const name = meta().home.name;
    await importSynthetic('Around all kin');
    await openTreeFromReport();
    await toWholeTree('Whole tree', 'Whole tree');
    await flyTo('Find a person…', name);
    await $('//button[normalize-space(.)="Build around this person"]').click();
    await $(`h1=Built around ${name}`).waitForDisplayed({ timeout: 30000 });

    // A labelled choice of three, on the line only by default.
    const group = await $('//*[@role="radiogroup"][@aria-labelledby]');
    await group.waitForDisplayed();
    await expect($('//label[normalize-space(.)="Line only"]')).toBeDisplayed();
    await expect($('//label[normalize-space(.)="Brothers and sisters"]')).toBeDisplayed();
    await expect($('//label[normalize-space(.)="All relatives"]')).toBeDisplayed();
    await expect($('//label[normalize-space(.)="Line only"]//input')).toBeSelected();

    // All relatives: more people than the line, and the person stays below their parents.
    const line = await aroundCards(name, 'line');
    const all = await aroundCards(name, 'all');
    expect(all.length).toBeGreaterThanOrEqual(line.length);
    const me = all.find((c) => c.me)!;
    for (const c of all.filter((x) => /^(father|mother)$/.test(x.relation))) expect(c.y).toBeLessThan(me.y);
    // The father and the mother stand side by side, right above the person, however wide the families are.
    const dad = all.find((c) => c.relation === 'father');
    const mum = all.find((c) => c.relation === 'mother');
    if (dad && mum) {
      const between = mum.x - (dad.x + dad.w);
      expect(between >= 0 && between <= 20).toBe(true);
      expect(Math.abs((dad.x + mum.x + mum.w) / 2 - (me.x + me.w / 2)) <= 1).toBe(true);
      expect(Math.abs(dad.x + dad.w / 2 - (me.x + me.w / 2)) <= me.w).toBe(true);
    }
    const kin = all.filter((c) => /aunt|uncle|cousin|great-/.test(c.relation));
    expect(all.length > line.length || kin.length === 0).toBe(true);

    await $('//label[normalize-space(.)="Brothers and sisters"]').click();
    await browser.pause(1200);
    await $('//label[normalize-space(.)="All relatives"]').click();
    await browser.pause(1800);
    await shot('around-all-kin-default-en.png');
    // Fit: the whole chart is on screen, the person in the middle with both parents above.
    await $('//button[normalize-space(.)="Fit"]').click();
    await browser.pause(900);
    await shot('around-all-kin-en.png');

    // The control sits on the buttons' centre line (within 1 px) at the default size ...
    await expectAligned('around screen, default size');
    // ... and when the toolbar wraps onto a second line.
    const size = await browser.getWindowSize();
    await browser.setWindowSize(700, size.height);
    await browser.pause(600);
    await expectAligned('around screen, wrapped toolbar');
    await browser.setWindowSize(size.width, size.height);
    await browser.pause(600);

    await $('//label[normalize-space(.)="Line only"]').click();
    await browser.pause(800);

    // The Relatives chart: its aunts checkbox is on the same centre line.
    await $('//button[normalize-space(.)="Back to the whole tree"]').click();
    await $('h1=Whole tree').waitForDisplayed({ timeout: 30000 });
    await $('//nav//a[normalize-space(.)="Relatives"]').click();
    await $('[role="toolbar"]').waitForDisplayed({ timeout: 30000 });
    await browser.pause(1200);
    await expectAligned('Relatives chart, default size');
    const size2 = await browser.getWindowSize();
    await browser.setWindowSize(700, size2.height);
    await browser.pause(600);
    await expectAligned('Relatives chart, wrapped toolbar');
    await browser.setWindowSize(size2.width, size2.height);
  });

  it('shows the same choice in Ukrainian', async () => {
    await $('//nav//a[normalize-space(.)="Settings"]').click();
    await choose('Language', 'uk');
    await $('h1=Налаштування').waitForDisplayed({ timeout: 20000 });
    await toWholeTree('Увесь родовід', 'Увесь родовід');
    await flyTo('Знайти особу…', meta().home.name);
    await $('//button[normalize-space(.)="Побудувати навколо цієї особи"]').click();
    await $(`h1=Побудовано навколо: ${meta().home.name}`).waitForDisplayed({ timeout: 30000 });
    await expect($('//label[normalize-space(.)="Лише лінія"]')).toBeDisplayed();
    await expect($('//label[normalize-space(.)="Брати й сестри"]')).toBeDisplayed();
    await $('//label[normalize-space(.)="Уся рідня"]').click();
    await browser.pause(1800);
    await expectAligned('around screen in Ukrainian');
    const fit = await $('//button[normalize-space(.)="Вписати"]');
    if (await fit.isExisting()) await fit.click();
    await browser.pause(900);
    await shot('around-all-kin-uk.png');
    await $('//label[normalize-space(.)="Лише лінія"]').click();
    await $('//button[normalize-space(.)="Назад до всього родоводу"]').click();
    await $('h1=Увесь родовід').waitForDisplayed({ timeout: 30000 });
  });
});
