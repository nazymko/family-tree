import { en } from './en';
import { uk } from './uk';
import type { Widen } from './types';

export type Messages = Widen<typeof en>;
export type LanguageCode = 'en' | 'uk';
export type LanguagePref = LanguageCode | 'system';

export interface LanguageInfo {
  code: LanguageCode;
  /** The language's own name, shown in the Settings list. */
  name: string;
  messages: Messages;
}

/** Every interface language. Adding one means a catalogue file and one line here. */
export const LANGUAGES: readonly LanguageInfo[] = [
  { code: 'en', name: 'English', messages: en },
  { code: 'uk', name: 'Українська', messages: uk },
];

const BASE: LanguageCode = 'en';

/** The OS or browser language mapped to a supported one; anything unknown gives English. */
export function resolveSystemLanguage(tag: string | undefined = typeof navigator === 'undefined' ? undefined : navigator.language): LanguageCode {
  const primary = (tag ?? '').toLowerCase().split(/[-_]/)[0];
  return LANGUAGES.find((l) => l.code === primary)?.code ?? BASE;
}

class LocaleStore {
  pref = $state<LanguagePref>('system');
  /** The active language; follows `pref`, and the system language when `pref` is `system`. */
  get locale(): LanguageCode {
    return this.pref === 'system' ? resolveSystemLanguage() : this.pref;
  }

  set(pref: LanguagePref): void {
    this.pref = pref;
    if (typeof document !== 'undefined') document.documentElement.lang = this.locale;
  }
}

export const localeStore = new LocaleStore();

/** Sets the language preference (a code, or `system`) and `<html lang>`. */
export const setLanguage = (pref: LanguagePref): void => localeStore.set(pref);
export const currentLocale = (): LanguageCode => localeStore.locale;

export type Params = Record<string, string | number>;

/** Formats a number for the active language (digit grouping, decimal mark). */
export function formatNumber(n: number): string {
  return new Intl.NumberFormat(localeStore.locale).format(n);
}
