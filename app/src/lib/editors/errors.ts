
import { t } from '../i18n';
import { backendError } from '../i18n/backend';

/** A readable message from an ApiError-like rejection (a plain object or an Error). */
export function errMessage(e: unknown): string {
  if (e && typeof e === 'object' && 'message' in e && typeof (e as { message: unknown }).message === 'string') {
    const m = (e as { message: string }).message;
    if (m) return backendError(m);
  }
  return t('edit.notSaved');
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
