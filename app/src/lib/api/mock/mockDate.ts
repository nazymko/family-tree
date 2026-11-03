import type { DatePreview, DayOrder } from '../types';
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
