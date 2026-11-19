<script lang="ts">
  // "Old maps": scanned maps laid under the places. Add one (with its OziExplorer .map file when there is one), choose
  // which are shown (by series, one alone, all, or the ones under the middle of the view), opacity, order, place by
  // corners, edit, remove from the map (the picture stays in Media).
  import Button from '../ui/Button.svelte';
  import { getApi, getPlatform } from '../api/context';
  import { notices } from '../ui/notices.svelte';
  import { t } from '../i18n';
  import type { Id, MapCorner, MapLayerDto, MapLayerInput } from '../api/types';
  import { parseDegrees, reordered } from './layers';
  import { groupLayers, isNoChange, layersUnder, onlyThis, setAll, toggleGroup, triState, type LayerGroup, type VisibilityChange } from './layerGroups';

  let {
    layers,
    centre = null,
    onOpacity,
    onPlace,
    onAdded,
    onChanged,
    onClose,
  }: {
    /** Lowest first, as drawn. */
    layers: MapLayerDto[];
    /** The middle of the view, in degrees: the maps under it are offered first. */
    centre?: { lat: number; lon: number } | null;
    /** While the slider moves: the opacity to draw with, not saved yet. */
    onOpacity: (media: Id, opacity: number | null) => void;
    onPlace: (layer: MapLayerDto) => void;
    /** A map was added; `guess` holds corners to start placing from when its .map file did not fit the picture. */
    onAdded: (media: Id, placed: boolean, guess?: MapCorner[]) => void;
    onChanged: () => void;
    onClose: () => void;
  } = $props();

  const api = getApi();
  const platform = getPlatform();
  const PICTURES = ['jpg', 'jpeg', 'png', 'webp', 'gif', 'tif', 'tiff'];
  /** With more maps than this the series start folded, so the list stays short. */
  const OPEN_UP_TO = 12;

  const name = (l: MapLayerDto) => l.title?.trim() || t('map.layers.untitled');
  const top = $derived([...layers].reverse());
  const index = $derived(new Map(top.map((l, i) => [l.media, i])));

  let busy = $state(false);
  let error = $state('');
  let filter = $state('');
  const groups = $derived(groupLayers(top, filter));
  const under = $derived(centre && layers.length > 1 && !filter.trim() ? layersUnder(top, centre.lat, centre.lon) : []);
  /** Series the owner opened or folded; the others follow OPEN_UP_TO. */
  let opened = $state<Record<string, boolean>>({});
  /** With many maps a row shows only its box and «Only this»; these are the rows opened for the rest. */
  let details = $state<Record<Id, boolean>>({});
  const isOpen = (g: LayerGroup) => !!filter.trim() || (opened[g.key] ?? layers.length <= OPEN_UP_TO);
  const groupName = (g: LayerGroup) => g.series ?? t('map.layers.noSeries');

  interface Form {
    media: Id | null;
    path: string | null;
    title: string;
    series: string;
    year: string;
    source: string;
    ozi: string | null;
    corners: { lat: string; lon: string }[];
  }
  let form = $state<Form | null>(null);

  const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const deg = (v: number) => (v / 1e6).toFixed(6).replace(/\.?0+$/, '');
  const cornerFields = (c: MapCorner[] | null) => (c ?? []).map((p) => ({ lat: deg(p.latE6), lon: deg(p.lonE6) })).concat([0, 1, 2, 3].map(() => ({ lat: '', lon: '' }))).slice(0, 4);

  function input(l: MapLayerDto, patch: Partial<MapLayerInput> = {}): MapLayerInput {
    return { series: l.series ?? undefined, year: l.year ?? undefined, source: l.source ?? undefined, corners: l.corners ?? undefined, opacity: l.opacity, visible: l.visible, ...patch };
  }

  async function save(media: Id, layer: MapLayerInput | undefined, title?: string) {
    busy = true;
    error = '';
    try {
      await api.apply({ type: 'setMapLayer', media, ...(layer ? { layer: $state.snapshot(layer) } : {}), ...(title !== undefined ? { title } : {}) });
      onChanged();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }

  /** Shows and hides several maps as one change (one step to undo, one line in the change log) and says so once. */
  async function choose(change: VisibilityChange, message: string) {
    if (isNoChange(change)) return;
    busy = true;
    error = '';
    try {
      await api.apply({ type: 'setMapLayersVisible', shown: change.shown, hidden: change.hidden });
      notices.show({ message });
      onChanged();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }

  function toggleSeries(g: LayerGroup) {
    const change = toggleGroup(g.layers);
    const shown = change.shown.length > 0;
    void choose(change, t(shown ? 'map.layers.seriesShown' : 'map.layers.seriesHidden', { series: groupName(g), n: g.layers.length }));
  }

  function showOnly(l: MapLayerDto) {
    void choose(onlyThis(layers, l.media), t('map.layers.onlyShown', { title: name(l) }));
  }

  function everything(visible: boolean) {
    const change = setAll(layers, visible);
    const n = change.shown.length + change.hidden.length;
    void choose(change, t(visible ? 'map.layers.allShown' : 'map.layers.allHidden', { n }));
  }

  /** Sets a group checkbox's third state, which has no attribute of its own. */
  function tri(node: HTMLInputElement, state: 'all' | 'none' | 'some') {
    const set = (s: typeof state) => {
      node.indeterminate = s === 'some';
      node.checked = s === 'all';
    };
    set(state);
    return { update: set };
  }

  async function add() {
    const path = await platform.pickFile({ title: t('map.layers.pickTitle'), extensions: PICTURES });
    if (!path) return;
    const ozi = await api.findOziMap(path).catch(() => null);
    form = { media: null, path, title: fileName(path).replace(/\.[^.]+$/, ''), series: '', year: '', source: '', ozi, corners: cornerFields(null) };
  }

  async function chooseOzi() {
    const path = await platform.pickFile({ title: t('map.layers.pickOzi'), extensions: ['map'] });
    if (!path || !form) return;
    if (form.media) {
      try {
        const o = await api.oziCorners(path, form.media);
        form.corners = cornerFields(o.corners);
        error = o.fits ? '' : t('map.layers.oziMismatchEdit');
      } catch (e) {
        error = (e as Error).message;
      }
    } else form.ozi = path;
  }

  function edit(l: MapLayerDto) {
    form = { media: l.media, path: null, title: l.title ?? '', series: l.series ?? '', year: l.year ?? '', source: l.source ?? '', ozi: null, corners: cornerFields(l.corners) };
    error = '';
  }

  /** The typed corners: none when every field is empty, null when something is wrong. */
  function typedCorners(f: Form): MapCorner[] | undefined | null {
    if (f.corners.every((c) => !c.lat.trim() && !c.lon.trim())) return undefined;
    const out: MapCorner[] = [];
    for (const c of f.corners) {
      const lat = parseDegrees(c.lat, 90);
      const lon = parseDegrees(c.lon, 180);
      if (lat === null || lon === null) return null;
      out.push({ latE6: lat, lonE6: lon });
    }
    return out;
  }

  async function submit(e: Event) {
    e.preventDefault();
    const f = form;
    if (!f) return;
    const text = (s: string) => s.trim() || undefined;
    if (f.media === null && f.path) {
      busy = true;
      error = '';
      try {
        // A .map file made for another framing of the picture would put it kilometres off: the owner corrects its
        // corners instead of the map landing there silently.
        const check = f.ozi ? await api.oziCheck(f.ozi, f.path) : null;
        const ozi = check?.fits ? f.ozi : null;
        const id = await api.addMapLayerFile(f.path, f.title.trim() || null, { series: text(f.series), year: text(f.year), source: text(f.source), opacity: 70, visible: true }, ozi);
        const title = f.title.trim() || t('map.layers.untitled');
        if (check && !check.fits) notices.show({ message: t('map.layers.oziMismatch', { title }) });
        else notices.show({ message: t('map.layers.added', { title }) });
        form = null;
        onAdded(id, !!ozi, check && !check.fits ? check.corners : undefined);
      } catch (err) {
        error = (err as Error).message || t('map.layers.failed');
      } finally {
        busy = false;
      }
      return;
    }
    const l = layers.find((x) => x.media === f.media);
    if (!l) return;
    const corners = typedCorners(f);
    if (corners === null) {
      error = t('map.layers.badCorner');
      return;
    }
    await save(l.media, input(l, { series: text(f.series), year: text(f.year), source: text(f.source), corners: corners ?? undefined }), f.title);
    if (!error) {
      notices.show({ message: t('map.layers.saved', { title: f.title.trim() || t('map.layers.untitled') }) });
      form = null;
    }
  }

  async function move(l: MapLayerDto, by: 1 | -1) {
    const order = reordered(
      layers.map((x) => x.media),
      l.media,
      by,
    );
    busy = true;
    try {
      await api.apply({ type: 'reorderMapLayers', order });
      onChanged();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }

  async function remove(l: MapLayerDto) {
    await save(l.media, undefined);
    if (!error) notices.show({ message: t('map.layers.removed', { title: name(l) }) });
  }
</script>

{#snippet visibleBox(l: MapLayerDto, here = false)}
  <label class="name">
    <input type="checkbox" checked={l.visible} disabled={busy} aria-label={t(here ? 'map.layers.underShown' : 'map.layers.shown', { title: name(l) })} onchange={(e) => save(l.media, input(l, { visible: e.currentTarget.checked }))} />
    <span>{name(l)}</span>
  </label>
{/snippet}

{#snippet onlyButton(l: MapLayerDto, here = false)}
  <Button onclick={() => showOnly(l)} disabled={busy} aria-label={t(here ? 'map.layers.underOnly' : 'map.layers.onlyThisFor', { title: name(l) })}>{t('map.layers.onlyThis')}</Button>
{/snippet}

<aside class="side" aria-label={t('map.layers.title')}>
  <header>
    <h2>{t('map.layers.title')}</h2>
    <Button onclick={onClose} aria-label={t('map.layers.close')}>×</Button>
  </header>
  <p class="dim small">{t('map.layers.intro')}</p>

  {#if form}
    <form class="form" onsubmit={submit}>
      <label>{t('map.layers.mapTitle')} <input bind:value={form.title} /></label>
      <label>{t('map.layers.series')} <input bind:value={form.series} /></label>
      <label>{t('map.layers.year')} <input bind:value={form.year} /></label>
      <label>{t('map.layers.source')} <input bind:value={form.source} /></label>
      {#if form.media === null}
        <p class="small">
          {#if form.ozi}{t('map.layers.ozi', { file: fileName(form.ozi) })}{:else}{t('map.layers.oziNone')}{/if}
        </p>
        <div class="row">
          <Button onclick={chooseOzi}>{t('map.layers.oziChoose')}</Button>
          {#if form.ozi}<Button onclick={() => form && (form.ozi = null)}>{t('map.layers.oziClear')}</Button>{/if}
        </div>
      {:else}
        <fieldset>
          <legend>{t('map.layers.corners')}</legend>
          {#each form.corners as c, i (i)}
            <div class="corner">
              <span>{t((['map.layers.tl', 'map.layers.tr', 'map.layers.br', 'map.layers.bl'] as const)[i])}</span>
              <input aria-label="{t((['map.layers.tl', 'map.layers.tr', 'map.layers.br', 'map.layers.bl'] as const)[i])}: {t('map.layers.lat')}" placeholder={t('map.layers.lat')} inputmode="decimal" bind:value={c.lat} />
              <input aria-label="{t((['map.layers.tl', 'map.layers.tr', 'map.layers.br', 'map.layers.bl'] as const)[i])}: {t('map.layers.lon')}" placeholder={t('map.layers.lon')} inputmode="decimal" bind:value={c.lon} />
            </div>
          {/each}
          <Button onclick={chooseOzi}>{t('map.layers.fromOzi')}</Button>
        </fieldset>
      {/if}
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <div class="row">
        <Button type="submit" variant="primary" disabled={busy}>{form.media === null ? t('map.layers.addIt') : t('map.layers.save')}</Button>
        <Button onclick={() => ((form = null), (error = ''))}>{t('map.layers.cancel')}</Button>
      </div>
    </form>
  {:else}
    <Button variant="secondary" onclick={add} disabled={busy}>{t('map.layers.add')}</Button>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    {#if layers.length === 0}
      <p class="dim">{t('map.layers.none')}</p>
    {:else}
      {#if layers.length > 1}
        <div class="row">
          <Button onclick={() => everything(true)} disabled={busy || layers.every((l) => l.visible)}>{t('map.layers.showAll')}</Button>
          <Button onclick={() => everything(false)} disabled={busy || layers.every((l) => !l.visible)}>{t('map.layers.hideAll')}</Button>
          <span class="dim small">{t('map.layers.shownCount', { shown: layers.filter((l) => l.visible).length, n: layers.length })}</span>
        </div>
        <input class="filter" type="search" bind:value={filter} placeholder={t('map.layers.filter')} aria-label={t('map.layers.filter')} />
      {/if}
      {#if under.length > 0}
        <section class="under" aria-label={t('map.layers.underCentre')}>
          <h3 class="small">{t('map.layers.underCentre')}</h3>
          <ul>
            {#each under as l (l.media)}
              <li class="compact">
                {@render visibleBox(l, true)}
                {@render onlyButton(l, true)}
              </li>
            {/each}
          </ul>
        </section>
      {/if}
      {#if groups.length === 0}
        <p class="dim">{t('map.layers.filterNone')}</p>
      {/if}
      {#each groups as g (g.key)}
        {@const state = triState(g.layers)}
        <section class="group" aria-label={groupName(g)}>
          <div class="ghead">
            <input type="checkbox" use:tri={state} disabled={busy} aria-label={t('map.layers.seriesShow', { series: groupName(g) })} onclick={(e) => (e.preventDefault(), toggleSeries(g))} />
            <button type="button" class="gname" aria-expanded={isOpen(g)} onclick={() => (opened = { ...opened, [g.key]: !isOpen(g) })}>
              <span aria-hidden="true">{isOpen(g) ? '▾' : '▸'}</span>
              <span>{groupName(g)}</span>
            </button>
            <span class="dim small count">{t('map.layers.shownCount', { shown: g.shown, n: g.layers.length })}</span>
          </div>
          {#if isOpen(g)}
            <ul>
              {#each g.layers as l (l.media)}
                {@const i = index.get(l.media) ?? 0}
                {@const more = layers.length <= OPEN_UP_TO || !!details[l.media]}
                <li>
                  <div class="line">
                    {@render visibleBox(l)}
                    {#if layers.length > OPEN_UP_TO}
                      {@render onlyButton(l)}
                      <Button onclick={() => (details = { ...details, [l.media]: !more })} aria-expanded={more} aria-label={t('map.layers.moreFor', { title: name(l) })}>⋯</Button>
                    {/if}
                  </div>
                  <div class="dim small">
                    {[l.year].filter(Boolean).join(', ')}{#if !l.corners}{l.year ? ' · ' : ''}{t('map.layers.notPlaced')}{/if}
                  </div>
                  {#if more}
                  <div class="row">
                    <input
                      type="range"
                      min="0"
                      max="100"
                      step="5"
                      value={l.opacity}
                      aria-label={t('map.layers.opacity', { title: name(l) })}
                      oninput={(e) => onOpacity(l.media, Number(e.currentTarget.value))}
                      onchange={async (e) => {
                        await save(l.media, input(l, { opacity: Number(e.currentTarget.value) }));
                        onOpacity(l.media, null);
                      }}
                    />
                    <output>{l.opacity}%</output>
                  </div>
                  <div class="row">
                    {#if layers.length > 1 && layers.length <= OPEN_UP_TO}{@render onlyButton(l)}{/if}
                    <Button onclick={() => move(l, 1)} disabled={busy || i === 0} aria-label={t('map.layers.up', { title: name(l) })}>↑</Button>
                    <Button onclick={() => move(l, -1)} disabled={busy || i === top.length - 1} aria-label={t('map.layers.down', { title: name(l) })}>↓</Button>
                    <Button onclick={() => onPlace(l)}>{t('map.layers.place')}</Button>
                    <Button onclick={() => edit(l)}>{t('map.layers.edit')}</Button>
                    <Button onclick={() => remove(l)} disabled={busy}>{t('map.layers.remove')}</Button>
                  </div>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </section>
      {/each}
    {/if}
  {/if}
</aside>

<style>
  .side {
    padding: var(--s-4);
    overflow: auto;
    background: var(--paper-1);
    border-left: 1px solid var(--rule);
    display: flex;
    flex-direction: column;
    gap: var(--s-3);
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: start;
    gap: var(--s-3);
  }
  h2 {
    margin: 0;
    font-size: var(--t-md);
  }
  h3 {
    margin: 0 0 var(--s-1, 4px);
    font-weight: 600;
  }
  p {
    margin: 0;
  }
  .dim {
    color: var(--ink-2);
  }
  .small {
    font-size: var(--t-xs);
  }
  .error {
    color: var(--danger, #9b2c3f);
    font-size: var(--t-sm);
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  li {
    padding: var(--s-2) 0;
    border-bottom: 1px solid var(--paper-2);
    display: flex;
    flex-direction: column;
    gap: var(--s-1, 4px);
  }
  li.compact {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    padding: var(--s-1, 4px) 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--s-2);
  }
  .line .name {
    flex: 1;
    min-width: 0;
  }
  .name {
    display: flex;
    gap: var(--s-2);
    align-items: center;
    font-weight: 600;
  }
  .row {
    display: flex;
    gap: var(--s-2);
    align-items: center;
    flex-wrap: wrap;
  }
  .filter {
    min-height: 2rem;
    padding: 0 var(--s-2);
    border: 1px solid var(--ink-2);
    background: var(--paper-0, #fff);
    color: var(--ink-0);
    border-radius: var(--r-1);
    font: inherit;
  }
  .under {
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    padding: var(--s-2);
    background: var(--paper-0, #fff);
  }
  .group {
    border-top: 1px solid var(--rule);
    padding-top: var(--s-2);
  }
  .ghead {
    display: flex;
    align-items: center;
    gap: var(--s-2);
  }
  .gname {
    display: flex;
    gap: var(--s-1, 4px);
    align-items: center;
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    padding: var(--s-1, 4px) 0;
    font: inherit;
    font-weight: 600;
    color: var(--ink-0);
    cursor: pointer;
    text-align: left;
  }
  .count {
    white-space: nowrap;
  }
  input[type='range'] {
    flex: 1;
    min-width: 6rem;
  }
  output {
    min-width: 3rem;
    font-variant-numeric: tabular-nums;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--s-2);
  }
  .form label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: var(--t-sm);
  }
  .form input:not([type]) {
    min-height: 2rem;
    padding: 0 var(--s-2);
    border: 1px solid var(--ink-2);
    background: var(--paper-0, #fff);
    color: var(--ink-0);
    border-radius: var(--r-1);
    font: inherit;
  }
  fieldset {
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    display: flex;
    flex-direction: column;
    gap: var(--s-2);
  }
  .corner {
    display: grid;
    grid-template-columns: 1fr 6rem 6rem;
    gap: var(--s-2);
    align-items: center;
    font-size: var(--t-xs);
  }
  .corner input {
    min-width: 0;
    min-height: 2rem;
    padding: 0 var(--s-2);
    border: 1px solid var(--ink-2);
    border-radius: var(--r-1);
    font: inherit;
  }
</style>
