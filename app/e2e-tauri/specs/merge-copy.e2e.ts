/**
 * Send a copy, open it as the relative, edit, send it back, merge, undo - on the REAL app (debug build, tauri-driver,
 * WebView2). The owner imports the synthetic GEDCOM, sends a zip, opens that zip as a tree of its own (the relative),
 * adds a note to the home person, sends that tree back as a zip, reopens the owner's tree and merges the zip.
 * The note must show in the preview, come across on Merge, and go again on "Undo this merge".
 *
 * Synthetic data only. NOT RUN by the author of this spec: window tests were not allowed in that session.
 */
import path from 'node:path';
import fs from 'node:fs';
import { expect } from '@wdio/globals';
import { $, browser, click, importSynthetic, meta, openTreeFromReport, pushDialog, ready, search, shot, slash, text, work } from './helpers';

const NOTE = 'Olena Koval remembers the mill by the river.';

async function openHome() {
  await search(meta().home.name);
  await $('[role="listbox"] [role="option"]').waitForDisplayed();
  await browser.keys('Enter');
  await $(`//h1[contains(.,"${meta().home.given}")]`).waitForDisplayed();
}

/** Export dialog -> "Send a copy as a zip…" -> "Make the zip…", saving to `zip`. */
async function sendCopy(zip: string) {
  await click('Export');
  await click('Send a copy as a zip…');
  // The warning must name what the copy holds, and the size, before anything is made.
  const body = await text();
  expect(body).toContain('Living people');
  expect(body).toContain('Records you marked private.');
  expect(body).toContain('old text and text you have since removed');
  expect(body).toContain('reports from assistants');
  expect(body).toMatch(/Size of the file: about/);
  await pushDialog(zip);
  await click('Make the zip…');
  await browser.waitUntil(() => Promise.resolve(fs.existsSync(zip)), { timeout: 60000, timeoutMsg: 'the zip was never written' });
}

describe('Send a copy, edit, send back, merge, undo', () => {
  const dir = path.join(work(), 'merge-e2e');
  const out = slash(path.join(dir, 'to-olena.zip'));
  const back = slash(path.join(dir, 'from-olena.zip'));
  const relativeFolder = slash(path.join(dir, 'olena-copy'));
  let ownerDir = '';

  before(async () => {
    fs.rmSync(dir, { recursive: true, force: true });
    fs.mkdirSync(dir, { recursive: true });
    await ready();
  });

  it('sends a copy as a zip', async () => {
    ownerDir = await importSynthetic('Merge owner');
    await openTreeFromReport();
    await sendCopy(out);
    await shot('merge-send-en.png');
  });

  it('opens the zip as the relative and adds a note', async () => {
    await click('Close tree');
    await pushDialog(out, relativeFolder);
    await click('Open a received copy');
    await $('aria/Main').waitForExist({ timeout: 60000 });
    await openHome();
    await click('Edit person');
    await click('Add note');
    const box = await $('textarea');
    await box.waitForDisplayed();
    await box.setValue(NOTE);
    await click('Save note');
    await $('textarea').waitForDisplayed({ reverse: true, timeout: 20000 });
  });

  it('sends the edited tree back as a zip', async () => {
    await sendCopy(back);
  });

  it('shows the note in the preview and merges it', async () => {
    await click('Close tree');
    await pushDialog(ownerDir);
    await click('Open a tree');
    await $('aria/Main').waitForExist({ timeout: 60000 });
    await click("Merge a relative's copy");
    await pushDialog(back);
    await click('Choose a zip…');
    await click('Look at it');
    await $('//*[contains(.,"Nothing has been changed yet.")]').waitForDisplayed({ timeout: 60000 });
    expect(await text()).toContain(NOTE);
    await shot('merge-preview-en.png');
    await click('Merge 1');
    await $('//*[contains(.,"A backup of your tree was made first")]').waitForDisplayed({ timeout: 60000 });
  });

  it('undoes the merge', async () => {
    await click('Undo this merge');
    await click('Undo');
    await $('//*[contains(.,"undone")]').waitForExist({ timeout: 30000 });
    await click('Done');
    await openHome();
    expect(await text()).not.toContain(NOTE);
  });
});
