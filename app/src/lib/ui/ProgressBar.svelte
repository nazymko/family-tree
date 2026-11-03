<script lang="ts">
  let { label, value, max = 100 }: { label: string; value: number; max?: number } = $props();
  const pct = $derived(max > 0 ? Math.min(100, Math.max(0, (value / max) * 100)) : 0);
</script>

<div class="bar" role="progressbar" aria-label={label} aria-valuemin="0" aria-valuemax={max} aria-valuenow={value}>
  <div class="fill" style:width="{pct}%"></div>
</div>

<style>
  .bar {
    height: 0.5rem;
    border: 1px solid var(--rule);
    border-radius: 999px;
    background: var(--paper-2);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--ribbon);
    transition: width var(--dur-base) var(--ease);
  }
</style>
