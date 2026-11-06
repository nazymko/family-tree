/**
 * A note on a family shows on the partner's page, says it is shared, and links to the family - on the REAL app.
 * Synthetic data only.
 */
import { expect } from '@wdio/globals';
import { $, browser, click, goTo, importSynthetic, meta, openTreeFromReport, ready, search, shot } from './helpers';

const NOTE = 'They moved to the village in spring.';

describe('Family notes on the person page', () => {
  before(async () => {
    await ready();
  });

  it('shows a note written on the family as a shared family note', async () => {
    await importSynthetic('Family notes');
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

    const heading = await $('//h3[contains(.,"Shared family notes")]');
    await heading.waitForDisplayed({ timeout: 20000 });
    await heading.scrollIntoView({ block: 'center' });
    await expect($(`//article//*[normalize-space(.)="${NOTE}"]`)).toBeDisplayed();
    await shot('family-notes-en.png');

    // The link next to the heading opens that family.
    await click('Open family');
    await $(dialog).waitForDisplayed({ timeout: 20000 });
    await browser.keys('Escape');
    await $(dialog).waitForDisplayed({ reverse: true, timeout: 20000 });

    // The Family screen shows the same note to read.
    await goTo('Family');
    const onFamily = await $('//h2[contains(.,"Shared family notes")]');
    await onFamily.waitForDisplayed({ timeout: 20000 });
    await onFamily.scrollIntoView({ block: 'center' });
    await expect($(`//article//*[normalize-space(.)="${NOTE}"]`)).toBeDisplayed();
    await shot('family-notes-read-en.png');
  });
});
