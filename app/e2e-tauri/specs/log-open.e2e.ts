/**
 * The change log's Open button takes the owner to the page that shows the record: a note on a family opens the
 * family page - on the REAL app. Synthetic data only.
 */
import { expect } from '@wdio/globals';
import { $, browser, click, goTo, importSynthetic, meta, openTreeFromReport, ready, search, shot } from './helpers';

const NOTE = 'They moved to the village in spring.';

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
});
