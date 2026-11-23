<script lang="ts">
  // The events of one place and the places under it: a chronological list of what happened there, from
  // people in the family tree and from place records about others (church books, confession lists...).
  import { untrack } from 'svelte';
  import PagedList from '../ui/PagedList.svelte';
  import Select from '../ui/Select.svelte';
  import TextField from '../ui/TextField.svelte';
  import Button from '../ui/Button.svelte';
  import Badge from '../ui/Badge.svelte';
  import Banner from '../ui/Banner.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import { getApi } from '../api/context';
  import { project } from '../state/project.svelte';
  import type { Id, PlaceEventDto, PlaceEventsPage, PlaceEventsWho, PlaceOption } from '../api/types';
  import { t, plural, type MessageKey } from '../i18n';

  let { place, onOpenPerson, refresh = 0 }: { place: PlaceOption; onOpenPerson: (id: Id) => void; refresh?: number } = $props();
  const api = getApi();

  const KINDS = ['birth', 'marriage', 'death', 'confession', 'revision', 'census', 'other'] as const;
  const PAGE = 200;

  let who = $state<PlaceEventsWho>('all');
  let kind = $state('');
  let yearFrom = $state('');
  let yearTo = $state('');
  let query = $state('');
  let page = $state<PlaceEventsPage | null>(null);
  let failed = $state(false);
  /** Bumped whenever the first page is read again, so the list starts over with it. */
  let epoch = $state(0);
  let seq = 0;

  const kindLabel = (k: string) => (KINDS as readonly string[]).includes(k) ? t(`places.kinds.${k}` as MessageKey) : k;
  const whoOptions = $derived([
    { value: 'all', label: t('places.whoAll') },
    { value: 'ours', label: t('places.whoOurs') },
    { value: 'others', label: t('places.whoOthers') },
  ]);
  const kindOptions = $derived([{ value: '', label: t('places.allKinds') }, ...KINDS.map((k) => ({ value: k, label: kindLabel(k) }))]);
  const filtered = $derived(who !== 'all' || kind !== '' || yearFrom !== '' || yearTo !== '' || query.trim() !== '');

  function year(s: string): number | undefined {
    const n = Number.parseInt(s, 10);
    return Number.isFinite(n) ? n : undefined;
  }
  function queryFor(offset: number, limit: number) {
    return {
      place: place.id,
      kinds: kind ? [kind] : undefined,
      yearFrom: year(yearFrom),
      yearTo: year(yearTo),
      who,
      query: query.trim() || undefined,
      offset,
      limit,
    };
  }

  async function loadFirst() {
    const mine = ++seq;
    try {
      const r = await api.placeEvents(queryFor(0, PAGE));
      if (mine !== seq) return;
      page = r;
      failed = false;
      epoch++;
    } catch {
      if (mine === seq) failed = true;
    }
  }

  // Any filter, another place, an import or an edit elsewhere reads the first page again.
  $effect(() => {
    void [place.id, who, kind, yearFrom, yearTo, query, refresh, project.rev];
    untrack(() => void loadFirst());
  });

  async function loadPage(offset: number, limit: number): Promise<unknown[]> {
    return (await api.placeEvents(queryFor(offset, limit))).items;
  }

  const maxDecade = $derived(Math.max(1, ...(page?.byDecade ?? []).map((d) => d.count)));
  const chosenDecade = $derived(yearFrom !== '' && yearFrom === String(year(yearFrom)) && year(yearTo) === (year(yearFrom) ?? 0) + 9 ? year(yearFrom) : undefined);
  function pickDecade(decade: number) {
    if (chosenDecade === decade) {
      yearFrom = '';
      yearTo = '';
    } else {
      yearFrom = String(decade);
      yearTo = String(decade + 9);
    }
  }
  function pickKind(k: string) {
    kind = kind === k ? '' : k;
  }
  function clear() {
    who = 'all';
    kind = '';
    yearFrom = '';
    yearTo = '';
    query = '';
  }
  const asEvent = (item: unknown) => item as PlaceEventDto | null;
  const names = (e: PlaceEventDto) => e.people.filter((p) => p.name || p.asWritten);
</script>

