<script lang="ts">
  import TextField from '../ui/TextField.svelte';
  import { getApi } from '../api/context';
  import { settings } from '../state/settings.svelte';

  let { value = $bindable(''), label = 'Date' }: { value?: string; label?: string } = $props();
  const api = getApi();
  let hint = $state<string | undefined>();
  let seq = 0;

  $effect(() => {
    const raw = value.trim();
    const order = settings.value.dayOrder;
    if (!raw) {
      hint = undefined;
      return;
    }
    const mine = ++seq;
    void api.parseDate(raw, order).then((p) => {
      if (mine !== seq) return;
      hint = p.status === 'unparsed' ? 'Not understood; it is kept as you typed it.' : `Understood as ${p.understoodAs}`;
    });
  });
</script>

<TextField {label} bind:value {hint} autocomplete="off" />
