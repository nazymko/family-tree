import { getContext, untrack } from 'svelte';
import { agentChanges, EDITOR_HOST, type EditorHost } from './agentChanges.svelte';

type Ids = readonly (string | null | undefined)[];

/**
 * Has an agent changed what this editor shows, since the data it was built from was read? Save is blocked then, because
 * saving would write the old values back over the agent's. Call during component setup.
 *
 * The baseline is fixed here: a later load does not change what the open form holds. It is `loadedAt` when the caller read
 * the data itself, else the page that opened the editor says, else the data is taken as read just now.
 */
export function staleGuard(watch: () => Ids, loadedAt?: () => number): { readonly stale: boolean } {
  const host = getContext<EditorHost | undefined>(EDITOR_HOST);
  const at = untrack(() => (loadedAt ?? host?.loadedAt)?.() ?? agentChanges.seq);
  const stale = $derived(agentChanges.seq > at && agentChanges.touchedSince(watch(), at));
  return {
    get stale() {
      return stale;
    },
  };
}
