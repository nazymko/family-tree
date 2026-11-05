<script lang="ts">
  import type { EditResult, MediaDto, MediaLinkDto, NoteDto, OwnerRef } from '../api/types';
  import { getApi, getPlatform } from '../api/context';
  import TextArea from '../ui/TextArea.svelte';
  import Select from '../ui/Select.svelte';
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import VoiceSamples, { voiceSamples } from '../media/VoiceSamples.svelte';
  import { AUDIO_EXTENSIONS, fileName, failureLine } from '../media/Dropzone.svelte';
  import EditorDialog from './EditorDialog.svelte';
  import { errMessage } from './errors';
  import { t } from '../i18n';

  let {
    target,
    note,
    onSaved,
    onCancel,
  }: { target: OwnerRef; note?: NoteDto; onSaved: (r: EditResult) => void; onCancel: () => void } = $props();

  const api = getApi();
  const platform = getPlatform();
  const KINDS = $derived(
    (['general', 'story', 'research', 'transcription', 'conclusion', 'other'] as const).map((value) => ({
      value,
      label: t(`noteKind.${value}`),
    })),
  );

  // svelte-ignore state_referenced_locally
  const init = { kind: note?.kind ?? 'general', text: note?.text ?? '' };
  let kind = $state<string>(init.kind);
  let text = $state(init.text);
  let saving = $state(false);
  let error = $state('');

  // Voice samples. On a note that exists they are attached and removed at once, each as its own undo step.
  // On a new note the picked files wait here and are attached right after the note is saved.
  // svelte-ignore state_referenced_locally
  let noteId = $state<string | undefined>(note?.id);
  // svelte-ignore state_referenced_locally
  let media = $state<MediaLinkDto[]>(note?.media ?? []);
  let staged = $state<string[]>([]);
  let failures = $state<string[]>([]);
  let working = $state(false);
  let hovering = $state(false);
  const dirty = $derived(kind !== init.kind || text !== init.text || staged.length > 0);
  const onNote = $derived<OwnerRef | null>(noteId ? { kind: 'note', id: noteId } : null);

  $effect(() => {
    return platform.onFilesDropped(
      (paths) => void addPaths(paths),
      (over) => (hovering = over),
    );
  });

  async function addPaths(paths: string[]) {
    if (paths.length === 0 || saving) return;
    failures = [];
    if (!onNote) {
      staged = [...new Set([...staged, ...paths])];
      return;
    }
    working = true;
    try {
      const r = await api.attachFiles(onNote, paths);
      const have = new Set(media.map((l) => l.media.id));
      media = [...media, ...r.attached.filter((m) => !have.has(m.id)).map((m) => ({ media: m, primary: false }))];
      failures = r.failed.map(failureLine);
    } catch (e) {
      failures = [errMessage(e)];
    } finally {
      working = false;
    }
  }

  async function pick() {
    await addPaths(await platform.pickFiles({ title: t('voice.pickTitle'), extensions: AUDIO_EXTENSIONS }));
  }

  async function removeSample(m: MediaDto) {
    if (!onNote) return;
    working = true;
    failures = [];
    try {
      await api.apply({ type: 'detachMedia', target: onNote, media: m.id });
      media = media.filter((l) => l.media.id !== m.id);
    } catch (e) {
      failures = [errMessage(e)];
    } finally {
      working = false;
    }
  }

  async function retitle(m: MediaDto, title: string) {
    if (title === (m.title ?? '')) return;
    working = true;
    failures = [];
    try {
      await api.apply({ type: 'updateMedia', id: m.id, title });
      media = media.map((l) => (l.media.id === m.id ? { ...l, media: { ...l.media, title: title || null } } : l));
    } catch (e) {
      failures = [errMessage(e)];
    } finally {
      working = false;
    }
  }

  /** The id of the note a first save made: the touched record that some owner holds as a note. */
  async function newNoteId(r: EditResult): Promise<string | undefined> {
    for (const id of r.touched) {
      const home = await api.recordHome('note', id).catch(() => null);
      if (home?.note) return home.note.id;
    }
    return undefined;
  }

  async function save() {
    saving = true;
    error = '';
    try {
      const r = await api.apply({
        type: 'saveNote',
        target,
        note: { ...(noteId ? { id: noteId } : {}), kind: kind as NoteDto['kind'], text },
      });
      if (!noteId && staged.length > 0) {
        const id = await newNoteId(r);
        if (!id) {
          error = t('voice.attachFailed');
          return;
        }
        noteId = id;
        const waiting = staged;
        staged = [];
        const res = await api.attachFiles({ kind: 'note', id }, waiting, { silent: true });
        media = [...media, ...res.attached.map((m) => ({ media: m, primary: false }))];
        if (res.failed.length > 0) {
          // The note exists now; stay open on it so the files that failed can be seen and tried again.
          failures = res.failed.map(failureLine);
          error = t('voice.attachFailed');
          return;
        }
      }
      onSaved(r);
    } catch (e) {
      error = errMessage(e);
    } finally {
      saving = false;
    }
  }
</script>

<EditorDialog
  title={note ? t('edit.editNote') : t('editor.addNote')}
  saveLabel={t('edit.saveNote')}
  watch={[noteId, target.id]}
  {dirty}
  {saving}
  {error}
  canSave={text.trim().length > 0}
  onsave={save}
  oncancel={onCancel}
>
  <Select label={t('editor.kindOfNote')} options={KINDS} bind:value={kind} />
  <TextArea
    label={t('editor.note')}
    hint={t('editor.noteHint')}
    ruled
    rows={10}
    bind:value={text}
  />

  <section class="voice" aria-labelledby="voice-h" data-testid="voice-section">
    <h3 id="voice-h">{t('voice.title')}</h3>
    {#if noteId}<p class="hint">{t('voice.savedAsAdded')}</p>{/if}
    <div class="add"><Button onclick={pick} disabled={working || saving}>{t('voice.add')}</Button></div>
    {#if failures.length > 0}
      <Banner kind="error">
        <p>{t('voice.notAttached')}</p>
        <ul>
          {#each failures as f (f)}<li>{f}</li>{/each}
        </ul>
      </Banner>
    {/if}
    <VoiceSamples items={media} onRemove={removeSample} onTitle={retitle} busy={working || saving} />
    {#if staged.length > 0}
      <p class="hint">{t('voice.staged')}</p>
      <ul class="staged">
        {#each staged as p (p)}
          <li>
            <span><bdi dir="auto">{fileName(p)}</bdi></span>
            <Button variant="quiet" aria-label={t('voice.stagedRemove', { name: fileName(p) })} onclick={() => (staged = staged.filter((x) => x !== p))}>{t('voice.remove')}</Button>
          </li>
        {/each}
      </ul>
    {:else if voiceSamples(media).length === 0}
      <p class="hint">{t('voice.none')}</p>
    {/if}
  </section>
</EditorDialog>

{#if hovering}
  <div class="overlay" data-testid="note-drop-overlay"><p>{t('voice.dropHere')}</p></div>
{/if}

<style>
  .voice {
    display: grid;
    gap: var(--s-3);
    margin-top: var(--s-5);
  }
  .voice h3 {
    margin: 0;
    font-size: var(--t-md);
  }
  .hint {
    margin: 0;
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  .add {
    justify-self: start;
  }
  ul {
    margin: var(--s-2) 0 0;
    padding-inline-start: var(--s-5);
  }
  .staged {
    list-style: none;
    padding: 0;
    display: grid;
    gap: var(--s-2);
  }
  .staged li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-3);
  }
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 400;
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
