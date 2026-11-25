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

/**
 * jsdom matches every stylesheet rule through nwsapi, whose compiled-selector caches are LRUs of 1000 entries. One
 * jsdom serves a whole worker (vmThreads), so the styles of every component rendered so far pile up in it; once they
 * pass 1000 selectors, each getComputedStyle walks the rules in order and evicts every entry before it is used again,
 * so a single `findByRole` (it asks for the style of each element) took 4 s instead of 0.2 s. Unbounded caches keep the
 * cost flat: the selectors are a fixed set, the app's own CSS.
 */
function unboundSelectorCaches() {
  type Cache = { get(k: string): unknown; set(k: string, v: unknown): unknown; has(k: string): boolean; clear(): void; size(): number };
  type Nwsapi = Record<'matchLambdas' | 'selectLambdas' | 'matchResolvers' | 'selectResolvers', Cache | undefined>;
  const implKey = Object.getOwnPropertySymbols(document).find((s) => s.description === 'impl');
  if (!implKey) return;
  const impl = (document as unknown as Record<symbol, Record<string, unknown>>)[implKey];
  if (impl.__gtUnbound) return;
  // Both nwsapi instances are made on first use: one by Element.matches, one by getComputedStyle with a rule to test.
  const style = document.createElement('style');
  style.textContent = '.gt-warm-selectors { color: red; }';
  document.head.append(style);
  document.body.matches('body');
  void getComputedStyle(document.body).color;
  style.remove();
  for (const api of [impl._nwsapi, impl._nwsapiDontThrow] as (Nwsapi | undefined)[]) {
    for (const name of ['matchLambdas', 'selectLambdas', 'matchResolvers', 'selectResolvers'] as const) {
      const cache = api?.[name];
      if (!cache) continue;
      const map = new Map<string, unknown>();
      cache.get = (k) => map.get(k);
      cache.set = (k, v) => (map.set(k, v), v);
      cache.has = (k) => map.has(k);
      cache.clear = () => map.clear();
      cache.size = () => map.size;
    }
  }
  impl.__gtUnbound = true;
}
unboundSelectorCaches();

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
