// Tests of scripts/i18n-missing.mjs and scripts/i18n-apply.mjs on a small catalogue in a temp folder.
// Run:  node --test "scripts/*.test.mjs"   (Node 22 needs the glob, not the folder)
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));

const EN = `export const en = {
  common: {
    save: 'Save',
    people: { one: '{n} person', other: '{n} people' },
  },
  nav: { family: 'Family', open: 'Open {dir}' },
};
`;
const UK = `import type { Messages } from './locale.svelte';
export const uk: Messages = {
  common: { save: 'Зберегти', people: { one: '{n} особа', few: '{n} особи', many: '{n} осіб', other: '{n} особи' } },
  nav: { family: 'Родина', open: 'Відкрити {dir}' },
};
`;

function fixture() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'i18n-test-'));
  fs.mkdirSync(path.join(dir, 'locales'));
  fs.writeFileSync(path.join(dir, 'en.ts'), EN);
  fs.writeFileSync(path.join(dir, 'uk.ts'), UK);
  return dir;
}
const run = (script, args) => spawnSync(process.execPath, [path.join(here, script), ...args], { encoding: 'utf8' });
const json = (dir, name, value) => {
  const f = path.join(dir, name);
  fs.writeFileSync(f, JSON.stringify(value));
  return f;
};
const localeText = (dir, code) => fs.readFileSync(path.join(dir, 'locales', `${code}.ts`), 'utf8');

test('missing: an untouched locale has every key missing', () => {
  const dir = fixture();
  const r = run('i18n-missing.mjs', ['--dir', dir, '--locale', 'de', '--json']);
  assert.equal(r.status, 0, r.stderr);
  const out = JSON.parse(r.stdout);
  assert.equal(out.summary.total, 4);
  assert.equal(out.summary.missing, 4);
  assert.deepEqual(Object.keys(out.items).sort(), ['common.people', 'common.save', 'nav.family', 'nav.open']);
  assert.equal(out.items['common.save'].en, 'Save');
  assert.equal(out.items['common.save'].uk, 'Зберегти');
  assert.deepEqual(out.items['common.people'].en, { one: '{n} person', other: '{n} people' });
  assert.deepEqual(out.pluralCategories, ['one', 'other']);
});

test('apply: merges valid translations, writes the file, the hashes and the coverage', () => {
  const dir = fixture();
  const f = json(dir, 'in.json', {
    'common.save': 'Speichern',
    'common.people': { one: '{n} Person', other: '{n} Personen' },
    'nav.open': '{dir} öffnen',
  });
  const r = run('i18n-apply.mjs', ['de', f, '--dir', dir]);
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, /3 added, 0 updated, 0 rejected/);
  const text = localeText(dir, 'de');
  assert.match(text, /"save": "Speichern"/);
  assert.match(text, /export default messages;/);
  assert.equal(Object.keys(JSON.parse(fs.readFileSync(path.join(dir, 'locales', 'de.meta.json'), 'utf8'))).length, 3);
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(dir, 'locales', 'coverage.json'), 'utf8')), { de: 75 });
  const m = JSON.parse(run('i18n-missing.mjs', ['--dir', dir, '--locale', 'de', '--json']).stdout);
  assert.deepEqual(Object.keys(m.items), ['nav.family']);
  assert.equal(m.summary.translated, 3);
});

test('apply: rejects a changed placeholder set, a wrong key, wrong plural shapes; applies the rest', () => {
  const dir = fixture();
  const f = json(dir, 'in.json', {
    'nav.open': 'Öffnen {path}',
    'nav.nothing': 'x',
    'common.people': { one: '{n} Person' },
    'nav.family': 'Familie',
    'common.save': '  ',
  });
  const r = run('i18n-apply.mjs', ['de', f, '--dir', dir]);
  assert.equal(r.status, 2);
  assert.match(r.stdout, /1 added, 0 updated, 4 rejected/);
  assert.match(r.stdout, /rejected nav\.open: placeholders differ/);
  assert.match(r.stdout, /rejected nav\.nothing: no such key/);
  assert.match(r.stdout, /rejected common\.people: plural form "other" is missing/);
  assert.match(r.stdout, /rejected common\.save: empty text/);
  assert.match(localeText(dir, 'de'), /"family": "Familie"/);
  assert.doesNotMatch(localeText(dir, 'de'), /Öffnen/);
});

