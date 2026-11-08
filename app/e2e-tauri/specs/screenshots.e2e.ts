/**
 * Screenshots of the REAL app (not the mock): imports the synthetic 5k-person file through the test hook and walks the
 * main screens at 1440x900. PNGs go to $GT_E2E_SHOTS (default: a folder inside the run's temp dir).
 */
import path from 'node:path';
import { expect } from '@wdio/globals';
import {
  $,
  $$,
  browser,
  click,
  goTo,
  importSynthetic,
  makeJpegs,
  meta,
  openTreeFromReport,
  pushDialog,
  ready,
  search,
  shot,
  text,
  work,
} from './helpers';

const HITS = '[role="listbox"][aria-label="Search results"] [role="option"]';

/** Focuses the generation-3 person of the "spine" line (ancestors above, children below) through the search palette. */
async function focusHome() {
  await search(meta().home.name);
  await $(HITS).waitForDisplayed();
  await browser.keys('Enter');
  await $(`//h1[contains(.,"${meta().home.given}")]`).waitForDisplayed();
}

const PERSON_NOTE =
  "He left the farm at nineteen and settled in the river town, where he learned the baker's trade. In the evenings he kept a small garden behind the shop.";
const FAMILY_NOTE =
  'They married in the town church and opened a bakery on the market square. It stayed in the family for three generations.';
const SHIFT = '';
// A granddaughter and her husband from the synthetic tree: a small family, so the charts stay readable when they fit the frame.
const SMALL = { name: 'Greta Aaberg', husband: 'Heinrich Ostturulm35' };

async function focusSmall() {
  await search(SMALL.name);
  await $(HITS).waitForDisplayed();
  await browser.keys('Enter');
  await $(`//h1[contains(.,"${SMALL.name}")]`).waitForDisplayed();
}

/** Closes every toast so none covers the picture. */
async function clearToasts() {
  await browser.execute(() =>
    document.querySelectorAll<HTMLButtonElement>('[role="region"][aria-live="polite"] button[aria-label="Dismiss"]').forEach((b) => b.click()),
  );
  await browser.pause(700);
}

/** Picks "<n> generations" and fits the chart so every card is whole. */
async function fitGenerations(text: string) {
  const select = await $('//select[@id=//label[starts-with(normalize-space(.),"Generations")]/@for]');
  await select.waitForDisplayed();
  await select.selectByVisibleText(text);
  await browser.pause(1200);
  // Ancestors on the left: the chart grows downward instead of sideways, so it fits the frame at a readable size.
  await $('//label[.//input[@data-direction="left"]]').click();
  await browser.pause(1500);
  await $('//button[normalize-space(.)="Fit"]').click();
  await browser.pause(1200);
}

/** Drags the chart by (dx, dy) pixels. */
async function dragChart(dx: number, dy: number) {
  const canvas = await $('canvas[role="application"]');
  const { x, y } = await canvas.getLocation();
  const { width, height } = await canvas.getSize();
  const sx = Math.round(x + width / 2);
  const sy = Math.round(y + height * 0.8);
  await browser.performActions([
    {
      type: 'pointer',
      id: 'mouse',
      parameters: { pointerType: 'mouse' },
      actions: [
        { type: 'pointerMove', duration: 0, x: sx, y: sy, origin: 'viewport' },
        { type: 'pointerDown', button: 0 },
        { type: 'pointerMove', duration: 400, x: sx + dx, y: sy + dy, origin: 'viewport' },
        { type: 'pointerUp', button: 0 },
      ],
    },
  ]);
  await browser.releaseActions();
  await browser.pause(800);
}

async function toWholeTreePage() {
  const a = await $('//nav//a[normalize-space(.)="Whole tree"]');
  await a.waitForClickable();
  await a.click();
  await $('h1=Whole tree').waitForDisplayed({ timeout: 30000 });
}

async function flyTo(findLabel: string, name: string) {
  const box = await $(`//input[@role="combobox"][@id=//label[starts-with(normalize-space(.),"${findLabel}")]/@for]`);
  await box.waitForDisplayed();
  await box.setValue(name);
  const option = await $(`//ul[@role="listbox"][@aria-label="${findLabel}"]//*[@role="option"][contains(.,"${name}")]`);
  await option.waitForDisplayed({ timeout: 10000 });
  await option.click();
  await browser.pause(900);
}

async function focusCanvas() {
  await browser.execute(() => (document.querySelector('canvas[role="application"]') as HTMLElement).focus());
}

