/**
 * Fonts for text drawn outside the page's own layout: the chart canvas, the PNG and SVG pictures and the A3 print.
 *
 * The bundled fonts are split by script (unicode-range), and the browser fetches a part only when the page lays out a
 * letter in it. A canvas does not wait for that: the first picture with an old letter (ѣ, ѳ, ѵ of a metric book name)
 * would be painted in a fallback font. `loadChartFonts` fetches those parts first. The print frame is a separate
 * document, so it gets the page's `@font-face` rules (`fontFaceCss`) or it would print in Georgia, which has no ѣ.
 */

import { RECORDED_FAMILY } from './namesPref.svelte';

/** Old Cyrillic letters, with ordinary Cyrillic and Latin, so every part a name may use is fetched. */
export const SAMPLE_TEXT = 'ѢѣѲѳѴѵІіЪъЖжAa';

/** Loads the parts of the chart fonts that the sample letters need. Never throws; does nothing without a font API. */
export async function loadChartFonts(doc: Document = document): Promise<void> {
  const fonts = doc.fonts as FontFaceSet | undefined;
  if (!fonts || typeof fonts.load !== 'function') return;
  const css = getComputedStyle(doc.documentElement);
  const serif = css.getPropertyValue('--font-serif').trim();
  const sans = css.getPropertyValue('--font-sans').trim();
  const specs = [
    ...(serif ? [`600 16px ${serif}`, `16px ${serif}`] : []),
    ...(sans ? [`16px ${sans}`, `italic 16px ${sans}`, `600 16px ${sans}`] : []),
  ];
  await Promise.all([...specs.map((s) => fonts.load(s, SAMPLE_TEXT).catch(() => [])), loadOldTypeface(doc)]);
}

/**
 * Fetches the old typeface (Old Standard TT) parts the old letters need; resolves when they are ready. A canvas redraws
 * its cards when this resolves, so a name as recorded is not painted in the fallback font first. Never throws.
 */
export async function loadOldTypeface(doc: Document = document): Promise<void> {
  const fonts = doc.fonts as FontFaceSet | undefined;
  if (!fonts || typeof fonts.load !== 'function') return;
  const specs = [`600 16px ${RECORDED_FAMILY}`, `400 16px ${RECORDED_FAMILY}`, `italic 400 16px ${RECORDED_FAMILY}`];
  await Promise.all(specs.map((s) => fonts.load(s, SAMPLE_TEXT).catch(() => [])));
}

/** The `@font-face` rules of the page's style sheets, joined (for a document printed from a frame). */
export function fontFaceCss(doc: Document = document): string {
  const out: string[] = [];
  for (const sheet of Array.from(doc.styleSheets)) {
    let rules: CSSRuleList;
    try {
      rules = sheet.cssRules;
    } catch {
      continue; // a sheet from another origin cannot be read
    }
    for (const r of Array.from(rules)) {
      if (r.cssText.startsWith('@font-face')) out.push(r.cssText);
    }
  }
  return out.join('\n');
}
