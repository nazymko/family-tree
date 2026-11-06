// A date as separate year, month and day, each kept as the text of its control ('' when not given). The raw text they
// make uses English month words, which the backend parser reads the same in any language and day order.

export interface DateParts {
  year: string;
  /** '1' to '12'. */
  month: string;
  day: string;
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const FULL = ['january', 'february', 'march', 'april', 'may', 'june', 'july', 'august', 'september', 'october', 'november', 'december'];

const isYear = (y: string) => /^\d{1,4}$/.test(y) && Number(y) >= 1;

/** Days in a month of the Gregorian calendar, the one the parser assumes for a date with no calendar named. */
export function daysInMonth(year: number, month: number): number {
  if (month === 2) return (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0 ? 29 : 28;
  return [4, 6, 9, 11].includes(month) ? 30 : 31;
}

/** The raw date text, or '' while there is no valid year: a month or day alone is not a date. */
export function composeDate({ year, month, day }: DateParts): string {
  if (!isYear(year)) return '';
  if (!month) return year;
  const name = MONTHS[Number(month) - 1];
  return day ? `${day} ${name} ${year}` : `${name} ${year}`;
}

/** The parts of a raw date, or null when they cannot show it exactly ("about 1850", a phrase, 03.04.1901). */
export function parseDateParts(raw: string): DateParts | null {
  const text = raw.trim();
  if (!text) return { year: '', month: '', day: '' };
  const m = /^(?:(?:(\d{1,2})\s+)?([a-z]+)\s+)?(\d{1,4})$/i.exec(text);
  if (!m) return null;
  const [, day, word, year] = m;
  if (!isYear(year)) return null;
  if (!word) return { year, month: '', day: '' };
  const w = word.toLowerCase();
  const index = FULL.findIndex((name) => name === w || name.slice(0, 3) === w);
  if (index < 0) return null;
  if (day && (Number(day) < 1 || Number(day) > daysInMonth(Number(year), index + 1))) return null;
  return { year, month: String(index + 1), day: day ? String(Number(day)) : '' };
}

export type DateKind = 'exact' | 'about' | 'before' | 'after' | 'between';

/** One date or range as the date picker shows it: a kind and one date (two for "between"). */
export interface DateChoice {
  kind: DateKind;
  a: DateParts;
  b: DateParts;
}

const EMPTY: DateParts = { year: '', month: '', day: '' };
const QUALIFIER: Record<string, DateKind> = { about: 'about', abt: 'about', bef: 'before', before: 'before', aft: 'after', after: 'after' };

export const emptyChoice = (): DateChoice => ({ kind: 'exact', a: { ...EMPTY }, b: { ...EMPTY } });

/**
 * The raw text for a choice, in words the backend parser reads ("about 1850", "between 1 Jan 1850 and 1860"), or null
 * while a range is still missing a year (nothing is written then, so half a range never replaces the text).
 */
export function composeChoice(c: DateChoice): string | null {
  const a = composeDate(c.a);
  if (c.kind === 'exact') return a;
  if (c.kind === 'between') {
    const b = composeDate(c.b);
    return a && b ? `between ${a} and ${b}` : null;
  }
  return a ? `${c.kind} ${a}` : null;
}

/** The choice that shows a raw date exactly, or null for anything else (a phrase, 03.04.1901, "from … to …"). */
export function parseChoice(raw: string): DateChoice | null {
  const text = raw.trim();
  const between = /^bet(?:ween)?\.?\s+(.+?)\s+and\s+(.+)$/i.exec(text);
  if (between) {
    const a = parseDateParts(between[1]);
    const b = parseDateParts(between[2]);
    return a && b ? { kind: 'between', a, b } : null;
  }
  const q = /^(about|abt|bef|before|aft|after)\.?\s+(.+)$/i.exec(text);
  if (q) {
    const a = parseDateParts(q[2]);
    return a ? { kind: QUALIFIER[q[1].toLowerCase()], a, b: { ...EMPTY } } : null;
  }
  const a = parseDateParts(text);
  return a ? { kind: 'exact', a, b: { ...EMPTY } } : null;
}

/** "2024-05-07" (a native date control's value) to parts; null for an empty or malformed value. */
export function partsFromIso(iso: string): DateParts | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!m || Number(m[1]) < 1) return null;
  return { year: String(Number(m[1])), month: String(Number(m[2])), day: String(Number(m[3])) };
}

/** Parts to a native date control's value, or '' unless year, month and day are all given. */
export function isoFromParts({ year, month, day }: DateParts): string {
  if (!isYear(year) || !month || !day) return '';
  return `${year.padStart(4, '0')}-${month.padStart(2, '0')}-${day.padStart(2, '0')}`;
}
