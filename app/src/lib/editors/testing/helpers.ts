import { MockApi } from '../../api/mock/MockApi';
import { fixtureTree } from '../../api/mock/fixtures';
import type { Api, PersonDetail, PersonQuery } from '../../api/api';
import WithApi from './WithApi.svelte';
import { render } from '@testing-library/svelte';
import type { Component } from 'svelte';

export const ALL: PersonQuery = {
  sort: 'surname',
  desc: false,
  text: '',
  living: 'any',
  noSource: false,
  noBirthDate: false,
  privateOnly: false,
};

export function newApi(n = 120): MockApi {
  return new MockApi(fixtureTree(n));
}

/** First person (in list order) whose detail satisfies `pred`. */
export async function findPerson(api: Api, pred: (d: PersonDetail) => boolean): Promise<PersonDetail> {
  for (let off = 0; off < 400; off += 100) {
    const page = await api.listPersons(ALL, off, 100);
    for (const row of page.items) {
      const d = await api.getPerson(row.id);
      if (pred(d)) return d;
    }
    if (off + 100 >= page.total) break;
  }
  throw new Error('no fixture person matches');
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function renderWithApi(api: Api, comp: Component<any>, props: Record<string, unknown> = {}) {
  return render(WithApi, { props: { api, comp, props } });
}