async function shiftClickCentre() {
  const canvas = await $('canvas[role="application"]');
  const { x, y } = await canvas.getLocation();
  const { width, height } = await canvas.getSize();
  const px = Math.round(x + width / 2);
  const py = Math.round(y + height / 2);
  await browser.performActions([
    { type: 'key', id: 'keyboard', actions: [{ type: 'keyDown', value: SHIFT }, { type: 'pause' }, { type: 'pause' }, { type: 'pause' }, { type: 'keyUp', value: SHIFT }] },
    {
      type: 'pointer',
      id: 'mouse',
      parameters: { pointerType: 'mouse' },
      actions: [
        { type: 'pause' },
        { type: 'pointerMove', duration: 0, x: px, y: py, origin: 'viewport' },
        { type: 'pointerDown', button: 0 },
        { type: 'pointerUp', button: 0 },
        { type: 'pause' },
      ],
    },
  ]);
  await browser.releaseActions();
}

async function markBoth(findLabel: string, first: string, second: string) {
  await flyTo(findLabel, first);
  const marked = $('[data-testid="marked-count"]');
  for (let i = 0; i < 3 && !(await marked.isDisplayed()); i++) {
    await focusCanvas();
    await browser.keys(' ');
    await browser.pause(700);
  }
  await marked.waitForDisplayed();
  await flyTo(findLabel, second);
  await shiftClickCentre();
  await expect($('[data-testid="marked-count"]')).toHaveText('2 people selected');
}

