<script lang="ts">
  // Imports place records from TSV index files: choose the files, see the books and villages found, give each
  // an existing place or a new one, preview what would be added, then import with a progress bar.
  import Dialog from '../ui/Dialog.svelte';
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import ProgressBar from '../ui/ProgressBar.svelte';
  import PlacePicker from '../editors/PlacePicker.svelte';
  import { getApi, getPlatform } from '../api/context';
  import { errMessage } from '../editors/errors';
  import type { Id, PlaceImportMapDto, PlaceImportPreviewDto, PlaceImportResultDto, PlaceImportScanDto, PlaceOption } from '../api/types';
  import { t, plural } from '../i18n';

  let {
    open,
    onclose,
    defaultPlace = null,
    ondone,
  }: {
    open: boolean;
    onclose: () => void;
    /** The place shown behind the dialog: offered for every book and village at first. */
    defaultPlace?: PlaceOption | null;
    /** Told when records were added, so the list behind reads again. */
    ondone?: (r: PlaceImportResultDto) => void;
  } = $props();

  const api = getApi();
  const platform = getPlatform();

  type Choice = { placeId?: Id; placeText?: string } | null;
  let paths = $state<string[]>([]);
  let scan = $state<PlaceImportScanDto | null>(null);
  let choices = $state<Record<string, Choice>>({});
  let preview = $state<PlaceImportPreviewDto | null>(null);
  let busy = $state(false);
  let error = $state('');
  let progress = $state<{ done: number; total: number } | null>(null);
  let result = $state<PlaceImportResultDto | null>(null);

  const keyOf = (g: { book: string; village: string }) => `${g.book}\u0000${g.village}`;

  $effect(() => {
    if (!open) return;
    paths = [];
    scan = null;
    choices = {};
    preview = null;
    error = '';
    progress = null;
    result = null;
    busy = false;
  });

  async function choose() {
    error = '';
    const picked = await platform.pickFiles({ title: t('placeImport.pickTitle'), extensions: ['tsv', 'txt'] });
    if (picked.length === 0) return;
    busy = true;
    try {
      paths = picked;
      scan = await api.placeImportScan(picked);
      preview = null;
      choices = Object.fromEntries(
        scan.groups.map((g) => [keyOf(g), defaultPlace ? ({ placeId: defaultPlace.id } as Choice) : null]),
      );
    } catch (e) {
      scan = null;
      error = errMessage(e, t('placeImport.scanFailed'));
    } finally {
      busy = false;
    }
  }

  /** New places are made once each, here, before the mapping goes to the app. */
  async function resolve(): Promise<PlaceImportMapDto[]> {
    const made = new Map<string, Id>();
    const out: PlaceImportMapDto[] = [];
    for (const g of scan?.groups ?? []) {
      const c = choices[keyOf(g)];
      let place: Id | null = c?.placeId ?? null;
      if (!place && c?.placeText) {
        let id = made.get(c.placeText);
        if (!id) {
          const r = await api.apply({ type: 'createPlace', text: c.placeText }, { silent: true });
          id = r.focus ?? undefined;
          if (id) made.set(c.placeText, id);
        }
        place = id ?? null;
        choices[keyOf(g)] = place ? { placeId: place } : c;
      }
      out.push({ book: g.book, village: g.village, place });
    }
    return out;
  }

  async function showPreview() {
    busy = true;
    error = '';
    try {
      preview = await api.placeImportPreview(paths, await resolve());
    } catch (e) {
      error = errMessage(e, t('placeImport.failed'));
    } finally {
      busy = false;
    }
  }

  async function run() {
    busy = true;
    error = '';
    progress = { done: 0, total: preview?.totalNew ?? 0 };
    try {
      const r = await api.placeImportRun(paths, await resolve(), (p) => (progress = p));
      result = r;
      ondone?.(r);
    } catch (e) {
      error = errMessage(e, t('placeImport.failed'));
    } finally {
      busy = false;
    }
  }

  const years = (g: { yearFrom: number | null; yearTo: number | null }) =>
    g.yearFrom == null ? '' : g.yearFrom === g.yearTo ? String(g.yearFrom) : `${g.yearFrom}–${g.yearTo}`;
  const line = (g: { book: string; village: string }) => preview?.lines.find((l) => l.book === g.book && l.village === g.village);
  const anyMapped = $derived(Object.values(choices).some((c) => !!c));
</script>

