/** Word-level comparison of two texts for the before/after tables of the change history. */

export type DiffKind = 'eq' | 'del' | 'ins';
export interface DiffSeg {
  kind: DiffKind;
  text: string;
}
/** A piece of one side: unchanged, changed, or `gap` for unchanged text left out. */
export interface ViewSeg {
  kind: DiffKind | 'gap';
  text: string;
}
export interface TextDiff {
  segs: DiffSeg[];
  removedWords: number;
  addedWords: number;
  /** Words the two texts share; a diff of texts with none is not worth marking. */
  sharedWords: number;
}

/** A word (letters, digits and marks, with inner apostrophes), a run of white space, or one other character. */
const TOKEN = /[\p{L}\p{N}\p{M}]+(?:['’ʼ][\p{L}\p{N}\p{M}]+)*|\s+|[^\s\p{L}\p{N}\p{M}]/gu;
const WORD = /^[\p{L}\p{N}\p{M}]/u;
/** The most cells of the comparison table one pass may fill. */
const MAX_CELLS = 3_000_000;

export function tokenize(s: string): string[] {
  return s.match(TOKEN) ?? [];
}
export const isWord = (tok: string): boolean => WORD.test(tok);
const countWords = (toks: string[]): number => toks.reduce((n, t) => n + (isWord(t) ? 1 : 0), 0);

/** Longest common subsequence as a list of [index in a, index in b] pairs. */
function lcs(a: string[], b: string[]): [number, number][] {
  const n = a.length;
  const m = b.length;
  const w = m + 1;
  const table = new Uint32Array((n + 1) * w);
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      table[i * w + j] = a[i] === b[j] ? table[(i + 1) * w + j + 1] + 1 : Math.max(table[(i + 1) * w + j], table[i * w + j + 1]);
    }
  }
  const out: [number, number][] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) out.push([i++, j++]);
    else if (table[(i + 1) * w + j] >= table[i * w + j + 1]) i++;
    else j++;
  }
  return out;
}

function push(out: DiffSeg[], kind: DiffKind, text: string): void {
  if (text === '') return;
  const last = out[out.length - 1];
  if (last && last.kind === kind) last.text += text;
  else out.push({ kind, text });
}

/** Emits the diff of two (already trimmed) lists through their longest common subsequence. */
function pairs(a: string[], b: string[], out: DiffSeg[]): void {
  let i = 0;
  let j = 0;
  for (const [x, y] of a.length && b.length ? lcs(a, b) : []) {
    push(out, 'del', a.slice(i, x).join(''));
    push(out, 'ins', b.slice(j, y).join(''));
    push(out, 'eq', a[x]);
    i = x + 1;
    j = y + 1;
  }
  push(out, 'del', a.slice(i).join(''));
  push(out, 'ins', b.slice(j).join(''));
}

/** Joins tokens into pieces that end after a sentence stop or a line break. */
function sentences(toks: string[]): string[] {
  const out: string[] = [];
  let cur = '';
  for (const t of toks) {
    cur += t;
    if (/^[.!?…]$/.test(t) || t.includes('\n')) {
      out.push(cur);
      cur = '';
    }
  }
  if (cur) out.push(cur);
  return out;
}

/** Diffs two token lists; when the table would be too large, falls back to sentence-sized pieces, then to all-or-nothing. */
function diffTokens(a: string[], b: string[], out: DiffSeg[]): void {
  // Trim what both ends share: it is most of a long text with one small edit.
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let endA = a.length;
  let endB = b.length;
  while (endA > start && endB > start && a[endA - 1] === b[endB - 1]) {
    endA--;
    endB--;
  }
  push(out, 'eq', a.slice(0, start).join(''));
  const ma = a.slice(start, endA);
  const mb = b.slice(start, endB);
  if ((ma.length + 1) * (mb.length + 1) <= MAX_CELLS) {
    pairs(ma, mb, out);
  } else {
    const sa = sentences(ma);
    const sb = sentences(mb);
    if ((sa.length + 1) * (sb.length + 1) <= MAX_CELLS) pairs(sa, sb, out);
    else {
      push(out, 'del', ma.join(''));
      push(out, 'ins', mb.join(''));
    }
  }
  push(out, 'eq', a.slice(endA).join(''));
}

export function diffText(before: string, after: string): TextDiff {
  const segs: DiffSeg[] = [];
  diffTokens(tokenize(before), tokenize(after), segs);
  const words = (kind: DiffKind) => segs.filter((s) => s.kind === kind).reduce((n, s) => n + countWords(tokenize(s.text)), 0);
  return { segs, removedWords: words('del'), addedWords: words('ins'), sharedWords: words('eq') };
}

/** One side of a diff: `before` keeps the unchanged and removed pieces, `after` the unchanged and added ones. */
export function sideOf(segs: DiffSeg[], side: 'before' | 'after'): ViewSeg[] {
  const drop: DiffKind = side === 'before' ? 'ins' : 'del';
  return segs.filter((s) => s.kind !== drop);
}

/** Keeps `ctx` words of every unchanged stretch next to a change and puts a gap where the rest was. */
export function collapse(view: ViewSeg[], ctx = 10): ViewSeg[] {
  const out: ViewSeg[] = [];
  view.forEach((seg, idx) => {
    if (seg.kind !== 'eq') {
      out.push(seg);
      return;
    }
    const toks = tokenize(seg.text);
    const needHead = idx !== 0;
    const needTail = idx !== view.length - 1;
    // Index after the ctx-th word from the start, and the index of the ctx-th word from the end.
    let headEnd = 0;
    let seen = 0;
    while (headEnd < toks.length && seen < ctx) if (isWord(toks[headEnd++])) seen++;
    let tailStart = toks.length;
    seen = 0;
    while (tailStart > 0 && seen < ctx) if (isWord(toks[--tailStart])) seen++;
    const eq = (a: number, b?: number): ViewSeg => ({ kind: 'eq', text: toks.slice(a, b).join('') });
    const gap: ViewSeg = { kind: 'gap', text: '…' };
    if (needHead && needTail) {
      if (tailStart > headEnd + 1) out.push(eq(0, headEnd), gap, eq(tailStart));
      else out.push(seg);
    } else if (needHead) {
      if (toks.length > headEnd) out.push(eq(0, headEnd), gap);
      else out.push(seg);
    } else if (needTail) {
      if (tailStart > 0) out.push(gap, eq(tailStart));
      else out.push(seg);
    } else out.push(seg);
  });
  return out;
}
