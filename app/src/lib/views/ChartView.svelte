<script module lang="ts">
  // The settings store has no per-chart field, so the choice lives for the session.
  const chosenGenerations = $state<Record<string, number>>({});
  const chosenAunts = $state({ on: true });
</script>

<script lang="ts">
  import { getApi } from '../api/context';
  import type { ChartKind, ChartMeta, Id } from '../api/types';
  import ChartCanvas from '../chart/ChartCanvas.svelte';
  import OutlineMirror from '../chart/OutlineMirror.svelte';
  import { OUTLINE_GROUP_BASE } from '../chart/outline';
  import { nav } from '../state/nav.svelte';
  import Button from '../ui/Button.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import Tabs from '../ui/Tabs.svelte';

  let { kind, onOpenPerson }: { kind: ChartKind; onOpenPerson?: (id: Id) => void } = $props();

  const api = getApi();
  const STORE_KEY = 'gt.chartView';
  const title = $derived(kind === 'pedigree' ? 'Pedigree' : kind === 'descendants' ? 'Descendants' : 'Relatives');

  function initialMode(): 'chart' | 'outline' {
    try {
      const v = localStorage.getItem(STORE_KEY);
      if (v === 'chart' || v === 'outline') return v;
    } catch {
      // Storage can be unavailable; fall through to the default.
    }
    return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches ? 'outline' : 'chart';
  }

  let mode = $state<string>(initialMode());
    let home = $state<Id | null>(null);
  let meta = $state<ChartMeta | null>(null);
  let rootName = $state('');
  let settingsLoaded = $state(false);
  let selectedNode = $state<number | null>(null);

  $effect(() => {
    void api
      .getSettings()
      .then((s) => (home = s.homePerson))
      .catch(() => {})
      .finally(() => (settingsLoaded = true));
  });

  $effect(() => {
    try {
      localStorage.setItem(STORE_KEY, mode);
    } catch {
      // Not persisted; the view still works.
    }
  });

  const root = $derived(nav.focusPerson ?? home);
  // On the relatives chart the choice follows you from person to person.
  const genKey = $derived(kind === 'relatives' ? kind : `${kind}:${root ?? ''}`);
  const generations = $derived(chosenGenerations[genKey] ?? (kind === 'pedigree' ? 4 : 3));

  // Relatives chart: the path of people the chart has been centred on, newest last.
  interface Crumb {
    id: Id;
    name: string;
    relation?: string;
  }
  const MAX_TRAIL = 12;
  let trail = $state<Crumb[]>([]);
  let pendingCrumb: { id: Id; relation?: string } | null = null;

  function updateTrail(id: Id, name: string): void {
    const i = trail.findIndex((c) => c.id === id);
    if (i >= 0) trail = [...trail.slice(0, i), { ...trail[i], name }];
    else if (pendingCrumb?.id === id && pendingCrumb.relation && trail.length > 0) {
      trail = [...trail, { id, name, relation: pendingCrumb.relation }].slice(-MAX_TRAIL);
    } else trail = [{ id, name }];
    pendingCrumb = null;
  }

  const crumbText = (c: Crumb) => (c.relation ? `${c.name} (${c.relation})` : c.name);

  async function opened(m: ChartMeta): Promise<void> {
    meta = m;
    rootName = '';
    const centre = root;
    try {
      const first = await api.chartOutline(m.chartId, null, 1);
      if (meta?.chartId === m.chartId) rootName = first[0]?.person?.name ?? '';
    } catch {
      // The helper text falls back to "this person".
    }
    if (kind === 'relatives' && centre && meta?.chartId === m.chartId) updateTrail(centre, rootName || 'This person');
  }

  /** Centre the chart on someone. On the relatives chart this is a new history entry, so Back returns to the last person. */
  function recentre(id: Id, relation?: string): void {
    if (kind !== 'relatives') {
      nav.focus(id);
      return;
    }
    if (id !== root) pendingCrumb = { id, relation };
    nav.visit(id);
  }
  /** Outline group headings are not chart nodes. */
  const select = (id: number) => (selectedNode = id < OUTLINE_GROUP_BASE ? id : selectedNode);
  const openPerson = (id: Id) => (onOpenPerson ? onOpenPerson(id) : nav.go({ name: 'person', id }));
  const who = $derived(rootName || 'this person');
  const isEmpty = $derived(meta !== null && meta.nodeCount <= 1);
