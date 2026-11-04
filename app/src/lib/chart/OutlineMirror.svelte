<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { getApi } from '../api/context';
  import type { Id, OutlineItem } from '../api/types';

  let {
    chartId,
    onPick,
    label = 'Chart outline',
    selected = null,
    onSelect,
  }: {
    chartId: number;
    onPick: (personId: Id, relation?: string) => void;
    label?: string;
    /** Selected chart node, shared with the canvas. Roving focus is tracked separately. */
    selected?: number | null;
    onSelect?: (nodeId: number) => void;
  } = $props();

  const api = getApi();
  const uid = $props.id();
  const LIMIT = 50;

  let roots = $state<OutlineItem[]>([]);
  // Everything below is keyed by PATH ("12/4/9"), not node id: on the relatives chart one person can be listed in
  // several places, and each listing needs its own open state, children and DOM id.
  let kids = $state<Record<string, OutlineItem[]>>({});
  let open = $state<Record<string, boolean>>({});
  let loading = $state<Record<string, boolean>>({});
  let focusId = $state<string | null>(null);
  const pathOf = (parentPath: string, it: OutlineItem) => (parentPath ? `${parentPath}/${it.nodeId}` : String(it.nodeId));
  let failed = $state('');
  // Non-reactive lookup filled while loading.
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  const parentOf = new Map<string, string | null>();

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
        for (const it of items) {
          parentOf.set(pathOf('', it), null);
        }
        focusId = items[0] ? pathOf('', items[0]) : null;
      })
      .catch((e) => {
        if (live) failed = e instanceof Error ? e.message : 'The outline could not be loaded.';
      });
    return () => {
      live = false;
    };
  });

  async function expand(it: OutlineItem, path: string): Promise<void> {
    if (!it.hasChildren) return;
    if (!kids[path] && !loading[path]) {
      loading[path] = true;
      try {
        const items = await api.chartOutline(chartId, it.nodeId, LIMIT);
        for (const k of items) {
          parentOf.set(pathOf(path, k), path);
        }
        kids[path] = items;
      } catch (e) {
        failed = e instanceof Error ? e.message : 'The outline could not be loaded.';
        return;
      } finally {
        loading[path] = false;
      }
    }
    open[path] = true;
  }

  /** The visible rows in order, as paths. Iterative, so a malformed outline can never overflow the stack. */
  function visible(): string[] {
    const out: string[] = [];
    const stack: { items: OutlineItem[]; path: string; i: number }[] = [{ items: roots, path: '', i: 0 }];
    while (stack.length) {
      const top = stack[stack.length - 1];
      if (top.i >= top.items.length) {
        stack.pop();
        continue;
      }
      const it = top.items[top.i++];
      const path = pathOf(top.path, it);
      out.push(path);
      if (open[path] && kids[path]) stack.push({ items: kids[path], path, i: 0 });
    }
    return out;
  }

  async function moveFocus(id: string | null | undefined): Promise<void> {
    if (id === null || id === undefined) return;
    focusId = id;
    await tick();
    document.getElementById(`${uid}-${id.replaceAll('/', '_')}`)?.focus();
  }

  function onkeydown(e: KeyboardEvent, it: OutlineItem, path: string): void {
    if (e.target !== e.currentTarget) return;
    const vis = visible();
    const i = vis.indexOf(path);
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
        if (it.hasChildren && !open[path]) void expand(it, path);
        else if (open[path] && kids[path]?.[0]) void moveFocus(pathOf(path, kids[path][0]));
        break;
      case 'ArrowLeft':
        if (open[path]) open[path] = false;
        else void moveFocus(parentOf.get(path));
        break;
      case 'Enter':
      case ' ':
        if (it.person) onPick(it.person.id, it.relation);
        break;
      default:
        return;
    }
    e.preventDefault();
    e.stopPropagation();
  }
</script>

{#snippet branch(items: OutlineItem[], level: number, parentPath: string)}
  {#each items as it (it.nodeId)}
    {@const path = pathOf(parentPath, it)}
    <li
      id="{uid}-{path.replaceAll('/', '_')}"
      role="treeitem"
      aria-level={level}
      aria-expanded={it.hasChildren ? !!open[path] : undefined}
      aria-selected={selected === it.nodeId}
      tabindex={focusId === path ? 0 : -1}
      onkeydown={(e) => onkeydown(e, it, path)}
      onfocus={(e) => {
        if (e.target !== e.currentTarget) return;
        focusId = path;
        if (selected !== it.nodeId) onSelect?.(it.nodeId);
      }}
    >
      <div class="row">
        {#if it.hasChildren}
          <button
            type="button"
            class="twist"
            tabindex="-1"
            aria-hidden="true"
            onclick={() => (open[path] ? (open[path] = false) : void expand(it, path))}
          >{open[path] ? '▾' : '▸'}</button>
        {:else}
          <span class="twist"></span>
        {/if}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <span class="text" ondblclick={() => it.person && onPick(it.person.id, it.relation)}>
          <span class="rel">{it.label}</span>
          {#if it.person}
            <bdi dir="auto" class="nm">{it.person.name}</bdi>
            {#if it.person.lifespan}<span class="life">{it.person.lifespan}</span>{/if}
          {/if}
        </span>
      </div>
      {#if open[path] && kids[path]?.length}
        <ul role="group">
          {@render branch(kids[path], level + 1, path)}
          {#if kids[path].length >= LIMIT}
            <li role="none" class="more">More people are not shown here. Centre the chart on this person to see them.</li>
          {/if}
        </ul>
      {/if}
    </li>
  {/each}
{/snippet}

{#if failed}
  <p class="err" role="alert">{failed}</p>
{/if}
<ul role="tree" aria-label={label} class="outline">
  {@render branch(roots, 1, '')}
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
  .more {
    margin: var(--s-2) 0 var(--s-2) var(--s-6);
    color: var(--ink-2);
    font-size: 0.875rem;
  }
  .err {
    color: var(--danger);
  }
</style>
