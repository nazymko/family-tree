<script lang="ts" module>
  export interface FieldIds {
    id: string;
    describedBy: string | undefined;
    invalid: 'true' | undefined;
  }
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    label,
    hint,
    error,
    children,
  }: { label: string; hint?: string; error?: string; children: Snippet<[FieldIds]> } = $props();

  const uid = $props.id();
  const id = `${uid}-control`;
  const hintId = `${uid}-hint`;
  const errId = `${uid}-error`;
  const ids: FieldIds = $derived({
    id,
    describedBy: [hint ? hintId : null, error ? errId : null].filter(Boolean).join(' ') || undefined,
    invalid: error ? 'true' : undefined,
  });
</script>

<div class="field">
  <label for={id}>{label}</label>
  {@render children(ids)}
  {#if hint}<p id={hintId} class="hint">{hint}</p>{/if}
  {#if error}<p id={errId} class="error">{error}</p>{/if}
</div>

<style>
  .field {
    display: grid;
    gap: var(--s-2);
    max-width: var(--measure);
  }
  label {
    font-weight: 500;
    color: var(--ink-0);
  }
  .hint {
    font-size: var(--t-sm);
    line-height: var(--t-sm-lh);
    color: var(--ink-1);
  }
  .error {
    font-size: var(--t-sm);
    line-height: var(--t-sm-lh);
    color: var(--danger);
    font-weight: 500;
  }
</style>
