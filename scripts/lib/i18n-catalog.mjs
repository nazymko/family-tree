// Shared code of scripts/i18n-missing.mjs and scripts/i18n-apply.mjs: reads the catalogues in app/src/lib/i18n, counts
// translated / missing / stale keys, validates and merges translations, writes locale files. See docs/i18n.md.
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const DEFAULT_DIR = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..', 'app', 'src', 'lib', 'i18n');

/** The 19 partial locales: code, native name, BCP-47 tag for Intl. Kept equal to LANGUAGES in locale.svelte.ts by a vitest test. */
export const LOCALES = [
  ['zh-Hans', '中文简体', 'zh-Hans'], ['hi', 'हिन्दी', 'hi'], ['es', 'Español', 'es'], ['ar', 'العربية', 'ar'],
  ['fr', 'Français', 'fr'], ['bn', 'বাংলা', 'bn'], ['pt', 'Português', 'pt'], ['id', 'Bahasa Indonesia', 'id'],
  ['ur', 'اردو', 'ur'], ['de', 'Deutsch', 'de'], ['ja', '日本語', 'ja'], ['mr', 'मराठी', 'mr'],
  ['vi', 'Tiếng Việt', 'vi'], ['te', 'తెలుగు', 'te'], ['ha', 'Hausa', 'ha'], ['tr', 'Türkçe', 'tr'],
  ['sw', 'Kiswahili', 'sw'], ['tl', 'Filipino', 'fil'], ['ta', 'தமிழ்', 'ta'],
].map(([code, name, tag]) => ({ code, name, tag }));

export const CATEGORIES = ['zero', 'one', 'two', 'few', 'many', 'other'];

/** The object literal of a catalogue file (`export const en = {...};`, `const messages: PartialMessages = {...};`). */
export function readCatalogFile(file) {
  const text = fs.readFileSync(file, 'utf8');
  const m = /^(?:export )?const \w+(?: *: *\w+)? = \{/m.exec(text);
  if (!m) throw new Error(`${file}: no catalogue object found`);
  const start = m.index + m[0].length - 1;
  const end = text.lastIndexOf('};');
  if (end < start) throw new Error(`${file}: catalogue object is not closed`);
  return new Function(`return (${text.slice(start, end + 1)});`)();
}

const isPluralShape = (o) =>
  o !== null && typeof o === 'object' && typeof o.other === 'string' && typeof o.one === 'string' && Object.keys(o).every((k) => CATEGORIES.includes(k));

/** English units in file order: [dotted key, string | plural object]. A plural entry is one unit. */
export function units(en, prefix = '') {
  const out = [];
  for (const [k, v] of Object.entries(en)) {
    const key = prefix + k;
    if (typeof v === 'string' || isPluralShape(v)) out.push([key, v]);
    else out.push(...units(v, `${key}.`));
  }
  return out;
}

export function dig(obj, key) {
  let node = obj;
  for (const part of key.split('.')) {
    if (node === null || typeof node !== 'object') return undefined;
    node = node[part];
  }
  return node;
}

export const hashOf = (enValue) => createHash('sha1').update(JSON.stringify(enValue)).digest('hex').slice(0, 10);

export const placeholders = (text) => [...new Set([...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]))].sort();

/** Whether a locale's entry counts as translated. */
export function hasValue(v) {
  if (typeof v === 'string') return v.trim() !== '';
  return v !== null && typeof v === 'object' && typeof v.other === 'string' && v.other.trim() !== '';
}

export function localeFile(dir, code) {
  return path.join(dir, 'locales', `${code}.ts`);
}
const metaFile = (dir, code) => path.join(dir, 'locales', `${code}.meta.json`);

export function loadEnglish(dir) {
  return readCatalogFile(path.join(dir, 'en.ts'));
}
export function loadUkrainian(dir) {
  const f = path.join(dir, 'uk.ts');
  return fs.existsSync(f) ? readCatalogFile(f) : {};
}

export function loadLocale(dir, code) {
  const f = localeFile(dir, code);
  const messages = fs.existsSync(f) ? readCatalogFile(f) : {};
  const mf = metaFile(dir, code);
  const meta = fs.existsSync(mf) ? JSON.parse(fs.readFileSync(mf, 'utf8')) : {};
  return { messages, meta };
}

/** Per-key status of one locale against English: translated, stale (English changed since) or missing. */
export function analyse(dir, code, en = loadEnglish(dir)) {
  const { messages, meta } = loadLocale(dir, code);
  const result = { code, translated: [], missing: [], stale: [], unknown: [] };
  const known = new Set();
  for (const [key, enValue] of units(en)) {
    known.add(key);
    const own = dig(messages, key);
    if (!hasValue(own)) result.missing.push(key);
    else if (meta[key] !== hashOf(enValue)) result.stale.push(key);
    else result.translated.push(key);
  }
  // Keys the locale has that English lacks. Walk by English's shapes: a plural entry may hold just {other} (ja, zh).
  const walk = (node, prefix) => {
    for (const [k, v] of Object.entries(node)) {
      const key = prefix + k;
      if (known.has(key)) continue;
      if (v !== null && typeof v === 'object' && [...known].some((x) => x.startsWith(`${key}.`))) walk(v, `${key}.`);
      else result.unknown.push(key);
    }
  };
  walk(messages, '');
  return result;
}

