import type { ApiError, ErrorCode } from '../types';
import { t } from '../../i18n';

const CODES: readonly ErrorCode[] = ['not_found', 'invalid', 'conflict', 'io', 'no_project', 'stale', 'internal'];

/**
 * Turns whatever `invoke` rejected with into an ApiError. The Rust side serialises `{ code, message }`; a bare
 * string (a Tauri-level failure such as an unknown command) or a malformed payload becomes `internal`.
 */
export function toApiError(e: unknown): ApiError {
  if (typeof e === 'object' && e !== null) {
    const { code, message } = e as { code?: unknown; message?: unknown };
    if (typeof code === 'string' && (CODES as readonly string[]).includes(code) && typeof message === 'string') {
      return { code: code as ErrorCode, message };
    }
    if (e instanceof Error) return { code: 'internal', message: e.message };
  }
  return { code: 'internal', message: typeof e === 'string' && e ? e : t('edit.wentWrong') };
}
