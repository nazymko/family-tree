import type { CitationDto } from '../api/api';

const SHIFR = /(?<![\p{L}\d])\d{1,5}-\d{1,5}-\d{1,6}(?![\d])/gu;

/** The archive shelf numbers (127-1012-1603) a text mentions. */
export function shifrsIn(text: string): string[] {
  return [...text.matchAll(SHIFR)].map((m) => m[0]);
}

function haystack(c: CitationDto): string {
  const s = c.source;
  return [s?.title, s?.abbreviation, s?.callNumber, c.page].filter(Boolean).join(' ');
}

/**
 * The citations a note's text refers to: those whose source shelf number or title appears in the text.
 * Empty when the text names none of them.
 */
export function citationsReferredTo(text: string, citations: CitationDto[]): CitationDto[] {
  const shifrs = shifrsIn(text);
  const lower = text.toLowerCase();
  return citations.filter((c) => {
    const h = haystack(c);
    if (shifrs.some((sh) => h.includes(sh))) return true;
    const title = c.source?.title?.trim().toLowerCase();
    return !!title && title.length >= 4 && lower.includes(title);
  });
}
