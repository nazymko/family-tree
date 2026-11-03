import type { DateDto, DatePreview, DayOrder } from '../types';
import { toPreview } from './mockParse';

// The live-preview used by MockApi.parseDate. It layers dual (Old Style / New Style) years such as
// "1750/51" on top of mockParse; exact dates, abt/bef/aft, between..and, from..to, d/m/y with a day
// order, a phrase in parentheses and unparsed text are handled there. The real parser is gt_core
// (crates/gt-core/src/date/parse.rs); this file only has to be good enough for the browser mock.

const DUAL = /\b(\d{4})\/(\d{1,2})\b/g;

function dualLabel(year: number, tail: string): string {
  const t = tail.length === 1 ? `0${tail}` : tail;
  return `${year}/${t}`;
}

export function previewDate(raw: string, order: DayOrder): DatePreview {
  const dual: { year: number; label: string }[] = [];
  const plain = raw.replace(DUAL, (_m, y: string, t: string) => {
    dual.push({ year: +y, label: dualLabel(+y, t) });
    return y;
  });
  if (dual.length === 0) return toPreview(raw, order);
  const p = toPreview(plain, order);
  if (p.status !== 'ok') return { ...p, raw };
  let understoodAs = p.understoodAs;
  let range = p.range;
  for (const d of dual) {
    understoodAs = understoodAs.replace(String(d.year), d.label);
    range = range ? range.replace(String(d.year), d.label) : range;
  }
  return {
    raw,
    understoodAs,
    range,
    status: 'ok',
    warnings: [...p.warnings, 'Double year: the first year is Old Style, the second New Style.'],
  };
}

/**
 * The year as STATED in a date, with a qualifier, for lifespans: about/est/calc and between/from..to
 * give "c.", before gives "bef.", after gives "aft.", an exact date gives no qualifier.
 */
export function statedYear(d: DateDto | null): { year: number; q: '' | 'c.' | 'bef.' | 'aft.' } | null {
  if (!d || d.status !== 'ok') return null;
  const m = /\b(\d{3,4})\b/.exec(d.display);
  if (!m) return null;
  const q = /^about /.test(d.display)
    ? 'c.'
    : /^before /.test(d.display)
      ? 'bef.'
      : /^after /.test(d.display)
        ? 'aft.'
        : /^(between|from) /.test(d.display)
          ? 'c.'
          : '';
  return { year: +m[1], q };
}

function stated(d: DateDto | null): string | null {
  const s = statedYear(d);
  return s ? `${s.q ? `${s.q} ` : ''}${s.year}` : null;
}

/** Lifespan text: "c. 1968 – 2021", "? – 2021", "b. 1850". Empty when neither date is known. */
export function lifespanText(birth: DateDto | null, death: DateDto | null): string {
  const b = stated(birth);
  const d = stated(death);
  if (b && d) return `${b} – ${d}`;
  if (b) return `b. ${b}`;
  if (d) return `? – ${d}`;
  return '';
}
