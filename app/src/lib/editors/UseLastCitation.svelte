<script lang="ts">
  import { errMessage } from './errors';
  // "Use the last citation": one step, plus the Mod+Shift+C shortcut while the host dialog is open.
  import Button from '../ui/Button.svelte';
  import { getApi } from '../api/context';
  import { notices } from '../ui/notices.svelte';
  import { undoNotice } from '../ui/undoable';
  import type { CitationDto, EditResult, OwnerRef } from '../api/api';
  import { t } from '../i18n';

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
      notices.show(undoNotice(api, t('edit.doneCitationAdded'), r.undo));
      onDone(r);
    } catch (e) {
      onError?.(errMessage(e, t('edit.wentWrong')));
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
  <Button aria-disabled={last ? undefined : 'true'} aria-describedby="{uid}-hint" onclick={use}>{t('citation.useLast')}</Button>
  <span id="{uid}-hint" class="hint">
    {#if last}{last.source?.title ?? t('citation.untitled')}{last.page ? `, ${last.page}` : ''}{:else}{t('citation.noneYet')}{/if}
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
