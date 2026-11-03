import { getContext, setContext } from 'svelte';
import type { Api, Platform } from './api';
import { MockApi } from './mock/MockApi';
import { MockPlatform } from './mock/MockPlatform';
import { fixtureTree } from './mock/fixtures';

const KEY = Symbol('gene-tree.api');

export interface ApiBundle {
  api: Api;
  platform: Platform;
}

/** Call once from the root component; descendants read with getApi() / getPlatform(). */
export function setApi(api: Api, platform: Platform): void {
  setContext<ApiBundle>(KEY, { api, platform });
}

/** For tests: the `context` option of testing-library's render(). */
export function apiContext(api: Api, platform: Platform): Map<symbol, ApiBundle> {
  return new Map([[KEY, { api, platform }]]);
}

function bundle(): ApiBundle {
  const b = getContext<ApiBundle | undefined>(KEY);
  if (!b) throw new Error('No Api in context. Call setApi(api, platform) in the root component.');
  return b;
}

export function getApi(): Api {
  return bundle().api;
}

export function getPlatform(): Platform {
  return bundle().platform;
}

let tauriFactory: (() => ApiBundle) | null = null;

/** Task 27 registers the Tauri adapter here so this module never imports `@tauri-apps/api`. */
export function registerTauri(factory: () => ApiBundle): void {
  tauriFactory = factory;
}

/** TauriApi inside the desktop shell (`'__TAURI_INTERNALS__' in window`), MockApi in a plain browser. */
export function defaultApi(): ApiBundle {
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
    if (!tauriFactory) throw new Error('Running inside Tauri but no TauriApi is registered.');
    return tauriFactory();
  }
  return { api: new MockApi(fixtureTree(500)), platform: new MockPlatform() };
}
