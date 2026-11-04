import { describe, expect, it } from 'vitest';
import { en } from './en';
import { uk } from './uk';

type Tree = { [k: string]: string | Tree };

/** Every leaf as [dotted path, text]; a plural entry is expanded to path.form. */
function leaves(node: Tree, prefix = ''): [string, string][] {
  return Object.entries(node).flatMap(([k, v]) =>
    typeof v === 'string' ? [[prefix + k, v] as [string, string]] : leaves(v, `${prefix}${k}.`),
  );
}

const placeholders = (text: string) => [...new Set([...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]))].sort();

const base = leaves(en as unknown as Tree);
const other = new Map(leaves(uk as unknown as Tree));

describe('check-i18n: the Ukrainian catalogue matches the English one', () => {
  it('has_no_missing_or_empty_key', () => {
    const bad = base.filter(([path, text]) => text.trim() !== '' && (other.get(path) ?? '').trim() === '').map(([p]) => p);
    expect(bad).toEqual([]);
  });

  it('has_no_key_english_lacks', () => {
    const known = new Set(base.map(([p]) => p));
    // A plural form that only Ukrainian needs (few, many) is allowed.
    const extra = [...other.keys()].filter((p) => !known.has(p) && !/\.(few|many)$/.test(p));
    expect(extra).toEqual([]);
  });

  it('keeps_the_same_placeholders', () => {
    const bad = base
      .filter(([path, text]) => other.has(path) && placeholders(text).join() !== placeholders(other.get(path)!).join())
      .map(([p]) => p);
    expect(bad).toEqual([]);
  });

  it('gives_every_plural_the_forms_ukrainian_needs', () => {
    const names = new Set(base.map(([p]) => p));
    const plurals = [...names].filter((p) => p.endsWith('.other') && names.has(p.slice(0, -'.other'.length) + '.one')).map((p) => p.slice(0, -'.other'.length));
    for (const p of plurals) {
      for (const form of ['one', 'few', 'many', 'other']) expect(other.get(`${p}.${form}`) ?? '', `${p}.${form}`).not.toBe('');
    }
  });

  it('keeps_the_placeholders_of_every_Ukrainian_plural_form', () => {
    const names = new Map(base);
    const bad: string[] = [];
    for (const [path, text] of other) {
      const m = /^(.*)\.(one|few|many|other)$/.exec(path);
      if (!m) continue;
      const en = names.get(`${m[1]}.other`);
      if (en !== undefined && placeholders(en).join() !== placeholders(text).join()) bad.push(path);
    }
    expect(bad).toEqual([]);
  });

  it('English_has_no_empty_text', () => {
    expect(base.filter(([, text]) => text.trim() === '').map(([p]) => p)).toEqual([]);
  });

  it('has_no_mojibake_or_replacement_characters', () => {
    // UTF-8 text read as Windows-1251 and saved again turns Cyrillic into Р/С-led pairs; guard against it.
    const texts = JSON.stringify(uk);
    expect(texts).not.toContain(String.fromCharCode(0xfffd));
    for (const bad of ['вЂ', 'Г—', 'Ð', 'Р°', 'Р±', 'Рµ', 'С‚', 'СЃ']) expect(texts).not.toContain(bad);
  });
});
