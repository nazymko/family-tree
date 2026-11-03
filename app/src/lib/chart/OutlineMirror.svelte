<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { getApi } from '../api/context';
  import type { Id, OutlineItem } from '../api/types';

  let { chartId, onPick, label = 'Chart outline' }: { chartId: number; onPick: (personId: Id) => void; label?: string } = $props();

  const api = getApi();
  const uid = $props.id();
  const LIMIT = 50;

  let roots = $state<OutlineItem[]>([]);
  let kids = $state<Record<number, OutlineItem[]>>({});
  let open = $state<Record<number, boolean>>({});
  let loading = $state<Record<number, boolean>>({});
  let focusId = $state<number | null>(null);
  let failed = $state('');
  // Non-reactive lookup filled while loading.
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  const parentOf = new Map<number, number | null>();

  $effect(() => {
    const id = chartId;
    untrack(() => {
      roots = [];
      kids = {};
      open = {};
      loading = {};
      parentOf.clear();
      failed = '';
    });
    let live = true;
    api
      .chartOutline(id, null, LIMIT)
      .then((items) => {
        if (!live) return;
        roots = items;
        for (const it of items) parentOf.set(it.nodeId, null);
        focusId = items[0]?.nodeId ?? null;
      })
      .catch((e) => {
        if (live) failed = e instanceof Error ? e.message : 'The outline could not be loaded.';
      });
    return () => {
      live = false;
    };
  });

  async function expand(it: OutlineItem): Promise<void> {
    if (!it.hasChildren) return;
    if (!kids[it.nodeId] && !loading[it.nodeId]) {
      loading[it.nodeId] = true;
      try {
        const items = await api.chartOutline(chartId, it.nodeId, LIMIT);
        for (const k of items) parentOf.set(k.nodeId, it.nodeId);
        kids[it.nodeId] = items;
      } catch (e) {
        failed = e instanceof Error ? e.message : 'The outline could not be loaded.';
        return;
      } finally {
        loading[it.nodeId] = false;
      }
    }
    open[it.nodeId] = true;
  }

  function visible(): number[] {
    const out: number[] = [];
    const walk = (items: OutlineItem[]) => {
      for (const it of items) {
        out.push(it.nodeId);
        if (open[it.nodeId] && kids[it.nodeId]) walk(kids[it.nodeId]);
      }
    };
    walk(roots);
    return out;
  }

  async function moveFocus(id: number | null | undefined): Promise<void> {
    if (id === null || id === undefined) return;
    focusId = id;
    await tick();
    document.getElementById(`${uid}-${id}`)?.focus();
  }

  function onkeydown(e: KeyboardEvent, it: OutlineItem): void {
    const vis = visible();
    const i = vis.indexOf(it.nodeId);
    switch (e.key) {
      case 'ArrowDown':
        void moveFocus(vis[i + 1]);
        break;
      case 'ArrowUp':
        void moveFocus(vis[i - 1]);
        break;
      case 'Home':
        void moveFocus(vis[0]);
        break;
      case 'End':
        void moveFocus(vis[vis.length - 1]);
        break;
      case 'ArrowRight':
        if (it.hasChildren && !open[it.nodeId]) void expand(it);
        else if (open[it.nodeId]) void moveFocus(kids[it.nodeId]?.[0]?.nodeId);
        break;
      case 'ArrowLeft':
        if (open[it.nodeId]) open[it.nodeId] = false;
        else void moveFocus(parentOf.get(it.nodeId));
        break;
      case 'Enter':
      case ' ':
        if (it.person) onPick(it.person.id);
        break;
      default:
        return;
    }
    e.preventDefault();
    e.stopPropagation();
  }
</script>

{#snippet branch(items: OutlineItem[], level: number)}
  {#each items as it (it.nodeId)}
    <li
      id="{uid}-{it.nodeId}"
      role="treeitem"
      aria-level={level}
      aria-expanded={it.hasChildren ? !!open[it.nodeId] : undefined}
      aria-selected={focusId === it.nodeId}
      tabindex={focusId === it.nodeId ? 0 : -1}
      onkeydown={(e) => onkeydown(e, it)}
      onfocus={() => (focusId = it.nodeId)}
    >
      <div class="row">
        {#if it.hasChildren}
          <button
            type="button"
            class="twist"
            tabindex="-1"
            aria-hidden="true"
            onclick={() => (open[it.nodeId] ? (open[it.nodeId] = false) : void expand(it))}
          >{open[it.nodeId] ? '▾' : '▸'}</button>
        {:else}
          <span class="twist"></span>
        {/if}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <span class="text" ondblclick={() => it.person && onPick(it.person.id)}>
          <span class="rel">{it.label}</span>
          {#if it.person}
            <bdi dir="auto" class="nm">{it.person.name}</bdi>
            {#if it.person.lifespan}<span class="life">{it.person.lifespan}</span>{/if}
          {/if}
        </span>
      </div>
      {#if open[it.nodeId] && kids[it.nodeId]?.length}
        <ul role="group">
          {@render branch(kids[it.nodeId], level + 1)}
        </ul>
      {/if}
    </li>
  {/each}
{/snippet}

{#if failed}
  <p class="err" role="alert">{failed}</p>
{/if}
<ul role="tree" aria-label={label} class="outline">
  {@render branch(roots, 1)}
</ul>

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  ul[role='group'] {
    margin-inline-start: var(--s-6);
    border-inline-start: 1px solid var(--rule);
  }
  .outline {
    padding: var(--s-3);
    background: var(--paper-1);
    border: 1px solid var(--rule);
    overflow: auto;
    min-height: 0;
  }
  li {
    outline: none;
  }
  li:focus-visible > .row {
    outline: 2px solid var(--focus);
    outline-offset: 1px;
  }
  .row {
    display: flex;
    align-items: baseline;
    gap: var(--s-2);
    padding: var(--s-2) var(--s-3);
  }
  li[aria-selected='true'] > .row {
    background: var(--ribbon-wash);
  }
  .twist {
    width: 1.25rem;
    flex: none;
    border: 0;
    background: transparent;
    color: var(--ink-1);
    cursor: pointer;
    padding: 0;
  }
  .rel {
    color: var(--ink-1);
    margin-inline-end: var(--s-3);
  }
  .nm {
    font-family: var(--font-serif);
    font-weight: 600;
    color: var(--ink-0);
    margin-inline-end: var(--s-3);
  }
  .life {
    color: var(--ink-2);
    font-size: 0.875rem;
  }
  .err {
    color: var(--danger);
  }
</style>
