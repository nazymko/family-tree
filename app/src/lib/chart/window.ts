import type { ChartWindow, Rect } from '../api/types';
import { lodFor, type Lod } from './renderer';

type Fetch = (r: Rect, zoom: number) => Promise<ChartWindow>;

const contains = (outer: Rect, inner: Rect): boolean =>
  inner.x >= outer.x && inner.y >= outer.y && inner.x + inner.w <= outer.x + outer.w && inner.y + inner.h <= outer.y + outer.h;

const inflate = (r: Rect): Rect => ({ x: r.x - r.w, y: r.y - r.h, w: r.w * 3, h: r.h * 3 });

interface Req {
  seq: number;
  rect: Rect;
  lod: Lod;
}

/** Client-side window cache: panning inside the cached rect never reaches IPC. */
export class WindowCache {
  #fetch: Fetch;
  #cur: { win: ChartWindow; rect: Rect; lod: Lod; seq: number } | null = null;
  #pending: Req[] = [];
  #queued: { view: Rect; zoom: number } | null = null;
  #seq = 0;
  #floor = 0;
  #cbs: (() => void)[] = [];
  #disposed = false;

  constructor(fetch: Fetch) {
    this.#fetch = fetch;
  }

  need(view: Rect, zoom: number): void {
    if (this.#disposed) return;
    const lod = lodFor(zoom);
    if (this.#cur && this.#cur.lod === lod && contains(this.#cur.rect, view)) return;
    if (this.#pending.some((p) => p.lod === lod && contains(p.rect, view))) return;
    if (this.#pending.length >= 2) {
      this.#queued = { view, zoom };
      return;
    }
    this.#start(view, zoom, lod);
  }

  #start(view: Rect, zoom: number, lod: Lod): void {
    const req: Req = { seq: ++this.#seq, rect: inflate(view), lod };
    this.#pending.push(req);
    const settle = () => {
      this.#pending = this.#pending.filter((p) => p !== req);
      const q = this.#queued;
      if (q && !this.#disposed) {
        this.#queued = null;
        this.need(q.view, q.zoom);
      }
    };
    this.#fetch(req.rect, zoom).then(
      (win) => {
        settle();
        if (this.#disposed || req.seq <= this.#floor || (this.#cur && this.#cur.seq > req.seq)) return;
        this.#cur = { win, rect: req.rect, lod, seq: req.seq };
        for (const cb of this.#cbs) cb();
      },
      () => settle(),
    );
  }

  /** Fetches `view` again without dropping what is drawn; the new window replaces it on arrival (the data changed, not the view). */
  refetch(view: Rect, zoom: number): void {
    if (this.#disposed) return;
    this.#start(view, zoom, lodFor(zoom));
  }

  current(): { win: ChartWindow; rect: Rect } | null {
    return this.#cur ? { win: this.#cur.win, rect: this.#cur.rect } : null;
  }

  /** Drop all cached and in-flight data (the chart was reopened); the next need() fetches again. */
  invalidate(): void {
    this.#cur = null;
    this.#pending = [];
    this.#queued = null;
    this.#floor = this.#seq;
  }

  onChange(cb: () => void): void {
    this.#cbs.push(cb);
  }

  dispose(): void {
    this.#disposed = true;
    this.#cbs = [];
  }
}
