<script lang="ts">
  // A compact one-of-few choice laid out in a row (toolbars): a native radio group, so the arrow keys, the
  // screen reader's "1 of 3" and the form semantics are the browser's own.
  let {
    legend,
    options,
    value,
    onchange,
    disabledValues = [],
  }: {
    legend: string;
    options: { value: string; label: string }[];
    value: string;
    onchange?: (value: string) => void;
    /** Choices that are shown but cannot be picked. */
    disabledValues?: string[];
  } = $props();
  const uid = $props.id();
</script>

<div class="seg" role="radiogroup" aria-labelledby="{uid}-legend">
  <span id="{uid}-legend" class="cap">{legend}</span>
  <div class="opts">
    {#each options as o (o.value)}
      <label class:on={value === o.value} class:off={disabledValues.includes(o.value)}>
        <input type="radio" name={uid} value={o.value} checked={value === o.value} disabled={disabledValues.includes(o.value)} onchange={() => onchange?.(o.value)} />
        <span>{o.label}</span>
      </label>
    {/each}
  </div>
</div>

<style>
  .seg {
    display: inline-flex;
    align-items: center;
    gap: var(--s-3);
  }
  .cap {
    font-weight: 500;
  }
  .opts {
    display: inline-flex;
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    overflow: hidden;
  }
  label {
    position: relative;
    display: inline-flex;
    align-items: center;
    min-height: calc(2.25rem - 2px);
    padding: 0 var(--s-4);
    line-height: var(--t-base-lh);
    background: var(--paper-1);
    cursor: pointer;
  }
  label + label {
    border-inline-start: 1px solid var(--rule);
  }
  label.off {
    color: var(--ink-2);
    cursor: not-allowed;
    opacity: 0.6;
  }
  label.off input {
    cursor: not-allowed;
  }
  label:hover:not(.on):not(.off) {
    background: var(--paper-2);
  }
  label.on {
    background: var(--ribbon-wash, var(--paper-2));
    font-weight: 600;
    box-shadow: inset 0 -2px 0 var(--ribbon, var(--ink-2));
  }
  input {
    position: absolute;
    inset: 0;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  label:has(input:focus-visible) {
    outline: 2px solid var(--ribbon, var(--ink-1));
    outline-offset: -2px;
  }
</style>
