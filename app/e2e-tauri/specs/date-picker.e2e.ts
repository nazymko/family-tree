/**
 * The date picker on a death date, on the REAL app (debug build, tauri-driver, WebView2), so against the real date
 * parser rather than the mock: a range is picked, shows in the raw text, is understood and saved; typing in the raw text
 * still works and the picker follows it. Screenshots in English and Ukrainian.
 *
 * Synthetic data only.
 */
import { expect } from '@wdio/globals';
import { $, $$, browser, click, importSynthetic, meta, openTreeFromReport, ready, search, shot, text } from './helpers';

const select = (label: string) => $(`//select[@id=//label[starts-with(normalize-space(.),"${label}")]/@for]`);
const dateBox = (label: string) => $(`//input[@id=//label[starts-with(normalize-space(.),"${label}")]/@for]`);
const nav = (name: string) => $(`//nav//a[normalize-space(.)="${name}"]`);

async function openHome() {
  await search(meta().home.name);
  await $('[role="listbox"] [role="option"]').waitForDisplayed();
  await browser.keys('Enter');
  await $(`//h1[contains(.,"${meta().home.given}")]`).waitForDisplayed();
}

/** The Edit button in the row of this event on the person page. */
async function editEvent(kind: string, edit: string) {
  const b = await $(`//*[normalize-space(text())="${kind}"]/ancestor::*[.//button[normalize-space(.)="${edit}"]][1]//button[normalize-space(.)="${edit}"]`);
  await b.scrollIntoView({ block: 'center' });
  await b.waitForClickable();
  await b.click();
}

describe('Date picker on a death date', () => {
  before(async () => {
    await ready();
  });

  it('picks a range, shows it in the raw text and saves it', async () => {
    await importSynthetic('Date tree');
    await openTreeFromReport();
    await openHome();
    await click('Edit person');
    await editEvent('Death', 'Edit');

    await click('Pick a date or a range');
    await (await select('Kind of date')).selectByAttribute('value', 'between');
    const years = await $$(`//input[@id=//label[normalize-space(.)="Year"]/@for]`);
    expect(years.length).toBe(2);
    await years[0].setValue('1704');
    await years[1].setValue('1712');

    await browser.waitUntil(async () => (await (await dateBox('Date')).getValue()) === 'between 1704 and 1712', { timeoutMsg: 'the raw text did not take the range' });
    await $('//*[@role="status"][contains(.,"Understood as: between 1704 and 1712 (1704 to 1712)")]').waitForDisplayed({ timeout: 10000 });
    await browser.pause(300);
    await shot('date-picker-death-en.png');

    await click('Save event');
    await browser.waitUntil(async () => (await text()).includes('between 1704 and 1712'), { timeoutMsg: 'the saved range is not on the person page' });
  });

  it('keeps typing in the raw text working, and the picker follows it', async () => {
    await editEvent('Death', 'Edit');
    const box = await dateBox('Date');
    await browser.waitUntil(async () => (await box.getValue()) === 'between 1704 and 1712', { timeoutMsg: 'opening the editor changed the stored value' });

    await click('Pick a date or a range');
    await box.setValue('about 1800');
    await $('//*[@role="status"][contains(.,"Understood as: about 1800")]').waitForDisplayed({ timeout: 10000 });
    await browser.waitUntil(async () => (await (await select('Kind of date')).getValue()) === 'about', { timeoutMsg: 'the picker did not follow the typed text' });

    await box.setValue('the winter of the big flood');
    await browser.pause(300);
    expect(await box.getValue()).toBe('the winter of the big flood');

    await box.setValue('between 1704 and 1712');
    await click('Save event');
  });

  it('reads in Ukrainian', async () => {
    await (await nav('Settings')).click();
    await (await select('Language')).selectByAttribute('value', 'uk');
    await $('h1=Налаштування').waitForDisplayed({ timeout: 20000 });
    await (await nav('Родина')).click();
    await openHome();
    await click('Змінити особу');
    await browser.waitUntil(async () => (await text()).includes('між 1704 і 1712'), { timeoutMsg: 'the saved range is not in Ukrainian on the person page' });
    await editEvent('Смерть', 'Змінити');
    await click('Вибрати дату або проміжок');
    await $('//*[@role="status"][contains(.,"Розпізнано як: між 1704 і 1712 (1704 – 1712)")]').waitForDisplayed({ timeout: 10000 });
    await browser.pause(300);
    await shot('date-picker-death-uk.png');
    await click('Скасувати');
  });

  after(async () => {
    await (await nav('Налаштування')).click();
    await (await select('Мова')).selectByAttribute('value', 'en');
    await $('h1=Settings').waitForDisplayed({ timeout: 20000 });
  });
});