describe('Screenshots of the real app', () => {
  before(async () => {
    await ready();
  });

  it('import report', async () => {
    await importSynthetic('Screenshot tree');
    await browser.pause(600);
    await shot('real-import-report.png');
  });

  it('family, people, person', async () => {
    await openTreeFromReport();
    await $('h1').waitForDisplayed();
    await browser.pause(800);
    await shot('real-family.png');

    await goTo('People');
    await $('[data-filter-input]').waitForDisplayed();
    await browser.waitUntil(async () => (await $$('[role="option"]')).length > 5);
    await browser.pause(800);
    await shot('real-people.png');

    await focusHome();
    await browser.pause(600);
    await click('Edit person');
    await $('h1').waitForDisplayed();
    await browser.pause(1000);
    await shot('real-person.png');

    // The sign before the word for sex, in the person's own form.
    const sex = await $('//select[@id=//label[starts-with(normalize-space(.),"Sex")]/@for]');
    await sex.scrollIntoView({ block: 'center' });
    await browser.pause(300);
    expect(await sex.getText()).toContain('♂ Male');
    await shot('real-person-sex.png');
  });

  it('pedigree and descendants', async () => {
    await goTo('Pedigree');
    await $('h1=Pedigree').waitForDisplayed();
    await $('//*[@role="tab"][normalize-space(.)="Chart"]').click();
    await $('canvas[aria-label="Pedigree chart"]').waitForDisplayed();
    await browser.pause(2500);
    await shot('real-pedigree.png');

    await goTo('Descendants');
    await $('h1=Descendants').waitForDisplayed();
    await $('//*[@role="tab"][normalize-space(.)="Chart"]').click();
    await $('canvas[aria-label="Descendant chart"]').waitForDisplayed();
    await browser.pause(2500);
    await shot('real-descendants.png');
  });

  it('search with a CJK query', async () => {
    await goTo('Family');
    const cjk = meta().scripts.cjk.surname;
    await search(cjk);
    await browser.waitUntil(async () => (await $$(HITS)).length > 0, { timeoutMsg: 'no CJK search results' });
    await browser.pause(600);
    expect(await text()).toContain(cjk);
    await shot('real-search.png');
    await browser.keys('Escape');
  });

  it('media thumbnails through gtmedia', async () => {
    await goTo('Media');
    await $('h1=Media').waitForDisplayed();
    const files = await makeJpegs(path.join(work(), 'shots-media'), 2);
    await $('button=Add files…').waitForDisplayed();
    await pushDialog(files.join('|'));
    await click('Add files…');
    const thumbs = () => $$('ul[aria-label="Photos and documents"] img');
    await browser.waitUntil(async () => (await thumbs()).length >= 2, { timeoutMsg: 'thumbnails did not appear' });
    await browser.waitUntil(
      () =>
        browser.execute(
          () => Array.from(document.querySelectorAll<HTMLImageElement>('ul[aria-label="Photos and documents"] img')).filter((i) => i.naturalWidth > 0).length >= 2,
        ),
      { timeoutMsg: 'thumbnails did not load' },
    );
    // Let the toast settle, then take the picture.
    await browser.pause(800);
    await shot('real-media.png');
  });

  it('export preview', async () => {
    await click('Export');
    await $('[role="region"][aria-label="Privacy preview"]').waitForDisplayed();
    await $('//dt[normalize-space(.)="Shown as \'Living\'"]/following-sibling::dd').waitForDisplayed({ timeout: 30000 });
    await browser.pause(600);
    await shot('real-export.png');
    await click('Close');
  });

  it('notes on a person and on the family', async () => {
    await goTo('Family');
    await focusHome();
    await click('Edit person');
    await $('h1').waitForDisplayed();

    await click('Add note');
    const box = await $('textarea');
    await box.waitForDisplayed();
    await box.setValue(PERSON_NOTE);
    await click('Save note');
    await $('textarea').waitForDisplayed({ reverse: true, timeout: 20000 });

    const dialog = '//*[@role="dialog"]';
    await click('Edit family');
    const add = await $(`${dialog}//button[normalize-space(.)="Add note"]`);
    await add.waitForClickable({ timeout: 20000 });
    await add.click();
    const famBox = await $(`(${dialog})[last()]//textarea`);
    await famBox.waitForDisplayed({ timeout: 20000 });
    await famBox.setValue(FAMILY_NOTE);
    await click('Save note');
    await $(`${dialog}//*[normalize-space(.)="${FAMILY_NOTE}"]`).waitForDisplayed({ timeout: 20000 });
    await browser.keys('Escape');
    await $(dialog).waitForDisplayed({ reverse: true, timeout: 20000 });

    const heading = await $('//h3[contains(.,"Shared family notes")]');
    await heading.waitForDisplayed({ timeout: 20000 });
    await $(`//article//*[normalize-space(.)="${PERSON_NOTE}"]`).waitForDisplayed({ timeout: 20000 });
    await (await $('//h2[normalize-space(.)="Notes"]')).scrollIntoView({ block: 'start' });
    await clearToasts();
    await shot('real-notes.png');
  });

  it('family page of the couple', async () => {
    await goTo('Family');
    const onFamily = await $('//h2[contains(.,"Shared family notes")]');
    await onFamily.waitForDisplayed({ timeout: 20000 });
    const open = await $('//button[normalize-space(.)="Family page"][ancestor::section[contains(@aria-label,"Shared family notes")]]');
    await open.scrollIntoView({ block: 'center' });
    await open.click();
    await $('//h1[starts-with(normalize-space(.),"Family:")]').waitForDisplayed({ timeout: 20000 });
    await clearToasts();
    await shot('real-family-page.png');
  });

  it('relatives chart', async () => {
    await goTo('Family');
    await focusSmall();
    await $('//nav//a[normalize-space(.)="Relatives"]').click();
    await $('[role="toolbar"]').waitForDisplayed({ timeout: 30000 });
    await $('canvas').waitForDisplayed();
    await browser.pause(1500);
    await fitGenerations('2 generations');
    await clearToasts();
    await shot('real-relatives.png');
  });

  it('whole tree', async () => {
    await toWholeTreePage();
    await flyTo('Find a person…', SMALL.name);
    await click('Zoom out');
    await browser.pause(500);
    await dragChart(0, -230);
    await browser.execute(() => (document.activeElement as HTMLElement | null)?.blur());
    await clearToasts();
    await browser.pause(1200);
    await shot('real-whole.png');
  });

  it('built around a married couple, all relatives', async () => {
    await toWholeTreePage();
    const first = SMALL.name;
    const second = SMALL.husband;
    await markBoth('Find a person…', first, second);
    await $('//button[normalize-space(.)="Build around these people (2)"]').waitForDisplayed();
    await $('//button[normalize-space(.)="Build around these people (2)"]').click();
    await $(`//h1[contains(.,"Built around:")][contains(.,"${first}")]`).waitForDisplayed({ timeout: 30000 });
    await $('canvas[role="application"]').waitForDisplayed();
    await $('//label[normalize-space(.)="Brothers and sisters"]').click();
    await browser.pause(1800);
    await fitGenerations('2 generations');
    await clearToasts();
    await shot('real-around.png');
    await $('//button[normalize-space(.)="Back to the whole tree"]').click();
    await $('h1=Whole tree').waitForDisplayed({ timeout: 30000 });
  });
});
