<script lang="ts">
  let {
    legend,
    options,
    value = $bindable(''),
    onchange,
  }: {
    legend: string;
    options: { value: string; label: string; hint?: string }[];
    value?: string;
    onchange?: (value: string) => void;
  } = $props();
  const uid = $props.id();
</script>

<fieldset>
  <legend>{legend}</legend>
  {#each options as o (o.value)}
    <div class="opt">
      <input
        id="{uid}-{o.value}"
        type="radio"
        name={uid}
        value={o.value}
        bind:group={value}
        aria-describedby={o.hint ? `${uid}-${o.value}-hint` : undefined}
        onchange={() => onchange?.(value)}
      />
      <label for="{uid}-{o.value}">{o.label}</label>
      {#if o.hint}<p id="{uid}-{o.value}-hint" class="hint">{o.hint}</p>{/if}
    </div>
  {/each}
</fieldset>

<style>
  fieldset {
    margin: 0;
    padding: 0;
    border: 0;
    display: grid;
    gap: var(--s-3);
    max-width: var(--measure);
  }
  legend {
    padding: 0;
    margin-bottom: var(--s-3);
    font-weight: 500;
  }
  .opt {
    display: grid;
    grid-template-columns: auto 1fr;
    column-gap: var(--s-3);
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
