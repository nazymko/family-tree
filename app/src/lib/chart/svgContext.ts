/**
 * A tiny stand-in for a canvas 2D context that writes SVG instead of pixels. It supports only what the chart draw
 * functions use (paths made of rect / moveTo / lineTo / arc, fill, stroke, text, a transform, dashes, alpha), so the
 * very same `draw` and `drawWhole` produce a vector picture that stays sharp at any size.
 */

const num = (v: number): string => String(Math.round(v * 100) / 100);

/** XML-safe text. Control characters other than tab and newline are not allowed in XML at all. */
export function escapeXml(s: string): string {
  return (
    s
      // eslint-disable-next-line no-control-regex
      .replace(/[\u0000-\u0008\u000B\u000C\u000E-\u001F]/g, '')
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
  );
}

interface FontParts {
  style: string;
  weight: string;
  size: number;
  family: string;
}

/** Splits a canvas font string such as `italic 600 13px "Segoe UI", sans-serif`. */
export function parseFont(font: string): FontParts {
  const m = /^\s*(?:(italic|oblique)\s+)?(?:(bold|bolder|lighter|normal|\d{3})\s+)?([\d.]+)px\s+(.+?)\s*$/i.exec(font);
  if (!m) return { style: 'normal', weight: 'normal', size: 12, family: 'sans-serif' };
  return { style: m[1] ?? 'normal', weight: m[2] ?? 'normal', size: Number(m[3]), family: m[4] };
}

type Measure = (text: string, font: string) => number;

/** Measures text with a real canvas when there is one, and with a rough width otherwise. */
function defaultMeasure(): Measure {
  let ctx: CanvasRenderingContext2D | null = null;
  try {
    ctx = typeof document !== 'undefined' ? document.createElement('canvas').getContext('2d') : null;
  } catch {
    ctx = null;
  }
  return (text, font) => {
    if (ctx) {
      ctx.font = font;
      const w = ctx.measureText(text).width;
      if (Number.isFinite(w) && w > 0) return w;
    }
    return text.length * parseFont(font).size * 0.55;
  };
}

export class SvgContext {
  fillStyle: string = '#000';
  strokeStyle: string = '#000';
  lineWidth = 1;
  globalAlpha = 1;
  font = '10px sans-serif';
  textAlign: CanvasTextAlign = 'left';
  textBaseline: CanvasTextBaseline = 'alphabetic';

  private body: string[] = [];
  private d: string[] = [];
  private dash: number[] = [];
  private m: [number, number, number, number, number, number] = [1, 0, 0, 1, 0, 0];
  private measure: Measure;

  constructor(
    readonly width: number,
    readonly height: number,
    measure?: Measure,
  ) {
    this.measure = measure ?? defaultMeasure();
  }

  // ---- state
  setTransform(a: number, b: number, c: number, d: number, e: number, f: number): void {
    this.m = [a, b, c, d, e, f];
  }
  setLineDash(segments: number[]): void {
    this.dash = [...segments];
  }
  clearRect(): void {
    // Nothing to clear: the picture starts empty.
  }

  // ---- paths
  beginPath(): void {
    this.d = [];
  }
  closePath(): void {
    this.d.push('Z');
  }
  moveTo(x: number, y: number): void {
    this.d.push(`M${num(x)} ${num(y)}`);
  }
  lineTo(x: number, y: number): void {
    this.d.push(`L${num(x)} ${num(y)}`);
  }
  rect(x: number, y: number, w: number, h: number): void {
    this.d.push(`M${num(x)} ${num(y)}h${num(w)}v${num(h)}h${num(-w)}Z`);
  }
  arc(x: number, y: number, r: number, start: number, end: number): void {
    const full = Math.abs(end - start) >= Math.PI * 2 - 1e-9;
    if (full) {
      // Two half circles make a full one.
      this.d.push(`M${num(x - r)} ${num(y)}a${num(r)} ${num(r)} 0 1 0 ${num(2 * r)} 0a${num(r)} ${num(r)} 0 1 0 ${num(-2 * r)} 0Z`);
      return;
    }
    const sx = x + r * Math.cos(start);
    const sy = y + r * Math.sin(start);
    const ex = x + r * Math.cos(end);
    const ey = y + r * Math.sin(end);
    const large = Math.abs(end - start) > Math.PI ? 1 : 0;
    const sweep = end >= start ? 1 : 0;
    this.d.push(`${this.d.length ? 'L' : 'M'}${num(sx)} ${num(sy)}A${num(r)} ${num(r)} 0 ${large} ${sweep} ${num(ex)} ${num(ey)}`);
  }

