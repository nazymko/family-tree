<script lang="ts">
  import type { EditResult, Id, NameDto, NameInput } from '../api/types';
  import { getApi } from '../api/context';
  import TextField from '../ui/TextField.svelte';
  import Select from '../ui/Select.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Button from '../ui/Button.svelte';
  import DateField from './DateField.svelte';
  import { settings } from '../state/settings.svelte';
  import EditorDialog from './EditorDialog.svelte';
  import { errMessage, opt, splitList } from './errors';
  import { t } from '../i18n';
  import { OLD_LETTERS, insertAtCaret } from './oldLetters';
  import { oldTypeface, RECORDED_FAMILY } from '../ui/namesPref.svelte';

  let {
    person,
    name,
    onSaved,
    onCancel,
  }: { person: Id; name?: NameDto; onSaved: (r: EditResult) => void; onCancel: () => void } = $props();

  const api = getApi();
  const KINDS = $derived(
    (['birth', 'married', 'aka', 'religious', 'nickname', 'immigrant', 'professional', 'as_recorded', 'other'] as const).map((value) => ({
      value,
      label: t(`nameKind.${value}`),
    })),
  );

  // svelte-ignore state_referenced_locally
  const init = {
    kind: (name?.kind ?? 'birth') as string,
    kindText: name?.kindText ?? '',
    prefix: name?.prefix ?? '',
    given: name?.given ?? '',
    call: name?.call ?? '',
    surnamePrefix: name?.surnamePrefix ?? '',
    surnames: (name?.surnames ?? []).join(', '),
    patronymic: name?.patronymic ?? '',
    suffix: name?.suffix ?? '',
    nickname: name?.nickname ?? '',
    lang: name?.lang ?? '',
    dateRaw: name?.date?.raw ?? '',
    preferred: name?.preferred ?? false,
    isPrivate: name?.private ?? false,
  };
  let f = $state({ ...init });
  /**
   * Fields most names never use. They stay folded so the surname is not typed into the surname prefix just above it
   * (a real tree had its married surname land there); a field that already has a value keeps the section open.
   */
  let more =$state([init.prefix, init.call, init.surnamePrefix, init.suffix, init.nickname, init.lang].some((v) => v.trim() !== ''));
  /** The name field the caret was last in: the old-letter buttons insert there (a button click moves focus off the field). */
  let lastField: HTMLInputElement | null = null;
  function rememberField(e: FocusEvent): void {
    const el = e.target;
    if (el instanceof HTMLInputElement && el.dataset.oldLetters !== undefined) lastField = el;
  }
  function insertLetter(ch: string): void {
    const el = lastField;
    if (!el || !el.isConnected) return;
    const at = insertAtCaret(el.value, el.selectionStart ?? el.value.length, el.selectionEnd ?? el.value.length, ch);
    el.value = at.value;
    // The field's own binding listens to `input`.
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.focus();
    el.setSelectionRange(at.caret, at.caret);
  }
  let saving = $state(false);
  let error = $state('');
  const dirty = $derived(JSON.stringify(f) !== JSON.stringify(init));

  async function save() {
    saving = true;
    error = '';
    const input: NameInput = {
      ...(name ? { id: name.id } : {}),
      kind: f.kind as NameInput['kind'],
      preferred: f.preferred,
      private: f.isPrivate,
      surnames: splitList(f.surnames),
    };
    const extras: Partial<NameInput> = {
      kindText: f.kind === 'other' ? opt(f.kindText) : undefined,
      prefix: opt(f.prefix),
      given: opt(f.given),
      call: opt(f.call),
      surnamePrefix: opt(f.surnamePrefix),
      patronymic: opt(f.patronymic),
      suffix: opt(f.suffix),
      nickname: opt(f.nickname),
      lang: opt(f.lang),
      dateRaw: opt(f.dateRaw),
    };
    for (const [k, v] of Object.entries(extras)) if (v !== undefined) (input as unknown as Record<string, unknown>)[k] = v;
    try {
      onSaved(await api.apply({ type: 'saveName', person, name: input }));
    } catch (e) {
      error = errMessage(e);
    } finally {
      saving = false;
    }
  }
</script>

<EditorDialog
  title={name ? t('edit.editName') : t('editor.addName')}
  saveLabel={t('editor.saveName')}
  watch={[person]}
  aboutPerson
  {dirty}
  {saving}
  {error}
  onsave={save}
  oncancel={onCancel}
>
  <Select label={t('editor.kindOfName')} options={KINDS} bind:value={f.kind} />
  {#if f.kind === 'other'}<TextField label={t('editor.describeKind')} bind:value={f.kindText} />{/if}
  <div class="name-fields" class:recorded={f.kind === 'as_recorded' && oldTypeface.value} onfocusin={rememberField}>
    <TextField label={t('editor.given')} data-old-letters="" bind:value={f.given} />
    <TextField label={t('editor.patronymic')} data-old-letters="" bind:value={f.patronymic} />
    <TextField label={t('editor.surnames')} hint={t('editor.surnamesHint')} data-old-letters="" bind:value={f.surnames} />
    <div class="old-letters" role="group" aria-label={t('editor.oldLetters')}>
      {#each OLD_LETTERS as l (l.ch)}
        {@const label = t('editor.insertOldLetter', { letter: l.ch, name: t(`editor.oldLetterName.${l.name}`) })}
        <button
          type="button"
          class="old-letter"
          aria-label={label}
          title={label}
          style:font-family="{RECORDED_FAMILY}, var(--font-serif)"
          onmousedown={(e) => e.preventDefault()}
          onclick={() => insertLetter(l.ch)}>{l.ch}</button
        >
      {/each}
    </div>
  </div>
  {#if more}
    <TextField label={t('editor.surnamePrefix')} hint={t('editor.surnamePrefixHint')} bind:value={f.surnamePrefix} />
    <TextField label={t('editor.prefix')} hint={t('editor.prefixHint')} bind:value={f.prefix} />
    <TextField label={t('editor.call')} hint={t('editor.callHint')} bind:value={f.call} />
    <TextField label={t('editor.suffix')} hint={t('editor.suffixHint')} bind:value={f.suffix} />
    <TextField label={t('editor.nickname')} bind:value={f.nickname} />
    <TextField label={t('editor.language')} hint={t('editor.languageHint')} bind:value={f.lang} />
  {:else}
    <div><Button variant="quiet" onclick={() => (more = true)}>{t('editor.moreNameFields')}</Button></div>
  {/if}
  <DateField label={t('editor.dateInUse')} value={f.dateRaw} onChange={(r) => (f.dateRaw = r)} order={settings.value.dayOrder} />
  <Checkbox label={t('editor.preferredName')} bind:checked={f.preferred} />
  <Checkbox label={t('editor.private')} bind:checked={f.isPrivate} />
</EditorDialog>

<style>
  .name-fields {
    display: flex;
    flex-direction: column;
    gap: var(--s-5);
  }
  .name-fields.recorded :global(input.control) {
    font-family: var(--font-recorded);
  }
  .old-letters {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
  .old-letter {
    min-width: 2rem;
    height: 2rem;
    padding: 0;
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background: var(--paper-1);
    color: var(--ink-0);
    font-size: 1.0625rem;
    cursor: pointer;
  }
  .old-letter:hover {
    background: var(--paper-2, var(--paper-1));
  }
  .old-letter:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
</style>
