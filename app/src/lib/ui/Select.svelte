<script lang="ts">
  import Field from './Field.svelte';
  import './controls.css';

  interface Option {
    value: string;
    label: string;
    disabled?: boolean;
  }
  let {
    label,
    hint,
    error,
    options,
    value = $bindable(''),
    onchange,
  }: {
    label: string;
    hint?: string;
    error?: string;
    options: Option[];
    value?: string;
    onchange?: (value: string) => void;
  } = $props();
</script>

<Field {label} {hint} {error}>
  {#snippet children(f)}
    <select
      class="control"
      id={f.id}
      aria-describedby={f.describedBy}
      aria-invalid={f.invalid}
      bind:value
      onchange={() => onchange?.(value)}
    >
      {#each options as o (o.value)}
        <option value={o.value} disabled={o.disabled}>{o.label}</option>
      {/each}
    </select>
  {/snippet}
</Field>
