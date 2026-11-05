<script lang="ts">
  import { tick, untrack } from 'svelte';
  import type { BatchDto, Id, LogFilterDto, LogPersonDto, LogRowDto, UndoReport } from '../api/types';
  import { getApi } from '../api/context';
  import { errMessage } from '../editors/errors';
  import { agentChanges } from '../state/agentChanges.svelte';
  import { nav } from '../state/nav.svelte';
  import { project } from '../state/project.svelte';
  import { toasts } from '../state/toast.svelte';
  import Banner from '../ui/Banner.svelte';
  import Badge from '../ui/Badge.svelte';
  import Button from '../ui/Button.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import Select from '../ui/Select.svelte';
  import { currentLocale, plural, t } from '../i18n';
  import { editLabel, factText, recordKindLabel } from '../i18n/backend';
  import LogChanges from './LogChanges.svelte';

  const PAGE = 50;
  const api = getApi();
  const uid = $props.id();

  let rows = $state<LogRowDto[]>([]);
  let total = $state(0);
  let batches = $state<BatchDto[]>([]);
  let loaded = $state(false);
  let loadError = $state('');

  let author = $state('');
  let batchId = $state('');
  let onlyUnreviewed = $state(false);
  /** Every agent seen so far, so the filter keeps its options while it narrows the rows. */
  let agents = $state<Record<string, string>>({});

  const filter = $derived<LogFilterDto>({
    unreviewedOnly: onlyUnreviewed,
    ...(author === 'user' || author === 'agent' ? { actorKind: author } : {}),
    ...(author.startsWith('agent:') ? { actorId: author.slice(6) } : {}),
    ...(batchId ? { batch: batchId } : {}),
  });
  const filtered = $derived(author !== '' || batchId !== '' || onlyUnreviewed);

  const authorOptions = $derived([
    { value: '', label: t('changeLog.anyAuthor') },
    { value: 'user', label: t('changeLog.you') },
    { value: 'agent', label: t('changeLog.anyAgent'), group: t('changeLog.agents') },
    ...Object.entries(agents).map(([id, name]) => ({ value: `agent:${id}`, label: name, group: t('changeLog.agents') })),
  ]);
  const batchOptions = $derived([{ value: '', label: t('changeLog.anyBatch') }, ...batches.map((b) => ({ value: b.id, label: b.title }))]);

  let loadSeq = 0;
  async function load(f: LogFilterDto, want: number) {
    const mine = ++loadSeq;
    try {
      const items: LogRowDto[] = [];
      let count = 0;
      do {
        const page = await api.changeLog(f, items.length, Math.min(want - items.length, PAGE));
        if (mine !== loadSeq) return;
        count = page.total;
        items.push(...page.items);
        if (page.items.length === 0) break;
      } while (items.length < want && items.length < count);
      const b = await api.changeBatches();
      if (mine !== loadSeq) return;
      rows = items;
      total = count;
      batches = b;
      loaded = true;
      loadError = '';
      const seen = items.flatMap((r) => (r.actor.kind === 'agent' && agents[r.actor.id] !== r.actor.name ? [r.actor] : []));
      if (seen.length > 0) agents = { ...agents, ...Object.fromEntries(seen.map((a) => [a.id, a.name])) };
    } catch (e) {
      if (mine === loadSeq) loadError = errMessage(e);
    }
  }

  let lastFilter = '';
  $effect(() => {
    void project.rev;
    const f = filter;
    untrack(() => {
      const key = JSON.stringify(f);
      const want = key === lastFilter ? Math.max(rows.length, PAGE) : PAGE;
      lastFilter = key;
      void load(f, want);
    });
  });

  /** Runs of rows from one batch, each under that batch's heading; rows from no batch stand alone. */
  const segments = $derived.by(() => {
    const out: { key: string; ref: LogRowDto['batch']; info: BatchDto | undefined; rows: LogRowDto[] }[] = [];
    const seen: Record<string, number> = {};
    for (const r of rows) {
      const id = r.batch?.id ?? '';
      const last = out[out.length - 1];
      if (last && (last.ref?.id ?? '') === id) {
        last.rows.push(r);
        continue;
      }
      const n = seen[id] ?? 0;
      seen[id] = n + 1;
      out.push({ key: `${id}:${n}`, ref: r.batch, info: batches.find((b) => b.id === id), rows: [r] });
    }
    return out;
  });

  let heading = $state<HTMLElement>();

  /** A control that had focus may have gone or gone disabled; never leave the keyboard user on nothing. */
  async function keepFocus() {
    await tick();
    const a = document.activeElement as (HTMLElement & { disabled?: boolean }) | null;
    if (!a || a === document.body || a.disabled) heading?.focus();
  }

  async function showMore() {
    await load(filter, rows.length + PAGE);
    await keepFocus();
  }

  async function review(scope: { kind: 'batch' | 'group'; id: string }) {
    try {
      await api.markReviewed(scope);
      await agentChanges.reload();
      await load(filter, Math.max(rows.length, PAGE));
      await keepFocus();
    } catch (e) {
      toasts.push({ text: errMessage(e), kind: 'error' });
    }
  }

  type Pending = { kind: 'group'; id: string } | { kind: 'batch'; id: string; title: string };
  let pending = $state<Pending | null>(null);
  let report = $state<UndoReport | null>(null);
  let busy = false;
  let undoError = $state('');
  let summary = $state<HTMLElement>();

  function ask(p: Pending) {
    undoError = '';
    report = null;
    pending = p;
  }

  async function confirmUndo() {
    if (busy || !pending) return;
    busy = true;
    undoError = '';
    try {
      report = pending.kind === 'group' ? await api.undoChangeGroup(pending.id) : await api.undoChangeBatch(pending.id);
      void agentChanges.reload();
      // The button just pressed has gone with the question.
      await tick();
      summary?.focus();
    } catch (e) {
      undoError = errMessage(e);
    } finally {
      busy = false;
    }
  }

  async function closeDialog() {
    pending = null;
    report = null;
    undoError = '';
    await keepFocus();
  }

  const dialogTitle = $derived(
    report ? t('changeLog.resultTitle') : pending?.kind === 'batch' ? t('changeLog.confirmBatchTitle') : t('changeLog.confirmChangeTitle'),
  );

  function openPerson(id: Id) {
    nav.focus(id);
    nav.go({ name: 'person', id });
  }

  function who(r: LogRowDto): string {
    if (r.actor.kind === 'user') return t('changeLog.you');
    return r.actor.kind === 'agent' ? r.actor.name : t('changeLog.beforeTracking');
  }

  /** The backend's word for a person with no name, in the active language. */
  const personName = (p: Pick<LogPersonDto, 'name'>) => factText({ kind: 'text', text: p.name });
