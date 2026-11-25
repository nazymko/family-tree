/**
 * The surname at birth shows under a name when the person goes by another surname - on the REAL app: the person page
 * (name, then "née/né Surname", then the dates) and a chart card. The case is made through the real "Surnames…" dialog
 * on the synthetic home person: a married surname becomes the main name, the birth surname stays under it.
 *
 * Synthetic data only.
 */
import { expect } from '@wdio/globals';
import { $, browser, click, goTo, importSynthetic, meta, openTreeFromReport, ready, search, shot } from './helpers';

const MARRIED = 'Bondar';
const BORN = /^(née|né) /;

describe('Surname at birth under the name', () => {
  before(async () => {
    await ready();
  });

  it('shows it on the person page and on the chart after the surname changed', async () => {
    await importSynthetic('Maiden name');
    await openTreeFromReport();
    await goTo('Family');
    await search(meta().home.name);
    await $('[role="listbox"] [role="option"]').waitForDisplayed();
    await browser.keys('Enter');
    await $(`//h1[contains(.,"${meta().home.given}")]`).waitForDisplayed();
    await click('Edit person');

    // Nothing yet: the person goes by the surname at birth.
    await $('h1').waitForDisplayed({ timeout: 20000 });
    expect(await $$maiden()).toBe(0);

    await click('Surnames…');
    const field = await $(`//input[@id=//label[starts-with(normalize-space(.),"Surname after marriage")]/@for]`);
    await field.waitForDisplayed({ timeout: 20000 });
    await field.setValue(MARRIED);
    // Ticked by default: a married surname being added becomes the main name.
    const box = await $(`//input[@id=//label[contains(.,"Show the surname after marriage as the main name")]/@for]`);
    expect(await box.isSelected()).toBe(true);
    await click('Save surnames');
    await $('//*[@role="dialog"]').waitForDisplayed({ reverse: true, timeout: 20000 });

    // The page: the new name, the birth surname on its own line under it, then the dates.
    const heading = await $(`//h1[contains(.,"${MARRIED}")]`);
    await heading.waitForDisplayed({ timeout: 20000 });
    const line = await $(`//h1/following-sibling::p[contains(@class,"maiden")]`);
    await line.waitForDisplayed({ timeout: 20000 });
    const text = (await line.getText()).trim();
    expect(text).toMatch(BORN);
    expect(text).toContain(meta().home.surname);
    await expect($('//h1/following-sibling::p[contains(@class,"maiden")]/following-sibling::p[contains(@class,"life")]')).toBeExisting();
    await shot('maiden-name-page-en.png');

    // A chart card: the same line, as a third row under the name.
    await goTo('Pedigree');
    await $('canvas').waitForDisplayed({ timeout: 30000 });
    await browser.waitUntil(async () => (await $$maiden()) > 0, { timeout: 20000, timeoutMsg: 'the chart never described the birth surname' });
    await browser.pause(600);
    await shot('maiden-name-chart-en.png');
  });
});

/** How many chart outline or screen-reader rows carry the birth surname line. */
async function $$maiden(): Promise<number> {
  return browser.execute(() => {
    const re = /(^|\s)(née|né) \S/;
    return Array.from(document.querySelectorAll('h1 ~ p, [role="tree"] *, [role="option"], .life')).filter((el) => re.test(el.textContent ?? '')).length;
  });
}
