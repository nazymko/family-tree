<script lang="ts">
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import TextField from '../ui/TextField.svelte';
  import RadioGroup from '../ui/RadioGroup.svelte';
  import ProgressBar from '../ui/ProgressBar.svelte';
  import IconButton from '../ui/IconButton.svelte';
  import { getApi, getPlatform } from '../api/context';
  import type { DayOrder, ImportProgress, LocationWarningDto } from '../api/types';
  import { nav } from '../state/nav.svelte';
  import { project } from '../state/project.svelte';
  import { settings } from '../state/settings.svelte';
  import { enterProject } from '../state/openTree';
  import { t } from '../i18n';
  import type { MessageKey } from '../i18n';

  let { mode, oncancel, thenTrace = false }: { mode: 'import' | 'create'; oncancel: () => void; thenTrace?: boolean } = $props();

  const api = getApi();
  const platform = getPlatform();

  const PHASES: ImportProgress['phase'][] = ['reading', 'parsing', 'copying_media', 'saving', 'indexing'];
  const PHASE_KEYS: Record<ImportProgress['phase'], MessageKey> = {
    reading: 'imp.phaseReading',
    parsing: 'imp.phaseParsing',
    copying_media: 'imp.phaseCopying',
    saving: 'imp.phaseSaving',
    indexing: 'imp.phaseIndexing',
  };
  // The backend detects the format from the file itself; the filter only narrows the picker.
  const IMPORT_EXTENSIONS = ['ged', 'gedcom', 'atdb', 'gramps', 'gdz'];

  let file = $state<string | null>(null);
  let name = $state('');
  let folder = $state<string | null>(null);
  let dayOrder = $state<string>('dmy');
  let mediaRoots = $state<string[]>([]);
  let warning = $state<LocationWarningDto | null>(null);
  let busy = $state(false);
  let error = $state('');
  let phase = $state<ImportProgress['phase'] | null>(null);
  let fraction = $state(0);

  const ready = $derived(name.trim().length > 0 && !!folder && (mode === 'create' || !!file));
  const progress = $derived(Math.round(fraction * 100));

  function baseName(p: string): string {
    const last = p.replace(/[\\/]+$/, '').split(/[\\/]/).pop() ?? '';
    return last.replace(/\.[^.]+$/, '');
  }

  async function chooseFile() {
    const f = await platform.pickFile({ title: t('imp.pickFileTitle'), extensions: IMPORT_EXTENSIONS });
    if (!f) return;
    file = f;
    if (!name.trim()) name = baseName(f);
  }

  async function chooseFolder() {
    const f = await platform.pickFolder(t('imp.pickKeepFolder'));
    if (!f) return;
    folder = f;
    warning = await api.checkLocation(f);
  }

  async function addMedia() {
    const f = await platform.pickFolder(t('imp.pickPhotoFolder'));
    if (f && !mediaRoots.includes(f)) mediaRoots = [...mediaRoots, f];
  }

  function onProgress(p: ImportProgress) {
    phase = p.phase;
    const i = PHASES.indexOf(p.phase);
    fraction = (i + (p.total > 0 ? p.done / p.total : 0)) / PHASES.length;
  }

  async function start(e: SubmitEvent) {
    e.preventDefault();
    if (!ready || busy) return;
    busy = true;
    error = '';
    try {
      const dir = `${folder!.replace(/[\\/]+$/, '')}/${name.trim()}`;
      if (mode === 'create') {
        const info = await api.createProject(dir, name.trim());
        await enterProject(api, info);
        if (thenTrace) nav.go({ name: 'trace' });
      } else {
        phase = 'reading';
        fraction = 0;
        const report = await api.importGedcom(
          { file: file!, projectDir: dir, projectName: name.trim(), mediaRoots, dayOrder: dayOrder as DayOrder },
          onProgress,
        );
        project.open(report.project, report);
        await settings.load(api).catch(() => {});
        nav.go({ name: 'importReport' });
      }
    } catch (err) {
      error = (err as Error).message;
      phase = null;
    } finally {
      busy = false;
    }
  }