/** Share of English keys the locale has (stale ones still count: they show a translation). */
export function percentOf(a, total) {
  return total === 0 ? 0 : Math.round(((a.translated.length + a.stale.length) / total) * 100);
}

export function pluralCategoriesOf(tag) {
  return new Intl.PluralRules(tag).resolvedOptions().pluralCategories;
}

/** Why `value` is not a valid translation of the English unit `enValue`, or undefined when it is fine. */
export function validate(enValue, value, tag) {
  if (typeof enValue === 'string') {
    if (typeof value !== 'string') return 'expected a string';
    if (value.trim() === '') return 'empty text';
    const a = placeholders(enValue).join();
    const b = placeholders(value).join();
    if (a !== b) return `placeholders differ: English {${a.split(',').filter(Boolean).join('}, {')}} vs {${b.split(',').filter(Boolean).join('}, {')}}`;
    return undefined;
  }
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return 'expected an object of plural forms';
  const need = pluralCategoriesOf(tag);
  for (const k of Object.keys(value)) if (!need.includes(k)) return `plural form "${k}" is not used by this language (it uses ${need.join(', ')})`;
  for (const k of need) if (typeof value[k] !== 'string' || value[k].trim() === '') return `plural form "${k}" is missing (this language needs ${need.join(', ')})`;
  const base = placeholders(enValue.other).join();
  // `other` must carry exactly the English placeholders; a special form (one, two, zero) may leave {n} out ("one person")
  // but must not invent a placeholder English lacks.
  const allowed = new Set(placeholders(enValue.other));
  for (const k of need) {
    const got = placeholders(value[k]);
    if (k === 'other' ? got.join() !== base : got.some((p) => !allowed.has(p))) return `placeholders of form "${k}" differ from English {${base}}`;
  }
  return undefined;
}

function setDeep(obj, key, value) {
  const parts = key.split('.');
  let node = obj;
  for (const p of parts.slice(0, -1)) node = node[p] ??= {};
  node[parts.at(-1)] = value;
}

/** The locale's units in English order, as a nested object (so the file is stable whatever order translations arrive in). */
export function nestInEnglishOrder(en, flat) {
  const out = {};
  for (const [key] of units(en)) if (flat.has(key)) setDeep(out, key, flat.get(key));
  return out;
}

function renderLiteral(value, indent) {
  const pad = '  '.repeat(indent);
  const entries = Object.entries(value);
  if (entries.length === 0) return '{}';
  const body = entries
    .map(([k, v]) => `${pad}  ${JSON.stringify(k)}: ${typeof v === 'string' ? JSON.stringify(v) : renderLiteral(v, indent + 1)},`)
    .join('\n');
  return `{\n${body}\n${pad}}`;
}

export function renderLocaleFile(code, name, tree) {
  return `/**
 * ${name} (${code}). A PARTIAL catalogue: generated by the translation task (scripts/i18n-apply.mjs, see docs/i18n.md).
 * Any key missing here falls back to English, key by key. Do not hand-edit during feature work: feature development
 * maintains en.ts and uk.ts only.
 */
import type { PartialMessages } from '../locale.svelte';

const messages: PartialMessages = ${renderLiteral(tree, 0)};

export default messages;
`;
}

/** Writes the locale file and its meta (hash of the English text each translated key was made from). */
export function writeLocale(dir, locale, en, values, hashes) {
  const tree = nestInEnglishOrder(en, values);
  fs.mkdirSync(path.join(dir, 'locales'), { recursive: true });
  fs.writeFileSync(localeFile(dir, locale.code), renderLocaleFile(locale.code, locale.name, tree));
  const meta = {};
  for (const [key] of units(en)) if (values.has(key)) meta[key] = hashes.get(key) ?? '';
  fs.writeFileSync(metaFile(dir, locale.code), JSON.stringify(meta, null, 1) + '\n');
}

/** Current entries of a locale: values (key -> string | plural object) and the hashes recorded for them. */
export function currentOf(dir, code, en) {
  const { messages, meta } = loadLocale(dir, code);
  const values = new Map();
  const hashes = new Map();
  for (const [key] of units(en)) {
    const own = dig(messages, key);
    if (hasValue(own)) {
      values.set(key, own);
      hashes.set(key, meta[key] ?? '');
    }
  }
  return { values, hashes };
}

/** Rewrites coverage.json (code -> percent) for every locale file present; the picker reads it. */
export function writeCoverage(dir, en = loadEnglish(dir)) {
  const total = units(en).length;
  const out = {};
  for (const l of LOCALES) {
    if (!fs.existsSync(localeFile(dir, l.code))) continue;
    out[l.code] = percentOf(analyse(dir, l.code, en), total);
  }
  fs.writeFileSync(path.join(dir, 'locales', 'coverage.json'), JSON.stringify(out, null, 1) + '\n');
  return out;
}