</script>

<section class="view" aria-labelledby="chart-title">
  <header>
    <h1 id="chart-title">{title}</h1>
    {#if root}
      <Tabs
        label="{title} display"
        tabs={[
          { id: 'chart', label: 'Chart' },
          { id: 'outline', label: 'Outline' },
        ]}
        bind:selected={mode}
      >
        {#snippet panel(id)}
          <span class="sr-only">{id === 'chart' ? 'Chart view' : 'Outline view'}</span>
        {/snippet}
      </Tabs>
    {/if}
  </header>

  {#if !root}
    {#if settingsLoaded}
      <EmptyState title="No person chosen">
        Choose a person in People, or set a home person in Settings, to see their {kind === 'pedigree' ? 'ancestors' : kind === 'descendants' ? 'descendants' : 'relatives'}.
      </EmptyState>
    {/if}
  {:else}
    {#if kind === 'relatives' && trail.length > 0}
      <nav class="trail" aria-label="People you have visited">
        <ol>
          {#each trail as c, i (c.id)}
            <li>
              {#if i < trail.length - 1}
                <button type="button" class="crumb" onclick={() => recentre(c.id)}>{crumbText(c)}</button>
              {:else}
                <span class="here" aria-current="location">{crumbText(c)}</span>
              {/if}
            </li>
          {/each}
        </ol>
      </nav>
    {/if}
    {#if isEmpty}
      <EmptyState title={kind === 'pedigree' ? 'No parents recorded' : kind === 'descendants' ? 'No children recorded' : 'No relatives recorded'}>
        {#if kind === 'pedigree'}No parents recorded for {who}. Add a father or mother.{:else if kind === 'descendants'}No children recorded for {who}. Add a child.{:else}No relatives recorded for {who}. Add a parent, a partner or a child.{/if}
        {#snippet action()}
          <Button variant="primary" onclick={() => openPerson(root)}>{kind === 'pedigree' ? 'Add a parent' : kind === 'descendants' ? 'Add a child' : 'Add a relative'}</Button>
        {/snippet}
      </EmptyState>
    {/if}
    <div class="body">
      <ChartCanvas
        {kind}
        {root}
        {generations}
        hidden={mode !== 'chart'}
        onRecentre={recentre}
        onOpenPerson={openPerson}
        onGenerations={(n) => (chosenGenerations[genKey] = n)}
        auntsUncles={chosenAunts.on}
        onAunts={kind === 'relatives' ? (v) => (chosenAunts.on = v) : undefined}
        onOpened={opened}
        selectId={selectedNode}
        onSelect={select}
      />
      {#if mode === 'outline' && meta}
        <div class="outline">
          <OutlineMirror
            chartId={meta.chartId}
            label="{title} outline"
            selected={selectedNode}
            onSelect={select}
            onPick={(id) => recentre(id)}
          />
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .view {
    display: flex;
    flex-direction: column;
    gap: var(--s-4);
    height: 100%;
    min-height: 0;
  }
  header {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--s-6);
  }
  h1 {
    margin: 0;
  }
  .body {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
  .trail {
    margin-bottom: var(--s-3);
  }
  .trail ol {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-1) var(--s-2);
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .trail li + li::before {
    content: '\203A';
    margin-inline-end: var(--s-2);
    color: var(--ink-2);
  }
  .crumb {
    background: none;
    border: 0;
    padding: var(--s-1) var(--s-2);
    font: inherit;
    color: var(--ribbon);
    text-decoration: underline;
    cursor: pointer;
  }
  .crumb:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 1px;
  }
  .here {
    padding: var(--s-1) var(--s-2);
    font-weight: 600;
    color: var(--ink-0);
  }
  .outline {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
</style>
