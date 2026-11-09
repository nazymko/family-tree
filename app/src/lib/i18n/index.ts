import type { LeafPaths, Plural, PluralForms } from './types';
import type { Messages, Params } from './locale.svelte';
import { en } from './en';
import { localeStore, formatNumber } from './locale.svelte';

export {
  LANGUAGES,
  LANGUAGE_CODES,
  AUTO_PICK_MIN_PERCENT,
  localeStore,
  formatNumber,
  setLanguage,
  loadLocale,
  currentLocale,
  currentTag,
  languageOfTag,
  resolveSystemLanguage,
} from './locale.svelte';
export type { LanguageCode, LanguagePref, LanguageInfo, Messages, PartialMessages, Params } from './locale.svelte';
export type { Plural, PluralForms, PluralCategory, WidenPartial } from './types';

/** Keys of plain messages, and of count messages, as dotted paths into the English catalogue. */
export type MessageKey = LeafPaths<typeof en, string>;
export type PluralKey = LeafPaths<typeof en, Plural>;

function dig(messages: unknown, key: string): string | PluralForms | undefined {
  let node: unknown = messages;
  for (const part of key.split('.')) {
    if (node === null || typeof node !== 'object') return undefined;
    node = (node as Record<string, unknown>)[part];
  }
  return node as string | PluralForms | undefined;
}

/** The entry for `key` in the active language, else in English (key by key; a partial catalogue may lack any key). */
function find(key: string): string | PluralForms | undefined {
  // Reading localeStore.messages makes every caller re-run when the language, or its catalogue, changes.
  const active = localeStore.messages;
  return (active && dig(active, key)) || dig(en as unknown as Messages, key);
}

function fill(text: string, params?: Params): string {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (whole, name: string) => (name in params ? String(params[name]) : whole));
}

/** The message for `key` in the active language (English where it has none), with `{name}` placeholders filled from `params`. */
export function t(key: MessageKey, params?: Params): string {
  const hit = find(key);
  return typeof hit === 'string' ? fill(hit, params) : key;
}

/**
 * The count message for `key` in the form that fits `n` under the active language's CLDR plural rules.
 * A language lacking that category uses its own `other`; a language lacking the key altogether uses English
 * with English rules. `{n}` in the text is the count, formatted for the language; other placeholders come from `params`.
 */
export function plural(key: PluralKey, n: number, params?: Params): string {
  const own = localeStore.messages ? dig(localeStore.messages, key) : undefined;
  if (own !== undefined && typeof own !== 'string' && own.other !== undefined) {
    const category = new Intl.PluralRules(localeStore.info.tag).select(n);
    return fill(own[category] ?? own.other, { ...params, n: formatNumber(n) });
  }
  const base = dig(en as unknown as Messages, key);
  if (base === undefined || typeof base === 'string') return key;
  const category = new Intl.PluralRules('en').select(n);
  return fill(base[category] ?? base.other ?? key, { ...params, n: formatNumber(n) });
}
