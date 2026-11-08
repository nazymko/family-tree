import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { toApiError } from '../api/tauri/errors';
import { errMessage } from '../editors/errors';
import { backendError, backendRule, dateWarningText, KIND_KEYS } from './backend';
import { setLanguage } from '.';
import { en } from './en';
import { constants, extractMessages, productionPart, type RustSource } from './rustMessages';

/**
 * Keeps the backend's error sentences translated. It reads the Rust sources, finds every message handed to
 * `ApiError::new` (or a helper that wraps it), every `ImportError`, and every thiserror text that can reach the
 * screen, and checks that each has an entry in the `be` section of the catalogue: back word for word in English,
 * and in Ukrainian with no English word left outside a path, an id or an operating-system error.
 * A new message without an entry fails here. To add one: write the English sentence under `be` in en.ts, the
 * Ukrainian one in uk.ts, and run this test.
 */

/** Vitest runs from `app/`. */
const repo = resolve(process.cwd(), '..');

function walk(dir: string, out: string[]): string[] {
  for (const name of readdirSync(dir)) {
    if (['target', 'node_modules', 'tests', '.git'].includes(name)) continue;
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path, out);
    else if (name.endsWith('.rs') && !/tests?\.rs$/.test(name) && !/_tests\.rs$/.test(name)) out.push(path);
  }
  return out;
}

const posix = (p: string) => p.replace(/\\/g, '/');
const sources: RustSource[] = [...walk(join(repo, 'crates'), []), ...walk(join(repo, 'app', 'src-tauri', 'src'), [])].map((p) => ({
  path: posix(p).slice(posix(repo).length + 1),
  src: readFileSync(p, 'utf8'),
}));

/**
 * Files whose messages are not shown by the app, with the reason. A message in one of them needs no entry.
 */
const EXCLUDED_FILES: Record<string, string> = {
  'crates/gt-session/src/agent.rs': 'answers only MCP agents (the sentences tell an agent what to do next); agents are served in English',
  'crates/gt-mcp/': 'the MCP server talks to agents, in English',
  'crates/gt-cli/': 'the command line tool prints for a terminal, in English',
  'app/src-tauri/src/agent_access.rs': 'agent-access settings: translated through the `agent.err*` entries and checked by agent-errors.test.ts',
  'app/src-tauri/src/settings_file.rs': 'same: `Could not save settings: {e}` is `agent.errSaveSettings`, checked by agent-errors.test.ts',
};

/**
 * Single messages that stay English on purpose. Each must still occur in the sources (checked below),
 * so this list cannot go stale.
 */
const EXCLUDED_TEXT: Record<string, string> = {
  'bad pixel buffer': 'internal: an image library invariant, not something a person can act on',
  'The picture must be stored by the session first.': 'internal: a programming-order check; the UI always stores the picture first',
  'Only a note or a citation has a holder, not {other}.': 'internal: the UI only ever asks for note and citation holders',
  '"{id}" is not a kind of event. Use one of: {EVENT_KINDS}, or custom:<your words>.': 'sent only to MCP agents, who type event kinds; the app offers a fixed list',
  'not a gene-tree project: {0}': 'replaced by `be.notAProject` before it leaves the session (see `From<StoreError> for ApiError`)',
  'project schema {found} is newer than supported {supported}': 'replaced by `be.newerSchema` before it leaves the session',
  'No secure random numbers are available.': 'agent access only; `agent.errNoRandom`',
  'only sound recordings can be attached to a note': 'worded by `failureLine` in Dropzone.svelte as `voice.onlySoundOnNote` (it adds the file name)',
  'sound recordings can only be attached to a note': 'worded by `failureLine` in Dropzone.svelte as `voice.soundOnlyOnNote`',
};

