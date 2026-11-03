import '@testing-library/jest-dom/vitest';
import { afterEach, vi } from 'vitest';
import { cleanup } from '@testing-library/svelte';
import { setLanguage } from './lib/i18n';

// Tests query by English text, so they run in English unless one switches the language itself.
setLanguage('en');
afterEach(() => {
  cleanup();
  setLanguage('en');
});

/** Reports an 800x600 box shortly after observe(), like a browser's first layout. */
class ResizeObserverStub {
  constructor(private cb: ResizeObserverCallback) {}
  observe() {
    queueMicrotask(() =>
      this.cb([{ contentRect: { width: 800, height: 600 } } as ResizeObserverEntry], this as unknown as ResizeObserver),
    );
  }
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver = ResizeObserverStub as unknown as typeof ResizeObserver;

if (!window.matchMedia) {
  window.matchMedia = (query: string) =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    }) as MediaQueryList;
}

/** Recording fake of CanvasRenderingContext2D: every call is appended to `calls`. */
export interface CanvasCall {
  op: string;
  args: unknown[];
}
export function makeFakeContext() {
  const calls: CanvasCall[] = [];
  const state: Record<string, unknown> = {};
  const ctx = new Proxy(state, {
    get(target, prop: string) {
      if (prop === 'calls') return calls;
      if (prop === 'canvas') return target.canvas;
      if (prop in target) return target[prop];
      if (prop === 'measureText') {
        return (text: string) => {
          calls.push({ op: 'measureText', args: [text] });
          return { width: text.length * 7 };
        };
      }
      return (...args: unknown[]) => {
        calls.push({ op: prop, args });
        return undefined;
      };
    },
    set(target, prop: string, value) {
      target[prop] = value;
      calls.push({ op: `set:${prop}`, args: [value] });
      return true;
    },
  });
  return ctx as unknown as CanvasRenderingContext2D & { calls: CanvasCall[] };
}

HTMLCanvasElement.prototype.getContext = vi.fn(function (this: HTMLCanvasElement) {
  const ctx = makeFakeContext();
  (ctx as unknown as Record<string, unknown>).canvas = this;
  return ctx;
}) as unknown as typeof HTMLCanvasElement.prototype.getContext;
