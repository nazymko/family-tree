<script lang="ts">
  import type { EditResult, NoteDto, OwnerRef } from '../api/types';
  import { getApi } from '../api/context';
  import TextArea from '../ui/TextArea.svelte';
  import Select from '../ui/Select.svelte';
  import EditorDialog from './EditorDialog.svelte';
  import { errMessage } from './errors';

  let {
    target,
    note,
    onSaved,
    onCancel,
  }: { target: OwnerRef; note?: NoteDto; onSaved: (r: EditResult) => void; onCancel: () => void } = $props();

  const api = getApi();
  const KINDS = [
    ['general', 'General'],
    ['story', 'Story'],
    ['research', 'Research'],
    ['transcription', 'Transcription'],
    ['conclusion', 'Conclusion'],
    ['other', 'Other'],
  ].map(([value, label]) => ({ value, label }));

  // svelte-ignore state_referenced_locally
  const init = { kind: note?.kind ?? 'general', text: note?.text ?? '' };
  let kind = $state<string>(init.kind);
  let text = $state(init.text);
  let saving = $state(false);
  let error = $state('');
  const dirty = $derived(kind !== init.kind || text !== init.text);

  async function save() {
    saving = true;
    error = '';
    try {
      onSaved(
        await api.apply({
          type: 'saveNote',
          target,
          note: { ...(note ? { id: note.id } : {}), kind: kind as NoteDto['kind'], text },
        }),
      );
    } catch (e) {
      error = errMessage(e);
    } finally {
      saving = false;
    }
  }
</script>

<EditorDialog
  title={note ? 'Edit note' : 'Add note'}
  saveLabel="Save note"
  {dirty}
  {saving}
  {error}
  canSave={text.trim().length > 0}
  onsave={save}
  oncancel={onCancel}
>
  <Select label="Kind of note" options={KINDS} bind:value={kind} />
  <TextArea
    label="Note"
    hint="Markdown is kept as you write it. Link to a person with genetree://person/ followed by their id."
    ruled
    rows={10}
    bind:value={text}
  />
</EditorDialog>
