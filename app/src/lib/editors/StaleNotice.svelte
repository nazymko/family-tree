<script lang="ts">
  import { tick } from 'svelte';
  import Banner from '../ui/Banner.svelte';
  import Button from '../ui/Button.svelte';
  import { t } from '../i18n';

  let {
    aboutPerson = false,
    dirty = false,
    onreload,
  }: {
    /** The editor is about a person as a whole, so the banner can say so. */
    aboutPerson?: boolean;
    /** Reloading would lose what was typed, so it asks first. */
    dirty?: boolean;
    /** Answers false when the load failed and the editor stays as it is; this notice then says so. */
    onreload: () => void | boolean | Promise<void | boolean>;
  } = $props();

  let box = $state<HTMLElement>();
  let asking = $state(false);
  let failed = $state(false);
  let reloading = false;

  // Save may have held focus when it went disabled.
  $effect(() => {
    if (!box) return;
    const a = document.activeElement as (HTMLElement & { disabled?: boolean }) | null;
    if (!a || a === document.body || a.disabled) box.querySelector('button')?.focus();
  });

  /** The button just pressed has gone with the question. */
  async function focusFirst() {
    await tick();
    box?.querySelector('button')?.focus();
  }

  async function reload() {
    if (reloading) return;
    reloading = true;
    failed = false;
    try {
      failed = (await onreload()) === false;
    } finally {
      reloading = false;
    }
  }

  function ask() {
    if (dirty) {
      asking = true;
      void focusFirst();
    } else {
      void reload();
    }
  }

  function keep() {
    asking = false;
    void focusFirst();
  }

  async function discard() {
    asking = false;
    await reload();
    void focusFirst();
  }
</script>

<div class="stale" bind:this={box}>
  <Banner kind="warning">{aboutPerson ? t('changeLog.staleBanner') : t('changeLog.staleBannerAny')}</Banner>
  {#if asking}
    <div class="ask">
      <span>{t('editor.discardAsk')}</span>
      <Button onclick={keep}>{t('editor.keepEditing')}</Button>
      <Button variant="danger" onclick={discard}>{t('editor.discard')}</Button>
    </div>
  {:else}
    <div class="ask">
      <Button onclick={ask}>{t('changeLog.reload')}</Button>
      {#if failed}<span class="failed" role="alert">{t('changeLog.reloadFailed')}</span>{/if}
    </div>
  {/if}
</div>

<style>
  .stale {
    display: grid;
    gap: var(--s-2);
    justify-items: start;
  }
  .ask {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-2);
  }
  .ask span {
    font-weight: 500;
  }
  .ask .failed {
    color: var(--danger);
  }
</style>
