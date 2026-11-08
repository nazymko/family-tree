/**
 * Six end-to-end flows on the real app (debug build, tauri-driver, WebView2 or WebKitGTK) over a synthetic 5k-person
 * GEDCOM. The tests share one imported project and run in order.
 */
import fs from 'node:fs';
import path from 'node:path';
import { expect } from '@wdio/globals';
import {
  $,
  $$,
  browser,
  button,
  chord,
  click,
  drop,
  ensureOpen,
  goTo,
  importSynthetic,
  makeJpegs,
  meta,
  openTreeFromReport,
  pushDialog,
  ready,
  relaunch,
  search,
  shot,
  slash,
  text,
  work, clearFocus
} from './helpers';

const TREE = 'Synthetic tree';
const CHILD = 'Zacharias';
const CHILD2 = 'Zephaniah';
const HITS = '[role="listbox"][aria-label="Search results"] [role="option"]';

/** Opens the add-child dialog for the person in focus and fills it; returns the page text with the date preview showing. */
async function fillChild(given: string, birth: string) {
  const menu = await $('button[aria-label="Add"][aria-haspopup="menu"]');
  await menu.waitForClickable();
  await menu.click();
  await $('//button[@role="menuitem"][normalize-space(.)="Add child"]').click();
  await $('aria/Given name').waitForDisplayed();
  await $('aria/Given name').setValue(given);
  await $('button=Type it another way (for example, about 1850)').click();
  await $('aria/Birth date').setValue(birth);
  await $('//*[@role="status"][contains(.,"Understood as")]').waitForExist({ timeout: 10000 });
  return text();
}

async function saveChild() {
  await click('Add child');
  await $('aria/Given name').waitForDisplayed({ reverse: true });
}

/** How many people the search palette finds for `given`. */
async function findPerson(given: string): Promise<number> {
  await search(given);
  await browser.pause(800);
  const n = (await $$(HITS)).length;
  await browser.keys('Escape');
  return n;
}

