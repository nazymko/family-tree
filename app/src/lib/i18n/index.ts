import type { LeafPaths, Plural } from './types';
import type { Messages, Params } from './locale.svelte';
import { en } from './en';
import { LANGUAGES, localeStore, formatNumber } from './locale.svelte';

export { LANGUAGES, localeStore, formatNumber, setLanguage, currentLocale, resolveSystemLanguage } from './locale.svelte';
export type { LanguageCode, LanguagePref, LanguageInfo, Messages, Params } from './locale.svelte';
export type { Plural } from './types';

/** Keys of plain messages, and of count messages, as dotted paths into the English catalogue. */
export type MessageKey = LeafPaths<typeof en, string>;
export type PluralKey = LeafPaths<typeof en, Plural>;

function dig(messages: Messages, key: string): string | Plural | undefined {
  let node: unknown = messages;
  for (const part of key.split('.')) {
    if (node === null || typeof node !== 'object') return undefined;
    node = (node as Record<string, unknown>)[part];
  }
  return node as string | Plural | undefined;
}

function find(key: string): string | Plural | undefined {
  // Reading localeStore.locale makes every caller re-run when the language changes.
  const active = LANGUAGES.find((l) => l.code === localeStore.locale)?.messages;
  return (active && dig(active, key)) || dig(en, key);
}

function fill(text: string, params?: Params): string {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (whole, name: string) => (name in params ? String(params[name]) : whole));
}

/** The message for `key` in the active language, with `{name}` placeholders filled from `params`. */
export function t(key: MessageKey, params?: Params): string {
  const hit = find(key);
  return typeof hit === 'string' ? fill(hit, params) : key;
}

/**
 * The count message for `key` in the form that fits `n` (Ukrainian: one, few, many, other).
 * `{n}` in the text is the count, formatted for the language; other placeholders come from `params`.
 */
export function plural(key: PluralKey, n: number, params?: Params): string {
  const hit = find(key);
  if (hit === undefined || typeof hit === 'string') return key;
  const category = new Intl.PluralRules(localeStore.locale).select(n);
  const text = (hit as unknown as Record<string, string | undefined>)[category] ?? hit.other;
  return fill(text, { ...params, n: formatNumber(n) });
}
