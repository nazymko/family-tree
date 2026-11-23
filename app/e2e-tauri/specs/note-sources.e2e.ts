/**
 * A research note keeps its archive references in the story view, a written story reads clean, and the person's
 * citations show under the note - on the REAL app. Synthetic data only.
 */
import { expect } from '@wdio/globals';
import { $, browser, click, goTo, importSynthetic, meta, openTreeFromReport, ready, search, shot } from './helpers';

const RESEARCH = 'Born 1890. Архів 1, ф. 1, оп. 2, спр. 3, PDF-с. 10, ч. 5 (1-2-3)';
const STORY = 'A written retelling of the family story.';

describe('Note sources on the person page', () => {
  before(async () => {
    await ready();
  });

  it('keeps the references of a research note and lists its citations', async () => {
    await importSynthetic('Note sources');
    await openTreeFromReport();
    await goTo('Family');
    await search(meta().home.name);
    await $('[role="listbox"] [role="option"]').waitForDisplayed();
    await browser.keys('Enter');
    await $(`//h1[contains(.,"${meta().home.given}")]`).waitForDisplayed();
    await click('Edit person');

    await click('Add note');
    const box = await $('textarea');
    await box.waitForDisplayed({ timeout: 20000 });
    await box.setValue(RESEARCH);
    await click('Save note');
    await $('textarea').waitForDisplayed({ reverse: true, timeout: 20000 });

    await click('Add note');
    const boxes = await $$('textarea');
    await boxes[0].waitForDisplayed({ timeout: 20000 });
    await boxes[0].setValue('Second note, with a story (PDF с. 29).');
    await boxes[1].setValue(STORY);
    await click('Save note');
    await $('textarea').waitForDisplayed({ reverse: true, timeout: 20000 });

    await click('Add citation');
    const src = await $('//input[@id=//label[starts-with(normalize-space(.),"Source")]/@for]');
    await src.waitForDisplayed({ timeout: 20000 });
    await src.setValue('Архів 1, метрична книга 1-2-3');
    const create = await $('//*[contains(normalize-space(.),"Create source")][@role="option"]');
    await create.waitForClickable({ timeout: 20000 });
    await create.click();
    await click('Save citation');
    await $('//*[@data-testid="note-citations"]').waitForDisplayed({ timeout: 20000 });

    await click('Story');
    const heading = await $('//h2[contains(.,"Notes")]');
    await heading.scrollIntoView({ block: 'start' });
    await expect($(`//article//*[contains(.,"PDF-с. 10")]`)).toBeDisplayed();
    await expect($(`//article//*[normalize-space(.)="${STORY}"]`)).toBeDisplayed();
    await shot('note-sources-after.png');
  });
});
