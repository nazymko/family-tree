/**
 * Voice samples on a note, on the REAL app (debug build, tauri-driver, WebView2): a generated wav is attached to a note
 * through the test hooks, then the person page must show a player whose audio element can load, report a finite
 * length and play through the gtmedia protocol. Screenshots in English and Ukrainian.
 *
 * Synthetic data only; the recording is a generated one-second tone.
 */
import fs from 'node:fs';
import path from 'node:path';
import { expect } from '@wdio/globals';
import { $, browser, click, importSynthetic, meta, openTreeFromReport, pushDialog, ready, search, shot, slash, work } from './helpers';

/** A one-second 440 Hz mono 16-bit 8 kHz PCM wav, built here so no real recording is ever involved. */
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

/** The select that belongs to a label, whatever language the label is in. */
async function choose(label: string, value: string) {
  const select = await $(`//select[@id=//label[starts-with(normalize-space(.),"${label}")]/@for]`);
  await select.waitForDisplayed();
  await select.selectByAttribute('value', value);
}

async function openHome() {
  await search(meta().home.name);
  await $('[role="listbox"] [role="option"]').waitForDisplayed();
  await browser.keys('Enter');
  await $(`//h1[contains(.,"${meta().home.given}")]`).waitForDisplayed();
}

/** Waits for the first audio element to report a finite length, then plays it and checks the clock moves. */
async function proveAudioPlays() {
  await browser.waitUntil(
    () =>
      browser.execute(() => {
        const a = document.querySelector('audio');
        return !!a && Number.isFinite(a.duration) && a.duration > 0.5;
      }),
    { timeout: 20000, timeoutMsg: 'the audio element never reported a finite length (gtmedia / media-src / range problem?)' },
  );
  const played = await browser.execute(
    () =>
      new Promise<{ ok: boolean; err: string }>((done) => {
        const a = document.querySelector('audio')!;
        a.muted = true;
        // Under load (the gate running beside the windows) the clock can take well over 500 ms to start moving.
        const until = Date.now() + 10000;
        const check = () => (a.currentTime > 0 ? done({ ok: true, err: '' }) : Date.now() > until ? done({ ok: false, err: '' }) : setTimeout(check, 100));
        a.play()
          .then(check)
          .catch((e: unknown) => done({ ok: false, err: String(e) }));
      }),
  );
  expect(played.err).toBe('');
  expect(played.ok).toBe(true);
  // Seeking needs ranged replies: a seek to the middle must land.
  const seeked = await browser.execute(
    () =>
      new Promise<number>((done) => {
        const a = document.querySelector('audio')!;
        a.addEventListener('seeked', () => done(a.currentTime), { once: true });
        a.currentTime = 0.5;
      }),
  );
  expect(seeked).toBeGreaterThan(0.4);
}

describe('Voice samples on a note', () => {
  before(async () => {
    await ready();
  });

  it('attaches a generated wav to a note and plays it on the person page', async () => {
    const wav = makeWav(path.join(work(), 'voice-e2e', 'Olena Koval - story.wav'));
    await importSynthetic('Voice tree');
    await openTreeFromReport();
    await openHome();
    await click('Edit person');

    await click('Add note');
    const box = await $('textarea');
    await box.waitForDisplayed();
    await box.setValue('Olena Koval tells the story of the mill.');
    await pushDialog(wav);
    await click('Add voice sample');
    // On a new note the file waits and is attached by the save.
    await $('//*[contains(.,"Olena Koval - story.wav")]').waitForDisplayed();
    await click('Save note');
    await $('textarea').waitForDisplayed({ reverse: true, timeout: 20000 });

    await $('audio').waitForDisplayed({ timeout: 20000 });
    await proveAudioPlays();
    await expect($('ul[aria-label="Voice samples"]')).toBeDisplayed();
    await $('audio').scrollIntoView({ block: 'center' });
    await browser.pause(500);
    await shot('voice-person-en.png');
  });

  it('shows the same in Ukrainian', async () => {
    const nav = await $('//nav//a[normalize-space(.)="Settings"]');
    await nav.click();
    await choose('Language', 'uk');
    await $('h1=Налаштування').waitForDisplayed({ timeout: 20000 });
    await $('//nav//a[normalize-space(.)="Родина"]').click();
    await openHome();
    await click('Змінити особу');
    await $('audio').waitForDisplayed({ timeout: 20000 });
    await proveAudioPlays();
    await expect($('ul[aria-label="Голосові записи"]')).toBeDisplayed();
    await $('audio').scrollIntoView({ block: 'center' });
    await browser.pause(500);
    await shot('voice-person-uk.png');
  });
});
