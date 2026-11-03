import type { Api } from '../api/api';
import type { ProjectInfo } from '../api/types';
import { nav } from './nav.svelte';
import { project } from './project.svelte';
import { undoState } from './undo.svelte';
import { settings } from './settings.svelte';

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
  // Settings belong to the tree (living threshold, home person): reload them for the tree that is now open.
  try {
    info = { ...info, homePerson: (await settings.load(api)).homePerson };
  } catch {
    /* keep the values already shown; Settings reloads on demand */
  }
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

/** Leaves the open tree: releases it in the backend and returns to Welcome. */
export async function closeTree(api: Api): Promise<void> {
  await api.closeProject();
  project.close();
  nav.reset();
  undoState.reset();
  try {
    await settings.load(api);
  } catch {
    /* app-level settings reload on the next open */
  }
}

/** Back must not return to the welcome screen of a project that is already open. */
function startHistoryHere(): void {
  nav.history = [nav.route];
  nav.foci = [nav.focusPerson];
  nav.index = 0;
}
