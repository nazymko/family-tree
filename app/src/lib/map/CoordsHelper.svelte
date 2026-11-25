<script lang="ts">
  // "Place coordinates": the places of the selection that have no point on the map, with suggestions from the offline
  // gazetteer. The only network use is the download the person asks for here. Nothing is saved without a click.
  import { onMount } from 'svelte';
  import Button from '../ui/Button.svelte';
  import ProgressBar from '../ui/ProgressBar.svelte';
  import { getApi } from '../api/context';
  import { notices } from '../ui/notices.svelte';
  import { currentTag, plural, t } from '../i18n';
  import type { GazetteerCandidateDto, GazetteerProgressDto, GazetteerStatusDto, Id } from '../api/types';
  import type { Missing } from './filters';

  let {
    missing,
    picking,
    onPick,
    onCancelPick,
    onSaved,
    onClose,
  }: {
    missing: Missing;
    /** The place the person is about to click on the map, or null. */
    picking: { id: Id; label: string } | null;
    onPick: (place: { id: Id; label: string }) => void;
    onCancelPick: () => void;
    onSaved: () => void;
    onClose: () => void;
  } = $props();

  const api = getApi();
  const COUNTRIES = ['UA', 'PL', 'BY', 'MD', 'RO', 'SK', 'CZ', 'HU', 'LT', 'LV', 'EE', 'DE', 'AT', 'RU', 'US', 'CA'];

  let status = $state<GazetteerStatusDto | null>(null);
  let country = $state('UA');
  let size = $state<number | null | undefined>(undefined);
  let progress = $state<GazetteerProgressDto | null>(null);
  let busy = $state(false);
  let error = $state('');
  let chosen = $state<Id | null>(null);
  let candidates = $state<GazetteerCandidateDto[]>([]);
  let asking = 0;

  const names = $derived.by(() => {
    try {
      return new Intl.DisplayNames([currentTag()], { type: 'region' });
    } catch {
      return null;
    }
  });
  const countryName = (cc: string) => names?.of(cc) ?? cc;
  const mb = (n: number) => `${(n / 1048576).toFixed(1)} MB`;
  const have = $derived(new Set(status?.countries.map((c) => c.country) ?? []));
  const place = $derived(missing.places.find((p) => p.place.id === chosen)?.place ?? null);

  async function refresh() {
    status = await api.gazetteerStatus();
  }

  onMount(() => {
    void refresh();
  });

  // The size is asked of GeoNames only when a country that is not on this computer is chosen.
  $effect(() => {
    const cc = country.trim().toUpperCase();
    size = undefined;
    if (cc.length !== 2 || have.has(cc)) return;
    let live = true;
    api.gazetteerRemoteSize(cc).then(
      (n) => live && (size = n),
      () => live && (size = null),
    );
    return () => {
      live = false;
    };
  });

  // The first place is chosen for the person; suggestions follow the chosen place.
  $effect(() => {
    if (!missing.places.some((p) => p.place.id === chosen)) chosen = missing.places[0]?.place.id ?? null;
  });
  $effect(() => {
    const id = chosen;
    void status; // new place data
    const mine = ++asking;
    candidates = [];
    if (!id || !status || status.countries.length === 0) return;
    void api.gazetteerSuggest(id, 8).then((c) => {
      if (mine === asking) candidates = c;
    });
  });

  async function download() {
    const cc = country.trim().toUpperCase();
    busy = true;
    error = '';
    progress = null;
    try {
      await api.gazetteerDownload(cc, (p) => (progress = p));
      await refresh();
    } catch (e) {
      error = (e as { message?: string })?.message || t('map.helper.downloadFailed');
    } finally {
      busy = false;
      progress = null;
    }
  }

  async function remove(cc: string) {
    await api.gazetteerRemove(cc);
    await refresh();
  }

  async function use(c: GazetteerCandidateDto) {
    if (!place) return;
    await api.apply({ type: 'setPlaceCoordinates', place: place.id, latE6: Math.round(c.lat * 1e6), lonE6: Math.round(c.lon * 1e6) });
    notices.show({ message: t('map.helper.saved', { place: place.label }) });
    onSaved();
  }

  const shown = (c: GazetteerCandidateDto) => (currentTag().startsWith('uk') && c.local ? c.local : c.name);
  const region = (c: GazetteerCandidateDto) => (currentTag().startsWith('uk') && c.regionLocal ? c.regionLocal : c.region);
</script>

