import { en } from './en';
import { uk } from './uk';
import coverage from './locales/coverage.json';
import type { Widen, WidenPartial } from './types';

export type Messages = Widen<typeof en>;
export type PartialMessages = WidenPartial<typeof en>;

export const LANGUAGE_CODES = [
  'en', 'uk', 'zh-Hans', 'hi', 'es', 'ar', 'fr', 'bn', 'pt', 'id', 'ur', 'de', 'ja', 'mr', 'vi', 'te', 'ha', 'tr', 'sw', 'tl', 'ta',
] as const;
export type LanguageCode = (typeof LANGUAGE_CODES)[number];
export type LanguagePref = LanguageCode | 'system';

export interface LanguageInfo {
  code: LanguageCode;
  /** The language's own name, shown in the Settings list. */
  name: string;
  /** The BCP-47 tag handed to `Intl` (dates, numbers, plural rules) and written to `<html lang>`. */
  tag: string;
  /** Writing direction; `rtl` for Arabic and Urdu. */
  dir: 'ltr' | 'rtl';
  /** Share of the English keys this language has translated, 0..100 (100 for English and Ukrainian). */
  percent: number;
}

const lang = (code: LanguageCode, name: string, tag: string = code, dir: 'ltr' | 'rtl' = 'ltr'): LanguageInfo => ({
  code,
  name,
  tag,
  dir,
  percent: code === 'en' || code === 'uk' ? 100 : ((coverage as Record<string, number>)[code] ?? 0),
});

/**
 * Every interface language. English and Ukrainian come first (the two kept complete by feature work); the rest are
 * partial catalogues, ordered by code. Adding one means a file in `locales/`, its code in `LANGUAGE_CODES` and one line here.
 */
export const LANGUAGES: readonly LanguageInfo[] = [
  lang('en', 'English'),
  lang('uk', 'Українська'),
  lang('ar', 'العربية', 'ar', 'rtl'),
  lang('bn', 'বাংলা'),
  lang('de', 'Deutsch'),
  lang('es', 'Español'),
  lang('fr', 'Français'),
  lang('ha', 'Hausa'),
  lang('hi', 'हिन्दी'),
  lang('id', 'Bahasa Indonesia'),
  lang('ja', '日本語'),
  lang('mr', 'मराठी'),
  lang('pt', 'Português'),
  lang('sw', 'Kiswahili'),
  lang('ta', 'தமிழ்'),
  lang('te', 'తెలుగు'),
  lang('tl', 'Filipino', 'fil'),
  lang('tr', 'Türkçe'),
  lang('ur', 'اردو', 'ur', 'rtl'),
  lang('vi', 'Tiếng Việt'),
  lang('zh-Hans', '中文简体', 'zh-Hans'),
];

const BASE: LanguageCode = 'en';

/** A partial language is picked automatically from the system language only when at least this share is translated. */
export const AUTO_PICK_MIN_PERCENT = 50;

/** Which language a navigator tag asks for, whatever its translated share; undefined when the app has none. */
export function languageOfTag(tag: string): LanguageCode | undefined {
  const parts = tag.trim().replace(/_/g, '-').split('-');
  const primary = (parts[0] ?? '').toLowerCase();
  if (primary === 'zh') {
    // Traditional Chinese (zh-TW, zh-HK, zh-Hant) has no catalogue; only Simplified is offered.
    const rest = parts.slice(1).map((p) => p.toLowerCase());
    return rest.some((p) => ['hant', 'tw', 'hk', 'mo'].includes(p)) ? undefined : 'zh-Hans';
  }
  if (primary === 'fil') return 'tl';
  return LANGUAGES.find((l) => l.code.toLowerCase() === primary)?.code;
}

/**
 * The OS or browser language mapped to a supported one. Navigator tags are tried in order; the first whose language is
 * English, Ukrainian or a partial catalogue at least `AUTO_PICK_MIN_PERCENT` translated wins. Anything else gives English.
 */
export function resolveSystemLanguage(
  tags: string | readonly string[] | undefined = typeof navigator === 'undefined' ? undefined : navigator.languages?.length ? navigator.languages : [navigator.language],
): LanguageCode {
  const list = typeof tags === 'string' ? [tags] : (tags ?? []);
  for (const tag of list) {
    const code = languageOfTag(tag);
    const info = LANGUAGES.find((l) => l.code === code);
    if (info && info.percent >= AUTO_PICK_MIN_PERCENT) return info.code;
  }
  return BASE;
}

const loaders = import.meta.glob<{ default: PartialMessages }>('./locales/*.ts');

class LocaleStore {
  pref = $state<LanguagePref>('uk');
  /** Partial catalogues loaded so far, by code. Replaced as a whole on every load so readers re-run. */
  loaded = $state.raw<Partial<Record<LanguageCode, PartialMessages>>>({});

  /** The active language; follows `pref`, and the system language when `pref` is `system`. */
  get locale(): LanguageCode {
    return this.pref === 'system' ? resolveSystemLanguage() : this.pref;
  }

  get info(): LanguageInfo {
    return LANGUAGES.find((l) => l.code === this.locale) ?? LANGUAGES[0];
  }

  /** The catalogue of the active language when it is loaded (English and Ukrainian always are). */
  get messages(): PartialMessages | undefined {
    const code = this.locale;
    if (code === 'en') return en as unknown as PartialMessages;
    if (code === 'uk') return uk as unknown as PartialMessages;
    return this.loaded[code];
  }

  set(pref: LanguagePref): void {
    this.pref = pref;
    const info = this.info;
    if (typeof document !== 'undefined') {
      document.documentElement.lang = info.tag;
      document.documentElement.dir = info.dir;
    }
    void this.load(info.code);
  }

  /** Loads the catalogue of `code` (a dynamic import, one chunk per language). Until it lands, English text shows. */
  load(code: LanguageCode): Promise<void> {
    if (code === 'en' || code === 'uk' || this.loaded[code]) return Promise.resolve();
    const loader = loaders[`./locales/${code}.ts`];
    if (!loader) return Promise.resolve();
    return loader().then(
      (m) => {
        this.loaded = { ...this.loaded, [code]: m.default };
      },
      () => undefined, // a chunk that fails to load leaves English in place
    );
  }
}

export const localeStore = new LocaleStore();

/** Sets the language preference (a code, or `system`), `<html lang>` and `<html dir>`. */
export const setLanguage = (pref: LanguagePref): void => localeStore.set(pref);
/** Resolves once the catalogue of `code` (default: the active language) is loaded; for tests and screenshots. */
export const loadLocale = (code: LanguageCode = localeStore.locale): Promise<void> => localeStore.load(code);
export const currentLocale = (): LanguageCode => localeStore.locale;
/** The BCP-47 tag of the active language for `Intl` and `toLocale*String`. */
export const currentTag = (): string => localeStore.info.tag;

export type Params = Record<string, string | number>;

/** Formats a number for the active language (digit grouping, decimal mark). */
export function formatNumber(n: number): string {
  return new Intl.NumberFormat(localeStore.info.tag).format(n);
}
