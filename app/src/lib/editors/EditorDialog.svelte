<script lang="ts">
  import type { Snippet } from 'svelte';
  import Dialog from '../ui/Dialog.svelte';
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';

  let {
    title,
    saveLabel,
    dirty = false,
    canSave = true,
    saving = false,
    error = '',
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
    onsave: () => void;
    oncancel: () => void;
    children: Snippet;
    extra?: Snippet;
  } = $props();

  let confirming = $state(false);

  function requestCancel() {
    if (confirming) confirming = false;
    else if (dirty) confirming = true;
    else oncancel();
  }

  function submit() {
    if (canSave && !saving) onsave();
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
    {#if error}<Banner kind="error">{error}</Banner>{/if}
    {@render children()}
  </form>
  {#snippet footer()}
    {#if extra && !confirming}<div class="extra">{@render extra()}</div>{/if}
    {#if confirming}
      <span class="ask">Discard changes?</span>
      <Button onclick={() => (confirming = false)}>Keep editing</Button>
      <Button variant="danger" onclick={oncancel}>Discard changes</Button>
    {:else}
      <Button onclick={requestCancel}>Cancel</Button>
      <Button variant="primary" disabled={!canSave || saving} onclick={submit}>{saveLabel}</Button>
    {/if}
  {/snippet}
</Dialog>

<style>
  .form {
    display: grid;
    gap: var(--s-5);
  }
  .extra {
    margin-inline-end: auto;
  }
  .ask {
    align-self: center;
    font-weight: 500;
  }
</style>