test('apply: a language uses its own plural categories (ja: other only, ar: six)', () => {
  const dir = fixture();
  const ja = json(dir, 'ja.json', { 'common.people': { other: '{n} 人' } });
  assert.equal(run('i18n-apply.mjs', ['ja', ja, '--dir', dir]).status, 0);
  const jaBad = json(dir, 'ja2.json', { 'common.people': { one: '{n} 人', other: '{n} 人' } });
  const r = run('i18n-apply.mjs', ['ja', jaBad, '--dir', dir]);
  assert.equal(r.status, 2);
  assert.match(r.stdout, /plural form "one" is not used/);
  const ar = json(dir, 'ar.json', { 'common.people': { zero: '{n} شخص', one: 'شخص', two: 'شخصان', few: '{n} أشخاص', many: '{n} شخصًا', other: '{n} شخص' } });
  const ok = run('i18n-apply.mjs', ['ar', ar, '--dir', dir]);
  assert.equal(ok.status, 0, ok.stdout);
  assert.match(localeText(dir, 'ar'), /"two": "شخصان"/);
});

test('missing: a changed English text makes the translation stale, re-applying clears it', () => {
  const dir = fixture();
  assert.equal(run('i18n-apply.mjs', ['de', json(dir, 'a.json', { 'common.save': 'Speichern' }), '--dir', dir]).status, 0);
  fs.writeFileSync(path.join(dir, 'en.ts'), EN.replace("save: 'Save'", "save: 'Save now'"));
  const stale = JSON.parse(run('i18n-missing.mjs', ['--dir', dir, '--locale', 'de', '--json']).stdout);
  assert.equal(stale.summary.stale, 1);
  assert.equal(stale.items['common.save'].status, 'stale');
  assert.equal(stale.items['common.save'].current, 'Speichern');
  const table = run('i18n-missing.mjs', ['--dir', dir]).stdout;
  assert.match(table, /^de\s+0\s+1\s+3/m);
  assert.equal(run('i18n-apply.mjs', ['de', json(dir, 'b.json', { 'common.save': 'Jetzt speichern' }), '--dir', dir]).status, 0);
  const after = JSON.parse(run('i18n-missing.mjs', ['--dir', dir, '--locale', 'de', '--json']).stdout);
  assert.equal(after.summary.stale, 0);
  assert.equal(after.summary.translated, 1);
});

test('apply: accepts the items shape of i18n-missing and an unknown locale is an error', () => {
  const dir = fixture();
  const f = json(dir, 'items.json', { items: { 'nav.family': { status: 'missing', en: 'Family', translation: 'Familie' } } });
  assert.equal(run('i18n-apply.mjs', ['de', f, '--dir', dir]).status, 0);
  assert.equal(run('i18n-apply.mjs', ['xx', f, '--dir', dir]).status, 2);
});

test('apply --dry-run writes nothing', () => {
  const dir = fixture();
  const f = json(dir, 'in.json', { 'nav.family': 'Familie' });
  const r = run('i18n-apply.mjs', ['de', f, '--dir', dir, '--dry-run']);
  assert.equal(r.status, 0);
  assert.equal(fs.existsSync(path.join(dir, 'locales', 'de.ts')), false);
});

test('the real catalogue: every locale file parses and lists no unknown key', () => {
  const r = run('i18n-missing.mjs', ['--json']);
  assert.equal(r.status, 0, r.stderr);
  const out = JSON.parse(r.stdout);
  assert.equal(out.locales.length, 19);
  for (const l of out.locales) assert.equal(l.unknown, 0, l.locale);
});
