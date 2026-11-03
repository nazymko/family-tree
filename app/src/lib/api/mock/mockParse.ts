import type { DateDto, DatePreview, DayOrder } from '../types';

// A small subset of gt_core GenDate::parse, enough for the mock: exact dates, abt/bef/aft,
// between..and, d/m/y with a day order, a phrase in parentheses, else unparsed (raw kept verbatim).

const MONTHS = [
  'january', 'february', 'march', 'april', 'may', 'june',
  'july', 'august', 'september', 'october', 'november', 'december',
];
const MON = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

export interface ParsedDate {
  raw: string;
  display: string;
  status: 'ok' | 'phrase' | 'unparsed';
  sortMin: number | null;
  sortMax: number | null;
  understoodAs: string;
  range: string | null;
  warnings: string[];
}

interface Single {
  display: string;
  min: number;
  max: number;
  precise: boolean;
  warnings: string[];
}

function epochDay(y: number, m: number, d: number): number {
  const t = new Date(0);
  t.setUTCFullYear(y, m - 1, d);
  return Math.floor(t.getTime() / 86_400_000);
}

function daysIn(y: number, m: number): number {
  const t = new Date(0);
  t.setUTCFullYear(y, m, 0);
  return t.getUTCDate();
}

function monthIndex(token: string): number | null {
  const t = token.toLowerCase().replace(/\.$/, '');
  if (t.length < 3) return null;
  const i = MONTHS.findIndex((m) => m.startsWith(t));
  return i >= 0 ? i + 1 : null;
}

function build(y: number, m: number | null, d: number | null, warnings: string[] = []): Single | null {
  if (y < 1 || y > 9999) return null;
  if (m === null) {
    return { display: String(y), min: epochDay(y, 1, 1), max: epochDay(y, 12, 31), precise: false, warnings };
  }
  if (m < 1 || m > 12) return null;
  if (d === null) {
    return {
      display: `${MON[m - 1]} ${y}`,
      min: epochDay(y, m, 1),
      max: epochDay(y, m, daysIn(y, m)),
      precise: false,
      warnings,
    };
  }
  if (d < 1 || d > daysIn(y, m)) return null;
  const day = epochDay(y, m, d);
  return { display: `${d} ${MON[m - 1]} ${y}`, min: day, max: day, precise: true, warnings };
}

function parseSingle(text: string, order: DayOrder): Single | null {
  const s = text.trim().replace(/\s+/g, ' ');
  let m: RegExpExecArray | null;

  if ((m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s))) return build(+m[1], +m[2], +m[3]);

  if ((m = /^(\d{1,2})[/.-](\d{1,2})[/.-](\d{4})$/.exec(s))) {
    const a = +m[1];
    const b = +m[2];
    const [day, month] = order === 'dmy' ? [a, b] : [b, a];
    const res = build(+m[3], month, day);
    if (res && a <= 12 && b <= 12 && a !== b) {
      res.warnings.push(
        order === 'dmy'
          ? `Read as day first (${res.display}). Write the month as a word to avoid doubt.`
          : `Read as month first (${res.display}). Write the month as a word to avoid doubt.`,
      );
    }
    return res;
  }

  if ((m = /^(\d{1,2}) ([a-z]+)\.? (\d{1,4})$/i.exec(s))) {
    const mo = monthIndex(m[2]);
    return mo ? build(+m[3], mo, +m[1]) : null;
  }
  if ((m = /^([a-z]+)\.? (\d{1,2}),? (\d{1,4})$/i.exec(s))) {
    const mo = monthIndex(m[1]);
    return mo ? build(+m[3], mo, +m[2]) : null;
  }
  if ((m = /^([a-z]+)\.? (\d{3,4})$/i.exec(s))) {
    const mo = monthIndex(m[1]);
    return mo ? build(+m[2], mo, null) : null;
  }
  if ((m = /^(\d{3,4})$/.exec(s))) return build(+m[1], null, null);
  return null;
}

