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