  // ---- painting
  private transformAttr(): string {
    const [a, b, c, d, e, f] = this.m;
    return a === 1 && b === 0 && c === 0 && d === 1 && e === 0 && f === 0 ? '' : ` transform="matrix(${[a, b, c, d, e, f].map((v) => Math.round(v * 1e4) / 1e4).join(' ')})"`;
  }
  private opacityAttr(): string {
    return this.globalAlpha < 1 ? ` opacity="${num(this.globalAlpha)}"` : '';
  }
  private paint(path: string, fill: boolean): void {
    if (!path) return;
    const tr = this.transformAttr() + this.opacityAttr();
    if (fill) {
      this.body.push(`<path d="${path}" fill="${escapeXml(String(this.fillStyle))}"${tr}/>`);
    } else {
      const dash = this.dash.length ? ` stroke-dasharray="${this.dash.map(num).join(' ')}"` : '';
      this.body.push(`<path d="${path}" fill="none" stroke="${escapeXml(String(this.strokeStyle))}" stroke-width="${num(this.lineWidth)}"${dash}${tr}/>`);
    }
  }
  fill(): void {
    this.paint(this.d.join(''), true);
  }
  stroke(): void {
    this.paint(this.d.join(''), false);
  }
  fillRect(x: number, y: number, w: number, h: number): void {
    this.paint(`M${num(x)} ${num(y)}h${num(w)}v${num(h)}h${num(-w)}Z`, true);
  }
  strokeRect(x: number, y: number, w: number, h: number): void {
    this.paint(`M${num(x)} ${num(y)}h${num(w)}v${num(h)}h${num(-w)}Z`, false);
  }

  // ---- text
  measureText(text: string): { width: number } {
    return { width: this.measure(text, this.font) };
  }
  fillText(text: string, x: number, y: number): void {
    if (!text) return;
    const f = parseFont(this.font);
    const anchor = this.textAlign === 'right' || this.textAlign === 'end' ? 'end' : this.textAlign === 'center' ? 'middle' : 'start';
    const baseline =
      this.textBaseline === 'middle' ? 'central' : this.textBaseline === 'top' || this.textBaseline === 'hanging' ? 'hanging' : this.textBaseline === 'bottom' ? 'text-after-edge' : 'alphabetic';
    const style = f.style !== 'normal' ? ` font-style="${f.style}"` : '';
    const weight = f.weight !== 'normal' ? ` font-weight="${f.weight}"` : '';
    this.body.push(
      `<text x="${num(x)}" y="${num(y)}" font-family="${escapeXml(f.family)}" font-size="${num(f.size)}"${style}${weight}` +
        ` text-anchor="${anchor}" dominant-baseline="${baseline}" fill="${escapeXml(String(this.fillStyle))}"${this.transformAttr()}${this.opacityAttr()}>${escapeXml(text)}</text>`,
    );
  }

  /** The finished document: a paper-coloured page, the chart clipped to its own area, then `after` (the caption). */
  toSvg(paper: string, chart: { w: number; h: number }, after = ''): string {
    const w = num(this.width);
    const h = num(this.height);
    return [
      '<?xml version="1.0" encoding="UTF-8"?>',
      `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">`,
      `<rect width="${w}" height="${h}" fill="${escapeXml(paper)}"/>`,
      `<svg x="0" y="0" width="${num(chart.w)}" height="${num(chart.h)}" viewBox="0 0 ${num(chart.w)} ${num(chart.h)}" overflow="hidden">`,
      ...this.body,
      '</svg>',
      after + '</svg>',
      '',
    ].join('\n');
  }
}
