import type { TextRegionDto, TextRegionInput } from '../api/api';

/** Apostrophe look-alikes that count as one letter when searching: ' ’ ʼ ‘ ` and the modifier/prime forms. */
const APOSTROPHES = /['’ʼ‘ʹ`´]/u;

/** One character folded for comparison: lower case (by the language rules), one kind of apostrophe. */
function foldChar(ch: string): string {
  if (APOSTROPHES.test(ch)) return "'";
  return ch.toLocaleLowerCase();
}

interface Folded {
  text: string;
  /** For each unit of `text`: where the character it came from starts and ends in the original. */
  from: number[];
  to: number[];
}

function fold(s: string): Folded {
  let text = '';
  const from: number[] = [];
  const to: number[] = [];
  let at = 0;
  for (const ch of s) {
    const f = foldChar(ch);
    text += f;
    for (let i = 0; i < f.length; i++) {
      from.push(at);
      to.push(at + ch.length);
    }
    at += ch.length;
  }
  return { text, from, to };
}

/** The text to look for, folded; empty when nothing is typed (no search). */
export function foldQuery(q: string): string {
  return fold(q.trim()).text;
}

/** Spans [start, end) of `s` that match the folded query, in order and not overlapping. */
export function matchRanges(s: string, folded: string): [number, number][] {
  if (!folded || !s) return [];
  const f = fold(s);
  const out: [number, number][] = [];
  let i = f.text.indexOf(folded);
  while (i !== -1) {
    const start = f.from[i];
    const end = f.to[i + folded.length - 1];
    const last = out[out.length - 1];
    if (last && start < last[1]) last[1] = Math.max(last[1], end);
    else out.push([start, end]);
    i = f.text.indexOf(folded, i + folded.length);
  }
  return out;
}

/** A piece of text cut into parts, the matching ones flagged, for building nodes (never HTML). */
export function highlightParts(s: string, folded: string): { text: string; hit: boolean }[] {
  const ranges = matchRanges(s, folded);
  if (ranges.length === 0) return [{ text: s, hit: false }];
  const out: { text: string; hit: boolean }[] = [];
  let at = 0;
  for (const [a, b] of ranges) {
    if (a > at) out.push({ text: s.slice(at, a), hit: false });
    out.push({ text: s.slice(a, b), hit: true });
    at = b;
  }
  if (at < s.length) out.push({ text: s.slice(at), hit: false });
  return out;
}

/** The key of "no speaker" among the speaker filter's choices. */
export const NO_SPEAKER = '';

/** The distinct speakers with their piece counts: by count, most first, then by name; "no speaker" is `NO_SPEAKER`, last. */
export function speakerCounts(pieces: readonly TextRegionDto[]): { speaker: string; count: number }[] {
  const m = new Map<string, number>();
  for (const p of pieces) {
    const k = p.speaker?.trim() || NO_SPEAKER;
    m.set(k, (m.get(k) ?? 0) + 1);
  }
  const named = [...m].filter(([k]) => k !== NO_SPEAKER).sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
  const none = m.get(NO_SPEAKER);
  const out = named.map(([speaker, count]) => ({ speaker, count }));
  if (none) out.push({ speaker: NO_SPEAKER, count: none });
  return out;
}

/** Whether a piece passes the speaker choice (none picked: all pass) and the search (empty: all pass). Both must hold. */
export function pieceShown(p: TextRegionDto, picked: ReadonlySet<string>, folded: string): boolean {
  if (picked.size > 0 && !picked.has(p.speaker?.trim() || NO_SPEAKER)) return false;
  if (!folded) return true;
  return fold(p.text).text.includes(folded) || fold(p.speaker ?? '').text.includes(folded);
}

/**
 * Every piece of the stored transcript as the save command takes it, in stored order, with
 * `speaker(piece)` deciding each piece's speaker (undefined or blank: none). Ids, times and text are
 * kept as they are; `known` is every id seen, so a piece stored meanwhile (by an agent) stays.
 */
export function withSpeakers(
  stored: readonly TextRegionDto[],
  speaker: (p: TextRegionDto) => string | undefined,
): { regions: TextRegionInput[]; known: string[] } {
  const regions = stored.map((p): TextRegionInput => {
    const s = speaker(p)?.trim();
    return {
      id: p.id,
      text: p.text,
      ...(p.startMs != null ? { startMs: p.startMs } : {}),
      ...(p.endMs != null ? { endMs: p.endMs } : {}),
      ...(s ? { speaker: s } : {}),
    };
  });
  return { regions, known: stored.map((p) => p.id) };
}
