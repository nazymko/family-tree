/**
 * Choosing several people on the whole tree and showing them in one tree, on the REAL app (debug build, tauri-driver,
 * WebView2): one person is marked with Space, the second with a real Shift+click on the canvas, "Show in Pedigree"
 * opens the combined tree, "Show all descendants" lays it out again, and the button in the header goes back.
 * Screenshots in English and Ukrainian.
 *
 * Synthetic data only.
 */
import { expect } from '@wdio/globals';
import { $, browser, click, importSynthetic, meta, openTreeFromReport, ready, shot } from './helpers';

const SHIFT = '';

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

/** Space on the selected card, from the chart: picking a search result leaves the focus in the search box. */
async function markWithSpace() {
  await browser.execute(() => (document.querySelector('canvas[role="application"]') as HTMLElement).focus());
  await browser.keys(' ');
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

/** Two different people of the synthetic tree: the home person and one of the others. */
function names(): { first: string; second: string } {
  const first = meta().home.name;
  const other = Object.values(meta().scripts).find((p) => `${p.given} ${p.surname}` !== first);
  if (!other) throw new Error('the synthetic tree has no second named person');
  return { first, second: `${other.given} ${other.surname}` };
}

describe('A tree of several chosen people', () => {
  before(async () => {
    await ready();
  });

  it('marks two people, shows them in one tree and adds all descendants', async () => {
    const { first, second } = names();

    await importSynthetic('Selection tree');
    await openTreeFromReport();
    await toWholeTree('Whole tree', 'Whole tree');

    await flyTo('Find a person…', first);
    await markWithSpace();
    await $('[data-testid="marked-count"]').waitForDisplayed();
    await expect($('[data-testid="marked-count"]')).toHaveText('1 person selected');

    await flyTo('Find a person…', second);
    await shiftClickCentre();
    await expect($('[data-testid="marked-count"]')).toHaveText('2 people selected');
    await shot('selection-marked-en.png');

    await $('aria/Show in Pedigree').click();
    await $('h1=Tree of selected people').waitForDisplayed({ timeout: 30000 });
    await expect($('[data-testid="chosen"]')).toHaveText(expect.stringContaining('2 people selected'));
    await $('canvas[role="application"]').waitForDisplayed();
    await browser.pause(800);
    await shot('selection-tree-en.png');

    const box = await $('//label[normalize-space(.)="Show all descendants"]');
    await box.click();
    await browser.pause(1200);
    await expect($('//input[@type="checkbox"][@id=//label[normalize-space(.)="Show all descendants"]/@for]')).toBeSelected();
    await shot('selection-descendants-en.png');

    await click('Back to the whole tree');
    await $('h1=Whole tree').waitForDisplayed({ timeout: 30000 });
  });

  it('shows the same page in Ukrainian', async () => {
    await $('//nav//a[normalize-space(.)="Settings"]').click();
    await choose('Language', 'uk');
    await $('h1=Налаштування').waitForDisplayed({ timeout: 20000 });
    await toWholeTree('Увесь родовід', 'Увесь родовід');
    await flyTo('Знайти особу…', names().first);
    await markWithSpace();
    await flyTo('Знайти особу…', names().second);
    await shiftClickCentre();
    await $('[data-testid="marked-count"]').waitForDisplayed();
    await shot('selection-marked-uk.png');
    await $('aria/Показати в родоводі').click();
    await $('h1=Родовід вибраних осіб').waitForDisplayed({ timeout: 30000 });
    await expect($('//label[normalize-space(.)="Показати всіх нащадків"]')).toBeDisplayed();
    await shot('selection-tree-uk.png');
  });
});
