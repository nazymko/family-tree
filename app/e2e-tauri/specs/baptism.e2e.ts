/**
 * A baptism with its godparents on the REAL app: added from the person page through the event editor (one godparent from
 * the family tree, one by name as a metric book writes it), shown on the card line under the birth, and the linked
 * godparent's own page listing the godchild.
 *
 * Synthetic data only; the written godparent is invented.
 */
import { expect } from '@wdio/globals';
import { $, browser, click, goTo, importSynthetic, meta, openTreeFromReport, ready, search, shot } from './helpers';

const WRITTEN = 'Hanna Ostapivna Bondar, a widow of Ivanivka';

const fieldFor = (label: string) => $(`//input[@id=//label[starts-with(normalize-space(.),"${label}")]/@for]`);

describe('Baptism and godparents', () => {
  before(async () => {
    await ready();
  });

  it('adds a baptism with godparents and shows them on the card and on the godparent page', async () => {
    await importSynthetic('Baptism');
    await openTreeFromReport();
    await goTo('Family');
    await search(meta().home.name);
    await $('[role="listbox"] [role="option"]').waitForDisplayed();
    await browser.keys('Enter');
    await $(`//h1[contains(.,"${meta().home.given}")]`).waitForDisplayed();
    await click('Edit person');
    await $('h1').waitForDisplayed({ timeout: 20000 });

    // Baptized a few days after the birth year the page shows.
    const born = ((await $('h1 ~ p.life').getText()).match(/\d{4}/) ?? ['1906'])[0];
    await click('Add baptism');
    const year = await fieldFor('Year');
    await year.waitForDisplayed({ timeout: 20000 });
    await year.setValue(born);
    await $(`//select[@id=//label[starts-with(normalize-space(.),"Month")]/@for]`).selectByAttribute('value', '2');
    await $(`//select[@id=//label[starts-with(normalize-space(.),"Day")]/@for]`).selectByAttribute('value', '6');

    // A godparent from the family tree.
    const picker = await fieldFor('Add a godparent from the family tree');
    await picker.scrollIntoView({ block: 'center' });
    await picker.setValue(meta().home.surname);
    const option = await $('[data-godparents] [role="listbox"] [role="option"], [role="listbox"] [role="option"]');
    await option.waitForDisplayed({ timeout: 20000 });
    await option.click();
    await $('[data-godparents] [data-godparent]').waitForDisplayed({ timeout: 20000 });

    // A godparent by name, as the record writes it.
    await click('Add a godparent by name');
    const written = await fieldFor('Name of godparent 2');
    await written.waitForDisplayed();
    await written.setValue(WRITTEN);
    await $(`//select[@id=//label[starts-with(normalize-space(.),"Role of Hanna Ostapivna Bondar")]/@for]`).selectByAttribute('value', 'godmother');
    await (await fieldFor('Note on Hanna Ostapivna Bondar')).setValue('signed with a cross');
    await $('[data-godparents]').scrollIntoView({ block: 'center' });
    await browser.pause(300);
    await shot('baptism-event-editor-en.png');

    await click('Save event');
    await $('//*[@role="dialog"]').waitForDisplayed({ reverse: true, timeout: 20000 });

    // The card: "bap. 6 Feb 1906, Ivanivka" after the birth, the godparents on the next line.
    const bap = await $('[data-baptism]');
    await bap.waitForDisplayed({ timeout: 20000 });
    expect(await bap.getText()).toContain(born);
    const line = await $('p[data-godparents]');
    expect(await line.getText()).toContain('Hanna Ostapivna Bondar');
    expect(await line.getText()).not.toContain('widow');
    await $('h1').scrollIntoView({ block: 'start' });
    await browser.pause(300);
    await shot('baptism-card-header-en.png');

    // The linked godparent's page lists the godchild.
    await (await line.$('button.godlink')).click();
    const section = await $('[data-godchildren]');
    await section.waitForDisplayed({ timeout: 20000 });
    expect(await section.getText()).toContain(meta().home.given);
    await section.scrollIntoView({ block: 'start' });
    await browser.pause(300);
    await shot('baptism-godchildren-en.png');
  });
});
