<script lang="ts">
  import Field from './Field.svelte';
  import type { HTMLInputAttributes } from 'svelte/elements';
  import './controls.css';

  interface Props extends Omit<HTMLInputAttributes, 'value'> {
    label: string;
    hint?: string;
    error?: string;
    value?: string;
    /** Star after the label and aria-required; `missing` highlights it until filled (see Field). */
    required?: boolean;
    missing?: boolean;
  }
  let { label, hint, error, value = $bindable(''), required = false, missing = false, ...rest }: Props = $props();
</script>

<Field {label} {hint} {error} {required} {missing}>
  {#snippet children(f)}
    <input class="control" id={f.id} aria-describedby={f.describedBy} aria-invalid={f.invalid} aria-required={required || undefined} bind:value {...rest} />
  {/snippet}
</Field>
