/**
 * The type of a source on the REAL app: create a source, set its type to the registry extract («Витяг з реєстру
 * (ЦНАП/РАЦС)» in Ukrainian), fill the issuer and number, save, open it again from another source, and filter the list by
 * the type. Screenshots of the editor and the list in English and Ukrainian.
 *
 * Synthetic data only.
 */
import { expect } from '@wdio/globals';
import { $, $$, browser, click, goTo, importSynthetic, openTreeFromReport, ready, shot } from './helpers';

const byLabel = (tag: 'select' | 'input', label: string) => $(`//${tag}[@id=//label[starts-with(normalize-space(.),"${label}")]/@for]`);
const inDialog = (tag: 'select' | 'input', label: string) =>
  $(`//*[@role="dialog"]//${tag}[@id=//label[starts-with(normalize-space(.),"${label}")]/@for]`);
const nav = (name: string) => $(`//nav//a[normalize-space(.)="${name}"]`);
const rows = () => $$('//ul[@aria-label="Sources" or @aria-label="Джерела"]/li');

describe('Type of a source', () => {
  before(async () => {
    await ready();
  });

  it('creates a source with a type and shows it in the list and on the page', async () => {
    await importSynthetic('Source kind tree');
    await openTreeFromReport();
    await goTo('Sources');
    await click('New source');
    await (await inDialog('input', 'Title')).setValue('Extract from the registry, Koval');
    await (await inDialog('select', 'Type of source')).selectByAttribute('value', 'civil_registry_extract');
    await (await inDialog('input', 'Issued by')).setValue('CNAP of Lviv');
    await (await inDialog('input', 'Document number')).setValue('No 12/34');
    await click('Add source');
    const row = await $('//ul[@aria-label="Sources"]/li[.//*[contains(.,"Extract from the registry, Koval")]]');
    await row.waitForDisplayed({ timeout: 20000 });
    expect(await row.getText()).toContain('Registry extract (CNAP/civil registry)');
    await browser.pause(300);
    await shot('source-kind-editor-en.png');
  });

  it('keeps the type after reopening and filters the list by it', async () => {
    // Away to another source and back: the form is read again from the saved source.
    const list = await rows();
    expect(list.length).toBeGreaterThan(1);
    const other = await $('//ul[@aria-label="Sources"]/li[not(.//*[contains(.,"Extract from the registry")])]//button');
    await other.click();
    const mine = await $('//ul[@aria-label="Sources"]/li[.//*[contains(.,"Extract from the registry, Koval")]]//button');
    await mine.click();
    await browser.waitUntil(async () => (await (await byLabel('select', 'Type of source')).getValue()) === 'civil_registry_extract', {
      timeoutMsg: 'the type was not kept',
    });
    expect(await (await byLabel('input', 'Issued by')).getValue()).toBe('CNAP of Lviv');
    expect(await (await byLabel('input', 'Document number')).getValue()).toBe('No 12/34');

    await (await byLabel('select', 'Filter by type')).selectByAttribute('value', 'civil_registry_extract');
    await browser.waitUntil(async () => (await rows()).length === 1, { timeoutMsg: 'the filter did not narrow the list to one source' });
    expect(await (await rows())[0].getText()).toContain('Extract from the registry, Koval');
    await browser.pause(300);
    await shot('source-kind-list-en.png');
  });

  it('reads in Ukrainian', async () => {
    await (await nav('Settings')).click();
    await (await byLabel('select', 'Language')).selectByAttribute('value', 'uk');
    await $('h1=Налаштування').waitForDisplayed({ timeout: 20000 });
    await (await nav('Джерела')).click();
    const row = await $('//ul[@aria-label="Джерела"]/li[.//*[contains(.,"Extract from the registry, Koval")]]');
    await row.waitForDisplayed({ timeout: 20000 });
    expect(await row.getText()).toContain('Витяг з реєстру (ЦНАП/РАЦС)');
    await (await row.$('button')).click();
    await browser.waitUntil(async () => (await (await byLabel('select', 'Тип джерела')).getValue()) === 'civil_registry_extract', {
      timeoutMsg: 'the editor did not show the type in Ukrainian',
    });
    await browser.pause(300);
    await shot('source-kind-list-uk.png');
    await (await byLabel('select', 'Фільтр за типом')).selectByAttribute('value', '');
    await browser.pause(300);
    await shot('source-kind-editor-uk.png');
  });

  after(async () => {
    await browser.keys('Escape');
    await browser.pause(300);
    await (await nav('Налаштування')).click();
    await (await byLabel('select', 'Мова')).selectByAttribute('value', 'en');
    await $('h1=Settings').waitForDisplayed({ timeout: 20000 });
  });
});