/** Messages passed to a helper as a variable, so no text can be read from the call. Reviewed one by one. */
const DYNAMIC_OK: Record<string, string> = {
  'crates/gt-import/src/lib.rs|e.to_string()': 'the text of an `AtdbError`: its `#[error]` texts are checked here',
  'crates/gt-session/src/import_format.rs|e.message': 'the text of an `ImportError`: its messages are checked here',
  'crates/gt-session/src/merge/copy.rs|message': 'the helper `refuse` itself',
  'crates/gt-session/src/ops/mod.rs|msg': 'the helpers `invalid` and `conflict` themselves',
  'crates/gt-session/src/undo.rs|why': 'one of the three constants of undo.rs, translated by name (checked below)',
  'app/src-tauri/src/ai.rs|msg': 'the helper `err` itself',
  'app/src-tauri/src/commands.rs|msg.to_string()': 'the helper `internal`: an `Internal` error is a bug report, not advice',
  'app/src-tauri/src/commands.rs|m': 'the closure `invalid` of save_picture_impl itself',
  'app/src-tauri/src/settings_file.rs|e.to_string()': 'agent-access settings, see above',
};

const { messages, dynamic } = extractMessages(sources, /gt-session\/src\/error\.rs$/);
const excludedFile = (path: string) => Object.keys(EXCLUDED_FILES).some((f) => path.startsWith(f));
const shown = messages.filter((m) => !excludedFile(m.path) && !(m.text in EXCLUDED_TEXT));

