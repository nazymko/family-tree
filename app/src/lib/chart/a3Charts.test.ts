import { describe, expect, it } from 'vitest';
import type { ChartKind, ChartNode } from '../api/types';
import { relationText } from '../i18n/relation';
import { NAME_PX, TEXT_ASCENT, TEXT_DESCENT, layoutA3, nameFloorPx, printStretch, stretchScene } from './a3';
import { sceneBounds } from './picture';
import { draw, printCardsNeed } from './renderer';
import { SvgContext, parseFont } from './svgContext';
import { BIG, makeApi, richestRoot } from './testing/helpers';

// Whole print charts with synthetic three-word Ukrainian-style names (given name, patronymic, surname): on every card
// the name lines, the dates, the relationship and the ancestor number keep apart, inside the card, nothing cut.

const GIVEN = ['Святослав', 'Ярослава', 'Олександра', 'Мирослав', 'Іван', 'Христина', 'Володимир', 'Анна'];
const PATRONYMIC = ['Олександрович', 'Костянтинівна', 'Володимирівна', 'Святославович', 'Петрович', 'Мирославівна', 'Ярославович', 'Іванівна'];
const SURNAME = ['Тестенко', 'Прикладенко', 'Вигаданчук', 'Зразковський', 'Довгопрізвищенко', 'Пробний'];
const nameOf = (i: number) => `${GIVEN[i % GIVEN.length]} ${PATRONYMIC[(i * 3) % PATRONYMIC.length]} ${SURNAME[(i * 5) % SURNAME.length]}`;

/** Stand-in for the fonts: a name letter (semibold) is 0.6 of the size wide, other text 0.55. */
const measure = (s: string, font: string) => s.length * parseFont(font).size * (parseFont(font).weight === '600' ? 0.6 : 0.55);
const theme = { paper: '#fff', rule: '#ccc', ink: '#000', ink2: '#666', ribbon: '#f00', wash: '#fee', font: 'sans-serif', serif: 'serif' };

interface TextBox {
  text: string;
  x0: number;
  x1: number;
  y0: number;
  y1: number;
  bx: number;
  by: number;
}

function textBoxes(svg: string): TextBox[] {
  const out: TextBox[] = [];
  for (const m of svg.matchAll(/<text ([^>]*)>([^<]*)<\/text>/g)) {
    const a = (k: string) => new RegExp(`\\b${k}="([^"]*)"`).exec(m[1])?.[1];
    const px = Number(a('font-size'));
    const font = `${a('font-style') ?? ''} ${a('font-weight') ?? ''} ${px}px serif`.trim().replace(/\s+/g, ' ');
    const text = m[2].replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>');
    const w = measure(text, font);
    const x = Number(a('x'));
    const y = Number(a('y'));
    const end = a('text-anchor') === 'end';
    out.push({ text, x0: end ? x - w : x, x1: end ? x : x + w, y0: y - TEXT_ASCENT * px, y1: y + TEXT_DESCENT * px, bx: end ? x - 0.01 : x + 0.01, by: y });
  }
  return out;
}

const inside = (n: ChartNode, x: number, y: number) => x >= n.x && x <= n.x + n.w && y >= n.y && y <= n.y + n.h;
// The SVG keeps two decimals: boxes that only touch may seem to overlap by a rounding step.
const EPS = 0.02;
const overlap = (a: TextBox, b: TextBox) => a.x0 < b.x1 - EPS && b.x0 < a.x1 - EPS && a.y0 < b.y1 - EPS && b.y0 < a.y1 - EPS;

async function printChart(kind: ChartKind, small: boolean) {
  const { api, fx } = makeApi(300);
  const root = await richestRoot(api, fx, kind);
  const meta = await api.chartOpen({ kind, root, generations: 5, print: true });
  const laid = await api.chartWindow(meta.chartId, BIG, 1);
  await api.chartClose(meta.chartId);
  laid.nodes.forEach((n, i) => {
    if (n.person && !n.person.siblingMarker) n.person = { ...n.person, name: nameOf(i) };
  });
  const meter = new SvgContext(1, 1, measure) as unknown as CanvasRenderingContext2D;
  const scale0 = layoutA3(sceneBounds(laid), 'auto').scale;
  // `small`: as if the chart were printed so small its names are under the floor already (the owner's print), so long
  // names wrap onto three lines instead of shrinking.
  const minPx = small ? NAME_PX : nameFloorPx(scale0);
  const opts = { theme, kind, focus: meta.rootNode, focusMore: new Set(meta.chosenNodes ?? []), nameMinPx: minPx };
  const need = printCardsNeed(meter, laid.nodes, opts);
  const k = printStretch(sceneBounds(laid), need, 'auto');
  const scene = stretchScene(laid, k);
  const b = sceneBounds(scene);
  const ctx = new SvgContext(b.w, b.h, measure);
  draw(ctx as unknown as CanvasRenderingContext2D, scene, { x: 0, y: 0, zoom: 1 }, { w: b.w, h: b.h, dpr: 1 }, { ...opts, selected: null, handles: false, lod: 'full', compact: true });
  return { scene, need, k, focus: meta.rootNode, svg: ctx.toSvg('#fff', b) };
}

describe('A3 print charts with three-word names', () => {
  for (const kind of ['around', 'pedigree', 'descendants'] as ChartKind[])
    for (const small of [true, false])
      it(`${kind}${small ? '_printed_small' : ''}_cards_keep_names_dates_and_relationships_apart_and_whole`, async () => {
        const { scene, need, k, focus, svg } = await printChart(kind, small);
        expect(svg).not.toContain('…');
        const people = scene.nodes.filter((n) => n.person && !n.person.siblingMarker);
        expect(people.length).toBeGreaterThan(3);
        // The cards themselves never overlap (so each text belongs to the one card it starts in).
        for (const a of people) for (const b of people) if (a !== b) expect(a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h).toBe(false);
        if (small) expect(need).toBeGreaterThan(1);
        expect(k).toBeGreaterThanOrEqual(1);
        const boxes = textBoxes(svg);
        const problems: string[] = [];
        let threeLines = 0;
        for (const n of people) {
          const mine = boxes.filter((t) => inside(n, t.bx, t.by));
          for (const t of mine) if (t.x0 < n.x - 1e-6 || t.x1 > n.x + n.w + 1e-6 || t.y0 < n.y - 1e-6 || t.y1 > n.y + n.h + 1e-6) problems.push(`${n.id}: ${t.text} leaves its card`);
          for (let i = 0; i < mine.length; i++) for (let j = i + 1; j < mine.length; j++) if (overlap(mine[i], mine[j])) problems.push(`${n.id}: ${mine[i].text} / ${mine[j].text}`);
          // The whole name is on the card, in order.
          const words = n.person!.name.split(' ');
          if (words.every((w) => mine.some((t) => t.text === w))) threeLines++;
          if (!mine.map((t) => t.text).join('').replace(/\s/g, '').includes(n.person!.name.replace(/\s/g, ''))) problems.push(`name not whole on ${n.id}`);
          // The relationship is printed whole (never cut) on every card that has one.
          if (kind === 'around' && n.dupOf === null && n.id !== focus) {
            const rel = relationText(n);
            if (rel && !mine.some((t) => t.text === rel)) problems.push(`relationship missing on ${n.id}`);
          }
        }
        expect(problems).toEqual([]);
        if (small) expect(threeLines).toBeGreaterThan(0);
      });
});
