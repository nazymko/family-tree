/**
 * The story view of a note that has no written story: the original text with the reference clutter taken out
 * (archive marks, pages, record numbers, file names, hypothesis tags, FamilySearch ids), so what is left reads
 * as plain sentences. A pure function; the research view always shows the original text.
 */

/** Letters of any alphabet (Cyrillic included): `\b` does not know them. */
const L = '\\p{L}';
const NOT_LETTER_BEFORE = `(?<![${L}\\d])`;
const NOT_LETTER_AFTER = `(?![${L}\\d])`;

const RULES: RegExp[] = [
  // Web addresses (FamilySearch ark links and the like).
  /https?:\/\/\S+/giu,
  /\bark:\/\S+/giu,
  // File names: scan.jpg, page_12.pdf, 0042.png
  new RegExp(`${NOT_LETTER_BEFORE}[\\p{L}\\p{N}_()\\-./\\\\]*[\\p{L}\\p{N}_)]\\.(?:jpe?g|png|pdf|tiff?|gif|webp)${NOT_LETTER_AFTER}`, 'giu'),
  // Archive shelf marks: ДАХеО 259-51-14, ЦДІАК 127-1012-59, ДАХмО 315-1-7189
  new RegExp(`${NOT_LETTER_BEFORE}\\p{Lu}{2,}[${L}]{0,4}\\s*\\d{1,5}-\\d{1,5}-\\d{1,6}[${L}]?${NOT_LETTER_AFTER}`, 'gu'),
  // Fond, inventory, file and sheet: ф. 259, оп. 51, спр. 14, арк. 5зв
  new RegExp(`${NOT_LETTER_BEFORE}(?:ф|оп|спр|арк)\\.\\s*\\d[\\d\\-–/]*(?:\\s*(?:зв|об)\\.?)?[${L}]?(?![\\d])`, 'giu'),
  // Pages: PDF с. 29, с. 29, стор. 29-30, p. 5
  new RegExp(`(?:PDF\\s*)?${NOT_LETTER_BEFORE}(?:с|стор|pp?|pag)\\.\\s*\\d+(?:\\s*[-–]\\s*\\d+)?`, 'giu'),
  // Record numbers: № 47, ж. № 38, запис № 5
  new RegExp(`(?:${NOT_LETTER_BEFORE}(?:ж|ч|запис|акт)\\.?\\s*)?(?:№|N[oо]\\.?)\\s*\\d+[${L}]?`, 'giu'),
  // Hypothesis tags: ГІПОТЕЗИ H24, гіпотеза H7, P29, H7
  new RegExp(`${NOT_LETTER_BEFORE}гіпотез[аиі]\\s*[HP]?\\s*\\d+`, 'giu'),
  new RegExp(`${NOT_LETTER_BEFORE}[HP]\\d{1,3}${NOT_LETTER_AFTER}`, 'gu'),
  // FamilySearch ids: DGS 4810397, image i782
  new RegExp(`${NOT_LETTER_BEFORE}DGS\\s*\\d+`, 'giu'),
  new RegExp(`${NOT_LETTER_BEFORE}i\\d{3,}${NOT_LETTER_AFTER}`, 'gu'),
];

/** A label left with nothing after it: "Джерело:" alone on a line. */
const LONELY_LABEL = /^\s*(?:джерел[ао]|джерела|source|sources|архів|archive)\s*:?\s*$/iu;

/** Removes the references from one line and tidies what they leave behind. */
function cleanLine(line: string): string {
  let s = line;
  for (const re of RULES) s = s.replace(re, '');
  // Whatever stood between the references: empty brackets, doubled or hanging punctuation.
  for (let i = 0; i < 4; i++) {
    const before = s;
    s = s
      .replace(/\(\s*[,;:\-–\s]*\)/g, '')
      .replace(/\[\s*[,;:\-–\s]*\]/g, '')
      .replace(/«\s*»/g, '')
      .replace(/\(\s*[,;:]\s*/g, '(')
      .replace(/\s*[,;:]\s*\)/g, ')')
      .replace(/([,;:])(\s*[,;:])+/g, '$1')
      .replace(/\s+([,.;:!?)])/g, '$1')
      .replace(/([(\[«])\s+/g, '$1')
      .replace(/[ \t]{2,}/g, ' ')
      .replace(/\s*[,;:–-]\s*([.!?])/g, '$1');
    if (s === before) break;
  }
  return s.replace(/^[\s,;:–-]+(?=\p{L}|\d|["«(])/u, '').trim();
}

/** The note text without its references, ready to read as sentences. Empty when nothing readable is left. */
export function cleanForStory(text: string): string {
  const lines = text
    .replace(/\r\n?/g, '\n')
    .split('\n')
    .map(cleanLine)
    .filter((l, i, all) => !LONELY_LABEL.test(l) && !(l === '' && (i === 0 || all[i - 1] === '')));
  return lines.join('\n').trim();
}

/** What a note shows in the story view: its written story, else its text without the references. */
export interface StoryView {
  text: string;
  /** True when `text` is the cleaned research text and not a written story: show the "shortened" hint. */
  cleaned: boolean;
}

export function storyOf(note: { text: string; story?: string | null }): StoryView {
  const written = note.story?.trim();
  if (written) return { text: written, cleaned: false };
  const text = cleanForStory(note.text);
  // Nothing left to read: show the original rather than an empty note.
  if (!text) return { text: note.text, cleaned: false };
  return { text, cleaned: text !== note.text.trim() };
}
