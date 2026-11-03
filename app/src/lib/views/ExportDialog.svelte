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

  let { onClose }: { onClose: () => void } = $props();

  const api = getApi();
  const platform = getPlatform();
  const WARN = "Living people's details will be written to the file.";

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
    const dest = await platform.pickSavePath({ title: 'Export GEDCOM', defaultName: 'Family tree.ged', extensions: ['ged'] });
    if (!dest) return;
    exporting = true;
    try {
      const r = await api.exportGedcom(preview.options, dest, preview.token);
      notices.show({
        message: `Exported ${Math.max(1, Math.round(r.bytes / 1024))} KB to ${fileOf(dest)}`,
        actionLabel: 'Reveal',
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
    const dest = await platform.pickFolder('Choose an empty folder for the copy of your tree');
    if (!dest) return;
    saving = true;
    try {
      await api.saveCopy(dest);
      notices.show({ message: `Copy saved in the folder ${fileOf(dest)}`, actionLabel: 'Reveal', onaction: () => void platform.reveal(dest) });
    } catch (e) {
      exportError = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
  }
</script>

<Dialog open title="Export" onclose={onClose}>
  <div class="export">
    {#if exportError}<Banner kind="error">{exportError}</Banner>{/if}
    <div class="options">
      <div>
        <RadioGroup
          legend="Format"
          options={[
            { value: '5.5.1', label: 'GEDCOM 5.5.1' },
            { value: '7.0', label: 'GEDCOM 7.0' },
          ]}
          bind:value={version}
          onchange={() => (versionTouched = true)}
        />
        <p class="hint">Defaults to the version your file came in, so your other programs can read it.</p>
      </div>
      <div>
        <RadioGroup
          legend="Living people"
          options={[
            { value: 'placeholder', label: 'Show as Living (default)' },
            { value: 'exclude', label: 'Leave out' },
            { value: 'all', label: 'Include everyone' },
          ]}
          bind:value={living}
        />
        {#if living === 'all'}<div class="warn"><Banner kind="warning">{WARN}</Banner></div>{/if}
      </div>
      <div>
        <Checkbox label="Include records marked private" bind:checked={includePrivate} />
        {#if includePrivate}<div class="warn"><Banner kind="warning">{WARN}</Banner></div>{/if}
      </div>
    </div>

    <div class="preview" role="region" aria-label="Privacy preview" aria-busy={loading}>
      <h3>Privacy preview</h3>
      <p class="fixed">Photos are not included in a GEDCOM file.</p>
      {#if previewError}
        <Banner kind="error">{previewError}</Banner>
      {:else if preview}
        {#if loading || !inSync}
          <div class="body skeleton" aria-hidden="true">
            <div class="bar"></div>
            <div class="bar short"></div>
          </div>
          <p class="hint">Updating the preview…</p>
        {:else}
        <div class="body">
          <dl>
            <div><dt>People in the file</dt><dd>{preview.exportedPersons + preview.placeholderPersons}</dd></div>
            <div><dt>Named</dt><dd>{preview.exportedPersons}</dd></div>
            <div><dt>Shown as 'Living'</dt><dd>{preview.placeholderPersons}</dd></div>
            <div><dt>Left out</dt><dd>{leftOut}</dd></div>
            <div>
              <dt>Other records removed</dt>
              <dd>{preview.removedRecords}</dd>
            </div>
          </dl>
          <p class="hint">Other records removed: private items, and events and notes of couples with a living partner</p>
          {#if preview.mentionWarnings.length}
            <h4>Notes that may name hidden people</h4>
            <ul>
              {#each preview.mentionWarnings as w, i (i)}
                <li>“{w.noteExcerpt}” mentions <bdi dir="auto">{w.hiddenPerson}</bdi></li>
              {/each}
            </ul>
          {/if}
          <p class="hint">
            {preview.mentionWarnings.length ? '' : 'No notes name hidden people. '}This check is best effort: nicknames and initials are not caught.
          </p>
          {#if preview.groupMedia.length}
            <h4>Photos that also show hidden people: you decide whether to keep them in the shared copy</h4>
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
        <p class="hint">Checking what would be shared…</p>
      {/if}
    </div>

    <section class="copy">
      <h3>Save a copy of the whole tree</h3>
      <p class="hint">This copy is not filtered. It includes living people, private records and history; keep it to yourself.</p>
      <p class="hint">The copy is a tree folder, not a zip file. You can open it in Gene Tree as a tree of its own; choose an empty or new folder.</p>
      <div><Button disabled={saving} onclick={saveCopy}>Save a copy to a folder…</Button></div>
    </section>
  </div>
  {#snippet footer()}
    <Button onclick={onClose}>Close</Button>
    <Button variant="primary" disabled={!canExport} onclick={doExport}>Export…</Button>
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
  .copy {
    display: grid;
    gap: var(--s-3);
    padding-top: var(--s-5);
    border-top: 1px solid var(--rule);
  }
</style>
