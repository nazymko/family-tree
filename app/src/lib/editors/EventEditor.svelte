<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { EditResult, EventDto, EventInput, Id, PersonRef, PlaceOption } from '../api/types';
  import { getApi } from '../api/context';
  import { settings } from '../state/settings.svelte';
  import Select from '../ui/Select.svelte';
  import TextField from '../ui/TextField.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Button from '../ui/Button.svelte';
  import DateField from './DateField.svelte';
  import PlacePicker from './PlacePicker.svelte';
  import PersonPicker from './PersonPicker.svelte';
  import EditorDialog from './EditorDialog.svelte';
  import { errMessage, opt } from './errors';
  import { t, type MessageKey } from '../i18n';
  import { eventKindLabel } from '../i18n/backend';

  let {
    owner,
    event,
    onSaved,
    onCancel,
    citations,
    newKind,
  }: {
    owner: { kind: 'person' | 'family'; id: Id };
    event?: EventDto;
    /** The kind a NEW event starts with (default birth for a person, marriage for a family). */
    newKind?: string;
    onSaved: (r: EditResult) => void;
    onCancel: () => void;
    /** Task 12 renders the citations section here. */
    citations?: Snippet<[EventDto | undefined]>;
  } = $props();

  const api = getApi();

  // svelte-ignore state_referenced_locally
  const ownerKind = owner.kind;
  const LIFE = ['birth', 'christening', 'baptism', 'death', 'burial', 'cremation'];
  const FAMILY = ['marriage', 'divorce', 'annulment', 'engagement', 'separation'];
  const FACTS = [
    'occupation',
    'residence',
    'education',
    'religion',
    'nationality',
    'title',
    'census',
    'emigration',
    'immigration',
    'naturalization',
    'probate',
    'will',
    'military',
  ];
  const opts = (list: string[], group: MessageKey) =>
    list.map((v) => ({ value: v, label: eventKindLabel(v), group: t(group) }));
  const KINDS = $derived([
    ...(ownerKind === 'person' ? opts(LIFE, 'editor.groupLife') : opts(FAMILY, 'editor.groupFamily')),
    ...opts(FACTS, 'editor.groupFacts'),
    { value: 'custom', label: t('editor.otherKind'), group: t('editor.groupFacts') },
  ]);
  const KNOWN = new Set([...LIFE, ...FAMILY, ...FACTS]);
  const ROLES = $derived(
    [
      'principal',
      'witness',
      'godparent',
      'clergy',
      'officiator',
      'informant',
      'parent',
      'father',
      'mother',
      'child',
      'spouse',
      'friend',
      'neighbor',
      'other',
    ].map((r) => ({ value: r, label: t(`eventRole.${r}` as MessageKey) })),
  );

  function splitKind(k: string | undefined): { kind: string; custom: string } {
    if (!k) return { kind: ownerKind === 'family' ? 'marriage' : 'birth', custom: '' };
    if (k.startsWith('custom:')) return { kind: 'custom', custom: k.slice(7) };
    if (KNOWN.has(k)) return { kind: k, custom: '' };
    return { kind: 'custom', custom: k };
  }

  // svelte-ignore state_referenced_locally
  const k0 = splitKind(event?.kind ?? newKind);
  // svelte-ignore state_referenced_locally
  const init = {
    kind: k0.kind,
    custom: k0.custom,
    date: event?.date?.raw ?? '',
    placeId: event?.place?.id as Id | undefined,
    placeText: '',
    description: event?.description ?? '',
    age: event?.age ?? '',
    cause: event?.cause ?? '',
    negative: event?.negative ?? false,
    isPrivate: event?.private ?? false,
    preferred: event?.preferred ?? false,
    participants: (event?.participants ?? []).map((p) => ({ person: p.person, role: p.role })),
  };
  let f = $state({ ...init, participants: init.participants.map((p) => ({ ...p })) });
  // svelte-ignore state_referenced_locally
  const startPlace: PlaceOption | null = event?.place ?? null;
  let saving = $state(false);
  let error = $state('');
  let pickerKey = $state(0);

  const dirty = $derived(JSON.stringify(f) !== JSON.stringify(init));
  const showCause = $derived(f.kind === 'death' || f.kind === 'burial');
  const kindValue = $derived(f.kind === 'custom' ? `custom:${f.custom.trim()}` : f.kind);
  const canSave = $derived(f.kind !== 'custom' || f.custom.trim().length > 0);

  function addParticipant(p: PersonRef | null) {
    if (!p) return;
    if (!f.participants.some((x) => x.person.id === p.id)) f.participants.push({ person: p, role: 'witness' });
    pickerKey += 1;
  }

  async function save() {
    saving = true;
    error = '';
    const input: EventInput = {
      kind: kindValue,
      negative: f.negative,
      private: f.isPrivate,
      preferred: f.preferred,
    };
    if (event) input.id = event.id;
    const date = opt(f.date);
    if (date) input.dateRaw = date;
    if (f.placeId !== undefined) input.placeId = f.placeId;
    else if (opt(f.placeText)) input.placeText = opt(f.placeText);
    else if (event?.place) input.placeText = ''; // the place was cleared
    const description = opt(f.description);
    if (description) input.description = description;
    const age = opt(f.age);
    if (age) input.age = age;
    const cause = showCause ? opt(f.cause) : undefined;
    if (cause) input.cause = cause;
    if (f.participants.length > 0 || (event?.participants.length ?? 0) > 0) {
      input.participants = f.participants.map((p) => ({ person: p.person.id, role: p.role }));
    }
    try {
      onSaved(await api.apply({ type: 'saveEvent', owner, event: input }));
    } catch (e) {
      error = errMessage(e);
    } finally {
      saving = false;
    }
  }

  async function remove() {
    if (!event) return;
    saving = true;
    try {
      onSaved(await api.apply({ type: 'deleteEvent', owner, event: event.id }));
    } catch (e) {
      error = errMessage(e);
    } finally {
      saving = false;
    }
  }
