import type { Api } from '../api/api';
import type { ProjectInfo } from '../api/types';
import { nav } from './nav.svelte';
import { project } from './project.svelte';
import { undoState } from './undo.svelte';

const BY_SURNAME = {
  sort: 'surname',
  desc: false,
  text: '',
  living: 'any',
  noSource: false,
  noBirthDate: false,
  privateOnly: false,
} as const;

/** Makes `info` the open project and routes to the family view of its home person (or People when it is empty). */
export async function enterProject(api: Api, info: ProjectInfo): Promise<void> {
  project.open(info);
  nav.reset();
  undoState.reset();
  if (info.personCount === 0) {
    nav.focus(null);
    nav.go({ name: 'people' });
    startHistoryHere();
    return;
  }
  let id = info.homePerson;
  if (!id) {
    const first = await api.listPersons({ ...BY_SURNAME }, 0, 1);
    id = first.items[0]?.id ?? null;
  }
  nav.focus(id);
  nav.go({ name: 'family' });
  startHistoryHere();
}

/** Back must not return to the welcome screen of a project that is already open. */
function startHistoryHere(): void {
  nav.history = [nav.route];
  nav.index = 0;
}
