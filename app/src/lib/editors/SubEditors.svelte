<script lang="ts">
  import { getContext, setContext, type Snippet } from 'svelte';
  import { EDITOR_HOST, type EditorHost } from '../state/agentChanges.svelte';

  let { children }: { children: Snippet } = $props();

  /**
   * Editors opened over another editor. The page cannot reopen one of these without remounting the editor underneath and
   * losing what is changed there, so their Reload closes only themselves. The editor underneath is outside this scope and
   * keeps the page's own Reload.
   */
  const host = getContext<EditorHost | undefined>(EDITOR_HOST);
  if (host) setContext<EditorHost>(EDITOR_HOST, { loadedAt: host.loadedAt, reopen: async () => false });
</script>

{@render children()}
