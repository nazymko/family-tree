#!/usr/bin/env node
// Merges translations into a partial locale file.
//
//   node scripts/i18n-apply.mjs <code> <translations.json> [--dir <catalogue dir>] [--dry-run]
//
// The JSON is a flat object  { "nav.family": "Familia", "common.people": { "one": "{n} persona", "other": "{n} personas" } }
// (the shape of "items" from `i18n-missing.mjs --json` also works: an item may be {"translation": ...}).
// Checked per key, and a key that fails is rejected (the rest is still applied; exit code 2):
//   - the key exists in English and is the same kind (text or plural forms)
//   - the {placeholders} are exactly the English ones
//   - a plural entry has exactly the CLDR categories of the language (e.g. ar: zero one two few many other; ja: other)
// Updates locales/<code>.ts, locales/<code>.meta.json (hashes of the English texts) and locales/coverage.json.
import fs from 'node:fs';
import { DEFAULT_DIR, LOCALES, currentOf, dig, hashOf, loadEnglish, validate, writeCoverage, writeLocale, analyse, percentOf, units } from './lib/i18n-catalog.mjs';

const args = process.argv.slice(2);
let dir = DEFAULT_DIR;
let dry = false;
const pos = [];
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--dir') dir = args[++i];
  else if (args[i] === '--dry-run') dry = true;
  else pos.push(args[i]);
}
const [code, file] = pos;
if (!code || !file) {
  console.error('Usage: node scripts/i18n-apply.mjs <code> <translations.json> [--dir <dir>] [--dry-run]');
  process.exit(2);
}
const locale = LOCALES.find((l) => l.code === code);
if (!locale) {
  console.error(`Unknown locale "${code}". Known: ${LOCALES.map((l) => l.code).join(', ')}`);
  process.exit(2);
}

let input = JSON.parse(fs.readFileSync(file, 'utf8'));
if (input && typeof input === 'object' && input.items && typeof input.items === 'object') input = input.items;
if (input && typeof input === 'object' && input.translations && typeof input.translations === 'object') input = input.translations;

const en = loadEnglish(dir);
const { values, hashes } = currentOf(dir, code, en);
const known = new Map(units(en));
const added = [];
const updated = [];
const rejected = [];

for (const [key, raw] of Object.entries(input)) {
  const enValue = known.get(key);
  if (enValue === undefined) {
    rejected.push([key, 'no such key in English']);
    continue;
  }
  const value = raw !== null && typeof raw === 'object' && 'translation' in raw ? raw.translation : raw;
  const problem = validate(enValue, value, locale.tag);
  if (problem) {
    rejected.push([key, problem]);
    continue;
  }
  (values.has(key) ? updated : added).push(key);
  values.set(key, typeof value === 'string' ? value : Object.fromEntries(Object.keys(value).sort((a, b) => ['zero', 'one', 'two', 'few', 'many', 'other'].indexOf(a) - ['zero', 'one', 'two', 'few', 'many', 'other'].indexOf(b)).map((k) => [k, value[k]])));
  hashes.set(key, hashOf(enValue));
}

if (!dry) {
  writeLocale(dir, locale, en, values, hashes);
  writeCoverage(dir, en);
}
const a = analyse(dir, code, en);
console.log(`${code} (${locale.name}): ${added.length} added, ${updated.length} updated, ${rejected.length} rejected${dry ? ' (dry run, nothing written)' : ''}`);
for (const [k, why] of rejected) console.log(`  rejected ${k}: ${why}`);
if (!dry) console.log(`  now ${a.translated.length} translated, ${a.stale.length} stale, ${a.missing.length} missing (${percentOf(a, known.size)} %)`);
process.exit(rejected.length ? 2 : 0);