<section class="events" aria-labelledby="pe-title">
  <h2 id="pe-title">{t('places.eventsTitle', { place: place.full })}</h2>
  <p class="note">{t('places.eventsNote')}</p>

  <div class="filters">
    <Segmented legend={t('places.whoLabel')} options={whoOptions} value={who} onchange={(v) => (who = v as PlaceEventsWho)} />
    <Select label={t('places.kindLabel')} options={kindOptions} bind:value={kind} />
    <TextField label={t('places.yearFrom')} inputmode="numeric" bind:value={yearFrom} />
    <TextField label={t('places.yearTo')} inputmode="numeric" bind:value={yearTo} />
    <TextField label={t('places.name')} placeholder={t('places.nameHint')} bind:value={query} data-filter-input />
    {#if filtered}<Button onclick={clear}>{t('places.clear')}</Button>{/if}
  </div>

  {#if failed}
    <Banner kind="error">{t('places.loadFailed')}</Banner>
  {/if}

  {#if page && page.overviewTotal > 0}
    <div class="overview" aria-label={t('places.overview')}>
      <ul class="kinds">
        {#each KINDS as k (k)}
          {@const n = page.byKind.find((x) => x.kind === k)?.count ?? 0}
          {#if n > 0}
            <li>
              <button type="button" class="chip" class:on={kind === k} aria-pressed={kind === k} onclick={() => pickKind(k)}>
                {kindLabel(k)} <b>{n}</b>
              </button>
            </li>
          {/if}
        {/each}
      </ul>
      {#if page.byDecade.length > 0}
        <ul class="decades" aria-label={t('places.byDecade')}>
          {#each page.byDecade as d (d.decade)}
            <li>
              <button
                type="button"
                class="bar"
                class:on={chosenDecade === d.decade}
                aria-pressed={chosenDecade === d.decade}
                title={t('places.decadeTitle', { decade: d.decade, n: d.count })}
                onclick={() => pickDecade(d.decade)}
              >
                <span class="col" style:height="{Math.max(6, Math.round((d.count / maxDecade) * 100))}%"></span>
                <span class="lab">{d.decade}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}

  {#if page}
    <p class="count" aria-live="polite">
      {plural('places.total', page.total)}{#if filtered && page.total !== page.overviewTotal} · {t('places.shown', { shown: page.total, total: page.overviewTotal })}{/if}
    </p>
    {#if page.total === 0}
      <EmptyState title={page.overviewTotal === 0 ? t('places.noEvents') : t('places.noMatch')} />
    {:else}
      <div class="list">
        {#key epoch}
          <PagedList
            total={page.total}
            rowHeight={64}
            pageSize={PAGE}
            {loadPage}
            initialPage={{ offset: 0, items: page.items }}
            selectedIndex={-1}
            onSelect={() => {}}
            onActivate={() => {}}
            ariaLabel={t('places.list')}
            keyPrefix="place-events"
          >
            {#snippet row(item)}
              {@const e = asEvent(item)}
              {#if e}
                <div class="ev" class:ours={e.ours} data-source={e.source}>
                  <div class="when">
                    <span class="year">{e.date || (e.year ?? t('places.undated'))}</span>
                    <Badge tone={e.ours ? 'ribbon' : 'neutral'}>{kindLabel(e.kind)}</Badge>
                  </div>
                  <div class="who">
                    {#each names(e).slice(0, 6) as p, i (i)}
                      {#if p.person}
                        <button type="button" class="rel" title={t('places.openPerson', { name: p.name })} onclick={() => onOpenPerson(p.person!)}>{p.name}</button>
                      {:else}
                        <span class="other" title={p.asWritten}>{p.name || p.asWritten}</span>
                      {/if}
                      {#if p.role}<span class="role">{p.role}{p.age ? `, ${p.age}` : ''}</span>{/if}
                    {/each}
                    {#if names(e).length > 6}<span class="role">+{names(e).length - 6}</span>{/if}
                  </div>
                  <div class="src">
                    {#if e.book}
                      <span>{t('places.sourceBook', { book: e.book })}{e.page ? `, ${t('places.sourcePage', { page: e.page })}` : ''}</span>
                    {/if}
                    {#if e.unsure}<span class="unsure" title={t('places.unsureTitle')}>? {t('places.unsure')}</span>{/if}
                  </div>
                </div>
              {/if}
            {/snippet}
          </PagedList>
        {/key}
      </div>
    {/if}
  {/if}
</section>

<style>
  .events {
    display: flex;
    flex-direction: column;
    gap: var(--s-4);
    min-height: 0;
  }
  h2 {
    margin: 0;
  }
  .note {
    margin: 0;
    color: var(--ink-3);
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-4);
    align-items: flex-end;
  }
  .overview {
    display: flex;
    flex-direction: column;
    gap: var(--s-3);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .kinds {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
  .chip {
    border: 1px solid var(--rule);
    border-radius: 999px;
    background: var(--paper-1);
    padding: 0 var(--s-4);
    min-height: 2rem;
    cursor: pointer;
    color: inherit;
  }
  .chip.on {
    border-color: var(--ribbon);
    background: var(--paper-2);
  }
  .decades {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 5rem;
    overflow-x: auto;
  }
  .decades li {
    height: 100%;
  }
  .bar {
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    align-items: center;
    height: 100%;
    min-width: 2.4rem;
    border: 0;
    background: transparent;
    cursor: pointer;
    color: inherit;
    padding: 0;
  }
  .bar .col {
    width: 70%;
    background: var(--rule);
    border-radius: 2px 2px 0 0;
  }
  .bar.on .col,
  .bar:hover .col {
    background: var(--ribbon);
  }
  .lab {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .count {
    margin: 0;
    color: var(--ink-3);
  }
  .list {
    height: 28rem;
    min-height: 12rem;
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    overflow: hidden;
  }
  .ev {
    display: grid;
    grid-template-columns: 9rem 1fr auto;
    gap: var(--s-4);
    align-items: center;
    height: 100%;
    width: 100%;
    box-sizing: border-box;
    padding: 0 var(--s-4);
    border-bottom: 1px solid var(--rule);
    overflow: hidden;
  }
  .ev.ours {
    background: var(--paper-2);
    border-left: 3px solid var(--ribbon);
  }
  .when {
    display: flex;
    flex-direction: column;
    gap: var(--s-1);
    align-items: flex-start;
  }
  .year {
    font-variant-numeric: tabular-nums;
    font-weight: 500;
  }
  .who {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-1) var(--s-3);
    align-items: baseline;
    max-height: 100%;
    overflow: hidden;
  }
  .rel {
    border: 0;
    background: none;
    padding: 0;
    font: inherit;
    font-weight: 600;
    color: var(--ribbon);
    text-decoration: underline;
    cursor: pointer;
  }
  .role {
    color: var(--ink-3);
    font-size: var(--t-xs);
  }
  .src {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    color: var(--ink-3);
    font-size: var(--t-xs);
  }
  .unsure {
    color: var(--warn, #a05a00);
    font-weight: 600;
  }
</style>
