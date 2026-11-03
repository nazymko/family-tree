import { getContext, setContext } from 'svelte';
import type { Api, Platform } from './api';
import { MockApi } from './mock/MockApi';
import { MockPlatform } from './mock/MockPlatform';
import { fixtureTree } from './mock/fixtures';
import { TauriApi } from './tauri/TauriApi';
import { TauriPlatform } from './tauri/TauriPlatform';

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

/** TauriApi inside the desktop shell (`'__TAURI_INTERNALS__' in window`), MockApi in a plain browser. */
export function defaultApi(): ApiBundle {
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
    return { api: new TauriApi(), platform: new TauriPlatform() };
  }
  return { api: new MockApi(fixtureTree(500)), platform: new MockPlatform() };
}
