/**
 * Text on pictures and the recording player's transcript, on the REAL app (debug build, tauri-driver, WebView2).
 *
 * - A generated picture gets two text pieces marked by dragging on it; hovering one shows its text beside the pointer.
 * - A generated recording gets a long transcript; in the player only ONE container scrolls (the transcript list), in a
 *   wide window and in a narrow one where the panels stack.
 *
 * Synthetic data only: a canvas-drawn picture, a generated one-second tone. Screenshots in English and Ukrainian.
 */
import fs from 'node:fs';
import path from 'node:path';
import { expect } from '@wdio/globals';
import { $, $$, browser, click, drop, goTo, importSynthetic, makeJpegs, openTreeFromReport, pushDialog, ready, shot, slash, work } from './helpers';

function makeWav(file: string): string {
  const rate = 8000;
  const n = rate;
  const buf = Buffer.alloc(44 + n * 2);
  buf.write('RIFF', 0);
  buf.writeUInt32LE(36 + n * 2, 4);
  buf.write('WAVE', 8);
  buf.write('fmt ', 12);
  buf.writeUInt32LE(16, 16);
  buf.writeUInt16LE(1, 20);
  buf.writeUInt16LE(1, 22);
  buf.writeUInt32LE(rate, 24);
  buf.writeUInt32LE(rate * 2, 28);
  buf.writeUInt16LE(2, 32);
  buf.writeUInt16LE(16, 34);
  buf.write('data', 36);
  buf.writeUInt32LE(n * 2, 40);
  for (let i = 0; i < n; i++) buf.writeInt16LE(Math.round(8000 * Math.sin((2 * Math.PI * 440 * i) / rate)), 44 + i * 2);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, buf);
  return slash(file);
}

/** Drags across the picture's text layer from one point to another, as offsets from the layer's centre. */
async function dragOnPicture(from: [number, number], to: [number, number]) {
  const layer = await $('.layer');
  await browser
    .action('pointer')
    .move({ origin: layer, x: from[0], y: from[1] })
    .down()
    .move({ origin: layer, x: to[0], y: to[1], duration: 150 })
    .up()
    .perform();
}

/** How many elements in the open dialog scroll by themselves right now (overflow set and content really taller). */
async function scrollingContainers(): Promise<string[]> {
  return browser.execute(() => {
    const dialog = document.querySelector('[role="dialog"]')!;
    const out: string[] = [];
    for (const el of [dialog, ...Array.from(dialog.querySelectorAll<HTMLElement>('*'))]) {
      const o = getComputedStyle(el).overflowY;
      if ((o === 'auto' || o === 'scroll') && el.scrollHeight > el.clientHeight + 1) out.push(`${el.tagName.toLowerCase()}.${el.className.replace(/svelte-\w+/g, '').trim()}`);
    }
    return out;
  });
}

/** The dialog's own close button (its label depends on the language). */
async function closeDialog() {
  await (await $('[role="dialog"] header button')).click();
  await $('[role="dialog"]').waitForExist({ reverse: true, timeout: 10000 });
}

async function chooseLanguage(code: string, heading: string) {
  const nav = await $('//nav//a[normalize-space(.)="Settings" or normalize-space(.)="Налаштування"]');
  await nav.click();
  const select = await $(`//select[@id=//label[starts-with(normalize-space(.),"Language") or starts-with(normalize-space(.),"Мова")]/@for]`);
  await select.waitForDisplayed();
  await select.selectByAttribute('value', code);
  await $(`h1=${heading}`).waitForDisplayed({ timeout: 20000 });
}

/** One key press at a time: WebKitWebDriver drops the second of two equal letters sent in one keys() call ("bloom" came out "blom"). */
async function typeText(text: string) {
  for (const ch of text) await browser.keys(ch);
}

