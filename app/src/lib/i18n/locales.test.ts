import { afterEach, describe, expect, it } from 'vitest';
import { en } from './en';
import coverage from './locales/coverage.json';
import { LANGUAGES, LANGUAGE_CODES, languageOfTag, loadLocale, localeStore, plural, setLanguage, t, currentTag, formatNumber } from '.';
import type { PartialMessages } from '.';

afterEach(() => {
  setLanguage('en');
  localeStore.loaded = {};
});

type Tree = { [k: string]: string | Tree };
const CATS = ['zero', 'one', 'two', 'few', 'many', 'other'];

const isPluralShape = (v: unknown): v is Tree =>
  typeof v === 'object' && v !== null && 'one' in v && 'other' in v && Object.keys(v).every((c) => CATS.includes(c));

/** English units: a string, or a plural entry. Decided on English's shape, as the tooling does. */
function enUnits(node: Tree, prefix = ''): Map<string, string | Tree> {
  const out = new Map<string, string | Tree>();
  for (const [k, v] of Object.entries(node)) {
    if (typeof v === 'string' || isPluralShape(v)) out.set(prefix + k, v);
    else for (const [kk, vv] of enUnits(v as Tree, `${prefix}${k}.`)) out.set(kk, vv);
  }
  return out;
}
const placeholders = (s: string) => [...new Set([...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]))].sort().join();
const base = enUnits(en as unknown as Tree);

/** The entries of a partial catalogue, looked up by English's units. Keys English lacks are reported as such. */
function ownUnits(messages: Tree): { found: Map<string, string | Tree>; stray: string[] } {
  const found = new Map<string, string | Tree>();
  const seen = new Set<string>();
  for (const [key] of base) {
    let node: unknown = messages;
    for (const part of key.split('.')) node = node && typeof node === 'object' ? (node as Tree)[part] : undefined;
    if (node !== undefined) found.set(key, node as string | Tree);
    seen.add(key);
  }
  const stray: string[] = [];
  const walk = (node: Tree, prefix: string) => {
    for (const [k, v] of Object.entries(node)) {
      const key = prefix + k;
      if (seen.has(key)) continue;
      if (typeof v === 'string' || !Array.from(base.keys()).some((b) => b.startsWith(`${key}.`))) stray.push(key);
      else walk(v, `${key}.`);
    }
  };
  walk(messages, '');
  return { found, stray };
}

const files = import.meta.glob<{ default: PartialMessages }>('./locales/*.ts', { eager: true });
const locales = Object.entries(files).map(([path, m]) => [/\/([^/]+)\.ts$/.exec(path)![1], m.default] as const);

describe('partial locale files', () => {
  it('there_is_one_file_for_each_of_the_19_partial_languages', () => {
    expect(locales.map(([c]) => c).sort()).toEqual(LANGUAGE_CODES.filter((c) => c !== 'en' && c !== 'uk').sort());
  });

  it.each(locales)('%s_has_only_keys_english_has_with_matching_placeholders', (code, messages) => {
    const { found, stray } = ownUnits(messages as unknown as Tree);
    const bad: string[] = stray.map((k) => `${k}: not in English`);
    for (const [key, value] of found) {
      const english = base.get(key)!;
      if (typeof english === 'string') {
        if (typeof value !== 'string' || placeholders(value) !== placeholders(english)) bad.push(`${key}: text or placeholders differ`);
      } else if (typeof value === 'string' || typeof value.other !== 'string') {
        bad.push(`${key}: plural shape`);
      } else if (placeholders(value.other as string) !== placeholders(english.other as string)) {
        bad.push(`${key}: plural placeholders`);
      }
    }
    expect(bad, code).toEqual([]);
  });

  it('coverage_json_matches_the_files', () => {
    for (const [code, messages] of locales) {
      const share = Math.round((ownUnits(messages as unknown as Tree).found.size / base.size) * 100);
      expect((coverage as Record<string, number>)[code], code).toBe(share);
    }
  });
});

