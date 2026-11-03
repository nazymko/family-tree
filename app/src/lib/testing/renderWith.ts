import { render } from '@testing-library/svelte';
import type { Component } from 'svelte';
import Host from './SourcesHost.svelte';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import type { Api } from '../api/api';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function renderWith(comp: Component<any>, props: Record<string, unknown> = {}, opts: { api?: Api; platform?: MockPlatform; n?: number } = {}) {
  const api = opts.api ?? new MockApi(fixtureTree(opts.n ?? 80));
  const platform = opts.platform ?? new MockPlatform();
  const view = render(Host, { props: { api, platform, comp, props } });
  return { api, platform, ...view };
}

export const ALL_PERSONS = { sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false } as const;
