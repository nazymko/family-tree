<script module lang="ts">
  // The settings store has no per-chart field, so the choice lives for the session.
  const chosenGenerations = $state<Record<string, number>>({});
</script>

<script lang="ts">
  import { getApi } from '../api/context';
  import type { ChartMeta, Id } from '../api/types';
  import ChartCanvas from '../chart/ChartCanvas.svelte';
  import OutlineMirror from '../chart/OutlineMirror.svelte';
  import { nav } from '../state/nav.svelte';
  import Button from '../ui/Button.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import Tabs from '../ui/Tabs.svelte';
  import { t } from '../i18n';

  let { kind, onOpenPerson }: { kind: 'pedigree' | 'descendants'; onOpenPerson?: (id: Id) => void } = $props();

  const api = getApi();
  const STORE_KEY = 'gt.chartView';
  const title = $derived(kind === 'pedigree' ? t('chart.pedigree') : t('chart.descendants'));

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
  const genKey = $derived(`${kind}:${root ?? ''}`);
  const generations = $derived(chosenGenerations[genKey] ?? (kind === 'descendants' ? 3 : 4));

  async function opened(m: ChartMeta): Promise<void> {
    meta = m;
    rootName = '';
    try {
      const first = await api.chartOutline(m.chartId, null, 1);
      if (meta?.chartId === m.chartId) rootName = first[0]?.person?.name ?? '';
    } catch {
      // The helper text falls back to "this person".
    }
  }

  const recentre = (id: Id) => nav.focus(id);
  const openPerson = (id: Id) => (onOpenPerson ? onOpenPerson(id) : nav.go({ name: 'person', id }));
  const who = $derived(rootName || t('chart.thisPerson'));
  const isEmpty = $derived(meta !== null && meta.nodeCount <= 1);
</script>

<section class="view" aria-labelledby="chart-title">
  <header>
    <h1 id="chart-title">{title}</h1>
    {#if root}
      <Tabs
        label={t('chart.display', { title })}
        tabs={[
          { id: 'chart', label: t('chart.tabChart') },
          { id: 'outline', label: t('chart.tabOutline') },
        ]}
        bind:selected={mode}
      >
        {#snippet panel(id)}
          <span class="sr-only">{id === 'chart' ? t('chart.chartView') : t('chart.outlineView')}</span>
        {/snippet}
      </Tabs>
    {/if}
  </header>

  {#if !root}
    {#if settingsLoaded}
      <EmptyState title={t('chart.noPerson')}>
        {kind === 'pedigree' ? t('chart.choosePedigree') : t('chart.chooseDescendants')}
      </EmptyState>
    {/if}
  {:else}
    {#if isEmpty}
      <EmptyState title={kind === 'pedigree' ? t('chart.noParentsTitle') : t('chart.noChildrenTitle')}>
        {#if kind === 'pedigree'}{t('chart.noParentsFor', { who })}{:else}{t('chart.noChildrenFor', { who })}{/if}
        {#snippet action()}
          <Button variant="primary" onclick={() => openPerson(root)}>{kind === 'pedigree' ? t('chart.addParent') : t('chart.addChild')}</Button>
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
        onOpened={opened}
        selectId={selectedNode}
        onSelect={(id) => (selectedNode = id)}
      />
      {#if mode === 'outline' && meta}
        <div class="outline">
          <OutlineMirror
            chartId={meta.chartId}
            label={t('chart.outline', { title })}
            selected={selectedNode}
            onSelect={(id) => (selectedNode = id)}
            onPick={recentre}
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
  .outline {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
</style>
