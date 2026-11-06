<script lang="ts">
  import { errMessage } from '../editors/errors';
  import Banner from '../ui/Banner.svelte';
  import Button from '../ui/Button.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import TextField from '../ui/TextField.svelte';
  import { getApi } from '../api/context';
  import type { AiReadResult } from '../api/types';
  import { t } from '../i18n';

  interface Props {
    open: boolean;
    onclose: () => void;
    onresult: (r: AiReadResult) => void;
  }
  let { open, onclose, onresult }: Props = $props();

  const api = getApi();
  let understood = $state(false);
  let key = $state('');
  let hasKey = $state(false);
  let busy = $state(false);
  let error = $state('');
  // Bumped on open and on cancel, so a late answer from an abandoned read is ignored.
  let round = 0;

  // Every time the dialog opens it starts again: the box is unticked and the key field is empty.
  $effect(() => {
    if (!open) return;
    understood = false;
    key = '';
    error = '';
    busy = false;
    round += 1;
    void api.aiKeyStatus().then((v) => (hasKey = v)).catch(() => (hasKey = false));
  });

  const needsKey = $derived(!hasKey && key.trim().length === 0);
  const canSend = $derived(understood && !busy && !needsKey);

  async function send() {
    if (!canSend) return;
    busy = true;
    error = '';
    const mine = ++round;
    try {
      const r = await api.aiReadPicture(understood, key.trim() || null);
      if (mine !== round) return;
      key = '';
      onresult(r);
    } catch (e) {
      if (mine !== round) return;
      error = errMessage(e, t('edit.wentWrong'));
      // The picture may already have left the computer: ask again before any retry.
      understood = false;
    } finally {
      if (mine === round) busy = false;
    }
  }

  async function cancel() {
    if (busy) {
      round += 1;
      busy = false;
      try {
        await api.aiCancelRead();
      } catch {
        // The dialog closes either way.
      }
    }
    onclose();
  }

  async function removeKey() {
    try {
      await api.aiRemoveKey();
      hasKey = false;
    } catch (e) {
      error = errMessage(e, t('edit.wentWrong'));
    }
  }
</script>

<Dialog {open} title={t('trace.aiTitle')} onclose={() => void cancel()}>
  <div class="body">
    <p>{t('trace.aiWarn')}</p>
    <p>{t('trace.aiSuggestionsNote')}</p>

    {#if hasKey}
      <p class="key" role="status">{t('trace.aiKeyStored')}</p>
      <div><Button onclick={() => void removeKey()} disabled={busy}>{t('trace.aiRemoveKey')}</Button></div>
    {:else}
      <TextField
        label={t('trace.aiKeyLabel')}
        type="password"
        bind:value={key}
        autocomplete="off"
        hint={t('trace.aiKeyHint')}
      />
    {/if}

    <Checkbox bind:checked={understood} label={t('trace.aiUnderstood')} />

    {#if error}<Banner kind="error">{error}</Banner>{/if}
    {#if busy}<p role="status">{t('trace.aiBusy')}</p>{/if}
  </div>
  {#snippet footer()}
    <Button onclick={() => void cancel()}>{t('common.cancel')}</Button>
    <Button variant="primary" disabled={!canSend} onclick={() => void send()}>{t('trace.aiSend')}</Button>
  {/snippet}
</Dialog>

<style>
  .body {
    display: grid;
    gap: var(--s-4);
    max-width: 34rem;
  }
  .key {
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
</style>
