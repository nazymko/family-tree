/**
 * "Arrange around this person" on the whole tree, on the REAL app (debug build, tauri-driver, WebView2): a person
 * is found (the chart flies to them and selects them), the toolbar button redraws the same chart around them, the
 * header names them, "Usual order" goes back and Back returns to the arrangement. Screenshots in English and Ukrainian.
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
