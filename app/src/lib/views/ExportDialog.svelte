<script lang="ts">
  import { untrack } from 'svelte';
  import Dialog from '../ui/Dialog.svelte';
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import RadioGroup from '../ui/RadioGroup.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import { getApi, getPlatform } from '../api/context';
  import { notices } from '../ui/notices.svelte';
  import type { ExportOptions, PrivacyPreview } from '../api/api';
  import { t, formatNumber } from '../i18n';

  let { onClose, onSendCopy }: { onClose: () => void; onSendCopy?: () => void } = $props();

  const api = getApi();
  const platform = getPlatform();
  const WARN = $derived(t('exp.warn'));

  let version = $state<ExportOptions['version']>('7.0');
  let versionTouched = false;
  let living = $state<ExportOptions['living']>('placeholder');
  let includePrivate = $state(false);

  let preview = $state<PrivacyPreview | null>(null);
  let loading = $state(true);
  let previewError = $state('');
  let exportError = $state('');
  let exporting = $state(false);
  let saving = $state(false);
  let seq = 0;

  const key = (o: ExportOptions) => `${o.version}|${o.living}|${o.includePrivate}`;
  const current = $derived<ExportOptions>({ version, living, includePrivate });
  const inSync = $derived(!!preview && key(preview.options) === key(current));
  const canExport = $derived(!loading && inSync && !exporting);
  const leftOut = $derived(preview ? Math.max(0, preview.hiddenPersons - preview.placeholderPersons) : 0);

  $effect(() => {
    const opts: ExportOptions = { version, living, includePrivate };
    const mine = ++seq;
    loading = true;
    previewError = '';
    api.previewExport(opts).then(
      (p) => {
        if (mine !== seq) return; // a newer request is in flight: ignore this answer
        preview = p;
        loading = false;
        untrack(() => {
          if (!versionTouched && p.defaultVersion !== version) version = p.defaultVersion;
        });
      },
      (e) => {
        if (mine !== seq) return;
        loading = false;
        previewError = e instanceof Error ? e.message : String(e);
      },
    );
  });

  const fileOf = (path: string) => path.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || path;

  async function doExport() {
    if (!preview || !canExport) return;
    exportError = '';
    const dest = await platform.pickSavePath({ title: t('exp.pickTitle'), defaultName: t('exp.defaultName'), extensions: ['ged'] });
    if (!dest) return;
    exporting = true;
    try {
      const r = await api.exportGedcom(preview.options, dest, preview.token);
      notices.show({
        message: t('exp.exported', { kb: formatNumber(Math.max(1, Math.round(r.bytes / 1024))), file: fileOf(dest) }),
        actionLabel: t('exp.reveal'),
        onaction: () => void platform.reveal(dest),
      });
    } catch (e) {
      exportError = e instanceof Error ? e.message : String(e);
    } finally {
      exporting = false;
    }
  }

  async function saveCopy() {
    exportError = '';
    const dest = await platform.pickFolder(t('exp.pickCopyFolder'));
    if (!dest) return;
    saving = true;
    try {
      await api.saveCopy(dest);
      notices.show({ message: t('exp.copySaved', { file: fileOf(dest) }), actionLabel: t('exp.reveal'), onaction: () => void platform.reveal(dest) });
    } catch (e) {
      exportError = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
  }
</script>

<Dialog open title={t('exp.title')} onclose={onClose}>
  <div class="export">
    {#if exportError}<Banner kind="error">{exportError}</Banner>{/if}
    <div class="options">
      <div>
        <RadioGroup
          legend={t('exp.format')}
          options={[
            { value: '5.5.1', label: 'GEDCOM 5.5.1' },
            { value: '7.0', label: 'GEDCOM 7.0' },
          ]}
          bind:value={version}
          onchange={() => (versionTouched = true)}
        />
        <p class="hint">{t('exp.formatHint')}</p>
      </div>
      <div>
        <RadioGroup
          legend={t('exp.livingPeople')}
          options={[
            { value: 'placeholder', label: t('exp.asLiving') },
            { value: 'exclude', label: t('exp.leaveOut') },
            { value: 'all', label: t('exp.includeEveryone') },
          ]}
          bind:value={living}
        />
        {#if living === 'all'}<div class="warn"><Banner kind="warning">{WARN}</Banner></div>{/if}
      </div>
      <div>
        <Checkbox label={t('exp.includePrivate')} bind:checked={includePrivate} />
        {#if includePrivate}<div class="warn"><Banner kind="warning">{WARN}</Banner></div>{/if}
      </div>
    </div>

    <div class="preview" role="region" aria-label={t('exp.preview')} aria-busy={loading}>
      <h3>{t('exp.preview')}</h3>
      <p class="fixed">{t('exp.noPhotos')}</p>
      {#if previewError}
        <Banner kind="error">{previewError}</Banner>
      {:else if preview}
        {#if loading || !inSync}
          <div class="body skeleton" aria-hidden="true">
            <div class="bar"></div>
            <div class="bar short"></div>
          </div>
          <p class="hint">{t('exp.updating')}</p>
        {:else}
        <div class="body">
          <dl>
            <div><dt>{t('exp.peopleInFile')}</dt><dd>{formatNumber(preview.exportedPersons + preview.placeholderPersons)}</dd></div>
            <div><dt>{t('exp.named')}</dt><dd>{formatNumber(preview.exportedPersons)}</dd></div>
            <div><dt>{t('exp.shownAsLiving')}</dt><dd>{formatNumber(preview.placeholderPersons)}</dd></div>
            <div><dt>{t('exp.leftOut')}</dt><dd>{formatNumber(leftOut)}</dd></div>
            <div>
              <dt>{t('exp.otherRemoved')}</dt>
              <dd>{formatNumber(preview.removedRecords)}</dd>
            </div>
          </dl>
          <p class="hint">{t('exp.otherRemovedHint')}</p>
          {#if preview.mentionWarnings.length}
            <h4>{t('exp.mentionTitle')}</h4>
            <ul>
              {#each preview.mentionWarnings as w, i (i)}
                <li>{t('exp.mentions', { excerpt: w.noteExcerpt })} <bdi dir="auto">{w.hiddenPerson}</bdi></li>
              {/each}
            </ul>
          {/if}
          <p class="hint">
            {preview.mentionWarnings.length ? '' : t('exp.noneNamed')}{t('exp.bestEffort')}
          </p>
          {#if preview.groupMedia.length}
            <h4>{t('exp.groupMedia')}</h4>
            <ul>
              {#each preview.groupMedia as g, i (i)}<li>{g.title}</li>{/each}
            </ul>
          {/if}
          {#if preview.losslessNotes.length}
            <ul class="notes">
              {#each preview.losslessNotes as n (n)}<li>{n}</li>{/each}
            </ul>
          {/if}
        </div>
        {/if}
      {:else}
        <p class="hint">{t('exp.checking')}</p>
      {/if}
    </div>

    <section class="copy">
      <h3>{t('exp.copyTitle')}</h3>
      <p class="hint">{t('exp.copyHint1')}</p>
      <p class="hint">{t('exp.copyHint2')}</p>
      <div class="copyButtons">
        <Button disabled={saving} onclick={saveCopy}>{t('exp.saveCopy')}</Button>
        {#if onSendCopy}<Button disabled={saving} onclick={onSendCopy}>{t('exp.sendCopy')}</Button>{/if}
      </div>
    </section>
  </div>
  {#snippet footer()}
    <Button onclick={onClose}>{t('common.close')}</Button>
    <Button variant="primary" disabled={!canExport} onclick={doExport}>{t('exp.exportButton')}</Button>
  {/snippet}
</Dialog>

<style>
  .export {
    display: grid;
    gap: var(--s-6);
  }
  .options {
    display: grid;
    gap: var(--s-5);
  }
  .hint {
    font-size: var(--t-sm);
    color: var(--ink-1);
    max-width: var(--measure);
  }
  .warn {
    margin-top: var(--s-3);
  }
  .preview {
    display: grid;
    gap: var(--s-3);
    padding: var(--s-5);
    background: var(--paper-2);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
  }
  .body {
    display: grid;
    gap: var(--s-3);
  }
  .skeleton .bar {
    height: 2.5rem;
    background: var(--rule);
    opacity: 0.4;
    border-radius: var(--r-1);
  }
  .skeleton .bar.short {
    width: 60%;
  }
  dl {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(7rem, 1fr));
    gap: var(--s-4);
    margin: 0;
  }
  dl div {
    display: grid;
  }
  dt {
    font-size: var(--t-sm);
    color: var(--ink-1);
  }
  dd {
    margin: 0;
    font-family: var(--font-serif);
    font-size: var(--t-lg);
    font-variant-numeric: tabular-nums;
  }
  h4 {
    font-size: var(--t-base);
    font-family: var(--font-sans);
    font-weight: 600;
    margin: var(--s-3) 0 0;
  }
  ul {
    margin: 0;
    padding-inline-start: var(--s-5);
  }
  .copyButtons {
    display: flex;
    gap: var(--s-3);
    flex-wrap: wrap;
  }
  .copy {
    display: grid;
    gap: var(--s-3);
    padding-top: var(--s-5);
    border-top: 1px solid var(--rule);
  }
</style>
