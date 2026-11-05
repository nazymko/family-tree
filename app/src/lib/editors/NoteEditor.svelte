<script lang="ts">
  import type { EditResult, NoteDto, OwnerRef } from '../api/types';
  import { getApi } from '../api/context';
  import TextArea from '../ui/TextArea.svelte';
  import Select from '../ui/Select.svelte';
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
  title={note ? t('edit.editNote') : t('editor.addNote')}
  saveLabel={t('edit.saveNote')}
  watch={[note?.id, target.id]}
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
</EditorDialog>
