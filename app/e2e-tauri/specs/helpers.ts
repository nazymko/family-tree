import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { browser, $, $$ } from '@wdio/globals';

export const ged = () => process.env.GT_E2E_GED!;
export const work = () => process.env.GT_E2E_WORK ?? os.tmpdir();
export const meta = () =>
  JSON.parse(fs.readFileSync(`${ged()}.meta.json`, 'utf8')) as {
    persons: number;
    home: { given: string; surname: string; name: string };
    uniqueLivingSurnames: string[];
    scripts: Record<string, { given: string; surname: string }>;
  };

/** Forward-slash path; the app accepts it on every OS. */
export const slash = (p: string) => p.replaceAll('\\', '/');

type Bridge = { pushDialog(s: string): void; drop(a: string[]): void };

/** Waits until the page has asked the backend for the test hooks. */
export async function ready() {
  await browser.setWindowSize(1440, 900);
  await browser.waitUntil(() => browser.execute(() => !!(window as unknown as { __gtE2e?: unknown }).__gtE2e), {
    timeout: 20000,
    timeoutMsg: 'test hooks (window.__gtE2e) never appeared: is this a debug build run with GT_E2E=1?',
  });
}

export async function pushDialog(...results: string[]) {
  for (const r of results) {
    await browser.execute((v) => (window as unknown as { __gtE2e: Bridge }).__gtE2e.pushDialog(v), r);
  }
}

export async function drop(paths: string[]) {
  await browser.execute((p) => (window as unknown as { __gtE2e: Bridge }).__gtE2e.drop(p), paths);
}

export async function button(name: string) {
  const b = await $(`//button[normalize-space(.)="${name}"]`);
  await b.waitForClickable({ timeout: 20000 });
  return b;
}

export async function click(name: string) {
  await (await button(name)).click();
}

/** Ctrl+<key>; the app also answers Cmd on macOS, where this suite does not run. */
export async function chord(key: string, shift = false) {
  const keys = shift ? ['Control', 'Shift', key] : ['Control', key];
  await browser.keys(keys);
}

export async function text(): Promise<string> {
  return (await $('body').getText()) ?? '';
}

/**
 * Imports the synthetic GEDCOM into a fresh folder below `<work>/OneDrive/` (so the sync warning shows) and lands on
 * the import report. Returns the project directory.
 */
export async function importSynthetic(name: string): Promise<string> {
  const parent = slash(path.join(work(), 'OneDrive', 'trees'));
  fs.mkdirSync(parent, { recursive: true });
  await click('Import a GEDCOM or Древо Жизни file');
  await pushDialog(slash(ged()));
  await click('Choose file…');
  await pushDialog(parent);
  await click('Choose folder…');
  const field = await $('aria/Tree name');
  await field.setValue(name);
  await click('Start import');
  await $('h1*=Imported').waitForExist({ timeout: 180000 });
  return `${parent}/${name}`;
}

export async function openTreeFromReport() {
  await click('Open the tree');
  await $('aria/Main').waitForExist({ timeout: 60000 });
}

export async function shot(file: string) {
  const dir = process.env.GT_E2E_SHOTS!;
  fs.mkdirSync(dir, { recursive: true });
  await browser.saveScreenshot(path.join(dir, file));
}

/** Opens the tree named `name` from the Welcome list (after a relaunch) or does nothing when a tree is already open. */
export async function ensureOpen(name: string) {
  await ready();
  if (await $('aria/Main').isExisting()) return;
  const row = await $(`//a[contains(@class,"row")][.//*[normalize-space(.)="${name}"]]`);
  await row.waitForClickable({ timeout: 20000 });
  await row.click();
  await $('aria/Main').waitForExist({ timeout: 60000 });
}

/** Starts the app again, like quitting and launching it: a new WebDriver session is a new process. */
export async function relaunch() {
  await browser.reloadSession();
  await ready();
}

export async function goTo(item: 'Family' | 'People' | 'Pedigree' | 'Descendants' | 'Sources' | 'Media' | 'Settings') {
  const a = await $(`//nav[@aria-label="Main"]//a[normalize-space(.)="${item}"]`);
  await a.waitForClickable();
  await a.click();
}

/** Opens the search palette and types the query. */
export async function search(query: string) {
  await $('body').click();
  await chord('k');
  const box = await $('input[role="combobox"]');
  await box.waitForDisplayed();
  await box.setValue(query);
}

/** Draws `count` gradient pictures on a canvas in the page and saves them as real JPEG files. */
export async function makeJpegs(dir: string, count: number): Promise<string[]> {
  fs.mkdirSync(dir, { recursive: true });
  const out: string[] = [];
  for (let i = 0; i < count; i++) {
    const b64 = await browser.execute((n: number) => {
      const c = document.createElement('canvas');
      c.width = 640;
      c.height = 480;
      const g = c.getContext('2d')!;
      const grad = g.createLinearGradient(0, 0, 640, 480);
      grad.addColorStop(0, ['#2f5d8a', '#8a4b2f', '#3f7a4a'][n % 3]);
      grad.addColorStop(1, ['#e8d9b0', '#d9c0a8', '#b9d4c0'][n % 3]);
      g.fillStyle = grad;
      g.fillRect(0, 0, 640, 480);
      g.fillStyle = 'rgba(255,255,255,0.85)';
      g.beginPath();
      g.arc(320, 190, 80, 0, Math.PI * 2);
      g.fill();
      g.fillRect(200, 290, 240, 150);
      g.fillStyle = '#222';
      g.font = '40px serif';
      g.fillText(`Portrait ${n + 1}`, 220, 465);
      return c.toDataURL('image/jpeg', 0.9).split(',')[1];
    }, i);
    const file = path.join(dir, `portrait-${i + 1}.jpg`);
    fs.writeFileSync(file, Buffer.from(b64, 'base64'));
    out.push(slash(file));
  }
  return out;
}

export { $, $$, browser };
