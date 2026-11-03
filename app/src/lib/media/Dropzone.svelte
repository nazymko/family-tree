<script lang="ts" module>
  export const MEDIA_EXTENSIONS = ['jpg', 'jpeg', 'png', 'webp', 'tif', 'tiff', 'gif', 'pdf'];
  const UNSUPPORTED = 'unsupported type; we attach JPEG, PNG, WebP, TIFF, GIF and PDF files';
  export const fileName = (path: string) => path.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || path;
  /** One line per failed file, e.g. "scan.exe: unsupported type; we attach ...". */
  export function failureLine(f: { path: string; reason: string }): string {
    const name = fileName(f.path);
    const ext = /\.([A-Za-z0-9]{1,8})$/.exec(name)?.[1]?.toLowerCase();
    if (ext && !MEDIA_EXTENSIONS.includes(ext)) return `${name}: ${UNSUPPORTED}`;
    return `${name}: ${f.reason}`;
  }
</script>

<script lang="ts">
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import { getApi, getPlatform } from '../api/context';
  import { notices } from '../ui/notices.svelte';
  import { undoNotice } from '../ui/undoable';
  import type { AttachResult, OwnerRef } from '../api/api';

  let {
    target,
    targetName,
    onAttached,
    onUndone,
    onSuggestEvent,
  }: {
    target: OwnerRef;
    /** Name shown in the drop overlay; looked up for a person when omitted. */
    targetName?: string;
    onAttached: (r: AttachResult) => void;
    /** Called after the toast's Undo has run. */
    onUndone?: () => void;
    /** The user accepted the photo-date suggestion: open the event editor pre-filled with this date. */
    onSuggestEvent?: (date: string, target: OwnerRef) => void;
  } = $props();

  const api = getApi();
  const platform = getPlatform();
  let hovering = $state(false);
  let failures = $state<string[]>([]);
  let suggestion = $state<string | null>(null);
  let looked = $state<string | null>(null);
  const name = $derived(targetName ?? looked ?? 'this record');

  $effect(() => {
    if (targetName || target.kind !== 'person') return;
    let live = true;
    void api.getPerson(target.id).then((p) => {
      if (live) looked = p.ref.name;
    });
    return () => {
      live = false;
    };
  });

  $effect(() => {
    return platform.onFilesDropped(
      (paths) => void attach(paths),
      (over) => (hovering = over),
    );
  });

  async function attach(paths: string[]) {
    if (paths.length === 0) return;
    failures = [];
    suggestion = null;
    let r: AttachResult;
    try {
      r = await api.attachFiles(target, paths, { silent: true });
    } catch (e) {
      failures = [e instanceof Error ? e.message : String(e)];
      return;
    }
    failures = r.failed.map(failureLine);
    const n = r.attached.length;
    if (n > 0) {
      notices.show(undoNotice(api, `Attached ${n} ${n === 1 ? 'file' : 'files'}`, await api.undoState(), onUndone));
      if (n === 1 && r.attached[0].mime?.startsWith('image/')) {
        const ex = await api.exifSuggestion(r.attached[0].id).catch(() => null);
        suggestion = ex?.date ?? null;
      }
    }
    onAttached(r);
  }

  async function pick() {
    const paths = await platform.pickFiles({ title: 'Add photos and documents', extensions: MEDIA_EXTENSIONS });
    await attach(paths);
  }
</script>

<div class="dz">
  <Button onclick={pick}>Add files…</Button>
  {#if failures.length}
    <Banner kind="error">
      <p>These files were not attached:</p>
      <ul>
        {#each failures as f (f)}<li>{f}</li>{/each}
      </ul>
    </Banner>
  {/if}
  {#if suggestion}
    <Banner kind="info">
      <span>Use the photo's date ({suggestion}) for a new event?</span>
      <span class="acts">
        <Button
          onclick={() => {
            const d = suggestion!;
            suggestion = null;
            onSuggestEvent?.(d, target);
          }}>Create event</Button
        >
        <Button onclick={() => (suggestion = null)}>No thanks</Button>
      </span>
    </Banner>
  {/if}
</div>

{#if hovering}
  <div class="overlay" data-testid="drop-overlay">
    <p>Drop photos and documents to attach them to {name}</p>
  </div>
{/if}

<style>
  .dz {
    display: grid;
    gap: var(--s-4);
    justify-items: start;
  }
  ul {
    margin: var(--s-2) 0 0;
    padding-inline-start: var(--s-5);
  }
  .acts {
    display: flex;
    gap: var(--s-3);
    margin-top: var(--s-3);
  }
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 150;
    display: grid;
    place-items: center;
    background: var(--scrim);
    pointer-events: none;
  }
  .overlay p {
    padding: var(--s-7) var(--s-8);
    background: var(--paper-1);
    border: 2px dashed var(--ribbon);
    border-radius: var(--r-2);
    font-family: var(--font-serif);
    font-size: var(--t-lg);
  }
</style>