describe('Family Tree end to end', () => {
  before(async () => {
    await ready();
  });

  it('import_report_then_family_view', async () => {
    const dir = await importSynthetic(TREE);
    const body = await text();
    expect(body).toContain('Imported 5,000 people');
    expect(body).toMatch(/Families\s*\d+/);
    expect(body).toMatch(/Sources\s*\d+/);
    // The media references in the file point at pictures that are not there.
    expect(body).toContain('Photos and files we could not find');
    // The project folder is below a folder called OneDrive, so the sync warning shows.
    expect(body).toContain('This folder is inside OneDrive');
    expect(fs.existsSync(dir)).toBe(true);
    await openTreeFromReport();
    const heading = await $('h1');
    await heading.waitForDisplayed();
    await expect(heading).toHaveText(expect.stringContaining(meta().home.surname));
    expect(await text()).toContain('5,000 people');
  });

  it('add_child_edit_undo_redo_persists_across_restart', async () => {
    await ensureOpen(TREE);
    expect(await fillChild(CHILD, 'abt 1850')).toContain('Understood as: about 1850');
    await saveChild();
    expect(await text()).toContain(CHILD);

    await relaunch();
    await ensureOpen(TREE);
    await search(CHILD);
    await $(HITS).waitForDisplayed();
    expect((await $$(HITS)).length).toBe(1);
    await browser.keys('Enter');
    await $(`//h1[contains(.,"${CHILD}")]`).waitForDisplayed();
    expect(await text()).toContain('about 1850');

    // Second session: add another child, undo it, redo it, undo it again.
    await search(meta().home.surname);
    await $(HITS).waitForDisplayed();
    await browser.keys('Enter');
    await $('//h1[contains(.,"' + meta().home.surname + '")]').waitForDisplayed();
    await fillChild(CHILD2, '1851');
    await saveChild();
    expect(await findPerson(CHILD2)).toBe(1);
    await clearFocus();
    await chord('z');
    await browser.waitUntil(async () => (await findPerson(CHILD2)) === 0, { timeoutMsg: 'undo did not remove the child' });
    await clearFocus();
    await chord('z', true);
    await browser.waitUntil(async () => (await findPerson(CHILD2)) === 1, { timeoutMsg: 'redo did not bring the child back' });
    await clearFocus();
    await chord('z');
    await browser.waitUntil(async () => (await findPerson(CHILD2)) === 0);

    await relaunch();
    await ensureOpen(TREE);
    expect(await findPerson(CHILD2)).toBe(0);
    expect(await findPerson(CHILD)).toBe(1);
  });

  it('export_preview_matches_file', async () => {
    await ensureOpen(TREE);
    await click('Export');
    await $('[role="region"][aria-label="Privacy preview"]').waitForDisplayed();
    const dd = await $('//dt[normalize-space(.)="Shown as \'Living\'"]/following-sibling::dd');
    await dd.waitForDisplayed({ timeout: 30000 });
    const placeholders = Number((await dd.getText()).replace(/\D/g, '')); // shown with a thousands separator
    expect(placeholders).toBeGreaterThan(0);

    const out = slash(path.join(work(), 'export.ged'));
    await pushDialog(out);
    const exportButton = await button('Export…');
    await exportButton.waitForEnabled({ timeout: 30000 });
    await exportButton.click();
    await browser.waitUntil(() => fs.existsSync(out) && fs.statSync(out).size > 0, { timeout: 60000, timeoutMsg: 'no export file' });
    await browser.pause(500);
    const file = fs.readFileSync(out, 'utf8');
    const livingNames = file.split(/\r?\n/).filter((l) => /^1 NAME Living(\s*\/\/)?\s*$/.test(l)).length;
    expect(livingNames).toBe(placeholders);
    // Whole surnames: "Barjunlom1" is a prefix of the unrelated "Barjunlom11".
    const surnamesInFile = new Set(Array.from(file.matchAll(/^\d NAME [^/\r\n]*\/([^/\r\n]*)\//gm), (m) => m[1]));
    expect(meta().uniqueLivingSurnames.length).toBeGreaterThan(0);
    expect(meta().uniqueLivingSurnames.filter((s) => surnamesInFile.has(s))).toEqual([]);
    expect(surnamesInFile.size).toBeGreaterThan(10);
    await click('Close');
  });

  it('drop_files_attach', async () => {
    await ensureOpen(TREE);
    await goTo('Media');
    await $('h1=Media').waitForDisplayed();
    const dir = path.join(work(), 'drop');
    const [a, b] = await makeJpegs(dir, 2);
    const exe = slash(path.join(dir, 'setup.exe'));
    fs.writeFileSync(exe, 'MZ not a picture');
    await $('button=Add files…').waitForDisplayed();
    const thumbs = () => $$('ul[aria-label="Photos and documents"] img');
    const before = (await thumbs()).length;
    await drop([a, b, exe]);
    await browser.waitUntil(async () => (await thumbs()).length === before + 2, { timeoutMsg: 'two thumbnails should appear' });
    const errors = await $$('//li[contains(.,"setup.exe")]');
    expect(errors.length).toBe(1);
    expect(await errors[0].getText()).toContain('unsupported type');
    await browser.waitUntil(
      () =>
        browser.execute(
          () => Array.from(document.querySelectorAll<HTMLImageElement>('ul[aria-label="Photos and documents"] img')).filter((i) => i.naturalWidth > 0).length >= 2,
        ),
      { timeoutMsg: 'thumbnails did not load through gtmedia' },
    );
  });

  it('pedigree_outline_and_chart_agree', async () => {
    await ensureOpen(TREE);
    // The line this person belongs to has recorded ancestors for at least three generations.
    await search(meta().home.name);
    await $(HITS).waitForDisplayed();
    await browser.keys('Enter');
    await $(`//h1[contains(.,"${meta().home.given}")]`).waitForDisplayed();
    await goTo('Pedigree');
    await $('h1=Pedigree').waitForDisplayed();
    await $('canvas[aria-label="Pedigree chart"]').waitForDisplayed();
    await browser.pause(1500);
    await browser.execute(() => document.querySelector<HTMLElement>('canvas[aria-label="Pedigree chart"]')!.focus());
    await browser.keys('Home');
    // Walk three steps towards the ancestors (always the father): the selected person is then in generation 3 and the live region counts that generation.
    const announced = async () => {
      for (const el of await $$('div.sr-only[role="status"]')) {
        const t = await el.getText();
        if (/in this generation/.test(t)) return t;
      }
      return '';
    };
    let said = '';
    for (let i = 0; i < 3; i++) {
      await browser.keys('ArrowRight');
      await browser.pause(400);
      said = await announced();
    }
    // "ancestor 8" is Ahnentafel number 8: generation 3 holds the numbers 8 to 15.
    const m = /ancestor (\d+),(?: [^,]+,)? (\d+) of (\d+) in this generation/.exec(said);
    if (!m || Number(m[1]) < 8 || Number(m[1]) > 15) throw new Error(`live region said: "${said}"`);
    const chartCount = Number(m[3]);
    expect(chartCount).toBeGreaterThan(0);

    await $('//*[@role="tab"][normalize-space(.)="Outline"]').click();
    await $('[role="tree"]').waitForDisplayed();
    // Expand three levels with the keyboard, then count the items in generation 3 (level 4).
    for (let level = 1; level <= 3; level++) {
      await browser.execute((lv: number) => {
        document.querySelectorAll<HTMLElement>(`[role="treeitem"][aria-level="${lv}"][aria-expanded="false"]`).forEach((li) => {
          li.focus();
          li.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
        });
      }, level);
      // The rows load on demand: the next level can only be expanded once they are there.
      await browser.waitUntil(() => browser.execute((lv: number) => document.querySelectorAll(`[role="treeitem"][aria-level="${lv}"]`).length > 0, level + 1), {
        timeout: 15000,
        timeoutMsg: `the outline should show level ${level + 1}`,
      });
    }
    const outlineCount = () => browser.execute(() => document.querySelectorAll('[role="treeitem"][aria-level="4"]').length);
    await browser.waitUntil(async () => (await outlineCount()) === chartCount, {
      timeout: 15000,
      timeoutMsg: `the outline should show ${chartCount} people in generation 3`,
    });
    expect(await outlineCount()).toBe(chartCount);
  });

  it('rtl_and_cjk_names_visible', async () => {
    await ensureOpen(TREE);
    await goTo('People');
    const filter = await $('[data-filter-input]');
    await filter.waitForDisplayed();
    for (const script of ['cjk', 'heb', 'cyr'] as const) {
      const s = meta().scripts[script];
      await filter.setValue(s.surname);
      await browser.waitUntil(async () => (await text()).includes(s.surname), { timeoutMsg: `no ${script} surname in the list` });
      const dirs = await browser.execute((sur: string) => {
        return Array.from(document.querySelectorAll<HTMLElement>('[role="option"] bdi'))
          .filter((b) => (b.textContent ?? '').includes(sur))
          .map((b) => b.getAttribute('dir'));
      }, s.surname);
      expect(dirs.length).toBeGreaterThan(0);
      expect(dirs.every((d) => d === 'auto')).toBe(true);
    }
  });

  it('settings_about_shows_the_version_without_asking_the_network', async () => {
    await ensureOpen(TREE);
    await goTo('Settings');
    const line = await $('[data-testid="version-line"]');
    await line.waitForDisplayed({ timeout: 20000 });
    // A local build reads "Version 0.3.0+12 (a1b2c3d, 8 Nov 2026)", a release "Version 0.3.0". The check button is not pressed: no network in the specs.
    expect(await line.getText()).toMatch(/^Version \d+\.\d+\.\d+/);
    await $('button=Check for a newer version').waitForDisplayed();
    await line.scrollIntoView();
    await shot('about-section.png');
  });
});