</script>

<EditorDialog
  title={event ? t('editor.editEvent', { kind: eventKindLabel(event.kind, event.label).toLowerCase() }) : t('editor.addEvent')}
  saveLabel={t('edit.saveEvent')}
  {dirty}
  {saving}
  {error}
  {canSave}
  onsave={save}
  oncancel={onCancel}
>
  <Select label={t('editor.kindOfEvent')} options={KINDS} bind:value={f.kind} />
  {#if f.kind === 'custom'}
    <TextField label={t('editor.customName')} bind:value={f.custom} />
  {/if}
  <DateField label={t('editor.date')} value={f.date} onChange={(raw) => (f.date = raw)} order={settings.value.dayOrder} />
  <PlacePicker
    label={t('editor.place')}
    value={startPlace}
    text=""
    onChange={(v) => {
      f.placeId = v?.placeId;
      f.placeText = v?.placeText ?? '';
    }}
  />
  <TextField label={t('editor.description')} bind:value={f.description} />
  <TextField label={t('editor.age')} hint={t('editor.ageHint')} bind:value={f.age} />
  {#if showCause}<TextField label={t('editor.cause')} bind:value={f.cause} />{/if}

  <fieldset class="parts">
    <legend>{t('editor.participants')}</legend>
    {#if f.participants.length > 0}
      <table>
        <thead>
          <tr><th scope="col">{t('editor.person')}</th><th scope="col">{t('editor.role')}</th><th scope="col"><span class="sr">{t('editor.actions')}</span></th></tr>
        </thead>
        <tbody>
          {#each f.participants as p, i (p.person.id)}
            <tr>
              <td><bdi dir="auto">{p.person.name}</bdi></td>
              <td><Select label={t('editor.roleOf', { name: p.person.name })} options={ROLES} bind:value={p.role} /></td>
              <td>
                <Button variant="quiet" aria-label={t('editor.removeNamed', { name: p.person.name })} onclick={() => f.participants.splice(i, 1)}
                  >{t('common.remove')}</Button
                >
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
    {#key pickerKey}
      <PersonPicker
        label={t('editor.addParticipant')}
        value={null}
        exclude={f.participants.map((p) => p.person.id)}
        onPick={addParticipant}
      />
    {/key}
  </fieldset>

  <Checkbox
    label={t('editor.negative')}
    hint={t('editor.negativeHint')}
    bind:checked={f.negative}
  />
  <Checkbox label={t('editor.private')} bind:checked={f.isPrivate} />
  <Checkbox label={t('editor.preferred')} bind:checked={f.preferred} />

  {#if citations}{@render citations(event)}{/if}

  {#snippet extra()}
    {#if event}<Button variant="danger" onclick={remove}>{t('editor.deleteEvent')}</Button>{/if}
  {/snippet}
</EditorDialog>

<style>
  .parts {
    display: grid;
    gap: var(--s-3);
    margin: 0;
    padding: 0;
    border: 0;
    min-inline-size: 0;
  }
  legend {
    padding: 0;
    font-weight: 500;
    margin-bottom: var(--s-3);
  }
  table {
    border-collapse: collapse;
    width: 100%;
    font-size: var(--t-sm);
  }
  th {
    text-align: start;
    font-weight: 500;
    color: var(--ink-1);
    padding: var(--s-2) var(--s-3) var(--s-2) 0;
  }
  td {
    padding: var(--s-2) var(--s-3) var(--s-2) 0;
    border-top: 1px solid var(--rule);
    vertical-align: middle;
  }
  .sr {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
</style>
