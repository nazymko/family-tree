<script lang="ts">
  import type { EditResult, Id, NameDto, NameInput } from '../api/types';
  import { getApi } from '../api/context';
  import TextField from '../ui/TextField.svelte';
  import Select from '../ui/Select.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import EditorDialog from './EditorDialog.svelte';
  import { errMessage, opt, splitList } from './errors';

  let {
    person,
    name,
    onSaved,
    onCancel,
  }: { person: Id; name?: NameDto; onSaved: (r: EditResult) => void; onCancel: () => void } = $props();

  const api = getApi();
  const KINDS = [
    ['birth', 'Birth name'],
    ['married', 'Married name'],
    ['aka', 'Also known as'],
    ['religious', 'Religious name'],
    ['nickname', 'Nickname'],
    ['immigrant', 'Immigrant name'],
    ['professional', 'Professional name'],
    ['other', 'Other'],
  ].map(([value, label]) => ({ value, label }));

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
  title={name ? 'Edit name' : 'Add name'}
  saveLabel="Save name"
  {dirty}
  {saving}
  {error}
  onsave={save}
  oncancel={onCancel}
>
  <Select label="Kind of name" options={KINDS} bind:value={f.kind} />
  {#if f.kind === 'other'}<TextField label="Describe the kind" bind:value={f.kindText} />{/if}
  <TextField label="Title or prefix" hint="For example Dr or Rev" bind:value={f.prefix} />
  <TextField label="Given name" bind:value={f.given} />
  <TextField label="Call name" hint="The given name the person was known by" bind:value={f.call} />
  <TextField label="Surname prefix" hint="For example van, de or von" bind:value={f.surnamePrefix} />
  <TextField label="Surnames" hint="Separate several surnames with a comma" bind:value={f.surnames} />
  <TextField label="Patronymic" bind:value={f.patronymic} />
  <TextField label="Suffix" hint="For example Jr or III" bind:value={f.suffix} />
  <TextField label="Nickname" bind:value={f.nickname} />
  <TextField label="Language" hint="A language code such as de or ru" bind:value={f.lang} />
  <TextField label="Date in use" bind:value={f.dateRaw} />
  <Checkbox label="Preferred name" bind:checked={f.preferred} />
  <Checkbox label="Private" bind:checked={f.isPrivate} />
</EditorDialog>
