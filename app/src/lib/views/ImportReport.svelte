<script lang="ts">
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import { getApi, getPlatform } from '../api/context';
  import type { ImportReportDto, MissingMedia } from '../api/types';
  import { nav } from '../state/nav.svelte';
  import { project } from '../state/project.svelte';
  import { toasts } from '../state/toast.svelte';
  import { enterProject } from '../state/openTree';
  import { t, plural, formatNumber } from '../i18n';
  import { backendError, repairKindLabel } from '../i18n/backend';
  import type { MessageKey } from '../i18n';

  let { report: reportProp }: { report?: ImportReportDto } = $props();

  const api = getApi();
  const platform = getPlatform();

  const report = $derived(reportProp ?? project.report);
  let stillMissing = $state<string[] | null>(null);
  let confirm = $state(false);
  let busy = $state(false);

  const missingList = $derived<MissingMedia[]>(
    (report?.missingMedia ?? []).filter((m) => stillMissing === null || stillMissing.includes(m.path)),
  );
  const byReason = (r: MissingMedia['reason']) => missingList.filter((m) => m.reason === r);
  const notFound = $derived(byReason('notFound'));
  const outside = $derived(byReason('outsideRoots'));
  const ambiguous = $derived(byReason('ambiguous'));
  // The real importer counts records by GEDCOM tag; the mock counts by word.
  const COUNT_KEYS: Record<string, MessageKey> = {
    persons: 'imp.countPeople',
    families: 'imp.countFamilies',
    sources: 'imp.countSources',
    media: 'imp.countMedia',
    notes: 'imp.countNotes',
    places: 'imp.countPlaces',
    INDI: 'imp.countPeople',
    FAM: 'imp.countFamilies',
    SOUR: 'imp.countSources',
    OBJE: 'imp.countMedia',
    NOTE: 'imp.countNotes',
    SNOTE: 'imp.countSharedNotes',
    REPO: 'imp.countRepositories',
    SUBM: 'imp.countSubmitters',
  };
  const label = (k: string) => (COUNT_KEYS[k] ? t(COUNT_KEYS[k]) : k.charAt(0).toUpperCase() + k.slice(1).replace(/_/g, ' '));

  const repairGroups = $derived.by(() => {
    const groups: [string, NonNullable<typeof report>['repairs']][] = [];
    for (const r of report?.repairs ?? []) {
      const g = groups.find(([k]) => k === r.kind);
      if (g) g[1].push(r);
      else groups.push([r.kind, [r]]);
    }
    return groups;
  });
  const tags = $derived(Object.entries(report?.unmappedTags ?? {}));

  async function locate() {
    const folder = await platform.pickFolder(t('imp.pickMediaFolder'));
    if (!folder) return;
    const r = await api.relinkMedia(folder);
    stillMissing = r.stillMissing;
    toasts.push({ text: plural('imp.linked', r.relinked, { missing: formatNumber(r.stillMissing.length) }) });
  }

  async function saveReport() {
    const dest = await platform.pickSavePath({
      title: t('imp.saveReportTitle'),
      defaultName: 'import-report.txt',
      extensions: ['txt'],
    });
    if (!dest) return;
    await api.saveImportReport(dest);
    toasts.push({ text: t('imp.reportSaved') });
  }

  async function discard() {
    busy = true;
    try {
      await api.discardImport();
      confirm = false;
      project.close();
      nav.reset();
    } catch (e) {
      confirm = false;
      toasts.push({ text: (e as Error).message, kind: 'error' });
    } finally {
      busy = false;
    }
  }

  async function openTree() {
    if (report) await enterProject(api, report.project);
  }
</script>