</script>

<form class="wiz" onsubmit={start} aria-labelledby="wiz-title">
  <h2 id="wiz-title">{mode === 'import' ? t('imp.wizImport') : thenTrace ? t('imp.wizTrace') : t('imp.wizCreate')}</h2>

  {#if error}<Banner kind="error">{error}</Banner>{/if}

  {#if mode === 'import'}
    <div class="pick">
      <span class="k" id="file-k">{t('imp.treeFile')}</span>
      <Button onclick={chooseFile} disabled={busy} aria-describedby="file-k">{t('imp.chooseFile')}</Button>
      <bdi dir="auto" class="v">{file ?? t('imp.noFile')}</bdi>
      <p class="hint">{t('imp.formats')}</p>
    </div>
  {/if}

  <TextField label={t('imp.treeName')} bind:value={name} autocomplete="off" disabled={busy} />

  <div class="pick">
    <span class="k" id="folder-k">{t('imp.keepFolder')}</span>
    <Button onclick={chooseFolder} disabled={busy} aria-describedby="folder-k">{t('imp.chooseFolder')}</Button>
    <bdi dir="auto" class="v">{folder ?? t('imp.noFolder')}</bdi>
  </div>

  {#if warning}
    <Banner kind="warning">
      {t('imp.syncWarning', { provider: warning.provider ?? t('imp.networkShare') })}
    </Banner>
  {/if}

  {#if mode === 'import'}
    <div>
      <RadioGroup
        legend={t('settings.dateOrder')}
        bind:value={dayOrder}
        options={[
          { value: 'dmy', label: t('imp.dayFirst') },
          { value: 'mdy', label: t('imp.monthFirst') },
        ]}
      />
      <p class="hint">{t('imp.dateOrderHint')}</p>
    </div>

    <div class="media">
      <span class="k">{t('imp.photosOptional')}</span>
      {#if mediaRoots.length > 0}
        <ul>
          {#each mediaRoots as m (m)}
            <li>
              <bdi dir="auto">{m}</bdi>
              <IconButton
                label={t('imp.removeFolder', { path: m })}
                icon="x"
                onclick={() => (mediaRoots = mediaRoots.filter((x) => x !== m))}
              />
            </li>
          {/each}
        </ul>
      {/if}
      <div><Button onclick={addMedia} disabled={busy}>{t('imp.addMediaFolder')}</Button></div>
    </div>
  {/if}

  {#if busy && mode === 'import'}
    <div class="prog">
      <ProgressBar label={t('imp.progress')} value={progress} />
      <p role="status">{phase ? t(PHASE_KEYS[phase]) : ''}…</p>
    </div>
  {/if}

  <div class="actions">
    <Button onclick={oncancel} disabled={busy}>{t('common.cancel')}</Button>
    <Button variant="primary" type="submit" disabled={!ready || busy}>
      {mode === 'import' ? t('imp.startImport') : t('imp.createTree')}
    </Button>
  </div>
</form>

<style>
  .wiz {
    display: grid;
    gap: var(--s-5);
    max-width: 38rem;
    padding: var(--s-6);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background: var(--paper-1);
  }
  .pick {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: var(--s-2) var(--s-4);
  }
  .pick .k {
    grid-column: 1 / -1;
    font-weight: 500;
  }
  .k {
    font-weight: 500;
  }
  .v {
    color: var(--ink-1);
    font-size: var(--t-sm);
    overflow-wrap: anywhere;
  }
  .hint {
    color: var(--ink-1);
    font-size: var(--t-sm);
    margin-top: var(--s-2);
  }
  .pick .hint {
    grid-column: 1 / -1;
    margin: 0;
  }
  .media {
    display: grid;
    gap: var(--s-3);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-3);
    border-block-end: 1px solid var(--rule);
    font-size: var(--t-sm);
  }
  .prog {
    display: grid;
    gap: var(--s-3);
  }
  .prog p {
    color: var(--ink-1);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--s-3);
  }
</style>
