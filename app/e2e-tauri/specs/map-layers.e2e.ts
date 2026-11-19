/**
 * Old maps on the Map, in the REAL app and in Ukrainian: a synthetic "old map" picture (drawn on a canvas in the page)
 * with its OziExplorer .map file is added from the Old maps panel, is placed by the .map at once, is drawn at 60 %
 * opacity under the invented villages of a GEDCOM written here, and then shows its corner handles for placing by hand.
 * Screenshots go to $GT_E2E_SHOTS (map-layers-1-panel.png, map-layers-2-placed.png, map-layers-3-handles.png).
 *
 * Invented people, villages and map only.
 */
import fs from 'node:fs';
import path from 'node:path';
import { expect } from '@wdio/globals';
import { $, browser, click, goTo, pushDialog, ready, shot, slash, text, work } from './helpers';

// Villages inside the map's frame (49.9 to 50.6 N, 30.0 to 31.2 E).
const VILLAGES: [string, number, number][] = [
  ['Vyshneve Pole', 50.42, 30.31],
  ['Dubovyi Yar', 50.18, 30.74],
  ['Sosnivka Nova', 50.05, 31.02],
  ['Kalynivskyi Khutir', 50.51, 30.95],
];

function writeGedcom(file: string) {
  const out = ['0 HEAD', '1 SOUR MAP-LAYERS-E2E', '1 GEDC', '2 VERS 5.5.1', '2 FORM LINEAGE-LINKED', '1 CHAR UTF-8'];
  VILLAGES.forEach(([name, lat, lon], i) => {
    out.push(
      `0 @I${i + 1}@ INDI`,
      `1 NAME Ostap${i + 1} /Zarichnyi/`,
      '1 SEX M',
      '1 BIRT',
      `2 DATE ${1850 + i * 7}`,
      `2 PLAC ${name}, Zelena Volost`,
      '3 MAP',
      `4 LATI N${lat.toFixed(4)}`,
      `4 LONG E${lon.toFixed(4)}`,
    );
  });
  out.push('0 TRLR', '');
  fs.writeFileSync(file, out.join('\n'), 'utf8');
}

/** A synthetic three-verst-like sheet: paper, a grid, a river, woods and village dots, saved as a real JPEG. */
async function drawOldMap(file: string, w: number, h: number) {
  const b64 = await browser.execute(
    (w: number, h: number) => {
      const c = document.createElement('canvas');
      c.width = w;
      c.height = h;
      const g = c.getContext('2d')!;
      g.fillStyle = '#e9dcb8';
      g.fillRect(0, 0, w, h);
      g.strokeStyle = 'rgba(120, 80, 40, 0.5)';
      g.lineWidth = 3;
      for (let x = 0; x <= w; x += w / 6) {
        g.beginPath();
        g.moveTo(x, 0);
        g.lineTo(x, h);
        g.stroke();
      }
      for (let y = 0; y <= h; y += h / 4) {
        g.beginPath();
        g.moveTo(0, y);
        g.lineTo(w, y);
        g.stroke();
      }
      g.strokeStyle = '#4f7fa6';
      g.lineWidth = 14;
      g.beginPath();
      g.moveTo(0, h * 0.7);
      g.bezierCurveTo(w * 0.3, h * 0.4, w * 0.55, h * 0.95, w, h * 0.35);
      g.stroke();
      g.fillStyle = 'rgba(80, 120, 60, 0.55)';
      for (let i = 0; i < 40; i++) {
        g.beginPath();
        g.arc(w * 0.1 + ((i * 97) % 600), h * 0.15 + ((i * 53) % 250), 18, 0, Math.PI * 2);
        g.fill();
      }
      g.fillStyle = '#7a2e1f';
      g.font = `${Math.round(h / 18)}px serif`;
      g.fillText('Sheet XIV-7 (invented), 1865', w * 0.05, h * 0.95);
      g.lineWidth = 10;
      g.strokeStyle = '#3b2a1a';
      g.strokeRect(5, 5, w - 10, h - 10);
      return c.toDataURL('image/jpeg', 0.85).split(',')[1];
    },
    w,
    h,
  );
  fs.writeFileSync(file, Buffer.from(b64, 'base64'));
}

/** Its OziExplorer .map, calibrated (as the freemap files are) on a picture of another size than the JPEG. */
function writeOzi(file: string, picture: string, w: number, h: number) {
  const lines = [
    'OziExplorer Map Data File Version 2.2',
    picture,
    picture,
    '1 ,Map Code,',
    'WGS 84,WGS 84,   0.0000,   0.0000,WGS 84',
    'Reserved 1',
    'Reserved 2',
    'Magnetic Variation,,,E',
    'Map Projection,Latitude/Longitude,PolyCal,No,AutoCalOnly,No,BSBUseWPX,No',
    'Projection Setup,,,,,,,,,,',
    'MM0,Yes',
    'MMPNUM,4',
    `MMPXY,1,0,0`,
    `MMPXY,2,${w},0`,
    `MMPXY,3,${w},${h}`,
    `MMPXY,4,0,${h}`,
    'MMPLL,1,  30.000000,  50.600000',
    'MMPLL,2,  31.200000,  50.600000',
    'MMPLL,3,  31.200000,  49.900000',
    'MMPLL,4,  30.000000,  49.900000',
    `IWH,Map Image Width/Height,${w},${h}`,
  ];
  fs.writeFileSync(file, lines.join('\r\n') + '\r\n', 'latin1');
}