<Dialog {open} title={t('placeImport.title')} {onclose} width="60rem" dismissOnScrim={!busy}>
  <div class="body">
    <p>{t('placeImport.intro')}</p>
    {#if error}<Banner kind="error">{error}</Banner>{/if}

    {#if result}
      <Banner kind="info">{t('placeImport.done', { added: result.added, duplicates: result.duplicates })}</Banner>
    {:else}
      <p>
        <Button variant="secondary" disabled={busy} onclick={() => void choose()}>{t('placeImport.choose')}</Button>
        {#if paths.length > 0}<span class="files">{plural('placeImport.files', paths.length)}</span>{/if}
      </p>

      {#if scan}
        <p>{t('placeImport.found', { rows: scan.rows, records: scan.groups.reduce((n, g) => n + g.records, 0), groups: scan.groups.length })}</p>
        {#if scan.badRows > 0}<p class="warn">{t('placeImport.badRows', { n: scan.badRows })}</p>{/if}
        <p>{t('placeImport.mapIntro')}</p>
        <div class="table-wrap">
          <table>
            <thead>
              <tr>
                <th>{t('placeImport.book')}</th>
                <th>{t('placeImport.village')}</th>
                <th class="num">{t('placeImport.records')}</th>
                <th>{t('placeImport.years')}</th>
                <th>{t('placeImport.place')}</th>
                {#if preview}<th class="num">{t('placeImport.previewCols')}</th><th class="num">{t('placeImport.previewDup')}</th>{/if}
              </tr>
            </thead>
            <tbody>
              {#each scan.groups as g (keyOf(g))}
                {@const l = line(g)}
                <tr>
                  <td>{g.book}</td>
                  <td>{g.village || '—'}</td>
                  <td class="num">{g.records}</td>
                  <td>{years(g)}</td>
                  <td class="pick">
                    <PlacePicker
                      label={`${t('placeImport.place')}: ${g.book} ${g.village}`.trim()}
                      value={defaultPlace && choices[keyOf(g)]?.placeId === defaultPlace.id ? defaultPlace : null}
                      text=""
                      onChange={(v) => {
                        choices[keyOf(g)] = v;
                        preview = null;
                      }}
                    />
                    {#if choices[keyOf(g)]?.placeText}<small>{t('placeImport.createdAs', { text: choices[keyOf(g)]!.placeText! })}</small>{/if}
                  </td>
                  {#if preview}<td class="num">{l?.new ?? 0}</td><td class="num">{l?.duplicates ?? 0}</td>{/if}
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
        <p class="hint">{t('placeImport.createHint')}</p>
        {#if preview}
          <p class="total" aria-live="polite">
            {t('placeImport.previewTotal', { new: preview.totalNew, dup: preview.totalDuplicates, unmapped: preview.unmapped })}
            {#if preview.totalNew === 0}{t('placeImport.nothingToAdd')}{/if}
          </p>
        {/if}
        {#if progress && busy}
          <ProgressBar label={t('placeImport.progress')} value={progress.done} max={progress.total} />
          <p>{t('placeImport.running', { done: progress.done, total: progress.total })}</p>
        {/if}
      {/if}
    {/if}
  </div>

  {#snippet footer()}
    {#if result}
      <Button variant="primary" onclick={onclose}>{t('placeImport.close')}</Button>
    {:else}
      <Button disabled={busy} onclick={onclose}>{t('placeImport.close')}</Button>
      {#if scan}
        <Button variant="secondary" disabled={busy || !anyMapped} onclick={() => void showPreview()}>{t('placeImport.preview')}</Button>
        <Button variant="primary" disabled={busy || !preview || preview.totalNew === 0} onclick={() => void run()}>{t('placeImport.run')}</Button>
      {/if}
    {/if}
  {/snippet}
</Dialog>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--s-3);
  }
  p {
    margin: 0;
  }
  .files {
    margin-inline-start: var(--s-3);
    color: var(--ink-3);
  }
  .warn {
    color: var(--warn, #a05a00);
  }
  .hint,
  small {
    color: var(--ink-3);
    font-size: var(--t-xs);
  }
  .table-wrap {
    max-height: 24rem;
    overflow: auto;
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    text-align: start;
    padding: var(--s-2) var(--s-3);
    border-bottom: 1px solid var(--rule);
    vertical-align: top;
  }
  .num {
    text-align: end;
    font-variant-numeric: tabular-nums;
  }
  .pick {
    min-width: 18rem;
  }
  .total {
    font-weight: 500;
  }
</style>
