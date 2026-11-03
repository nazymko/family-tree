<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    total,
    rowHeight = 40,
    pageSize = 200,
    loadPage,
    row,
    selectedIndex,
    onSelect,
    onActivate,
    onDelete,
    ariaLabel,
    keyPrefix,
    initialPage,
    maxPages = 20,
  }: {
    total: number;
    rowHeight?: number;
    pageSize?: number;
    /** Called once per missing page. */
    loadPage: (offset: number, limit: number) => Promise<unknown[]>;
    /** `item` is null while the page is loading: render a static placeholder. */
    row: Snippet<[item: unknown | null, index: number, selected: boolean]>;
    selectedIndex: number;
    onSelect: (i: number) => void;
    onActivate: (i: number) => void;
    onDelete?: (i: number) => void;
    ariaLabel: string;
    keyPrefix: string;
    /** A page the caller already fetched (saves a duplicate request). */
    initialPage?: { offset: number; items: unknown[] };
    maxPages?: number;
  } = $props();

  const EXTRA = 10;

  let el = $state<HTMLElement>();
  let top = $state(0);
  let height = $state(600);
  let version = $state(0);

  // Plain (non-reactive) cache; `version` tells the template when it changed.
  const cache: Record<number, unknown[]> = {};
  const lru: number[] = [];
  const inflight: Record<number, true> = {};
  let alive = true;

  // Seed once; the parent re-creates this component (with {#key}) when the data set changes.
  // svelte-ignore state_referenced_locally
  if (initialPage && initialPage.offset % pageSize === 0) store(initialPage.offset / pageSize, initialPage.items);

  function touch(p: number) {
    const i = lru.indexOf(p);
    if (i >= 0) lru.splice(i, 1);
    lru.push(p);
  }

  function store(p: number, items: unknown[]) {
    cache[p] = items;
    touch(p);
    while (lru.length > maxPages) {
      const old = lru.shift()!;
      delete cache[old];
    }
  }

  const visible = $derived(Math.ceil(height / rowHeight));
  const start = $derived(Math.max(0, Math.min(Math.floor(top / rowHeight) - 3, total - 1)));
  const end = $derived(Math.min(total, start + visible + EXTRA));
  const indexes = $derived(Array.from({ length: Math.max(0, end - start) }, (_, i) => start + i));

  $effect(() => {
    if (end <= start) return;
    const first = Math.floor(start / pageSize);
    const last = Math.floor((end - 1) / pageSize);
    for (let p = first; p <= last; p++) {
      if (cache[p]) {
        touch(p);
      } else if (!inflight[p]) {
        inflight[p] = true;
        loadPage(p * pageSize, pageSize)
          .then((items) => {
            if (!alive) return;
            store(p, items);
            version++;
          })
          .catch(() => {})
          .finally(() => delete inflight[p]);
      }
    }
  });

  $effect(() => {
    alive = true;
    return () => {
      alive = false;
    };
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

  function itemAtRaw(i: number): unknown | null {
    return cache[Math.floor(i / pageSize)]?.[i % pageSize] ?? null;
  }

  /** `v` only exists so the template re-reads the cache when `version` changes. */
  function itemFor(i: number, v: number): unknown | null {
    return v >= 0 ? itemAtRaw(i) : null;
  }

  export function itemAt(i: number): unknown | null {
    return itemAtRaw(i);
  }

  export function scrollToOffset(i: number): void {
    if (!el) return;
    const y = Math.max(0, Math.min(i, total - 1)) * rowHeight;
    el.scrollTop = y;
    top = y;
  }

  function reveal(i: number) {
    if (!el) return;
    const y = i * rowHeight;
    if (y < el.scrollTop) el.scrollTop = y;
    else if (y + rowHeight > el.scrollTop + el.clientHeight) el.scrollTop = y + rowHeight - (el.clientHeight || height);
    top = el.scrollTop;
  }

  function move(i: number) {
    if (total === 0) return;
    const n = Math.max(0, Math.min(total - 1, i));
    onSelect(n);
    reveal(n);
  }

  function onkeydown(e: KeyboardEvent) {
    const page = Math.max(1, visible - 1);
    const at = Math.max(0, selectedIndex);
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        move(selectedIndex < 0 ? 0 : at + 1);
        break;
      case 'ArrowUp':
        e.preventDefault();
        move(at - 1);
        break;
      case 'PageDown':
        e.preventDefault();
        move(at + page);
        break;
      case 'PageUp':
        e.preventDefault();
        move(at - page);
        break;
      case 'Home':
        e.preventDefault();
        move(0);
        break;
      case 'End':
        e.preventDefault();
        move(total - 1);
        break;
      case 'Enter':
        if (selectedIndex >= 0) {
          e.preventDefault();
          onActivate(selectedIndex);
        }
        break;
      case 'Delete':
        if (selectedIndex >= 0 && onDelete) {
          e.preventDefault();
          onDelete(selectedIndex);
        }
        break;
    }
  }
</script>

<!-- the brief requires aria-rowcount / aria-rowindex on a listbox -->
<!-- svelte-ignore a11y_role_supports_aria_props -->
<div
  class="pl"
  bind:this={el}
  role="listbox"
  tabindex="0"
  aria-label={ariaLabel}
  aria-rowcount={total}
  aria-activedescendant={selectedIndex >= 0 ? `${keyPrefix}-${selectedIndex}` : undefined}
  onscroll={() => el && (top = el.scrollTop)}
  {onkeydown}
>
  <div class="spacer" style:height="{total * rowHeight}px">
    {#each indexes as i (i)}
      {@const item = itemFor(i, version)}
      <!-- svelte-ignore a11y_role_supports_aria_props -->
      <div
        id="{keyPrefix}-{i}"
        class="r"
        class:loading={item === null}
        role="option"
        tabindex="-1"
        aria-selected={i === selectedIndex}
        aria-rowindex={i + 1}
        data-loading={item === null ? 'true' : undefined}
        style:height="{rowHeight}px"
        style:transform="translateY({i * rowHeight}px)"
        onclick={() => onSelect(i)}
        ondblclick={() => onActivate(i)}
        onkeydown={() => {}}
      >
        {@render row(item, i, i === selectedIndex)}
      </div>
    {/each}
  </div>
</div>

<style>
  .pl {
    height: 100%;
    overflow: auto;
    position: relative;
    background: var(--paper-1);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
  }
  .spacer {
    position: relative;
    width: 100%;
  }
  .r {
    position: absolute;
    inset-inline: 0;
    top: 0;
    display: flex;
    align-items: stretch;
    cursor: default;
    border-block-end: 1px solid var(--rule);
    box-sizing: border-box;
  }
  .loading {
    opacity: 0.4;
  }
</style>