{#if report}
  <main class="report">
    <div class="col">
      <header>
        <h1>{report.producer ? plural('imp.heading', report.counts.INDI ?? report.counts.persons ?? report.project.personCount, { producer: report.producer }) : plural('imp.headingUnknown', report.counts.INDI ?? report.counts.persons ?? report.project.personCount)}</h1>
        <p class="sub">
          {report.version ? t('imp.gedcomVersion', { version: report.version }) : ''}{report.encoding}. {t('imp.savedAs')}
          <bdi dir="auto" class="dir">{report.project.name}</bdi>.
        </p>
      </header>

      {#if report.project.locationWarning}
        <Banner kind="warning">
          {t('imp.syncWarning', { provider: report.project.locationWarning.provider ?? t('imp.networkShare') })}
        </Banner>
      {/if}
      {#if report.truncated}
        <Banner kind="info">{t('imp.truncated')}</Banner>
      {/if}

      <table class="counts" aria-label={t('imp.imported')}>
        <tbody>
          {#each Object.entries(report.counts) as [k, n] (k)}
            <tr><th scope="row">{label(k)}</th><td class="tnum">{formatNumber(n)}</td></tr>
          {/each}
        </tbody>
      </table>

      {#if repairGroups.length > 0}
        <details open>
          <summary><h2>{t('imp.repairs')}</h2></summary>
          {#each repairGroups as [kind, items] (kind)}
            <h3>{repairKindLabel(kind)}</h3>
            <ul>
              {#each items as r, i (i)}
                <li>{r.line !== null ? `${t('imp.line', { n: r.line })} ` : ''}{backendError(r.detail)}</li>
              {/each}
            </ul>
          {/each}
        </details>
      {/if}

      {#if tags.length > 0}
        <details open>
          <summary><h2>{t('imp.unmappedTags')}</h2></summary>
          <ul>
            {#each tags as [tag, n] (tag)}
              <li><code>{tag}</code> <span class="tnum">{plural('imp.usedTimes', n)}</span></li>
            {/each}
          </ul>
        </details>
      {/if}

      {#if report.unparsedDates.length > 0}
        <details open>
          <summary><h2>{t('imp.badDates')}</h2></summary>
          <p class="note">{t('imp.keptAsWritten')}</p>
          <ul>
            {#each report.unparsedDates as d, i (i)}
              <li><span class="tnum">{t('imp.line', { n: d.line })}</span> <bdi dir="auto">{d.text}</bdi></li>
            {/each}
          </ul>
        </details>
      {/if}

      {#if missingList.length > 0}
        <details open>
          <summary><h2>{t('imp.missingFiles')}</h2></summary>
          {#if notFound.length > 0}
            <h3>{t('imp.notFound')}</h3>
            <ul>
              {#each notFound as m (m.path)}
                <li><bdi dir="auto">{m.path}</bdi> <span class="why">{t('imp.notFoundWhy')}</span></li>
              {/each}
            </ul>
            <Button onclick={locate}>{t('imp.locateFolder')}</Button>
          {/if}
          {#if outside.length > 0}
            <h3>{t('imp.outsideFolders')}</h3>
            <ul>
              {#each outside as m (m.path)}
                <li>
                  <bdi dir="auto">{m.path}</bdi>
                  <span class="why">{t('imp.outsideWhy')}</span>
                </li>
              {/each}
            </ul>
            <Button onclick={locate}>{t('imp.chooseFolder')}</Button>
          {/if}
          {#if ambiguous.length > 0}
            <h3>{t('imp.ambiguous')}</h3>
            <ul>
              {#each ambiguous as m (m.path)}
                <li>
                  <bdi dir="auto">{m.path}</bdi>
                  <span class="why">{t('imp.ambiguousWhy')}</span>
                  <ul class="cand">
                    {#each m.candidates as c (c)}<li><bdi dir="auto">{c}</bdi></li>{/each}
                  </ul>
                </li>
              {/each}
            </ul>
          {/if}
        </details>
      {/if}

      {#if report.mediaUrls.length > 0}
        <details open>
          <summary><h2>{t('imp.webLinks')}</h2></summary>
          <p class="note">{t('imp.webLinksNote')}</p>
          <ul>
            {#each report.mediaUrls as u (u)}
              <li><bdi dir="ltr">{u}</bdi></li>
            {/each}
          </ul>
        </details>
      {/if}

      <div class="actions">
        <Button onclick={saveReport}>{t('imp.saveReport')}</Button>
        <Button variant="danger" onclick={() => (confirm = true)}>{t('imp.undoImport')}</Button>
        <span class="grow"></span>
        <Button variant="primary" onclick={openTree}>{t('imp.openTree')}</Button>
      </div>
    </div>
  </main>

  <Dialog open={confirm} title={t('imp.undoImport')} onclose={() => (confirm = false)}>
    <p>{t('imp.undoConfirm')}</p>
    {#snippet footer()}
      <Button onclick={() => (confirm = false)}>{t('common.cancel')}</Button>
      <Button variant="danger" onclick={discard} disabled={busy}>{t('imp.deleteTree')}</Button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .report {
    min-height: 100vh;
    padding: var(--s-8) var(--s-7);
    background: var(--paper-0);
  }
  .col {
    display: grid;
    gap: var(--s-5);
    max-width: 52rem;
    margin-inline: auto;
  }
  .sub,
  .note {
    color: var(--ink-1);
  }
  .counts {
    border-collapse: collapse;
    background: var(--paper-1);
    border: 1px solid var(--rule);
    width: min(24rem, 100%);
  }
  .counts th,
  .counts td {
    padding: var(--s-2) var(--s-4);
    border-block-end: 1px solid var(--rule);
    text-align: start;
    font-weight: 400;
  }
  .counts td {
    text-align: end;
    font-weight: 500;
  }
  details {
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background: var(--paper-1);
    padding: var(--s-4) var(--s-5);
  }
  summary {
    cursor: pointer;
  }
  summary h2 {
    display: inline;
    font-size: var(--t-md);
  }
  details > :global(*:not(summary)) {
    margin-top: var(--s-3);
  }
  h3 {
    font-family: var(--font-sans);
    font-size: var(--t-sm);
    font-weight: 700;
    text-transform: none;
    color: var(--ink-1);
  }
  ul {
    margin: 0;
    padding-inline-start: var(--s-6);
  }
  li {
    margin-block: var(--s-2);
    overflow-wrap: anywhere;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-3);
    align-items: center;
    padding-top: var(--s-4);
  }
  .why {
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  .cand {
    margin-top: var(--s-2);
  }
  .grow {
    flex: 1;
  }
</style>
