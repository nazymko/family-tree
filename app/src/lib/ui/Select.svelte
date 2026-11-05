<script lang="ts">
  import Field from './Field.svelte';
  import './controls.css';

  interface Option {
    value: string;
    label: string;
    disabled?: boolean;
    /** Options sharing a group name (consecutive) render inside one optgroup. */
    group?: string;
  }
  let {
    label,
    hint,
    error,
    options,
    value = $bindable(''),
    onchange,
    disabled = false,
  }: {
    label: string;
    hint?: string;
    error?: string;
    options: Option[];
    value?: string;
    onchange?: (value: string) => void;
    disabled?: boolean;
  } = $props();

  const groups = $derived.by(() => {
    const out: { name: string; items: Option[] }[] = [];
    for (const o of options) {
      const name = o.group ?? '';
      const last = out[out.length - 1];
      if (last && last.name === name) last.items.push(o);
      else out.push({ name, items: [o] });
    }
    return out;
  });
</script>

<Field {label} {hint} {error}>
  {#snippet children(f)}
    <select
      class="control"
      id={f.id}
      aria-describedby={f.describedBy}
      aria-invalid={f.invalid}
      {disabled}
      bind:value
      onchange={() => onchange?.(value)}
    >
      {#each groups as g (g.name)}
        {#if g.name}
          <optgroup label={g.name}>
            {#each g.items as o (o.value)}<option value={o.value} disabled={o.disabled}>{o.label}</option>{/each}
          </optgroup>
        {:else}
          {#each g.items as o (o.value)}<option value={o.value} disabled={o.disabled}>{o.label}</option>{/each}
        {/if}
      {/each}
    </select>
  {/snippet}
</Field>
