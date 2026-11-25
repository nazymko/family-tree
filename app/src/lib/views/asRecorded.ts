import type { NameDto } from '../api/types';

/** One name as a record writes it, for the line under the main name on the person page. */
export interface AsRecordedLine {
  name: NameDto;
  /** The title of the first cited source, or null when none is cited yet. */
  source: string | null;
  /** The year of the record: from the citation's date, else from the name's date. */
  year: string | null;
  /** Every cited source with its page and the text the record has, one per line (for the tooltip). */
  detail: string;
}

function yearOf(display: string | null | undefined): string | null {
  if (!display) return null;
  const m = /\d{4}/.exec(display);
  return m ? m[0] : display;
}

/** The person's names of kind `as_recorded` that are not the preferred one, in their order. */
export function asRecordedLines(names: NameDto[]): AsRecordedLine[] {
  return names
    .filter((n) => n.kind === 'as_recorded' && !n.preferred)
    .map((name) => {
      const first = name.citations[0];
      const detail = name.citations
        .map((c) => [c.source?.title, c.page, c.date?.display, c.text].filter((x) => x && x.trim()).join(', '))
        .filter(Boolean)
        .join('\n');
      const source = first?.source?.title ?? null;
      const year = yearOf(first?.date?.display) ?? yearOf(name.date?.display);
      // A title that already says the year («Метрична книга ..., 1885») does not get it twice.
      return { name, source, year: year && source?.includes(year) ? null : year, detail };
    });
}
