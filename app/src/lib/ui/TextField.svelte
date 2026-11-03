<script lang="ts">
  import Field from './Field.svelte';
  import type { HTMLInputAttributes } from 'svelte/elements';
  import './controls.css';

  interface Props extends Omit<HTMLInputAttributes, 'value'> {
    label: string;
    hint?: string;
    error?: string;
    value?: string;
  }
  let { label, hint, error, value = $bindable(''), ...rest }: Props = $props();
</script>

<Field {label} {hint} {error}>
  {#snippet children(f)}
    <input class="control" id={f.id} aria-describedby={f.describedBy} aria-invalid={f.invalid} bind:value {...rest} />
  {/snippet}
</Field>
