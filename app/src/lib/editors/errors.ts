import { t } from '../i18n';
import { backendError } from '../i18n/backend';

/**
 * A readable message, in the app's language, from whatever a call rejected with: an ApiError (a plain object, which is
 * NOT an `Error`), an `Error`, or a string. `fallback` is shown when there is no text. Use this instead of
 * `e instanceof Error ? e.message : String(e)`, which prints "[object Object]" for an ApiError.
 */
export function errMessage(e: unknown, fallback?: string): string {
  const text = typeof e === 'string' ? e : e && typeof e === 'object' && typeof (e as { message?: unknown }).message === 'string' ? (e as { message: string }).message : '';
  return text ? backendError(text) : (fallback ?? t('edit.notSaved'));
}

/** Drop empty optional strings so commands carry only what the user entered. */
export function opt(s: string | null | undefined): string | undefined {
  const t = (s ?? '').trim();
  return t ? t : undefined;
}

export function splitList(s: string): string[] {
  return s
    .split(',')
    .map((x) => x.trim())
    .filter(Boolean);
}