</script>

{#snippet rowList(items: LogRowDto[], inBatch: boolean)}
  <ul>
    {#each items as r (r.group)}
      <li class="row" class:reverted={r.reverted}>
        <p class="meta">
          <span class="when tnum">{new Date(r.ts).toLocaleString(currentLocale())}</span>
          <span class="who">{who(r)}</span>
          <span>{editLabel(r.label)}</span>
          {#if r.reverted}<Badge>{t('changeLog.undone')}</Badge>{/if}
        </p>
        <LogChanges changes={r.changes} />
        {#if r.people.length > 0}
          <p class="people">
            {#each r.people as p (p.id)}
              {#if p.exists}
                <button type="button" class="link" onclick={() => openPerson(p.id)}><bdi dir="auto">{personName(p)}</bdi></button>
              {:else}
                <span>{t('changeLog.personGone', { name: personName(p) })}</span>
              {/if}
            {/each}
          </p>
        {/if}
        {#if r.actor.kind === 'agent'}
          <div class="actions">
            {#if !inBatch && !r.reverted && r.people.some((p) => p.reviewed === null)}
              <Button onclick={() => review({ kind: 'group', id: r.group })}>{t('changeLog.markReviewed')}</Button>
            {/if}
            <Button disabled={!r.canUndo} onclick={() => ask({ kind: 'group', id: r.group })}>{t('changeLog.undo')}</Button>
          </div>
        {/if}
      </li>
    {/each}
  </ul>
{/snippet}

<div class="page">
  <h1 tabindex="-1" bind:this={heading}>{t('changeLog.title')}</h1>
  <p class="intro">{t('changeLog.intro')}</p>

  <div class="filters">
    <Select label={t('changeLog.author')} options={authorOptions} bind:value={author} />
    <Select label={t('changeLog.batch')} options={batchOptions} bind:value={batchId} />
    <Checkbox label={t('changeLog.notReviewed')} bind:checked={onlyUnreviewed} />
  </div>

  {#if loadError}<Banner kind="error">{loadError}</Banner>{/if}

  <div class="rows" role="group" aria-label={t('changeLog.rows')}>
    {#if loaded && rows.length === 0}
      <p class="empty">{filtered ? t('changeLog.emptyFiltered') : t('changeLog.empty')}</p>
    {/if}
    {#each segments as s, i (s.key)}
      {#if s.ref}
        {@const id = `${uid}-b${i}`}
        <section class="batch" aria-labelledby={id}>
          <header>
            <h2 {id}><bdi dir="auto">{s.ref.title}</bdi></h2>
            {#if s.info}
              <p class="meta">
                <span>{t('changeLog.by', { agent: s.info.actorName })}</span>
                <span>{plural('changeLog.changes', s.info.groups)}</span>
                <span>{plural('common.people', s.info.people)}</span>
                {#if s.info.unreviewed > 0}<span>{plural('changeLog.unreviewed', s.info.unreviewed)}</span>{/if}
                {#if s.info.sourceFile}<span>{t('changeLog.sourceFile', { file: s.info.sourceFile })}</span>{/if}
                {#if s.info.ended === null}<Badge tone="warning">{t('changeLog.unfinished')}</Badge>{/if}
              </p>
              {#if s.info.report}
                <div class="report">
                  <h3>{t('changeLog.reportHeading')}</h3>
                  <p>{s.info.report}</p>
                </div>
              {/if}
              <div class="actions">
                {#if s.info.unreviewed > 0}
                  <Button onclick={() => review({ kind: 'batch', id: s.info!.id })}>{t('changeLog.markReviewed')}</Button>
                {/if}
                <Button onclick={() => ask({ kind: 'batch', id: s.info!.id, title: s.info!.title })}>{t('changeLog.undoBatch')}</Button>
              </div>
            {/if}
          </header>
          {@render rowList(s.rows, true)}
        </section>
      {:else}
        {@render rowList(s.rows, false)}
      {/if}
    {/each}
  </div>

  {#if rows.length < total}
    <div><Button onclick={showMore}>{t('changeLog.showMore')}</Button></div>
  {/if}
</div>

<Dialog open={pending !== null} title={dialogTitle} onclose={closeDialog}>
  {#if report}
    <p tabindex="-1" bind:this={summary} class="summary">
      {#if report.undone > 0 || report.skipped.length > 0}{plural('changeLog.resultUndone', report.undone)}{:else}{t('changeLog.resultNothing')}{/if}
    </p>
    {#if report.skipped.length > 0}
      <p>{t('changeLog.skippedHeading')}</p>
      <ul class="skipped">
        {#each report.skipped as s, i (i)}
          <li>
            {recordKindLabel(s.kind)}{#if s.person}, <bdi dir="auto">{personName(s.person)}</bdi>{/if}: <bdi dir="auto">{factText(s.fact)}</bdi>
          </li>
        {/each}
      </ul>
    {/if}
  {:else if pending}
    {#if undoError}<Banner kind="error">{undoError}</Banner>{/if}
    <p>{pending.kind === 'batch' ? t('changeLog.confirmBatchBody', { title: pending.title }) : t('changeLog.confirmChangeBody')}</p>
  {/if}
  {#snippet footer()}
    {#if report}
      <Button variant="primary" onclick={closeDialog}>{t('changeLog.done')}</Button>
    {:else}
      <Button onclick={closeDialog}>{t('changeLog.keep')}</Button>
      <Button variant="danger" onclick={confirmUndo}>{t('changeLog.undo')}</Button>
    {/if}
  {/snippet}
</Dialog>

<style>
  .page {
    display: grid;
    gap: var(--s-5);
    padding: var(--s-6);
    max-width: 70rem;
  }
  h1 {
    margin: 0;
    outline: none;
  }
  .intro,
  .empty {
    margin: 0;
    color: var(--ink-1);
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: var(--s-4);
  }
  .rows {
    display: grid;
    gap: var(--s-5);
  }
  .batch {
    display: grid;
    gap: var(--s-3);
    padding: var(--s-4);
    border: 1px solid var(--rule);
    border-radius: 4px;
  }
  header {
    display: grid;
    gap: var(--s-2);
  }
  h2 {
    margin: 0;
    font-size: var(--t-md, 1rem);
  }
  h3 {
    margin: 0;
    font-size: var(--t-sm);
    color: var(--ink-1);
  }
  .report p {
    margin: 0;
    white-space: pre-wrap;
  }
  ul {
    display: grid;
    gap: var(--s-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .row {
    display: grid;
    gap: var(--s-1);
    padding-block-end: var(--s-3);
    border-block-end: 1px solid var(--rule);
  }
  .reverted {
    color: var(--ink-1);
  }
  .meta,
  .people {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-1) var(--s-3);
    margin: 0;
  }
  .meta {
    font-size: var(--t-sm);
  }
  .when {
    color: var(--ink-1);
  }
  .who {
    font-weight: 600;
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-decoration: underline;
    cursor: pointer;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
  .summary {
    outline: none;
    font-weight: 500;
  }
  .skipped {
    gap: var(--s-2);
  }
</style>
