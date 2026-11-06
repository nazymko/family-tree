import type { ApiError, ErrorCode } from '../types';
import { t } from '../../i18n';
import { backendError } from '../../i18n/backend';

const CODES: readonly ErrorCode[] = ['not_found', 'invalid', 'conflict', 'io', 'no_project', 'stale', 'internal'];

/**
 * Turns whatever `invoke` rejected with into an ApiError. The Rust side serialises `{ code, message }`; a bare
 * string (a Tauri-level failure such as an unknown command) or a malformed payload becomes `internal`.
 *
 * This is the one place a backend sentence is put into the app's language (`backendError`), so every screen that
 * shows `e.message` shows the translated text and no screen has to translate. The result is a plain object, not an
 * `Error`: read its text with `errMessage` (editors/errors.ts), never with `e instanceof Error ? e.message : ...`.
 */
export function toApiError(e: unknown): ApiError {
  if (typeof e === 'object' && e !== null) {
    const { code, message } = e as { code?: unknown; message?: unknown };
    if (typeof code === 'string' && (CODES as readonly string[]).includes(code) && typeof message === 'string') {
      return { code: code as ErrorCode, message: backendError(message) };
    }
    if (e instanceof Error) return { code: 'internal', message: e.message };
  }
  return { code: 'internal', message: typeof e === 'string' && e ? e : t('edit.wentWrong') };
}