<aside class="helper" aria-label={t('map.helper.title')}>
  <header>
    <h2>{t('map.helper.title')}</h2>
    <Button onclick={onClose} aria-label={t('map.helper.close')}>×</Button>
  </header>
  <p class="intro">{t('map.helper.intro')}</p>

  <section class="data">
    {#if status && status.countries.length > 0}
      <h3>{t('map.helper.onThisComputer')}</h3>
      <ul class="have">
        {#each status.countries as c (c.country)}
          <li>
            <span>{countryName(c.country)} ({c.country})</span>
            <span class="dim">{plural('map.helper.rows', c.rows)} · {mb(c.bytes)}</span>
            <Button onclick={() => remove(c.country)}>{t('map.helper.remove')}</Button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="dim">{t('map.helper.noData')}</p>
    {/if}
    <div class="get">
      <label>
        <span>{t('map.helper.countryCode')}</span>
        <input list="map-countries" maxlength="2" bind:value={country} disabled={busy} autocapitalize="characters" />
      </label>
      <datalist id="map-countries">
        {#each COUNTRIES as cc (cc)}<option value={cc}>{countryName(cc)}</option>{/each}
      </datalist>
      <Button variant="secondary" onclick={download} disabled={busy || country.trim().length !== 2 || have.has(country.trim().toUpperCase())}>
        {t('map.helper.download')}
      </Button>
    </div>
    <p class="dim">
      {#if typeof size === 'number'}{t('map.helper.downloadSize', { size: mb(size) })}{:else if size === undefined}&nbsp;{:else}{t('map.helper.sizeUnknown')}{/if}
    </p>
    {#if busy}
      <p>{progress?.phase === 'index' ? t('map.helper.indexing', { country: country.toUpperCase() }) : t('map.helper.downloading', { country: country.toUpperCase() })}</p>
      <ProgressBar label={t('map.helper.download')} value={progress?.done ?? 0} max={progress?.total || 1} />
    {/if}
    {#if error}<p class="err" role="alert">{error}</p>{/if}
  </section>

  <section class="places">
    <h3>{t('map.helper.places')}</h3>
    {#if missing.places.length === 0}
      <p class="dim">{t('map.helper.allSet')}</p>
    {:else}
      <ul class="plist">
        {#each missing.places as m (m.place.id)}
          <li>
            <button type="button" class:on={chosen === m.place.id} aria-pressed={chosen === m.place.id} onclick={() => (chosen = m.place.id)}>
              <span>{m.place.label}</span>
              <span class="dim">{plural('map.helper.eventsAt', m.events)}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  {#if place}
    <section class="suggest">
      <h3>{t('map.helper.suggestions')}</h3>
      {#if picking}
        <p>{t('map.helper.picking', { place: picking.label })}</p>
        <Button onclick={onCancelPick}>{t('map.helper.cancel')}</Button>
      {:else}
        {#if status && status.countries.length > 0}
          {#if candidates.length === 0}
            <p class="dim">{t('map.helper.noSuggestions')}</p>
          {:else}
            <ul class="cands">
              {#each candidates as c, i (c.geonameId)}
                <li>
                  <div>
                    <strong>{shown(c)}</strong>
                    {#if region(c)}<span class="dim"> · {region(c)}</span>{/if}
                    <div class="dim small">
                      {c.code}{#if c.population > 0} · {t('map.helper.population', { n: c.population.toLocaleString(currentTag()) })}{/if}{#if c.regionMatch} · {t('map.helper.regionMatches')}{/if}
                    </div>
                  </div>
                  <Button variant={i === 0 ? 'primary' : 'secondary'} onclick={() => use(c)}>{t('map.helper.useThis')}</Button>
                </li>
              {/each}
            </ul>
          {/if}
        {/if}
        <Button onclick={() => onPick({ id: place.id, label: place.label })}>{t('map.helper.clickMap')}</Button>
      {/if}
    </section>
  {:else if missing.places.length > 0}
    <p class="dim">{t('map.helper.choose')}</p>
  {/if}

  <footer class="dim small">{t('map.helper.attribution')}</footer>
</aside>

<style>
  .helper {
    display: flex;
    flex-direction: column;
    gap: var(--s-4);
    padding: var(--s-4);
    overflow: auto;
    background: var(--paper-1);
    border-left: 1px solid var(--rule);
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  h2,
  h3 {
    margin: 0;
    font-size: var(--t-md);
  }
  h3 {
    font-size: var(--t-base);
  }
  .intro,
  p {
    margin: 0;
  }
  .dim {
    color: var(--ink-2);
  }
  .small {
    font-size: var(--t-xs);
  }
  .err {
    color: var(--danger);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .have li,
  .cands li {
    display: flex;
    gap: var(--s-3);
    align-items: center;
    justify-content: space-between;
    padding: var(--s-2) 0;
    border-bottom: 1px solid var(--paper-2);
  }
  .get {
    display: flex;
    gap: var(--s-3);
    align-items: end;
    margin-top: var(--s-3);
  }
  .get label {
    display: grid;
    gap: var(--s-1);
    font-size: var(--t-sm);
  }
  .get input {
    width: 5rem;
    min-height: 2.25rem;
    padding: 0 var(--s-3);
    text-transform: uppercase;
    border: 1px solid var(--ink-2);
    background: var(--paper-1);
    color: var(--ink-0);
    border-radius: var(--r-1);
  }
  .plist {
    max-height: 14rem;
    overflow: auto;
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
  }
  .plist button {
    display: flex;
    width: 100%;
    justify-content: space-between;
    gap: var(--s-3);
    padding: var(--s-2) var(--s-3);
    border: 0;
    background: transparent;
    color: var(--ink-0);
    text-align: start;
    cursor: pointer;
    font: inherit;
  }
  .plist button.on {
    background: var(--paper-2);
    font-weight: 600;
  }
  footer {
    margin-top: auto;
  }
</style>
