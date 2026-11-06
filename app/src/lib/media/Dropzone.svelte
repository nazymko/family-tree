<script lang="ts" module>
  import { errMessage } from '../editors/errors';
  import { backendError } from '../i18n/backend';
  import type { MessageKey } from '../i18n';
  import { AUDIO_EXTENSIONS } from './sound';
  /** Pictures and documents: accepted on everything but a note. */
  export const MEDIA_EXTENSIONS = ['jpg', 'jpeg', 'png', 'webp', 'tif', 'tiff', 'gif', 'pdf'];
  export const fileName = (path: string) => path.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || path;
  /** The backend's refusals that the app words itself, in the language in use. */
  const KNOWN_REASONS: Record<string, MessageKey> = {
    'only sound recordings can be attached to a note': 'voice.onlySoundOnNote',
    'sound recordings can only be attached to a note': 'voice.soundOnlyOnNote',
  };
  /** One line per failed file, e.g. "scan.exe: unsupported type; we attach ...". `owner` is what the files were for. */
  export function failureLine(f: { path: string; reason: string }, owner: 'note' | 'other' = 'other'): string {
    const name = fileName(f.path);
    const known = KNOWN_REASONS[f.reason];
    if (known) return `${name}: ${t(known)}`;
    const ext = /\.([A-Za-z0-9]{1,8})$/.exec(name)?.[1]?.toLowerCase();
    if (ext && !MEDIA_EXTENSIONS.includes(ext) && !AUDIO_EXTENSIONS.includes(ext)) {
      return `${name}: ${t(owner === 'note' ? 'voice.unsupported' : 'media.unsupported')}`;
    }
    return `${name}: ${backendError(f.reason)}`;
  }
</script>

<script lang="ts">
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import { getApi, getPlatform } from '../api/context';
  import { notices } from '../ui/notices.svelte';
  import { undoNotice } from '../ui/undoable';
  import type { AttachResult, OwnerRef } from '../api/api';
  import { t, plural } from '../i18n';

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
  let warnings = $state<string[]>([]);
  let suggestion = $state<string | null>(null);
  let looked = $state<string | null>(null);
  const name = $derived(targetName ?? looked ?? null);

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
    warnings = [];
    suggestion = null;
    let r: AttachResult;
    try {
      r = await api.attachFiles(target, paths, { silent: true });
    } catch (e) {
      failures = [errMessage(e, t('edit.wentWrong'))];
      return;
    }
    // A file can be in both lists: attached, but its preview could not be made. That is a warning, not a failure.
    const attachedNames = new Set(r.attached.map((m) => (m.originalName ?? '').toLowerCase()));
    const isWarning = (f: { path: string; reason: string }) =>
      /too large to preview/i.test(f.reason) && attachedNames.has(fileName(f.path).toLowerCase());
    failures = r.failed.filter((f) => !isWarning(f)).map((f) => failureLine(f));
    warnings = r.failed.filter(isWarning).map((f) => `${fileName(f.path)}: ${t('media.noPreview')}`);
    const n = r.attached.length;
    if (n > 0) {
      notices.show(undoNotice(api, plural('media.attached', n), await api.undoState(), onUndone));
      if (n === 1 && r.attached[0].mime?.startsWith('image/')) {
        const ex = await api.exifSuggestion(r.attached[0].id).catch(() => null);
        suggestion = ex?.date ?? null;
      }
    }
    onAttached(r);
  }

  async function pick() {
    const paths = await platform.pickFiles({ title: t('media.pickTitle'), extensions: MEDIA_EXTENSIONS });
    await attach(paths);
  }
</script>

<div class="dz">
  <Button onclick={pick}>{t('media.addFiles')}</Button>
  {#if failures.length}
    <Banner kind="error">
      <p>{t('media.notAttached')}</p>
      <ul>
        {#each failures as f (f)}<li>{f}</li>{/each}
      </ul>
    </Banner>
  {/if}
  {#if warnings.length}
    <Banner kind="warning">
      <ul>
        {#each warnings as w (w)}<li>{w}</li>{/each}
      </ul>
    </Banner>
  {/if}
  {#if suggestion}
    <Banner kind="info">
      <span>{t('media.useDate', { date: suggestion })}</span>
      <span class="acts">
        <Button
          onclick={() => {
            const d = suggestion!;
            suggestion = null;
            onSuggestEvent?.(d, target);
          }}>{t('media.createEvent')}</Button
        >
        <Button onclick={() => (suggestion = null)}>{t('media.noThanks')}</Button>
      </span>
    </Banner>
  {/if}
</div>

{#if hovering}
  <div class="overlay" data-testid="drop-overlay">
    <p>{name ? t('media.drop', { name }) : t('media.dropThis')}</p>
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
