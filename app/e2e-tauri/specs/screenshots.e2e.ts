/**
 * Screenshots of the REAL app (not the mock): imports the synthetic 5k-person file through the test hook and walks the
 * main screens at 1440x900. PNGs go to $GT_E2E_SHOTS (default: a folder inside the run's temp dir).
 */
import path from 'node:path';
import { expect } from '@wdio/globals';
import {
  $,
  $$,
  browser,
  click,
  goTo,
  importSynthetic,
  makeJpegs,
  meta,
  openTreeFromReport,
  pushDialog,
  ready,
  search,
  shot,
  text,
  work,
} from './helpers';

const HITS = '[role="listbox"][aria-label="Search results"] [role="option"]';

/** Focuses the generation-3 person of the "spine" line (ancestors above, children below) through the search palette. */
async function focusHome() {
  await search(meta().home.name);
  await $(HITS).waitForDisplayed();
  await browser.keys('Enter');
  await $(`//h1[contains(.,"${meta().home.given}")]`).waitForDisplayed();
}

describe('Screenshots of the real app', () => {
  before(async () => {
    await ready();
  });

  it('import report', async () => {
    await importSynthetic('Screenshot tree');
    await browser.pause(600);
    await shot('real-import-report.png');
  });

  it('family, people, person', async () => {
    await openTreeFromReport();
    await $('h1').waitForDisplayed();
    await browser.pause(800);
    await shot('real-family.png');

    await goTo('People');
    await $('[data-filter-input]').waitForDisplayed();
    await browser.waitUntil(async () => (await $$('[role="option"]')).length > 5);
    await browser.pause(800);
    await shot('real-people.png');

    await focusHome();
    await browser.pause(600);
    await click('Edit person');
    await $('h1').waitForDisplayed();
    await browser.pause(1000);
    await shot('real-person.png');

    // The sign before the word for sex, in the person's own form.
    const sex = await $('//select[@id=//label[starts-with(normalize-space(.),"Sex")]/@for]');
    await sex.scrollIntoView({ block: 'center' });
    await browser.pause(300);
    expect(await sex.getText()).toContain('♂ Male');
    await shot('real-person-sex.png');
  });

  it('pedigree and descendants', async () => {
    await goTo('Pedigree');
    await $('h1=Pedigree').waitForDisplayed();
    await $('//*[@role="tab"][normalize-space(.)="Chart"]').click();
    await $('canvas[aria-label="Pedigree chart"]').waitForDisplayed();
    await browser.pause(2500);
    await shot('real-pedigree.png');

    await goTo('Descendants');
    await $('h1=Descendants').waitForDisplayed();
    await $('//*[@role="tab"][normalize-space(.)="Chart"]').click();
    await $('canvas[aria-label="Descendant chart"]').waitForDisplayed();
    await browser.pause(2500);
    await shot('real-descendants.png');
  });

  it('search with a CJK query', async () => {
    await goTo('Family');
    const cjk = meta().scripts.cjk.surname;
    await search(cjk);
    await browser.waitUntil(async () => (await $$(HITS)).length > 0, { timeoutMsg: 'no CJK search results' });
    await browser.pause(600);
    expect(await text()).toContain(cjk);
    await shot('real-search.png');
    await browser.keys('Escape');
  });

  it('media thumbnails through gtmedia', async () => {
    await goTo('Media');
    await $('h1=Media').waitForDisplayed();
    const files = await makeJpegs(path.join(work(), 'shots-media'), 2);
    await $('button=Add files…').waitForDisplayed();
    await pushDialog(files.join('|'));
    await click('Add files…');
    const thumbs = () => $$('ul[aria-label="Photos and documents"] img');
    await browser.waitUntil(async () => (await thumbs()).length >= 2, { timeoutMsg: 'thumbnails did not appear' });
    await browser.waitUntil(
      () =>
        browser.execute(
          () => Array.from(document.querySelectorAll<HTMLImageElement>('ul[aria-label="Photos and documents"] img')).filter((i) => i.naturalWidth > 0).length >= 2,
        ),
      { timeoutMsg: 'thumbnails did not load' },
    );
    // Let the toast settle, then take the picture.
    await browser.pause(800);
    await shot('real-media.png');
  });

  it('export preview', async () => {
    await click('Export');
    await $('[role="region"][aria-label="Privacy preview"]').waitForDisplayed();
    await $('//dt[normalize-space(.)="Shown as \'Living\'"]/following-sibling::dd').waitForDisplayed({ timeout: 30000 });
    await browser.pause(600);
    await shot('real-export.png');
    await click('Close');
  });
});
