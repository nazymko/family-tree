<script lang="ts">
  // "Use the last citation": one step, plus the Mod+Shift+C shortcut while the host dialog is open.
  import Button from '../ui/Button.svelte';
  import { getApi } from '../api/context';
  import { notices } from '../ui/notices.svelte';
  import { undoNotice } from '../ui/undoable';
  import type { CitationDto, EditResult, OwnerRef } from '../api/api';

  let { target, onDone, onError }: { target: OwnerRef; onDone: (r: EditResult) => void; onError?: (m: string) => void } = $props();
  const api = getApi();
  const uid = $props.id();
  let last = $state<CitationDto | null>(null);
  let busy = false;

  $effect(() => {
    void api.lastCitation().then((c) => (last = c));
  });

  async function use() {
    if (!last || busy) return;
    busy = true;
    try {
      const r = await api.apply({ type: 'addCitation', target, citation: { reuseLast: true } }, { silent: true });
      notices.show(undoNotice(api, 'Citation added', r.undo));
      onDone(r);
    } catch (e) {
      onError?.(e instanceof Error ? e.message : String(e));
    } finally {
      busy = false;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    const mac = typeof navigator !== 'undefined' && /mac/i.test(navigator.platform);
    if (e.shiftKey && (mac ? e.metaKey : e.ctrlKey) && e.key.toLowerCase() === 'c') {
      e.preventDefault();
      void use();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="last">
  <Button aria-disabled={last ? undefined : 'true'} aria-describedby="{uid}-hint" onclick={use}>Use the last citation</Button>
  <span id="{uid}-hint" class="hint">
    {#if last}{last.source?.title ?? 'Untitled source'}{last.page ? `, ${last.page}` : ''}{:else}No citation yet{/if}
  </span>
</div>

<style>
  .last {
    display: flex;
    align-items: center;
    gap: var(--s-4);
  }
  .hint {
    font-size: var(--t-sm);
    color: var(--ink-1);
  }
  .last :global(button[aria-disabled='true']) {
    opacity: 0.55;
    cursor: not-allowed;
  }
</style>