describe('the language list', () => {
  it('lists_21_languages_by_native_name', () => {
    expect(LANGUAGES).toHaveLength(21);
    expect(LANGUAGES.slice(0, 2).map((l) => l.name)).toEqual(['English', 'Українська']);
    const names = LANGUAGES.map((l) => l.name);
    for (const n of ['中文简体', 'हिन्दी', 'Español', 'العربية', 'Français', 'বাংলা', 'Português', 'Bahasa Indonesia', 'اردو', 'Deutsch', '日本語', 'मराठी', 'Tiếng Việt', 'తెలుగు', 'Hausa', 'Türkçe', 'Kiswahili', 'Filipino', 'தமிழ்']) {
      expect(names).toContain(n);
    }
    expect(new Set(LANGUAGES.map((l) => l.code)).size).toBe(21);
  });

  it('only_arabic_and_urdu_are_right_to_left', () => {
    expect(LANGUAGES.filter((l) => l.dir === 'rtl').map((l) => l.code).sort()).toEqual(['ar', 'ur']);
  });

  it('english_and_ukrainian_are_complete_the_rest_report_their_share', () => {
    expect(LANGUAGES.find((l) => l.code === 'en')!.percent).toBe(100);
    expect(LANGUAGES.find((l) => l.code === 'uk')!.percent).toBe(100);
    for (const l of LANGUAGES.slice(2)) expect(l.percent).toBe((coverage as Record<string, number>)[l.code] ?? 0);
  });

  it('maps_navigator_tags_to_languages', () => {
    const of = (tag: string) => languageOfTag(tag);
    expect(of('zh-CN')).toBe('zh-Hans');
    expect(of('zh')).toBe('zh-Hans');
    expect(of('zh-Hans-SG')).toBe('zh-Hans');
    expect(of('zh-TW')).toBeUndefined();
    expect(of('pt-BR')).toBe('pt');
    expect(of('pt_PT')).toBe('pt');
    expect(of('fil')).toBe('tl');
    expect(of('fil-PH')).toBe('tl');
    expect(of('es-419')).toBe('es');
    expect(of('ar-EG')).toBe('ar');
    expect(of('ru')).toBeUndefined();
    expect(of('')).toBeUndefined();
  });
});

describe('fallback to English, key by key', () => {
  it('shows_a_translated_key_and_English_for_the_rest', async () => {
    setLanguage('de');
    await loadLocale('de');
    // Every real language is fully translated now, so a partial catalogue is put in by hand.
    localeStore.loaded = { ...localeStore.loaded, de: { nav: { people: 'Personen', family: 'Familie' } } };
    expect(t('nav.people')).toBe('Personen');
    expect(t('nav.family')).toBe('Familie');
    expect(t('agent.title')).toBe(en.agent.title);
  });

  it('shows_English_until_the_catalogue_has_loaded', () => {
    setLanguage('de');
    expect(t('nav.people')).toBe(en.nav.people);
  });

  it('fills_placeholders_in_the_fallback_text', async () => {
    setLanguage('de');
    await loadLocale('de');
    localeStore.loaded = { ...localeStore.loaded, de: { nav: { people: 'Personen' } } };
    expect(t('shell.openFolder', { dir: 'X' })).toBe(en.shell.openFolder.replace('{dir}', 'X'));
  });

  it('plural_uses_the_language_rules', async () => {
    setLanguage('ar');
    await loadLocale('ar');
    expect(plural('common.people', 2)).toContain('شخصان');
    expect(plural('common.people', 3)).toContain('أشخاص');
    expect(plural('common.people', 11)).toContain('شخصًا');
    setLanguage('es');
    await loadLocale('es');
    expect(plural('common.people', 1)).toBe('1 persona');
    expect(plural('common.people', 3)).toBe('3 personas');
    setLanguage('ja');
    await loadLocale('ja');
    expect(plural('common.people', 1)).toBe('1 人');
  });

  it('plural_without_the_selected_category_uses_the_locale_other', async () => {
    setLanguage('de');
    await loadLocale('de');
    localeStore.loaded = { ...localeStore.loaded, de: { common: { people: { other: '{n} Leute' } } } };
    expect(plural('common.people', 1)).toBe('1 Leute');
    expect(plural('common.people', 7)).toBe('7 Leute');
  });

  it('plural_missing_in_the_locale_uses_English_with_English_rules', async () => {
    setLanguage('ar');
    await loadLocale('ar');
    localeStore.loaded = { ...localeStore.loaded, ar: { nav: { people: 'الأشخاص' } } };
    expect(plural('common.sources', 1)).toBe('1 source');
    expect(plural('common.sources', 2)).toBe('2 sources');
    setLanguage('ja');
    await loadLocale('ja');
    localeStore.loaded = { ...localeStore.loaded, ja: { nav: { people: '人物' } } };
    expect(plural('common.sources', 1)).toBe('1 source');
  });

  it('never_shows_a_raw_key_when_English_has_it', async () => {
    for (const l of LANGUAGES) {
      setLanguage(l.code);
      await loadLocale(l.code);
      expect(t('common.save')).not.toBe('common.save');
      expect(plural('common.people', 5)).not.toBe('common.people');
    }
  });
});

describe('direction and language of the page', () => {
  it('sets_dir_and_lang_on_the_document', () => {
    setLanguage('ar');
    expect(document.documentElement.dir).toBe('rtl');
    expect(document.documentElement.lang).toBe('ar');
    setLanguage('ur');
    expect(document.documentElement.dir).toBe('rtl');
    setLanguage('en');
    expect(document.documentElement.dir).toBe('ltr');
    expect(document.documentElement.lang).toBe('en');
    setLanguage('zh-Hans');
    expect(document.documentElement.dir).toBe('ltr');
    expect(document.documentElement.lang).toBe('zh-Hans');
    setLanguage('tl');
    expect(document.documentElement.lang).toBe('fil');
    expect(currentTag()).toBe('fil');
  });

  it('numbers_follow_the_language_tag', () => {
    setLanguage('de');
    expect(formatNumber(1234.5)).toBe('1.234,5');
  });
});
