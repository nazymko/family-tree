import type { Api } from './api';
import { t, localeStore } from '../i18n';
import { dateText, eventSummaryText, lifespanText } from '../i18n/backend';

/**
 * The one place that words the backend's fixed English in the app's language, wherever a person is shown.
 *
 * The backend names a nameless person "Unknown" and writes lifespans and summaries with short English words
 * ("c. 1968 – 2021", "m. 1 Jun 1878, Hamburg", "about 3 Apr 1901"). MCP agents, GEDCOM and the stored data keep those
 * words. Every `Api` the UI gets passes through `localizeApi` (see `setApi` and `apiContext` in `context.ts`), which
 * turns those fields into properties that read the active language each time they are read. So a language switch
 * changes cards, charts and lists that are already on screen, and no component has to know. A person whose stored
 * name really is "Unknown" is shown the same way.
 *
 * Only fields that are *shown* are touched: a person's name, lifespan and dates in a `PersonRef`, `PersonRow`, change
 * log, agent connection, privacy warning and person search hit, a `DateDto.display`, an event summary. Name parts
 * (`given`, `surname`, a `NameDto`) and the `raw` date are data the owner edits and stay as stored; places and sources
 * named "Unknown" are never touched.
 *
 * The answer is changed in place, and the caller still gets the very promise the Api returned: wrapping it in a new
 * one would add a microtask, and the views' tests (and some races between a blur and a click) depend on the count.
 */
export const UNKNOWN_NAME = 'Unknown';

/** The word for a nameless person in the active language. */
export function unknownName(): string {
  return t('changeLog.unknownPerson');
}

type Obj = Record<string, unknown>;
type Reword = [key: string, show: (stored: string, o: Obj) => string];

/** Data objects only: not a Date, Map, typed array or Blob. */
const isData = (v: unknown): v is Obj => v !== null && typeof v === 'object' && Object.prototype.toString.call(v) === '[object Object]';

const CACHE_MAX = 20000;
const cache = new Map<string, string>();
/** `fn(s)` remembered per language: a chart reads thousands of lifespans on every frame. */
function memo(tag: string, fn: (s: string) => string): (s: string) => string {
  return (s) => {
    const key = `${localeStore.locale}|${tag}|${s}`;
    let out = cache.get(key);
    if (out === undefined) {
      if (cache.size >= CACHE_MAX) cache.clear();
      cache.set(key, (out = fn(s)));
    }
    return out;
  };
}
const lifespan = memo('l', lifespanText);
const date = memo('d', dateText);
const summary = memo('s', eventSummaryText);

const name: Reword['1'] = () => unknownName();

/** The fields of `o` that are shown in the app's language, and how. */
function rewords(o: Obj, method: string): Reword[] {
  const out: Reword[] = [];
  const text = (k: string) => typeof o[k] === 'string' && o[k] !== '';
  const stored = (k: string) => o[k] === UNKNOWN_NAME;
  const isRef = 'siblingMarker' in o && 'lifespan' in o;
  const isRow = isRef && 'surname' in o && 'given' in o && 'birth' in o;
  // PersonRef, the person of a change-log row (`siblingMarker` only), and PersonRow of the People list.
  if ('siblingMarker' in o && stored('name')) out.push(['name', name]);
  if (isRef && text('lifespan')) out.push(['lifespan', lifespan]);
  if (isRow) for (const k of ['birth', 'death']) if (text(k)) out.push([k, date]);
  // The agent behind a change.
  if (o.kind === 'agent' && stored('name')) out.push(['name', name]);
  // PrivacyPreview mention warning.
  if ('noteExcerpt' in o && stored('hiddenPerson')) out.push(['hiddenPerson', name]);
  // An agent connection, and the agents of the change-log filter.
  const keys = Object.keys(o);
  if (stored('name') && ((keys.length === 3 && 'id' in o && 'created' in o) || (method === 'changeLogAgents' && keys.length === 2))) {
    out.push(['name', name]);
  }
  // A person's search hit: the index label is empty for a nameless person, so it takes the person's name.
  if (o.kind === 'person' && 'snippet' in o && 'person' in o && (o.label === '' || stored('label'))) {
    out.push(['label', (s, self) => (s === '' ? String((self.person as { name?: unknown } | null)?.name ?? '') : unknownName())]);
  }
  // A date as the backend displays it, and the summaries of a marriage and of a person's birth and death.
  if ('raw' in o && 'status' in o && text('display')) out.push(['display', date]);
  if ('focusSummary' in o && text('focusSummary')) out.push(['focusSummary', summary]);
  if (('events' in o || 'parents' in o) && text('summary')) out.push(['summary', summary]);
  return out;
}

function apply(o: Obj, list: Reword[]): void {
  for (const [key, show] of list) {
    const stored = String(o[key]);
    try {
      Object.defineProperty(o, key, { enumerable: true, configurable: true, get: () => show(stored, o) });
    } catch {
      // A frozen object keeps the stored text.
    }
  }
}

function walk(v: unknown, method: string): void {
  if (Array.isArray(v)) {
    for (const item of v) walk(item, method);
    return;
  }
  if (!isData(v)) return;
  // Children first: a search hit's empty label reads its person, which must already be worded.
  for (const key of Object.keys(v)) walk(v[key], method);
  const list = rewords(v, method);
  if (list.length) apply(v, list);
}

/** Shows `result` with the backend's fixed English in the active language (changes it in place). Anything else is left as it is. */
export function localizeNames<T>(result: T, method = ''): T {
  walk(result, method);
  return result;
}

const wrapped = new WeakSet<object>();

/** An `Api` whose answers are worded in the active language. Wrapping twice does nothing. */
export function localizeApi(inner: Api): Api {
  if (wrapped.has(inner)) return inner;
  const proxy = new Proxy(inner, {
    get(target, prop, receiver) {
      const value = Reflect.get(target, prop, receiver);
      if (typeof value !== 'function' || typeof prop !== 'string') return value;
      return (...args: unknown[]) => {
        const r = (value as (...a: unknown[]) => unknown).apply(target, args);
        if (r instanceof Promise) {
          // A side branch: the caller gets `r` itself. Its own failure is handled by the caller.
          r.then((x) => walk(x, prop), () => {});
        } else {
          walk(r, prop);
        }
        return r;
      };
    },
  });
  wrapped.add(proxy);
  return proxy;
}
