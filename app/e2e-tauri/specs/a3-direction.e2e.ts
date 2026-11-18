/**
 * The A3 print keeps the chart's direction as on screen, on the REAL app: the pedigree is turned to "ancestors on top"
 * (its default is on the right), saved for A3 as a PNG, and the file is checked to be a whole A3 page. The picture menu
 * then shows the direction choice in Ukrainian, «Напрямок: як на екрані» ticked. Screenshots go to $GT_E2E_SHOTS
 * (a3-direction-menu-uk.png, a3-direction-saved-top.png: the saved page itself).
 *
 * Synthetic data only.
 */
import fs from 'node:fs';
import path from 'node:path';
import { expect } from '@wdio/globals';
import { $, browser, goTo, importSynthetic, openTreeFromReport, pushDialog, ready, shot, slash } from './helpers';

/** Width and height of a PNG, from its IHDR chunk. */
function pngSize(file: string): { w: number; h: number } {
  const b = fs.readFileSync(file);
  return { w: b.readUInt32BE(16), h: b.readUInt32BE(20) };
}

async function openPictureMenu(label: string) {
  const b = await $(`//button[normalize-space(.)="${label}"]`);
  await b.waitForClickable({ timeout: 20000 });
  await b.click();
  await $('[role="menu"]').waitForDisplayed({ timeout: 10000 });
}

describe('A3 print direction', () => {
  before(async () => {
    await ready();
    await importSynthetic('A3 direction');
    await openTreeFromReport();
  });

  it('saves_the_a3_page_in_the_direction_on_screen_and_offers_best_fit', async () => {
    await goTo('Pedigree');
    await $('canvas[aria-label="Pedigree chart"]').waitForDisplayed({ timeout: 30000 });
    // Ancestors on top: not the pedigree's default (right).
    const group = await $('[role="radiogroup"][aria-label="Where the ancestors are"]');
    const top = await group.$('input[aria-label="Ancestors on top"]');
    await (await top.parentElement()).click();
    await browser.waitUntil(() => top.isSelected(), { timeout: 10000 });
    await browser.pause(700);

    await openPictureMenu('Save as picture');
    const screenItem = await $('//*[@role="menuitemradio"][normalize-space(.)="Direction: as on screen"]');
    expect(await screenItem.getAttribute('aria-checked')).toBe('true');
    const dir = process.env.GT_E2E_SHOTS!;
    fs.mkdirSync(dir, { recursive: true });
    const out = path.join(dir, 'a3-direction-saved-top.png');
    fs.rmSync(out, { force: true });
    await pushDialog(slash(out));
    await $('//*[@role="menuitem"][normalize-space(.)="Save for A3 print…"]').click();
    await browser.waitUntil(() => fs.existsSync(out) && fs.statSync(out).size > 1000, { timeout: 120000, timeoutMsg: 'the A3 PNG was not saved' });
    await browser.pause(500);
    const size = pngSize(out);
    // A whole A3 page at 300 dpi, either way round.
    expect([size.w, size.h].sort((a, b) => a - b)).toEqual([3508, 4961]);

    // The menu in Ukrainian, with the direction choice.
    await goTo('Settings');
    const select = await $('//select[option[@value="zh-Hans"]]');
    await select.waitForDisplayed();
    await select.selectByAttribute('value', 'uk');
    await browser.waitUntil(async () => (await browser.execute(() => document.documentElement.lang)) === 'uk', { timeout: 10000 });
    const ped = await $('//nav[@aria-label="Головне меню" or @aria-label="Main"]//a[normalize-space(.)="Родовід"]');
    await ped.waitForClickable({ timeout: 10000 });
    await ped.click();
    await $('canvas').waitForDisplayed({ timeout: 30000 });
    await browser.pause(700);
    await openPictureMenu('Зберегти як зображення');
    await $('//*[@role="menuitemradio"][normalize-space(.)="Напрямок: як на екрані"]').waitForDisplayed();
    await $('//*[@role="menuitemradio"][normalize-space(.)="Напрямок: найкраще вміщення"]').waitForDisplayed();
    await shot('a3-direction-menu-uk.png');
    await browser.keys('Escape');
  });
});
