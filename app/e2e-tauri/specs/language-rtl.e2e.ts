/**
 * The interface in Arabic (right to left) and in Simplified Chinese: the sidebar navigation and the Settings language list,
 * with the share of each partial language. Checks <html dir> and <html lang>, and that the English fallback still fills
 * every key the seed translations lack. Screenshots go to $GT_E2E_SHOTS (language-ar-rtl.png, language-zh-hans.png).
 *
 * Synthetic data only.
 */
import { expect } from '@wdio/globals';
import { $, browser, goTo, importSynthetic, openTreeFromReport, ready, shot } from './helpers';

/** The language select, found by its options so that it is found in any language. */
const languageSelect = () => $('//select[option[@value="zh-Hans"]]');

async function chooseLanguage(code: string) {
  const select = await languageSelect();
  await select.waitForDisplayed();
  await select.selectByAttribute('value', code);
  // The catalogue loads with a dynamic import; wait for the page to carry the language.
  await browser.waitUntil(async () => (await browser.execute(() => document.documentElement.lang)) !== '', { timeout: 10000 });
  await browser.pause(600);
}

const html = () => browser.execute(() => ({ dir: document.documentElement.dir, lang: document.documentElement.lang }));

describe('Right-to-left and CJK interface languages', () => {
  before(async () => {
    await ready();
  });

  it('shows Arabic right to left and Chinese left to right, with the language list', async () => {
    await importSynthetic('Language tree');
    await openTreeFromReport();
    await goTo('Settings');

    // The list has all 21 languages, the partial ones with their share.
    const labels = await languageSelect().then((s) => s.$$('option').map((o) => o.getText()));
    expect(labels.length).toBe(22); // 21 languages + "Same as the system"
    expect(labels).toContain('English');
    expect(labels).toContain('Українська');
    expect(labels.some((l) => l.startsWith('العربية — '))).toBe(true);
    expect(labels.some((l) => l.startsWith('中文简体 — '))).toBe(true);

    await chooseLanguage('ar');
    expect(await html()).toEqual({ dir: 'rtl', lang: 'ar' });
    // A translated sidebar label and a key the seed does not translate (still English).
    await $('//nav//a[normalize-space(.)="الأشخاص"]').waitForDisplayed({ timeout: 10000 });
    await $('//h1[normalize-space(.)="الإعدادات"]').waitForDisplayed();
    await shot('language-ar-rtl.png');

    await chooseLanguage('zh-Hans');
    expect(await html()).toEqual({ dir: 'ltr', lang: 'zh-Hans' });
    await $('//nav//a[normalize-space(.)="人物"]').waitForDisplayed({ timeout: 10000 });
    await shot('language-zh-hans.png');

    await chooseLanguage('en');
    expect(await html()).toEqual({ dir: 'ltr', lang: 'en' });
  });
});
