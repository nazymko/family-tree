import { t, type MessageKey } from '../i18n';

/** The fixed kinds of a source, in the order the picker shows them. The ids are the stored ones. */
export const SOURCE_KIND_IDS = [
  'civil_registry_extract',
  'civil_certificate',
  'metric_book',
  'census',
  'household_book',
  'military_record',
  'archive_document',
  'personal_document',
  'photo',
  'gravestone',
  'oral_testimony',
  'book_or_article',
  'website_or_database',
  'genealogy_tree',
  'research_analysis',
  'other',
] as const;

const FIXED = new Set<string>(SOURCE_KIND_IDS);

/** The picker value of a stored kind id: a fixed id, or `other` with its own label. */
export function splitKind(kind: string | null | undefined): { pick: string; label: string } {
  if (!kind) return { pick: '', label: '' };
  if (kind.startsWith('other:')) return { pick: 'other', label: kind.slice(6) };
  return FIXED.has(kind) ? { pick: kind, label: '' } : { pick: 'other', label: kind };
}

/** The stored kind id from the picker value and the free label. */
export function joinKind(pick: string, label: string): string {
  if (pick === 'other' && label.trim()) return `other:${label.trim()}`;
  return pick;
}

/** A kind id in words; a free label as written. Empty when the source has no kind. */
export function kindLabel(kind: string | null | undefined): string {
  const { pick, label } = splitKind(kind);
  if (!pick) return '';
  if (pick === 'other' && label) return label;
  return t(`sourceKind.${pick}` as MessageKey);
}