async function importGedcom(ged: string, name: string) {
  const parent = slash(path.join(work(), 'trees'));
  fs.mkdirSync(parent, { recursive: true });
  await click('Import a family tree file');
  await pushDialog(slash(ged));
  await click('Choose file…');
  await pushDialog(parent);
  await click('Choose folder…');
  const field = await $(`//input[@id=//label[starts-with(normalize-space(.),"Tree name")]/@for]`);
  await field.setValue(name);
  await click('Start import');
  await $('h1*=Imported').waitForExist({ timeout: 180000 });
  await click('Open the tree');
  await $('aria/Main').waitForExist({ timeout: 180000 });
}

describe('Old maps on the Map', () => {
  before(async () => {
    await ready();
  });

  it('adds a sheet with its .map, draws it at 60 % under the villages, in Ukrainian', async () => {
    const dir = path.join(work(), 'map-layers');
    fs.mkdirSync(dir, { recursive: true });
    const ged = path.join(dir, 'villages.ged');
    writeGedcom(ged);
    await importGedcom(ged, 'Old maps tree');

    const picture = path.join(dir, 'sheet-xiv-7.jpg');
    await drawOldMap(picture, 2400, 1400);
    writeOzi(path.join(dir, 'sheet-xiv-7.map'), 'sheet-xiv-7.jpg', 2520, 1470);

    // Ukrainian.
    await goTo('Settings');
    const select = await $('//select[option[@value="zh-Hans"]]');
    await select.waitForDisplayed();
    await select.selectByAttribute('value', 'uk');
    await browser.waitUntil(async () => (await browser.execute(() => document.documentElement.lang)) === 'uk', { timeout: 10000 });

    const nav = await $('//nav//a[normalize-space(.)="Мапа"]');
    await nav.waitForClickable({ timeout: 10000 });
    await nav.click();
    await $('canvas').waitForDisplayed({ timeout: 20000 });
    await browser.pause(1200);

    await click('Старі мапи');
    const panel = await $('aside[aria-label="Старі мапи"]');
    await panel.waitForDisplayed();
    expect(await text()).toContain('Старих мап ще немає.');

    await pushDialog(slash(picture));
    await click('Додати мапу…');
    await $('//p[contains(.,"Кути з sheet-xiv-7.map")]').waitForDisplayed({
      timeout: 10000,
    });
    const year = await $('//aside//label[starts-with(normalize-space(.),"Рік")]//input');
    await year.setValue('1865');
    const series = await $('//aside//label[starts-with(normalize-space(.),"Серія")]//input');
    await series.setValue('Трьохверстівка (вигадана)');
    const source = await $('//aside//label[starts-with(normalize-space(.),"Джерело")]//input');
    await source.setValue('https://example.org/sheet-xiv-7');
    await shot('map-layers-1-panel.png');
    await click('Додати на мапу');

    // Placed by its .map at once: no placing notice, the row shows the sheet.
    const row = await $('//aside//li[.//span[normalize-space(.)="sheet-xiv-7"]]');
    await row.waitForDisplayed({ timeout: 20000 });
    expect(await text()).not.toContain('ще не розміщена');

    // 60 % opacity, as the owner sets it with the slider.
    const slider = await $('//input[@type="range" and @aria-label="Непрозорість: sheet-xiv-7"]');
    await browser.execute(
      (el: HTMLInputElement) => {
        el.value = '60';
        el.dispatchEvent(new Event('input', { bubbles: true }));
        el.dispatchEvent(new Event('change', { bubbles: true }));
      },
      slider as unknown as HTMLInputElement,
    );
    await browser.waitUntil(async () => (await row.getText()).includes('60%'), {
      timeout: 10000,
    });

    // The picture is drawn: the map under the sheet differs when the sheet is hidden.
    // (The canvas cannot be read back: the picture comes from the media protocol, another origin. The window's
    // screenshot can.)
    const sample = async () => (await $('canvas')).takeScreenshot();
    await browser.pause(2500);
    const shown = await sample();
    await shot('map-layers-2-placed.png');
    const box = await $('//input[@type="checkbox" and @aria-label="Показувати sheet-xiv-7"]');
    await box.click();
    await browser.waitUntil(async () => !(await box.isSelected()), {
      timeout: 10000,
    });
    await browser.pause(1500);
    expect(await sample()).not.toBe(shown);
    await box.click();
    await browser.waitUntil(async () => box.isSelected(), { timeout: 10000 });
    await browser.pause(1500);

    // Placing by hand: the four corners and the middle have handles; Cancel leaves it as it was.
    await click('Розмістити');
    for (const name of ['Верхній лівий кут', 'Верхній правий кут', 'Нижній правий кут', 'Нижній лівий кут', 'Пересунути всю мапу'])
      await $(`//button[@aria-label="${name}"]`).waitForDisplayed({
        timeout: 10000,
      });
    await browser.pause(800);
    await shot('map-layers-3-handles.png');
    await click('Скасувати');
    await $('//button[@aria-label="Верхній лівий кут"]').waitForExist({
      reverse: true,
      timeout: 10000,
    });
  });
});
