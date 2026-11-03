<script lang="ts">
  import Dialog from './Dialog.svelte';
  import Icon from './Icon.svelte';
  import { getApi } from '../api/context';
  import type { SearchHitDto } from '../api/types';

  let { open, onclose, onpick }: { open: boolean; onclose: () => void; onpick: (hit: SearchHitDto) => void } = $props();

  const api = getApi();
  const uid = $props.id();
  const GROUPS: { kind: SearchHitDto['kind']; title: string }[] = [
    { kind: 'person', title: 'People' },
    { kind: 'place', title: 'Places' },
    { kind: 'source', title: 'Sources' },
    { kind: 'note', title: 'Notes' },
  ];

  let text = $state('');
  let hits = $state<SearchHitDto[]>([]);
  let searched = $state(false);
  let failed = $state(false);
  let active = $state(0);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let seq = 0;

  const groups = $derived(
    GROUPS.map((g) => ({ ...g, items: hits.filter((h) => h.kind === g.kind) })).filter((g) => g.items.length > 0),
  );
  const flat = $derived(groups.flatMap((g) => g.items));
  const activeId = $derived(flat.length > 0 ? `${uid}-opt-${active}` : undefined);

  $effect(() => {
    if (!open) {
      text = '';
      hits = [];
      searched = false;
      failed = false;
      active = 0;
      clearTimeout(timer);
      seq++;
    }
  });

  function oninput(e: Event & { currentTarget: HTMLInputElement }) {
    text = e.currentTarget.value;
    clearTimeout(timer);
    const q = text.trim();
    if (!q) {
      seq++;
      hits = [];
      searched = false;
      return;
    }
    timer = setTimeout(async () => {
      const mine = ++seq;
      try {
        const found = await api.search(q, 20);
        if (mine !== seq) return;
        hits = found;
        failed = false;
        searched = true;
        active = 0;
      } catch {
        if (mine !== seq) return;
        hits = [];
        failed = true;
        searched = true;
      }
    }, 150);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' && flat.length > 0) {
      e.preventDefault();
      active = (active + 1) % flat.length;
    } else if (e.key === 'ArrowUp' && flat.length > 0) {
      e.preventDefault();
      active = (active - 1 + flat.length) % flat.length;
    } else if (e.key === 'Enter' && flat[active]) {
      e.preventDefault();
      onpick(flat[active]);
    }
  }

  $effect(() => {
    if (activeId) document.getElementById(activeId)?.scrollIntoView?.({ block: 'nearest' });
  });
</script>

<Dialog {open} title="Search" {onclose}>
  <div class="palette">
    <div class="box">
      <Icon name="search" />
      <input
        class="q"
        type="text"
        role="combobox"
        aria-label="Search the tree"
        aria-expanded={flat.length > 0}
        aria-controls="{uid}-list"
        aria-activedescendant={activeId}
        aria-autocomplete="list"
        autocomplete="off"
        spellcheck="false"
        placeholder="A name, place or source"
        value={text}
        {oninput}
        {onkeydown}
      />
    </div>
    <div id="{uid}-list" class="results" role="listbox" aria-label="Search results">
      {#each groups as g (g.kind)}
        <div role="group" aria-labelledby="{uid}-g-{g.kind}">
          <div class="grp" id="{uid}-g-{g.kind}">{g.title}</div>
          {#each g.items as h (h.id)}
            {@const i = flat.indexOf(h)}
            <div
              id="{uid}-opt-{i}"
              role="option"
              tabindex="-1"
              aria-selected={i === active}
              class="opt"
              class:active={i === active}
              onmousedown={(e) => e.preventDefault()}
              onclick={() => onpick(h)}
              onkeydown={() => {}}
            >
              <bdi dir="auto" class="label">{h.label}</bdi>
              {#if h.snippet}<span class="snip tnum">{h.snippet}</span>{/if}
            </div>
          {/each}
        </div>
      {/each}
    </div>
    {#if failed}
      <p class="none" role="alert">Search did not work. Try again; if it keeps failing, restart the app.</p>
    {:else if searched && flat.length === 0}
      <p class="none">No matches. Try a different spelling; search also finds similar-sounding names.</p>
    {:else if !searched}
      <p class="hint">Type a name, a place or a source title.</p>
    {/if}
  </div>
</Dialog>

<style>
  .palette {
    display: grid;
    gap: var(--s-3);
    min-width: min(36rem, 80vw);
  }
  .box {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    padding: 0 var(--s-4);
    border: 1px solid var(--ink-2);
    border-radius: var(--r-1);
    background: var(--paper-1);
    color: var(--ink-1);
  }
  .q {
    flex: 1;
    min-height: 2.5rem;
    border: 0;
    background: transparent;
    outline: none;
    color: var(--ink-0);
    font-size: var(--t-md);
  }
  .box:focus-within {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .results {
    max-height: 22rem;
    overflow: auto;
  }
  .grp {
    padding: var(--s-4) var(--s-3) var(--s-2);
    font-size: var(--t-xs);
    font-weight: 500;
    color: var(--ink-1);
    border-block-end: 1px solid var(--rule);
  }
  .opt {
    display: flex;
    justify-content: space-between;
    gap: var(--s-4);
    padding: var(--s-3);
    min-height: var(--row-h);
    align-items: center;
    cursor: pointer;
    border-inline-start: 3px solid transparent;
  }
  .opt.active {
    background: var(--ribbon-wash);
    border-inline-start-color: var(--ribbon);
  }
  .label {
    font-family: var(--font-serif);
    font-weight: 600;
  }
  .snip {
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  .none,
  .hint {
    color: var(--ink-1);
    font-size: var(--t-sm);
    padding: var(--s-2) var(--s-3);
  }
</style>