describe('Text on pictures and the recording transcript', () => {
  before(async () => {
    await ready();
  });

  it('shows the text of a piece when the pointer rests on it', async () => {
    await importSynthetic('Media text tree');
    await openTreeFromReport();
    await goTo('Media');
    await $('h1=Media').waitForDisplayed();
    const [pic] = await makeJpegs(path.join(work(), 'media-text'), 1);
    const thumbs = () => $$('ul[aria-label="Photos and documents"] button');
    await $('button=Add files…').waitForDisplayed();
    const now = (await thumbs()).length;
    await drop([pic]);
    await browser.waitUntil(async () => (await thumbs()).length === now + 1, { timeoutMsg: 'the picture should appear' });
    await (await thumbs())[now].click();
    await $('[role="dialog"] .frame').waitForDisplayed();

    await click('Mark text');
    await dragOnPicture([-170, -130], [20, -90]);
    await typeText('Dear Marta, the orchard is in bloom.');
    await browser.keys('Tab');
    await click('Mark text');
    await dragOnPicture([-170, 60], [60, 120]);
    await typeText('Lviv, May');
    await browser.keys('Tab');
    await browser.waitUntil(async () => (await $$('.layer .box')).length === 2, { timeoutMsg: 'two pieces should be marked' });

    // Out of the editor, the pieces are invisible but answer to the pointer.
    await click('Edit text');
    const box = (await $$('.layer .box'))[0];
    await box.moveTo();
    const tip = await $('[role="tooltip"]');
    await tip.waitForDisplayed({ timeout: 5000 });
    expect(await tip.getText()).toContain('the orchard is in bloom.');
    await shot('media-text-hover-en.png');
    await click('Text on image');
    await shot('media-text-outlines-en.png');
    await browser.keys('Escape');
  });

  it('shows the same in Ukrainian', async () => {
    await chooseLanguage('uk', 'Налаштування');
    await $('//nav//a[normalize-space(.)="Медіа"]').click();
    await $('ul[aria-label="Фото та документи"] button').click();
    await $('[role="dialog"] .frame').waitForDisplayed();
    const box = (await $$('.layer .box'))[0];
    await box.moveTo();
    const tip = await $('[role="tooltip"]');
    await tip.waitForDisplayed({ timeout: 5000 });
    expect(await tip.getText()).toContain('the orchard is in bloom.');
    await shot('media-text-hover-uk.png');
    await browser.keys('Escape');
    await chooseLanguage('en', 'Settings');
  });

  it('keeps a single scrollbar in the recording player, wide and narrow', async () => {
    // A recording is attached to a note as a voice sample, as the owner does it.
    const wav = makeWav(path.join(work(), 'media-text', 'Interview side A.wav'));
    await goTo('Family');
    await click('Edit person');
    await click('Add note');
    const box = await $('textarea');
    await box.waitForDisplayed();
    await box.setValue('The talk about the mill.');
    await pushDialog(wav);
    await click('Add voice sample');
    await $('//*[contains(.,"Interview side A.wav")]').waitForDisplayed();
    await click('Save note');
    await $('textarea').waitForDisplayed({ reverse: true, timeout: 20000 });
    const open = await $('//button[normalize-space(.)="Open in the player"]');
    await open.waitForClickable({ timeout: 20000 });
    await open.click();
    await $('[data-testid="recording-player"]').waitForDisplayed();

    await click('Edit transcript');
    const add = '//button[starts-with(normalize-space(.),"Add a piece at")]';
    for (let i = 0; i < 30; i++) await (await $(add)).click();
    await browser.execute(() => {
      document.querySelectorAll<HTMLTextAreaElement>('[data-testid="recording-player"] textarea').forEach((t, i) => {
        t.value = `Line ${i + 1}: the talk goes on about the old mill and the river.`;
        t.dispatchEvent(new Event('input', { bubbles: true }));
      });
    });
    await click('Save transcript');
    const list = await $('[data-testid="transcript-list"]');
    await list.waitForDisplayed({ timeout: 20000 });

    for (const [w, h] of [
      [1440, 900],
      [760, 900],
      [420, 800],
    ]) {
      await browser.setWindowSize(w, h);
      await browser.pause(500);
      const scrolling = await scrollingContainers();
      expect(scrolling.length).toBe(1);
      if (w > 900) {
        expect(scrolling[0]).toContain('pieces');
        // The transcript runs the full height of the dialog's body.
        const fits = await browser.execute(() => {
          const dialog = document.querySelector('[role="dialog"]')!.getBoundingClientRect();
          const list = document.querySelector('[data-testid="transcript-list"]')!.getBoundingClientRect();
          return { dialogBottom: dialog.bottom, listBottom: list.bottom, windowH: window.innerHeight, listH: list.height };
        });
        expect(fits.listH).toBeGreaterThan(300);
        expect(fits.dialogBottom - fits.listBottom).toBeLessThan(60);
      }
      await shot(`media-text-recording-${w}-en.png`);
    }
    await browser.setWindowSize(1440, 900);
  });

  it('shows the recording player in Ukrainian', async () => {
    await closeDialog();
    await chooseLanguage('uk', 'Налаштування');
    await $('//nav//a[normalize-space(.)="Родина"]').click();
    await click('Змінити особу');
    await (await $('[data-testid="voice-sample"] button')).click();
    await $('[data-testid="transcript-list"]').waitForDisplayed({ timeout: 20000 });
    expect((await scrollingContainers()).length).toBe(1);
    await shot('media-text-recording-uk.png');
    await closeDialog();
    await chooseLanguage('en', 'Settings');
  });
});
