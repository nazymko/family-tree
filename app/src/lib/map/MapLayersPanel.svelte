<script lang="ts">
  // "Old maps": scanned maps laid under the places. Add one (with its OziExplorer .map file when there is one), show or
  // hide, opacity, order, place by corners, edit, remove from the map (the picture stays in Media).
  import Button from '../ui/Button.svelte';
  import { getApi, getPlatform } from '../api/context';
  import { notices } from '../ui/notices.svelte';
  import { t } from '../i18n';
  import type { Id, MapCorner, MapLayerDto, MapLayerInput } from '../api/types';
  import { parseDegrees, reordered } from './layers';

  let {
    layers,
    onOpacity,
    onPlace,
    onAdded,
    onChanged,
    onClose,
  }: {
    /** Lowest first, as drawn. */
    layers: MapLayerDto[];
    /** While the slider moves: the opacity to draw with, not saved yet. */
    onOpacity: (media: Id, opacity: number | null) => void;
    onPlace: (layer: MapLayerDto) => void;
    onAdded: (media: Id, placed: boolean) => void;
    onChanged: () => void;
    onClose: () => void;
  } = $props();

  const api = getApi();
  const platform = getPlatform();
  const PICTURES = ['jpg', 'jpeg', 'png', 'webp', 'gif', 'tif', 'tiff'];

  const name = (l: MapLayerDto) => l.title?.trim() || t('map.layers.untitled');
  const top = $derived([...layers].reverse());

  let busy = $state(false);
  let error = $state('');

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
        form.corners = cornerFields(await api.oziCorners(path, form.media));
        error = '';
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
        const id = await api.addMapLayerFile(f.path, f.title.trim() || null, { series: text(f.series), year: text(f.year), source: text(f.source), opacity: 70, visible: true }, f.ozi);
        notices.show({ message: t('map.layers.added', { title: f.title.trim() || t('map.layers.untitled') }) });
        form = null;
        onAdded(id, !!f.ozi);
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
      <ul>
        {#each top as l, i (l.media)}
          <li>
            <label class="name">
              <input type="checkbox" checked={l.visible} aria-label={t('map.layers.shown', { title: name(l) })} onchange={(e) => save(l.media, input(l, { visible: e.currentTarget.checked }))} />
              <span>{name(l)}</span>
            </label>
            <div class="dim small">
              {[l.series, l.year].filter(Boolean).join(', ')}{#if !l.corners}{l.series || l.year ? ' · ' : ''}{t('map.layers.notPlaced')}{/if}
            </div>
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
              <Button onclick={() => move(l, 1)} disabled={busy || i === 0} aria-label={t('map.layers.up', { title: name(l) })}>↑</Button>
              <Button onclick={() => move(l, -1)} disabled={busy || i === top.length - 1} aria-label={t('map.layers.down', { title: name(l) })}>↓</Button>
              <Button onclick={() => onPlace(l)}>{t('map.layers.place')}</Button>
              <Button onclick={() => edit(l)}>{t('map.layers.edit')}</Button>
              <Button onclick={() => remove(l)} disabled={busy}>{t('map.layers.remove')}</Button>
            </div>
          </li>
        {/each}
      </ul>
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