/** The verbs `io_err("verb", ...)` is called with, for `Cannot {what} {path}: {e}`. */
const VERBS = [
  ...new Set(sources.filter((s) => !excludedFile(s.path)).flatMap((s) => [...s.src.matchAll(/\bio_err\(\s*"([^"]+)"/g)].map((m) => m[1]))),
].sort();

/** What `{what}` stands for in a message: a verb, or the Rust name of a kind of record. */
const whats = (text: string) => (text.startsWith('No {what}') ? Object.keys(KIND_KEYS) : VERBS);

/** The placeholders of a Rust message, in order: `{}`, `{name}`, `{0:?}`. */
const PLACEHOLDER = /\{[^{}]*\}/g;
const shape = (text: string) => text.replace(PLACEHOLDER, '{}');

/** A message with every placeholder filled by a marker word; `{what}` takes a real value. */
function filled(text: string, what?: string): { text: string; markers: string[] } {
  const markers: string[] = [];
  const out = text.replace(PLACEHOLDER, (p) => {
    if (p === '{what}' && what !== undefined) return what;
    const marker = `SMPL${markers.length + 1}`;
    markers.push(marker);
    return marker;
  });
  return { text: out, markers };
}

/** Variants of a message: `{what}` becomes each verb in turn. */
function variants(text: string): { text: string; markers: string[]; shape: string; what?: string }[] {
  if (!text.includes('{what}')) return [{ ...filled(text), shape: shape(text) }];
  return whats(text).map((what) => ({ ...filled(text, what), shape: shape(text.replace('{what}', what)), what }));
}

/** Words that stay Latin in Ukrainian text: names of the app and of file formats, and fixed table names. */
const KEEP = /\b(Gene|Family|Tree|tree\.db|GEDCOM|GEDZIP|Gramps|zip|gzip|XML|UTF|JPG|JPEG|PNG|png|svg|SVG|WebP|TIFF|GIF|PDF|Anthropic|API|Persons|Events|EventDetails|gedcom\.ged|atdb|db)\b/g;
const latinLeft = (text: string, markers: string[]) => {
  let rest = text;
  for (const m of markers) rest = rest.replaceAll(m, '');
  return (rest.replace(KEEP, '').match(/[A-Za-z]{2,}/g) ?? []).join(' ');
};

afterEach(() => setLanguage('en'));

describe('the backend messages found in the Rust sources', () => {
  it('reads_a_realistic_number_of_messages', () => {
    expect(messages.length).toBeGreaterThan(200);
    expect(shown.length).toBeGreaterThan(150);
    expect(VERBS).toEqual(expect.arrayContaining(['create', 'read', 'write', 'open', 'copy to']));
    // A few known ones, so a broken reader cannot pass by finding nothing.
    const texts = shown.map((m) => m.text);
    expect(texts).toContain('The folder {} already has files in it. Choose a new or empty folder.');
    expect(texts).toContain('Cannot {what} {}: {e}');
    expect(texts).toContain('A person cannot be their own child.');
  });

  it('has_no_stale_exclusions', () => {
    const all = new Set(messages.map((m) => m.text));
    for (const text of Object.keys(EXCLUDED_TEXT)) expect(all.has(text), `excluded text no longer in the sources: ${text}`).toBe(true);
    for (const file of Object.keys(EXCLUDED_FILES)) {
      expect(sources.some((s) => s.path.startsWith(file)), `excluded file does not exist: ${file}`).toBe(true);
    }
  });

  it('computes_a_message_only_where_it_has_been_reviewed', () => {
    const seen = dynamic.map((d) => `${d.path}|${d.expr}`);
    const unreviewed = dynamic.filter((d) => !excludedFile(d.path)).map((d) => `${d.path}|${d.expr}`).filter((key) => !(key in DYNAMIC_OK));
    expect(unreviewed, 'a new error built from a variable: give it a fixed sentence or review it and list it in DYNAMIC_OK').toEqual([]);
    const stale = Object.keys(DYNAMIC_OK).filter((key) => !seen.includes(key));
    expect(stale, 'reviewed call that is no longer in the sources').toEqual([]);
  });

  it('gives_every_message_a_catalogue_entry_that_says_the_same_in_English', () => {
    setLanguage('en');
    const bad: string[] = [];
    for (const m of shown) {
      for (const v of variants(m.text)) {
        const rule = backendRule(v.text);
        const entry = rule ? rule.key.split('.').reduce<unknown>((node, part) => (node as Record<string, unknown>)[part], en) : undefined;
        if (typeof entry !== 'string' || shape(v.what === undefined ? entry : entry.replace('{what}', v.what)) !== v.shape) bad.push(`${m.path}:${m.line} ${v.text} -> ${rule?.key ?? 'no entry'}`);
        // The change log words "No change/batch with id" as its own sentence in both languages (see `backendError`).
        else if (!/^No (change|batch) with id /.test(v.text)) expect(backendError(v.text), 'English comes back word for word').toBe(v.text);
      }
    }
    expect(bad).toEqual([]);
  });

  it('words_every_message_in_Ukrainian_with_no_English_left', () => {
    setLanguage('uk');
    const bad: string[] = [];
    for (const m of shown) {
      for (const v of variants(m.text)) {
        const out = backendError(v.text);
        if (out === v.text) bad.push(`${m.path}:${m.line} not translated: ${v.text}`);
        const left = latinLeft(out, v.markers);
        if (left) bad.push(`${m.path}:${m.line} English left (${left}): ${out}`);
        // "No change/batch with id" is the change log's own sentence, which names no id (see `backendError`).
        if (/^No (change|batch) with id /.test(v.text)) continue;
        for (const marker of v.markers) if (!out.includes(marker)) bad.push(`${m.path}:${m.line} lost ${marker}: ${out}`);
      }
    }
    expect(bad).toEqual([]);
  });

  it('translates_a_message_once_and_leaves_Ukrainian_alone', () => {
    setLanguage('uk');
    for (const m of shown) {
      for (const v of variants(m.text)) {
        const once = backendError(v.text);
        expect(backendError(once)).toBe(once);
      }
    }
  });

  it('has_no_catalogue_entry_that_no_message_uses', () => {
    const used = new Set<string>();
    for (const m of shown) for (const v of variants(m.text)) used.add(backendRule(v.text)?.key ?? '');
    const unused = Object.keys(en.be)
      .filter((k) => k !== 'kind' && k !== 'repairKind')
      .map((k) => `be.${k}`)
      .filter((k) => !used.has(k));
    // `saveRecent` is the one sentence of settings_file.rs (an excluded file) that agent access does not word.
    expect(unused).toEqual(['be.saveRecent']);
    setLanguage('uk');
    expect(backendError('Could not save the recent list: os error 5')).toBe('Не вдалося зберегти список недавніх родоводів: os error 5');
    expect(backendError('Could not save settings: os error 5')).not.toMatch(/Could not/);
  });

  it('knows_every_kind_of_record_not_found_can_name', () => {
    const names = new Set<string>();
    for (const s of sources) {
      const src = productionPart(s.src);
      for (const m of src.matchAll(/not_found\(\s*"([^"]+)"/g)) names.add(m[1]);
      for (const m of src.matchAll(/\brec!\(\s*\w+,\s*\w+,\s*\w+,\s*"([^"]+)"/g)) names.add(m[1]);
    }
    expect(names.size).toBeGreaterThan(10);
    setLanguage('uk');
    for (const name of names) {
      expect(KIND_KEYS, `no entry for the kind "${name}"`).toHaveProperty(name);
      const out = backendError(`No ${name} with id "x" in this project.`);
      expect(latinLeft(out, []), out).toBe('');
    }
  });

  it('words_the_three_constants_of_the_undo_code', () => {
    const undo = sources.find((s) => s.path === 'crates/gt-session/src/undo.rs')!;
    const texts = [...constants(productionPart(undo.src)).values()];
    expect(texts.length).toBe(3);
    setLanguage('uk');
    for (const text of texts) expect(latinLeft(backendError(text), [])).toBe('');
  });
});

describe('details the import report and the export screen show', () => {
  it('has_a_heading_for_every_kind_of_repair_the_importer_reports', async () => {
    const transfer = productionPart(sources.find((s) => s.path === 'crates/gt-session/src/transfer.rs')!.src);
    const kinds = [...transfer.matchAll(/\bdto\(\s*"(\w+)"/g)].map((m) => m[1]);
    expect(kinds.length).toBe(8);
    expect(Object.keys(en.be.repairKind).sort()).toEqual([...kinds].sort());
    const { repairKindLabel } = await import('./backend');
    setLanguage('uk');
    for (const kind of kinds) expect(repairKindLabel(kind), kind).not.toMatch(/[A-Za-z_]/);
    expect(repairKindLabel('something_new')).toBe('something new');
  });

  it('words_the_note_under_the_GEDCOM_5_5_1_export', () => {
    setLanguage('uk');
    const note = "Roles, 'did not happen' records, shared notes and external IDs are written as extension tags; other programs may drop them.";
    expect(backendError(note)).toMatch(/^Ролі, записи «не відбулося»/);
    expect(latinLeft(backendError(note), [])).toBe('');
  });

  it('words_the_date_parser_warning_and_the_reasons_a_file_was_not_attached', () => {
    setLanguage('uk');
    expect(dateWarningText('Not understood as a date. It will be saved exactly as you typed it.')).not.toMatch(/[A-Za-z]/);
    for (const reason of ['cannot read file', 'file is empty', 'already attached', 'unsupported type', 'image too large to preview']) {
      expect(backendError(reason), reason).not.toMatch(/[A-Za-z]/);
    }
  });
});

describe('a folder that already has files', () => {
  const english = 'The folder C:\\Users\\Ivan Koval\\Desktop already has files in it. Choose a new or empty folder.';

  it('keeps_the_path_and_leaves_no_English_word_in_Ukrainian', () => {
    setLanguage('uk');
    const out = backendError(english);
    expect(out).toBe('У теці C:\\Users\\Ivan Koval\\Desktop уже є файли. Виберіть нову або порожню теку.');
    expect(out.replace('C:\\Users\\Ivan Koval\\Desktop', '')).not.toMatch(/[A-Za-z]/);
  });

  it('is_worded_where_the_backend_error_is_made_so_every_screen_gets_it', () => {
    setLanguage('uk');
    const api = toApiError({ code: 'conflict', message: english });
    expect(api).toEqual({ code: 'conflict', message: 'У теці C:\\Users\\Ivan Koval\\Desktop уже є файли. Виберіть нову або порожню теку.' });
    // A second pass changes nothing.
    expect(toApiError(api).message).toBe(api.message);
  });

  it('reads_in_one_piece_inside_the_sentence_about_the_received_copy', async () => {
    setLanguage('uk');
    const { uk } = await import('./uk');
    const reason = toApiError({ code: 'invalid', message: english }).message;
    const shownText = uk.merge.openReceivedFailed.replace('{reason}', reason);
    expect(shownText).toBe(
      'Цей файл не вдалося відкрити як копію родоводу. У теці C:\\Users\\Ivan Koval\\Desktop уже є файли. Виберіть нову або порожню теку.',
    );
  });

  it('stays_English_in_English', () => {
    setLanguage('en');
    expect(toApiError({ code: 'conflict', message: english }).message).toBe(english);
  });
});

describe('nested and special messages', () => {
  it('translates_the_reason_inside_a_Gramps_failure_and_keeps_the_file_name', () => {
    setLanguage('uk');
    expect(backendError('Ivan Koval.gramps could not be imported as a Gramps file: the XML could not be parsed: unexpected end of file.')).toBe(
      'Не вдалося імпортувати Ivan Koval.gramps як файл Gramps: не вдалося розібрати XML: unexpected end of file.',
    );
  });

  it('keeps_the_operating_system_error_text_as_it_is', () => {
    setLanguage('uk');
    expect(backendError('Cannot read C:\\Trees\\Koval\\tree.db: Access is denied. (os error 5)')).toBe(
      'Не вдалося прочитати C:\\Trees\\Koval\\tree.db: Access is denied. (os error 5)',
    );
  });

  it('picks_the_most_specific_sentence_when_two_start_alike', () => {
    setLanguage('uk');
    expect(backendError('Cannot read this copy: the tree in it is damaged.')).toBe('Не вдалося прочитати цю копію: родовід у ній пошкоджено.');
    expect(backendError('Cannot read this copy: os error 32')).toBe('Не вдалося прочитати цю копію: os error 32');
    expect(backendError('Cannot copy to D:\\Backup: os error 3')).toBe('Не вдалося скопіювати до D:\\Backup: os error 3');
    expect(backendError('The folder D:\\A already has files in it. Choose a new or empty folder for the copy.')).toBe(
      'У теці D:\\A уже є файли. Виберіть нову або порожню теку для копії.',
    );
  });

  it('names_the_kind_of_record_in_the_nominative', () => {
    setLanguage('uk');
    expect(backendError('No person with id "7" in this project.')).toBe('У родоводі не знайдено: особа з ідентифікатором «7».');
    expect(backendError('No child link with id "7" in this project.')).toBe('У родоводі не знайдено: зв’язок дитини з родиною з ідентифікатором «7».');
  });

  it('leaves_text_it_does_not_know_as_the_backend_wrote_it', () => {
    setLanguage('uk');
    expect(backendError('Something nobody wrote a sentence for.')).toBe('Something nobody wrote a sentence for.');
    expect(backendError('Файл не вдалося відкрити.')).toBe('Файл не вдалося відкрити.');
  });

  it('keeps_the_same_placeholders_in_both_languages', async () => {
    const { uk } = await import('./uk');
    const names = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join();
    for (const key of Object.keys(en.be) as (keyof typeof en.be)[]) {
      if (key === 'kind' || key === 'repairKind') continue;
      expect(names(uk.be[key] as string), key).toBe(names(en.be[key] as string));
    }
  });
});

describe('errMessage', () => {
  it('reads_the_text_of_an_ApiError_that_is_not_an_Error', () => {
    // The Tauri client rejects with a plain object. `e instanceof Error ? e.message : String(e)` printed "[object Object]".
    const rejected = toApiError({ code: 'io', message: 'The folder X already has files in it. Choose a new or empty folder.' });
    expect(rejected instanceof Error).toBe(false);
    expect(String(rejected)).toBe('[object Object]');
    setLanguage('en');
    expect(errMessage(rejected, 'fallback')).toBe('The folder X already has files in it. Choose a new or empty folder.');
    setLanguage('uk');
    expect(errMessage(rejected, 'fallback')).toBe('У теці X уже є файли. Виберіть нову або порожню теку.');
  });

  it('reads_an_Error_and_a_string_and_falls_back_when_there_is_no_text', () => {
    setLanguage('en');
    expect(errMessage(new Error('boom'), 'fallback')).toBe('boom');
    expect(errMessage('plain text', 'fallback')).toBe('plain text');
    expect(errMessage({ code: 'io', message: '' }, 'fallback')).toBe('fallback');
    expect(errMessage(undefined, 'fallback')).toBe('fallback');
  });
});
