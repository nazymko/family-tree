// The mock's copy of gt-core's `orthography::modern_variants`: a name in the old (pre-1918) spelling is found by today's
// letters too. Only letters are mapped; there is no dictionary of names.

const OLD = /[ѣѢѳѲѵѴѷѶѡѠѯѮѱѰѕЅѧѦ]|\p{L}[ъЪ](?!\p{L})/u;
const READINGS: [string, string, string][] = [
  ['і', 'і', 'і'],
  ['е', 'и', 'и'],
  ['е', 'і', 'і'],
];

/** The text in today's letters, lower case, in each common reading; empty when it has no old spelling. */
export function modernVariants(s: string): string[] {
  if (!OLD.test(s)) return [];
  const lower = [...s.toLowerCase()];
  const out: string[] = [];
  for (const [yat, i, izhitsa] of READINGS) {
    let v = '';
    lower.forEach((c, k) => {
      const map: Record<string, string> = { ѣ: yat, і: i, ѵ: izhitsa, ѷ: izhitsa, ѳ: 'ф', ѡ: 'о', ѯ: 'кс', ѱ: 'пс', ѕ: 'з', ѧ: 'я' };
      if (c === 'ъ' && k > 0 && /\p{L}/u.test(lower[k - 1]) && !/\p{L}/u.test(lower[k + 1] ?? '')) return;
      v += map[c] ?? c;
    });
    if (!out.includes(v)) out.push(v);
  }
  return out;
}
