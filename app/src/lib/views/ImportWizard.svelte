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

  let { mode, oncancel }: { mode: 'import' | 'create'; oncancel: () => void } = $props();

  const api = getApi();
  const platform = getPlatform();

  const PHASES: ImportProgress['phase'][] = ['reading', 'parsing', 'copying_media', 'saving', 'indexing'];
  const PHASE_TEXT: Record<ImportProgress['phase'], string> = {
    reading: 'Reading the file',
    parsing: 'Reading people and families',
    copying_media: 'Copying photos and files',
    saving: 'Saving the tree',
    indexing: 'Building the search index',
  };

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
    const f = await platform.pickFile({ title: 'Choose a GEDCOM file', extensions: ['ged', 'gedcom'] });
    if (!f) return;
    file = f;
    if (!name.trim()) name = baseName(f);
  }

  async function chooseFolder() {
    const f = await platform.pickFolder('Choose where to keep the tree');
    if (!f) return;
    folder = f;
    warning = await api.checkLocation(f);
  }

  async function addMedia() {
    const f = await platform.pickFolder('Choose a folder with photos and files');
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
  <h2 id="wiz-title">{mode === 'import' ? 'Import a GEDCOM file' : 'Create a new tree'}</h2>

  {#if error}<Banner kind="error">{error}</Banner>{/if}

  {#if mode === 'import'}
    <div class="pick">
      <span class="k" id="file-k">GEDCOM file</span>
      <Button onclick={chooseFile} disabled={busy} aria-describedby="file-k">Choose file…</Button>
      <bdi dir="auto" class="v">{file ?? 'No file chosen'}</bdi>
    </div>
  {/if}

  <TextField label="Tree name" bind:value={name} autocomplete="off" disabled={busy} />

  <div class="pick">
    <span class="k" id="folder-k">Folder to keep the tree in</span>
    <Button onclick={chooseFolder} disabled={busy} aria-describedby="folder-k">Choose folder…</Button>
    <bdi dir="auto" class="v">{folder ?? 'No folder chosen'}</bdi>
  </div>

  {#if warning}
    <Banner kind="warning">
      This folder is inside {warning.provider ?? 'a network share'}. Syncing can corrupt the database. Keep the tree
      outside synced folders and use backups to move it.
    </Banner>
  {/if}

  {#if mode === 'import'}
    <div>
      <RadioGroup
        legend="Date order"
        bind:value={dayOrder}
        options={[
          { value: 'dmy', label: 'Day, month, year' },
          { value: 'mdy', label: 'Month, day, year' },
        ]}
      />
      <p class="hint">Used only for dates like 03/04/1901</p>
    </div>

    <div class="media">
      <span class="k">Photos and files (optional)</span>
      {#if mediaRoots.length > 0}
        <ul>
          {#each mediaRoots as m (m)}
            <li>
              <bdi dir="auto">{m}</bdi>
              <IconButton
                label="Remove {m}"
                icon="x"
                onclick={() => (mediaRoots = mediaRoots.filter((x) => x !== m))}
              />
            </li>
          {/each}
        </ul>
      {/if}
      <div><Button onclick={addMedia} disabled={busy}>Add a media folder…</Button></div>
    </div>
  {/if}

  {#if busy && mode === 'import'}
    <div class="prog">
      <ProgressBar label="Import progress" value={progress} />
      <p role="status">{phase ? PHASE_TEXT[phase] : ''}…</p>
    </div>
  {/if}

  <div class="actions">
    <Button onclick={oncancel} disabled={busy}>Cancel</Button>
    <Button variant="primary" type="submit" disabled={!ready || busy}>
      {mode === 'import' ? 'Start import' : 'Create tree'}
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
