/**
 * The person page under the name card: the "Go to" block (charts, families, sections) and the Add menu, in English and
 * Ukrainian, at the normal and a narrow window width. Screenshots go to $GT_E2E_SHOTS.
 *
 * Synthetic data only.
 */
import { expect } from '@wdio/globals';
import { $, browser, click, expectNoWindowScroll, goTo, importSynthetic, meta, openTreeFromReport, ready, search, shot } from './helpers';

const nav = (name: string) => $(`//nav//a[normalize-space(.)="${name}"]`);

async function choose(label: string, value: string) {
  const select = await $(`//select[@id=//label[starts-with(normalize-space(.),"${label}")]/@for]`);
  await select.waitForDisplayed();
  await select.selectByAttribute('value', value);
}

describe('Person page quick navigation', () => {
  before(async () => {
    await ready();
  });

  it('shows the Go to block and the Add menu in both languages and a narrow window', async () => {
    await importSynthetic('Person nav');
    await openTreeFromReport();
    await goTo('Family');
    await search(meta().home.name);
    await $('[role="listbox"] [role="option"]').waitForDisplayed();
    await browser.keys('Enter');
    await $(`//h1[contains(.,"${meta().home.given}")]`).waitForDisplayed();
    await click('Edit person');
    await $('nav[aria-label="Go to"]').waitForDisplayed({ timeout: 20000 });
    for (const g of ['Charts', 'Families', 'Sections']) await $(`//nav[@aria-label="Go to"]//*[normalize-space(.)="${g}"]`).waitForDisplayed();
    await browser.pause(400);
    await shot('person-nav-en.png');

    await browser.setWindowSize(800, 900);
    await browser.pause(400);
    await expectNoWindowScroll('person page, narrow');
    await click('Add');
    await $('[role="menu"]').waitForDisplayed();
    await browser.pause(300);
    await shot('person-nav-en-narrow-add-menu.png');
    await browser.keys('Escape');
    await browser.setWindowSize(1440, 900);

    await goTo('Settings');
    await choose('Language', 'uk');
    await $('h1=Налаштування').waitForDisplayed({ timeout: 20000 });
    const family = await nav('Родина');
    await family.click();
    await click('Змінити особу');
    await $('//nav[@aria-label="Перейти до"]').waitForExist({ timeout: 20000 });
    await browser.pause(400);
    await shot('person-nav-uk.png');

    await browser.setWindowSize(800, 900);
    await browser.pause(400);
    await expectNoWindowScroll('person page uk, narrow');
    await shot('person-nav-uk-narrow.png');
    expect(await $('//nav[@aria-label="Перейти до"]//*[normalize-space(.)="Схеми"]').isExisting()).toBe(true);
  });
});
