import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { brotliDecompressSync } from 'node:zlib';
import { describe, expect, it } from 'vitest';

/**
 * Names from 19th-century metric books use letters gone from today's spelling (ѣ, ѳ, ѵ, і, a final ъ). This reads the
 * character map of the bundled font files and checks those letters are in them, so a font update cannot silently
 * drop them and leave empty boxes in names, charts and A3 prints.
 */

const MODULES = resolve(__dirname, '../../../node_modules');
const OLD = 'ѢѣѲѳѴѵІіЪъ';
const MODERN_UK = 'ЄєЇїҐґЙй';

function base128(b: Buffer, at: { p: number }): number {
  let v = 0;
  for (let i = 0; i < 5; i++) {
    const x = b[at.p++];
    v = (v << 7) | (x & 0x7f);
    if ((x & 0x80) === 0) return v;
  }
  throw new Error('bad UIntBase128');
}

/** The code points a WOFF2 file maps to glyphs (formats 4 and 12 of its `cmap`). */
function woff2CodePoints(file: string): Set<number> {
  const b = readFileSync(file);
  if (b.toString('latin1', 0, 4) !== 'wOF2') throw new Error(`${file}: not WOFF2`);
  const numTables = b.readUInt16BE(12);
  const compressed = b.readUInt32BE(20);
  const at = { p: 48 };
  let cmap: { offset: number; length: number } | null = null;
  let offset = 0;
  for (let i = 0; i < numTables; i++) {
    const flags = b[at.p++];
    const tagIndex = flags & 0x3f;
    let tag = '';
    if (tagIndex === 63) {
      tag = b.toString('latin1', at.p, at.p + 4);
      at.p += 4;
    }
    const version = flags >> 6;
    const orig = base128(b, at);
    // glyf (10) and loca (11) are transformed at version 0, every other table at a version other than 0.
    const transformed = tagIndex === 10 || tagIndex === 11 ? version === 0 : version !== 0;
    const length = transformed ? base128(b, at) : orig;
    if (tagIndex === 0 || tag === 'cmap') cmap = { offset, length };
    offset += length;
  }
  if (!cmap) throw new Error(`${file}: no cmap`);
  const data = brotliDecompressSync(b.subarray(at.p, at.p + compressed));
  const t = data.subarray(cmap.offset, cmap.offset + cmap.length);
  const out = new Set<number>();
  const n = t.readUInt16BE(2);
  for (let i = 0; i < n; i++) {
    const sub = t.readUInt32BE(4 + i * 8 + 4);
    const format = t.readUInt16BE(sub);
    if (format === 4) {
      const segs = t.readUInt16BE(sub + 6) / 2;
      const ends = sub + 14;
      const starts = ends + segs * 2 + 2;
      const deltas = starts + segs * 2;
      const ranges = deltas + segs * 2;
      for (let s = 0; s < segs; s++) {
        const end = t.readUInt16BE(ends + s * 2);
        const start = t.readUInt16BE(starts + s * 2);
        const delta = t.readUInt16BE(deltas + s * 2);
        const rangeAt = ranges + s * 2;
        const range = t.readUInt16BE(rangeAt);
        for (let c = start; c <= end && c !== 0xffff; c++) {
          let glyph: number;
          if (range === 0) glyph = (c + delta) & 0xffff;
          else {
            const g = t.readUInt16BE(rangeAt + range + (c - start) * 2);
            glyph = g === 0 ? 0 : (g + delta) & 0xffff;
          }
          if (glyph !== 0) out.add(c);
        }
      }
    } else if (format === 12) {
      const groups = t.readUInt32BE(sub + 12);
      for (let g = 0; g < groups; g++) {
        const start = t.readUInt32BE(sub + 16 + g * 12);
        const end = t.readUInt32BE(sub + 20 + g * 12);
        for (let c = start; c <= end; c++) out.add(c);
      }
    }
  }
  return out;
}

function covered(files: string[]): Set<number> {
  const all = new Set<number>();
  for (const f of files) for (const c of woff2CodePoints(resolve(MODULES, f))) all.add(c);
  return all;
}

const missing = (cps: Set<number>, letters: string) => [...letters].filter((c) => !cps.has(c.codePointAt(0)!));

describe('bundled fonts have the letters of old records', () => {
  it('Source Serif 4 (names, charts, print) has the old and the Ukrainian letters, upright and italic', () => {
    for (const style of ['normal', 'italic']) {
      const dir = '@fontsource-variable/source-serif-4/files/';
      const cps = covered([`${dir}source-serif-4-cyrillic-wght-${style}.woff2`, `${dir}source-serif-4-cyrillic-ext-wght-${style}.woff2`]);
      expect(missing(cps, OLD + MODERN_UK), style).toEqual([]);
    }
  });

  it('the sans fallback (Atkinson has no Cyrillic) has the old letters in every face fonts.css names', () => {
    const css = readFileSync(resolve(__dirname, '../../styles/fonts.css'), 'utf8');
    const files = [...css.matchAll(/url\(@fontsource\/noto-sans\/files\/([^)]+)\)/g)].map((m) => m[1]);
    expect(files.length).toBeGreaterThanOrEqual(3);
    for (const f of files) expect(missing(covered([`@fontsource/noto-sans/files/${f}`]), 'ѢѣѲѳѴѵ'), f).toEqual([]);
  });

  it('the old-letter face sits in both font stacks, right after the main font', () => {
    const tokens = readFileSync(resolve(__dirname, '../../styles/tokens.css'), 'utf8');
    expect(tokens).toMatch(/--font-serif: 'Source Serif 4 Variable', 'GT Old Cyrillic',/);
    expect(tokens).toMatch(/--font-sans: 'Atkinson Hyperlegible Next', 'GT Old Cyrillic',/);
  });

  it('the reader is not fooled: Atkinson Hyperlegible Next really lacks Cyrillic', () => {
    const cps = covered(['@fontsource/atkinson-hyperlegible-next/files/atkinson-hyperlegible-next-latin-400-normal.woff2']);
    expect(cps.has('A'.codePointAt(0)!)).toBe(true);
    expect(missing(cps, 'ѣ')).toEqual(['ѣ']);
  });
});
