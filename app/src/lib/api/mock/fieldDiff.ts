// Mirror of crates/gt-session/src/field_diff.rs over the mock's person records: which sex and name
// fields went from what to what. Same field ids and tokens; the mock has no proof, confidence,
// raw or unknown fields, so those never appear here.

import type { FieldChangeDto, NameDto } from '../types';
import type { PersonRec } from './model';

type Fields = [string, string | null][];

const yes = (b: boolean) => (b ? 'yes' : 'no');
const kindToken = (n: NameDto) => (n.kind === 'other' ? `other:${n.kindText ?? ''}` : n.kind);
const sexToken = (p: PersonRec) => (p.sex === 'other' ? `other:${p.sexText ?? ''}` : p.sex);
const count = (n: number) => (n > 0 ? String(n) : null);

function nameFields(n: NameDto): Fields {
  return [
    ['kind', kindToken(n)],
    ['preferred', yes(n.preferred)],
    ['private', yes(n.private)],
    ['prefix', n.prefix],
    ['given', n.given],
    ['call', n.call],
    ['surnamePrefix', n.surnamePrefix],
    ['surnames', n.surnames.length > 0 ? n.surnames.join(', ') : null],
    ['patronymic', n.patronymic],
    ['suffix', n.suffix],
    ['nickname', n.nickname],
    ['lang', n.lang],
    ['date', n.date ? n.date.raw : null],
    ['citations', count(n.citations.length)],
  ];
}

export function personFieldChanges(before: PersonRec, after: PersonRec): FieldChangeDto[] {
  const out: FieldChangeDto[] = [];
  if (before.sex !== after.sex || before.sexText !== after.sexText) {
    out.push({ field: 'sex', before: sexToken(before), after: sexToken(after) });
  }
  const several = before.names.length > 1 || after.names.length > 1;
  const at = (pos: number, n: NameDto) => (several ? { nameNumber: pos + 1, nameKind: kindToken(n) } : {});
  const keptBefore = before.names.filter((n) => after.names.some((o) => o.id === n.id)).map((n) => n.id);
  const keptAfter = after.names.filter((n) => before.names.some((o) => o.id === n.id)).map((n) => n.id);
  after.names.forEach((n, pos) => {
    const old = before.names.find((b) => b.id === n.id);
    if (old) {
      const was = nameFields(old);
      nameFields(n).forEach(([field, a], i) => {
        if (was[i][1] !== a) out.push({ field, ...at(pos, n), before: was[i][1], after: a });
      });
      const rb = keptBefore.indexOf(n.id);
      const ra = keptAfter.indexOf(n.id);
      if (rb !== ra) out.push({ field: 'position', ...at(pos, n), before: String(rb + 1), after: String(ra + 1) });
    } else {
      for (const [field, a] of nameFields(n)) if (a !== null) out.push({ field, ...at(pos, n), before: null, after: a });
    }
  });
  before.names.forEach((n, pos) => {
    if (after.names.some((a) => a.id === n.id)) return;
    for (const [field, b] of nameFields(n)) if (b !== null) out.push({ field, ...at(pos, n), before: b, after: null });
  });
  return out;
}
