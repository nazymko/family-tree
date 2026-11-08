import type { AppInfo } from '../api/types';
import { currentLocale, t } from '../i18n';

export const REPO = 'https://github.com/nazymko/family-tree';
export const RELEASES_URL = `${REPO}/releases`;
export const REPORT_URL = `${REPO}/issues/new/choose`;
export const LICENCE = 'MIT / Apache-2.0';

/** `2026-11-08` as a date in the language of the window; anything else is shown as it is. */
export function shortDate(iso: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(iso);
  if (!m) return iso;
  const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  return d.toLocaleDateString(currentLocale(), { dateStyle: 'medium' });
}

/**
 * "Version 0.3.0" for a release; for any other build "Version 0.3.0+12 (a1b2c3d, 8 Nov 2026)": the release it grew from,
 * the commits since, and which commit it is.
 */
export function versionLine(info: AppInfo): string {
  if (info.release) return t('about.version', { version: info.version });
  const build = [info.commit, info.date ? shortDate(info.date) : null].filter(Boolean).join(', ');
  return build ? t('about.versionBuild', { version: info.version, build }) : t('about.version', { version: info.version });
}
