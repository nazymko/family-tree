<script lang="ts">
  import Banner from '../ui/Banner.svelte';
  import Button from '../ui/Button.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import TextField from '../ui/TextField.svelte';
  import { getApi } from '../api/context';
  import type { AiReadResult } from '../api/types';

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

  // Every time the dialog opens it starts again: the box is unticked and the key field is empty.
  $effect(() => {
    if (!open) return;
    understood = false;
    key = '';
    error = '';
    busy = false;
    void api.aiKeyStatus().then((v) => (hasKey = v)).catch(() => (hasKey = false));
  });

  const needsKey = $derived(!hasKey && key.trim().length === 0);
  const canSend = $derived(understood && !busy && !needsKey);

  async function send() {
    if (!canSend) return;
    busy = true;
    error = '';
    try {
      const r = await api.aiReadPicture(understood, key.trim() || null);
      key = '';
      onresult(r);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      // The picture may already have left the computer: ask again before any retry.
      understood = false;
    } finally {
      busy = false;
    }
  }

  async function removeKey() {
    try {
      await api.aiRemoveKey();
      hasKey = false;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }
</script>

<Dialog {open} title="Read names with AI" onclose={() => !busy && onclose()}>
  <div class="body">
    <p>
      The picture will be sent over the internet to Anthropic's Claude service, which reads the names on it.
      Living relatives' names may be on the picture.
    </p>
    <p>What it finds is shown as suggestions. Nothing is added to your tree until you accept it and press “Add to tree”.</p>

    {#if hasKey}
      <p class="key" role="status">An API key is stored on this computer, in the system keychain.</p>
      <div><Button onclick={() => void removeKey()} disabled={busy}>Remove key</Button></div>
    {:else}
      <TextField
        label="Anthropic API key"
        type="password"
        bind:value={key}
        autocomplete="off"
        hint="Kept in the system keychain on this computer. Never saved in your tree."
      />
    {/if}

    <Checkbox bind:checked={understood} label="I understand the picture is sent to Anthropic" />

    {#if error}<Banner kind="error">{error}</Banner>{/if}
    {#if busy}<p role="status">Reading the picture. This can take a minute.</p>{/if}
  </div>
  {#snippet footer()}
    <Button onclick={onclose} disabled={busy}>Cancel</Button>
    <Button variant="primary" disabled={!canSend} onclick={() => void send()}>Send picture</Button>
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
