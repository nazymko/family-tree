import { formatNumber, t } from '../i18n';

/** A file size in words for the active language: 1536 becomes "1.5 KB" ("1,5 КБ"). */
export function sizeText(bytes: number): string {
  const one = (n: number) => formatNumber(Math.round(n * 10) / 10);
  if (bytes < 1024) return t('merge.units.b', { n: formatNumber(bytes) });
  if (bytes < 1024 * 1024) return t('merge.units.kb', { n: one(bytes / 1024) });
  if (bytes < 1024 * 1024 * 1024) return t('merge.units.mb', { n: one(bytes / (1024 * 1024)) });
  return t('merge.units.gb', { n: one(bytes / (1024 * 1024 * 1024)) });
}

/** The last part of a path, without a trailing slash. */
export const fileOf = (path: string) => path.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || path;

/** A name to suggest for a copy: the file or folder name without its extension. */
export const nameOf = (path: string) => fileOf(path).replace(/\.zip$/i, '');