function rangeText(a: Single): string | null {
  if (a.precise) return null;
  const lo = fromEpoch(a.min);
  const hi = fromEpoch(a.max);
  return `${lo} to ${hi}`;
}

function fromEpoch(day: number): string {
  const t = new Date(day * 86_400_000);
  return `${t.getUTCDate()} ${MON[t.getUTCMonth()]} ${t.getUTCFullYear()}`;
}

function unparsed(raw: string): ParsedDate {
  return {
    raw,
    display: raw,
    status: 'unparsed',
    sortMin: null,
    sortMax: null,
    understoodAs: 'Kept as written. It is not used for sorting.',
    range: null,
    warnings: [],
  };
}

export function parseGenDate(raw: string, order: DayOrder): ParsedDate {
  const s = raw.trim().replace(/\s+/g, ' ');
  if (!s) return unparsed(raw);

  let m: RegExpExecArray | null;
  if ((m = /^\((.+)\)$/.exec(s))) {
    return {
      raw,
      display: m[1],
      status: 'phrase',
      sortMin: null,
      sortMax: null,
      understoodAs: `Kept as the phrase "${m[1]}". It is not used for sorting.`,
      range: null,
      warnings: [],
    };
  }

  const ok = (display: string, min: number | null, max: number | null, range: string | null, warnings: string[]): ParsedDate => ({
    raw,
    display,
    status: 'ok',
    sortMin: min,
    sortMax: max,
    understoodAs: display,
    range,
    warnings,
  });

  if ((m = /^(?:bet(?:ween)?\.?) (.+?) and (.+)$/i.exec(s)) || (m = /^from (.+?) to (.+)$/i.exec(s))) {
    const a = parseSingle(m[1], order);
    const b = parseSingle(m[2], order);
    if (!a || !b) return unparsed(raw);
    const lo = Math.min(a.min, b.min);
    const hi = Math.max(a.max, b.max);
    const word = /^from/i.test(s) ? ['from', 'to'] : ['between', 'and'];
    return ok(`${word[0]} ${a.display} ${word[1]} ${b.display}`, lo, hi, `${a.display} to ${b.display}`, [...a.warnings, ...b.warnings]);
  }

  const q = /^(abt|about|approx|circa|ca|c|est|bef|before|aft|after)\.? (.+)$/i.exec(s);
  if (q) {
    const a = parseSingle(q[2], order);
    if (!a) return unparsed(raw);
    const kind = q[1].toLowerCase();
    if (kind.startsWith('bef')) return ok(`before ${a.display}`, null, a.min - 1, null, a.warnings);
    if (kind.startsWith('aft')) return ok(`after ${a.display}`, a.max + 1, null, null, a.warnings);
    const pad = 2 * 365;
    return ok(`about ${a.display}`, a.min - pad, a.max + pad, `${fromEpoch(a.min - pad)} to ${fromEpoch(a.max + pad)}`, a.warnings);
  }

  const one = parseSingle(s, order);
  if (!one) return unparsed(raw);
  return ok(one.display, one.min, one.max, rangeText(one), one.warnings);
}

export function toDateDto(raw: string | undefined | null, order: DayOrder): DateDto | null {
  if (raw === undefined || raw === null || raw.trim() === '') return null;
  const p = parseGenDate(raw, order);
  return { raw, display: p.display, status: p.status, sortMin: p.sortMin, sortMax: p.sortMax };
}

export function toPreview(raw: string, order: DayOrder): DatePreview {
  const p = parseGenDate(raw, order);
  return { raw, understoodAs: p.understoodAs, range: p.range, status: p.status, warnings: p.warnings };
}

/** Year of the earliest day a parsed date can mean, or null when it has none. */
export function yearOf(d: DateDto | null): number | null {
  const day = d?.sortMin ?? d?.sortMax ?? null;
  return day === null ? null : new Date(day * 86_400_000).getUTCFullYear();
}
