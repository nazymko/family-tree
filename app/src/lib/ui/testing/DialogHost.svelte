<script lang="ts">
  import Dialog from '../Dialog.svelte';
  let { onclose }: { onclose?: () => void } = $props();
  let open = $state(false);
</script>

<button onclick={() => (open = true)}>Open dialog</button>
<Dialog
  {open}
  title="Edit event"
  onclose={() => {
    open = false;
    onclose?.();
  }}
>
  <input aria-label="First" />
  <input aria-label="Second" />
  {#snippet footer()}
    <button>Save event</button>
  {/snippet}
</Dialog>
