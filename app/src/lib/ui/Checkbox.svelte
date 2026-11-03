<script lang="ts">
  let {
    label,
    checked = $bindable(false),
    disabled = false,
    hint,
    onchange,
  }: {
    label: string;
    checked?: boolean;
    disabled?: boolean;
    hint?: string;
    onchange?: (checked: boolean) => void;
  } = $props();
  const uid = $props.id();
</script>

<div class="cb">
  <input
    id="{uid}-cb"
    type="checkbox"
    bind:checked
    {disabled}
    aria-describedby={hint ? `${uid}-hint` : undefined}
    onchange={() => onchange?.(checked)}
  />
  <label for="{uid}-cb">{label}</label>
  {#if hint}<p id="{uid}-hint" class="hint">{hint}</p>{/if}
</div>

<style>
  .cb {
    display: grid;
    grid-template-columns: auto 1fr;
    column-gap: var(--s-3);
    align-items: start;
    max-width: var(--measure);
  }
  input {
    width: 1.125rem;
    height: 1.125rem;
    margin: 0.15rem 0 0;
    accent-color: var(--ribbon);
  }
  label {
    cursor: pointer;
  }
  .hint {
    grid-column: 2;
    font-size: var(--t-sm);
    line-height: var(--t-sm-lh);
    color: var(--ink-1);
  }
</style>
