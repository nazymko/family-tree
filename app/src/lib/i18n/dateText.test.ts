import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { backendError, dateText } from './backend';
import { setLanguage } from '.';
import { en } from './en';
import { uk } from './uk';

/** Vitest runs from `app/`; Vite will not serve files from outside it. */
const fromRepo = (path: string) => readFileSync(resolve(process.cwd(), '..', path), 'utf8');
const cases = fromRepo('crates/gt-core/tests/date_display_cases.txt');
const undoRs = fromRepo('crates/gt-session/src/undo.rs');
const logRs = fromRepo('crates/gt-session/src/log.rs');

afterEach(() => setLanguage('en'));

/** The table is checked against the real `display()` by `display_table_shared_with_the_frontend` in gt-core. */
const table = cases
  .split(/\r?\n/)
  .filter(Boolean)
  .map((l) => l.split('\t') as [gedcom: string, shown: string, ukrainian: string]);

describe('dateText', () => {
  it('has_the_whole_table', () => {
    expect(table.length).toBeGreaterThan(40);
    expect(table.every((c) => c.length === 3)).toBe(true);
  });

  it.each(table)('%s', (_gedcom, shown, ukrainian) => {
    setLanguage('en');
    expect(dateText(shown)).toBe(shown);
    setLanguage('uk');
    expect(dateText(shown)).toBe(ukrainian);
  });

  it('leaves_no_fixed_english_in_a_date_the_owner_did_not_word', () => {
    setLanguage('uk');
    const fixed = table.filter(([gedcom]) => !gedcom.includes('(') && gedcom !== 'some time in spring');
    expect(fixed.length).toBeGreaterThan(40);
    expect(fixed.filter(([, shown]) => /[A-Za-z]{2}/.test(dateText(shown))).map(([, shown]) => shown)).toEqual([]);
  });

  it('keeps_user_words_that_only_look_like_a_date', () => {
    setLanguage('uk');
    expect(dateText('about the time of the flood')).toBe('about the time of the flood');
    expect(dateText('between friends and family')).toBe('between friends and family');
    expect(dateText('the year (Julian)')).toBe('the year (Julian)');
  });
});

/** Every fixed message Rust sends for an undo: the named constants and the literals in `ApiError::new`. */
function backendMessages(): string[] {
  const unescape = (s: string) => s.replace(/\\\r?\n\s*/g, '').replace(/\\"/g, '"');
  const consts = [...undoRs.matchAll(/^const \w+: &str\s*=\s*"((?:[^"\\]|\\[\s\S])*)";/gm)].map((m) => unescape(m[1]));
  const literals = [...logRs.matchAll(/ApiError::new\(\s*ErrorCode::\w+,\s*"((?:[^"\\]|\\[\s\S])*)"/g)].map((m) => unescape(m[1]));
  return [...consts, ...literals];
}

describe('backendError', () => {
  it('finds_the_messages_it_must_translate', () => {
    expect(backendMessages()).toEqual(
      expect.arrayContaining(['An agent changed this since. Undo it from the change log first.', "Only an agent's change can be undone from the change log."]),
    );
    expect(backendMessages().length).toBeGreaterThanOrEqual(4);
  });

  it('every_message_from_undo_and_log_has_a_translation', () => {
    setLanguage('uk');
    const untranslated = backendMessages().filter((m) => /[A-Za-z]{2}/.test(backendError(m)));
    expect(untranslated).toEqual([]);
  });

  it('the_english_catalogue_matches_rust_word_for_word', () => {
    setLanguage('en');
    for (const m of backendMessages()) expect(backendError(m)).toBe(m);
  });

  it('the_dropped_steps_read_as_agreed', () => {
    setLanguage('uk');
    expect(backendError('This step was dropped: a record it needs no longer exists.')).toBe('Цей крок вилучено: потрібного запису більше немає.');
    expect(backendError('This step was dropped: a record it needs has changed since, and the change log cannot undo that.')).toBe(
      'Цей крок вилучено: потрібний запис відтоді змінився, і журнал змін не може цього скасувати.',
    );
    expect('undoDropped' in en.changeLog.errors || 'redoDropped' in uk.changeLog.errors).toBe(false);
  });
});
