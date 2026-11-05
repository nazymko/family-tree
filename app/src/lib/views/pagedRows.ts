import type { LogRowDto, Page } from '../api/types';

/** Rows shown at first, and added by each Show more. */
export const ROW_PAGE = 20;

/** The longest page the backend gives, whatever is asked. */
const BACKEND_MAX = 200;

/**
 * Reads rows from the start until `want` of them are in hand (or there are no more). One call asks for at most the
 * backend's longest page, so a list the owner has paged past 200 rows comes back whole instead of cut short.
 */
export async function loadRows(
  fetch: (offset: number, limit: number) => Promise<Page<LogRowDto>>,
  want: number,
): Promise<{ items: LogRowDto[]; total: number }> {
  const target = Math.max(ROW_PAGE, want);
  const items: LogRowDto[] = [];
  let total = 0;
  while (items.length < target) {
    const page = await fetch(items.length, Math.min(BACKEND_MAX, target - items.length));
    total = page.total;
    const fresh = page.items.filter((r) => !items.some((x) => x.group === r.group));
    items.push(...fresh);
    if (fresh.length === 0 || items.length >= total) break;
  }
  return { items, total };
}
