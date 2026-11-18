/**
 * Screenshots of the Map view in the REAL app, with the invented family of scripts/make-maps-fixture.py:
 * the three modes and the place-coordinates helper (the place data comes from a local server standing in for GeoNames,
 * through the debug-only GT_GAZETTEER_BASE). Run by scripts/maps-shots.mjs; PNGs go to $GT_E2E_SHOTS.
 */
import { expect } from '@wdio/globals';
import { $, browser, click, importSynthetic, openTreeFromReport, ready, search, shot, text } from './helpers';

async function openMap() {
  const a = await $('//nav[@aria-label="Main"]//a[normalize-space(.)="Map"]');
  await a.waitForClickable();
  await a.click();
  await $('canvas').waitForDisplayed({ timeout: 20000 });
  await browser.pause(1500); // the basemap decodes, the first frame draws
}

async function mode(name: string) {
  const r = await $(`//label[normalize-space(.)="${name}"]`);
  await r.click();
  await browser.pause(800);
}

async function choose(label: string, option: string) {
  const sel = await $(`//select[@id=//label[starts-with(normalize-space(.),"${label}")]/@for]`);
  await sel.selectByVisibleText(option);
  await browser.pause(800);
}

describe('Map view', () => {
  before(async () => {
    await ready();
    await importSynthetic('Maps');
    await openTreeFromReport();
  });

  it('shows places, heat and movement, then the coordinates helper', async () => {
    await search('Ivan Petrovych Koval');
    await $('[role="listbox"] [role="option"]').waitForDisplayed();
    await browser.keys('Enter');
    await $('//h1[contains(.,"Ivan")]').waitForDisplayed();

    await openMap();
    expect(await text()).toContain('Natural Earth');
    expect(await text()).toContain('have no coordinates');
    await shot('maps-1-places.png');

    await mode('Heat map');
    await shot('maps-2-heat.png');

    await mode('Movement');
    await choose('Show', 'Ancestors of');
    await shot('maps-3-movement.png');

    await mode('Places');
    await choose('Show', 'Whole tree');
    await click('Set them');
    await $('aside[aria-label="Place coordinates"]').waitForDisplayed();
    const code = await $('//input[@list="map-countries"]');
    await code.setValue('ZZ');
    await browser.pause(1200);
    await click('Download place data');
    await $('//button[normalize-space(.)="Use this"]').waitForDisplayed({ timeout: 30000 });
    await shot('maps-4-helper.png');

    // Confirm the first suggestion: the notice count goes down by one place.
    const before = (await text()).match(/(\d+) events? have? no coordinates|(\d+) events? has no coordinates/)?.[0];
    await (await $('//button[normalize-space(.)="Use this"]')).click();
    await browser.pause(1500);
    const after = (await text()).match(/(\d+) events? have? no coordinates|(\d+) events? has no coordinates/)?.[0];
    expect(after).not.toBe(before);
    await shot('maps-5-after-confirm.png');
  });
});
