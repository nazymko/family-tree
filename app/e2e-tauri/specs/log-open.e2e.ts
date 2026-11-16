/**
 * The change log's Open button takes the owner to the page that shows the record, scrolls to the record that changed,
 * marks it and moves keyboard focus to it: a note on a family, an event on a person page, a picture on a source - on
 * the REAL app. Synthetic data only.
 */
import { expect } from '@wdio/globals';
import path from 'node:path';
import { $, $$, browser, click, drop, goTo, importSynthetic, makeJpegs, meta, openTreeFromReport, ready, search, shot, work } from './helpers';

const NOTE = 'They moved to the village in spring.';

async function openHome() {
  await search(meta().home.name);
  await $('[role="listbox"] [role="option"]').waitForDisplayed();
  await browser.keys('Enter');
  await $(`//h1[contains(.,"${meta().home.given}")]`).waitForDisplayed();
}

async function openLog() {
  const log = await $('//nav//a[normalize-space(.)="Change log"]');
  await log.waitForClickable();
  await log.click();
  await $('h1=Change log').waitForDisplayed({ timeout: 20000 });
}

/** The marked record of the page, and whether keyboard focus is on it. */
async function revealed(attr: string) {
  const el = await $(`.revealed[${attr}]`);
  await el.waitForDisplayed({ timeout: 20000 });
  const focused = await browser.execute((a: string) => document.activeElement?.hasAttribute(a) ?? false, attr);
  return { el, focused };
}

describe('Change log: Open', () => {
  before(async () => {
    await ready();
  });

  it('opens the family page of a note written on a family', async () => {
    await importSynthetic('Log open');
    await openTreeFromReport();
    await goTo('Family');
    await search(meta().home.name);
    await $('[role="listbox"] [role="option"]').waitForDisplayed();
    await browser.keys('Enter');
    await $(`//h1[contains(.,"${meta().home.given}")]`).waitForDisplayed();
    await click('Edit person');
    await click('Edit family');
    const dialog = '//*[@role="dialog"]';
    const add = await $(`${dialog}//button[normalize-space(.)="Add note"]`);
    await add.waitForClickable({ timeout: 20000 });
    await add.click();
    const box = await $(`(${dialog})[last()]//textarea`);
    await box.waitForDisplayed({ timeout: 20000 });
    await box.setValue(NOTE);
    await click('Save note');
    await $(`${dialog}//*[normalize-space(.)="${NOTE}"]`).waitForDisplayed({ timeout: 20000 });
    await browser.keys('Escape');
    await $(dialog).waitForDisplayed({ reverse: true, timeout: 20000 });

    const log = await $('//nav//a[normalize-space(.)="Change log"]');
    await log.waitForClickable();
    await log.click();
    await $('h1=Change log').waitForDisplayed({ timeout: 20000 });
    const open = await $('//button[starts-with(@aria-label,"Open Note")]');
    await open.waitForDisplayed({ timeout: 20000 });
    await shot('log-open-en.png');
    await open.click();

    await $('//h1[starts-with(normalize-space(.),"Family:")]').waitForDisplayed({ timeout: 20000 });
    const note = await $(`//*[@data-note-id][.//*[normalize-space(.)="${NOTE}"]]`);
    await note.waitForDisplayed({ timeout: 20000 });
    await expect(note).toHaveElementClass('revealed');
    await shot('log-open-note-en.png');
  });

  it('opens the person page on the event row that changed, marked and focused', async () => {
    await openHome();
    await click('Edit person');
    const edit = await $(
      '//h2[normalize-space(.)="Events and facts"]/following::*[normalize-space(text())="Death"][1]/following::button[normalize-space(.)="Edit"][1]',
    );
    await edit.scrollIntoView({ block: 'center' });
    await edit.waitForClickable();
    await edit.click();
    const date = await $('//input[@id=//label[starts-with(normalize-space(.),"Date")]/@for]');
    await date.waitForDisplayed();
    await date.setValue('between 1704 and 1712');
    await click('Save event');
    await $('//*[@role="dialog"]').waitForDisplayed({ reverse: true, timeout: 20000 });

    await openLog();
    const open = await $('//button[starts-with(@aria-label,"Open Event")]');
    await open.waitForDisplayed({ timeout: 20000 });
    await open.click();
    await $(`//h1[contains(.,"${meta().home.given}")]`).waitForDisplayed({ timeout: 20000 });
    const { el, focused } = await revealed('data-event-id');
    expect(await el.getText()).toContain('Death');
    expect(focused).toBe(true);
    await shot('log-open-event-en.png');
  });

  it('opens the sources page on the picture added to a source, marked and focused', async () => {
    await goTo('Sources');
    await $('h1=Sources').waitForDisplayed({ timeout: 20000 });
    const first = await $('ul[aria-label="Sources"] button');
    await first.waitForDisplayed({ timeout: 20000 });
    await first.click();
    await $('button=Add files…').waitForDisplayed({ timeout: 20000 });
    const [jpg] = await makeJpegs(path.join(work(), 'log-open'), 1);
    await drop([jpg]);
    const thumbs = () => $$('ul[aria-label="Photos and documents"] img');
    await browser.waitUntil(async () => (await thumbs()).length >= 1, { timeoutMsg: 'the picture did not appear on the source' });

    await openLog();
    const open = await $('//button[starts-with(@aria-label,"Open Media")]');
    await open.waitForDisplayed({ timeout: 20000 });
    await open.click();
    await $('h1=Sources').waitForDisplayed({ timeout: 20000 });
    const { focused } = await revealed('data-media-id');
    expect(focused).toBe(true);
    await shot('log-open-media-en.png');
  });
});
