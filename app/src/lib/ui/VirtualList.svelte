<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    label,
    count,
    rowHeight = 40,
    overscan = 6,
    row,
    onrange,
  }: {
    label: string;
    count: number;
    rowHeight?: number;
    overscan?: number;
    row: Snippet<[number]>;
    /** Called when the rendered window changes: [start, end). Use it to page data in. */
    onrange?: (start: number, end: number) => void;
  } = $props();

  let el = $state<HTMLElement>();
  let top = $state(0);
  let height = $state(480);

  const start = $derived(Math.max(0, Math.floor(top / rowHeight) - overscan));
  const end = $derived(Math.min(count, Math.ceil((top + height) / rowHeight) + overscan));
  const indexes = $derived(Array.from({ length: Math.max(0, end - start) }, (_, i) => start + i));

  $effect(() => {
    onrange?.(start, end);
  });

  $effect(() => {
    if (!el) return;
    height = el.clientHeight || height;
    const ro = new ResizeObserver(() => {
      if (el) height = el.clientHeight || height;
    });
    ro.observe(el);
    return () => ro.disconnect();
  });

  export function scrollToIndex(i: number) {
    if (!el) return;
    const y = i * rowHeight;
    if (y < el.scrollTop) el.scrollTop = y;
    else if (y + rowHeight > el.scrollTop + el.clientHeight) el.scrollTop = y + rowHeight - el.clientHeight;
  }
</script>

<div
  class="vl"
  bind:this={el}
  role="list"
  aria-label={label}
  onscroll={() => el && (top = el.scrollTop)}
>
  <div class="spacer" style:height="{count * rowHeight}px">
    {#each indexes as i (i)}
      <div
        class="row"
        role="listitem"
        aria-setsize={count}
        aria-posinset={i + 1}
        style:height="{rowHeight}px"
        style:transform="translateY({i * rowHeight}px)"
      >
        {@render row(i)}
      </div>
    {/each}
  </div>
</div>

<style>
  .vl {
    position: relative;
    height: 100%;
    overflow: auto;
  }
  .spacer {
    position: relative;
  }
  .row {
    position: absolute;
    inset-inline: 0;
    top: 0;
    display: flex;
    align-items: center;
  }
</style>
