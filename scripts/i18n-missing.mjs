#!/usr/bin/env node
// Reports, per locale, how many UI strings are translated / missing / stale, and can emit the work for a translator.
//
//   node scripts/i18n-missing.mjs                      a table of all 19 partial locales
//   node scripts/i18n-missing.mjs --locale es          one locale: counts and the first missing keys
//   node scripts/i18n-missing.mjs --locale es --json   the missing and stale keys with their English text (and the
//                                                      Ukrainian text as a second reference) as JSON for a translator
//   node scripts/i18n-missing.mjs --json               the table as JSON
//   options: --limit N (with --locale --json: only the first N keys), --section nav (only keys under that prefix),
//            --only missing|stale, --dir <catalogue dir> (default app/src/lib/i18n)
//
// "stale" = translated, but the English text changed after the translation was made (a hash of the English source is
// kept in locales/<code>.meta.json). Hand the JSON to a translator, then merge the answer with scripts/i18n-apply.mjs.
import { DEFAULT_DIR, LOCALES, analyse, dig, hashOf, loadEnglish, loadLocale, loadUkrainian, percentOf, pluralCategoriesOf, units } from './lib/i18n-catalog.mjs';

function parseArgs(argv) {
  const o = { json: false, locale: undefined, limit: undefined, section: undefined, only: undefined, dir: DEFAULT_DIR };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--json') o.json = true;
    else if (a === '--locale') o.locale = argv[++i];
    else if (a === '--limit') o.limit = Number(argv[++i]);
    else if (a === '--section') o.section = argv[++i];
    else if (a === '--only') o.only = argv[++i];
    else if (a === '--dir') o.dir = argv[++i];
    else {
      console.error(`Unknown argument: ${a}`);
      process.exit(2);
    }
  }
  return o;
}

const o = parseArgs(process.argv.slice(2));
const en = loadEnglish(o.dir);
const uk = loadUkrainian(o.dir);
const all = units(en);
const total = all.length;

if (o.locale) {
  const locale = LOCALES.find((l) => l.code === o.locale);
  if (!locale) {
    console.error(`Unknown locale "${o.locale}". Known: ${LOCALES.map((l) => l.code).join(', ')}`);
    process.exit(2);
  }
  const a = analyse(o.dir, locale.code, en);
  const inSection = (k) => !o.section || k === o.section || k.startsWith(`${o.section}.`);
  const wanted = [...(o.only === 'stale' ? [] : a.missing.map((k) => [k, 'missing'])), ...(o.only === 'missing' ? [] : a.stale.map((k) => [k, 'stale']))].filter(([k]) => inSection(k));
  const limited = o.limit ? wanted.slice(0, o.limit) : wanted;
  if (o.json) {
    const { messages } = loadLocale(o.dir, locale.code);
    const items = {};
    for (const [key, status] of limited) {
      const enValue = dig(en, key);
      items[key] = { status, en: enValue, uk: dig(uk, key), ...(status === 'stale' ? { current: dig(messages, key) } : {}), englishHash: hashOf(enValue) };
    }
    const out = {
      locale: locale.code,
      name: locale.name,
      tag: locale.tag,
      pluralCategories: pluralCategoriesOf(locale.tag),
      summary: { total, translated: a.translated.length, stale: a.stale.length, missing: a.missing.length, percent: percentOf(a, total) },
      items,
    };
    console.log(JSON.stringify(out, null, 2));
  } else {
    console.log(`${locale.code} (${locale.name}): ${a.translated.length} translated, ${a.stale.length} stale, ${a.missing.length} missing of ${total} (${percentOf(a, total)} %)`);
    if (a.unknown.length) console.log(`  ${a.unknown.length} key(s) not in English: ${a.unknown.slice(0, 5).join(', ')}`);
    for (const [k, s] of limited.slice(0, o.limit ?? 20)) console.log(`  ${s.padEnd(7)} ${k}`);
    if (!o.limit && limited.length > 20) console.log(`  ... and ${limited.length - 20} more (use --json for all)`);
  }
} else {
  const rows = LOCALES.map((l) => {
    const a = analyse(o.dir, l.code, en);
    return { locale: l.code, name: l.name, translated: a.translated.length, stale: a.stale.length, missing: a.missing.length, unknown: a.unknown.length, percent: percentOf(a, total) };
  });
  if (o.json) console.log(JSON.stringify({ total, locales: rows }, null, 2));
  else {
    console.log(`English keys: ${total}`);
    console.log('locale   translated  stale  missing   share');
    for (const r of rows) console.log(`${r.locale.padEnd(8)} ${String(r.translated).padStart(10)} ${String(r.stale).padStart(6)} ${String(r.missing).padStart(8)} ${String(r.percent).padStart(6)} %${r.unknown ? `   (${r.unknown} unknown keys!)` : ''}`);
  }
}
