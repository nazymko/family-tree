<script lang="ts">
  import { getContext, untrack, type Snippet } from 'svelte';
  import Dialog from '../ui/Dialog.svelte';
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import { agentChanges, EDITOR_RELOAD } from '../state/agentChanges.svelte';
  import { t } from '../i18n';

  let {
    title,
    saveLabel,
    dirty = false,
    canSave = true,
    saving = false,
    error = '',
    watch = [],
    onsave,
    oncancel,
    children,
    extra,
  }: {
    title: string;
    saveLabel: string;
    dirty?: boolean;
    canSave?: boolean;
    saving?: boolean;
    error?: string;
    /** Records this editor shows. When an agent changes one, what is on screen is out of date and Save is blocked. */
    watch?: (string | null | undefined)[];
    onsave: () => void;
    oncancel: () => void;
    children: Snippet;
    extra?: Snippet;
  } = $props();

  let confirming = $state(false);

  const openedAt = untrack(() => agentChanges.seq);
  const stale = $derived(agentChanges.seq > openedAt && agentChanges.touchedSince(watch, openedAt));
  const reopen = getContext<(() => Promise<boolean>) | undefined>(EDITOR_RELOAD);
  /** The page that opened this dialog reloads and reopens it; if there is none, closing is the way to fresh data. */
  async function reload() {
    if (!(await reopen?.())) oncancel();
  }
  let staleBox = $state<HTMLElement>();

  // Save may have held focus when it went disabled.
  $effect(() => {
    if (!stale || !staleBox) return;
    const a = document.activeElement as (HTMLElement & { disabled?: boolean }) | null;
    if (!a || a === document.body || a.disabled) staleBox.querySelector('button')?.focus();
  });

  function requestCancel() {
    if (confirming) confirming = false;
    else if (dirty) confirming = true;
    else oncancel();
  }

  function submit() {
    if (canSave && !saving && !stale) onsave();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key !== 'Enter' || e.defaultPrevented || e.isComposing) return;
    const t = e.target as HTMLElement;
    if (t.tagName === 'INPUT' && (t as HTMLInputElement).type !== 'checkbox') {
      e.preventDefault();
      submit();
    }
  }
</script>

<Dialog open={true} {title} onclose={requestCancel}>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <form class="form" onsubmit={(e) => e.preventDefault()} {onkeydown}>
    {#if stale}
      <div class="stale" bind:this={staleBox}>
        <Banner kind="warning">{t('changeLog.staleBanner')}</Banner>
        <Button onclick={reload}>{t('changeLog.reload')}</Button>
      </div>
    {/if}
    {#if error}<Banner kind="error">{error}</Banner>{/if}
    {@render children()}
  </form>
  {#snippet footer()}
    {#if extra && !confirming}<div class="extra">{@render extra()}</div>{/if}
    {#if confirming}
      <span class="ask">{t('editor.discardAsk')}</span>
      <Button onclick={() => (confirming = false)}>{t('editor.keepEditing')}</Button>
      <Button variant="danger" onclick={oncancel}>{t('editor.discard')}</Button>
    {:else}
      <Button onclick={requestCancel}>{t('common.cancel')}</Button>
      <Button variant="primary" disabled={!canSave || saving || stale} onclick={submit}>{saveLabel}</Button>
    {/if}
  {/snippet}
</Dialog>

<style>
  .form {
    display: grid;
    gap: var(--s-5);
  }
  .stale {
    display: grid;
    gap: var(--s-2);
    justify-items: start;
  }
  .extra {
    margin-inline-end: auto;
  }
  .ask {
    align-self: center;
    font-weight: 500;
  }
</style>
