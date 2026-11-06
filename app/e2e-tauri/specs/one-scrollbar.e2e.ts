/**
 * The window itself never scrolls, on the REAL app (debug build, tauri-driver, WebView2): in a window shorter than the
 * sidebar, the sidebar scrolls by itself and each page scrolls in its own pane, so no outer scrollbar stands beside them.
 *
 * Synthetic data only.
 */
import { $, browser, click, expectNoWindowScroll, importSynthetic, openTreeFromReport, ready, shot } from './helpers';

const PAGES = ['Family', 'People', 'Whole tree', 'Pedigree', 'Descendants', 'Relatives'];

describe('One scrollbar', () => {
  before(async () => {
    await ready();
  });

  after(async () => {
    await browser.setWindowSize(1440, 900);
  });

  it('keeps the window still on every page, in a tall and in a short window', async () => {
    await importSynthetic('Scroll tree');
    await openTreeFromReport();

    for (const [w, h] of [
      [1440, 900],
      [1440, 560],
      [1000, 560],
    ]) {
      await browser.setWindowSize(w, h);
      for (const page of PAGES) {
        const a = await $(`//nav//a[normalize-space(.)="${page}"]`);
        await a.scrollIntoView({ block: 'center' });
        await a.waitForClickable();
        await a.click();
        await browser.pause(600);
        await expectNoWindowScroll(`${page} at ${w}x${h}`);
      }
      const family = await $('//nav//a[normalize-space(.)="Family"]');
      await family.scrollIntoView({ block: 'center' });
      await family.click();
      await click('Edit person');
      await browser.pause(600);
      await expectNoWindowScroll(`person page at ${w}x${h}`);
    }

    await browser.setWindowSize(1440, 560);
    const pedigree = await $('//nav//a[normalize-space(.)="Pedigree"]');
    await pedigree.scrollIntoView({ block: 'center' });
    await pedigree.click();
    await browser.pause(800);
    await shot('one-scrollbar-short-pedigree.png');
  });
});
