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

  let {
    owner,
    event,
    onSaved,
    onCancel,
    citations,
  }: {
    owner: { kind: 'person' | 'family'; id: Id };
    event?: EventDto;
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
  const cap = (s: string) => s[0].toUpperCase() + s.slice(1);
  const opts = (list: string[], group: string) => list.map((v) => ({ value: v, label: cap(v), group }));
  const KINDS = [
    ...(ownerKind === 'person' ? opts(LIFE, 'Life') : opts(FAMILY, 'Family')),
    ...opts(FACTS, 'Facts'),
    { value: 'custom', label: 'Other…', group: 'Facts' },
  ];
  const KNOWN = new Set([...LIFE, ...FAMILY, ...FACTS]);
  const ROLES = [
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
  ].map((r) => ({ value: r, label: cap(r) }));

  function splitKind(k: string | undefined): { kind: string; custom: string } {
    if (!k) return { kind: ownerKind === 'family' ? 'marriage' : 'birth', custom: '' };
    if (k.startsWith('custom:')) return { kind: 'custom', custom: k.slice(7) };
    if (KNOWN.has(k)) return { kind: k, custom: '' };
    return { kind: 'custom', custom: k };
  }

  // svelte-ignore state_referenced_locally
  const k0 = splitKind(event?.kind);
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
  title={event ? `Edit ${event.label.toLowerCase()}` : 'Add event'}
  saveLabel="Save event"
  {dirty}
  {saving}
  {error}
  {canSave}
  onsave={save}
  oncancel={onCancel}
>
  <Select label="Kind of event" options={KINDS} bind:value={f.kind} />
  {#if f.kind === 'custom'}
    <TextField label="Name of the event or fact" bind:value={f.custom} />
  {/if}
  <DateField label="Date" value={f.date} onChange={(raw) => (f.date = raw)} order={settings.value.dayOrder} />
  <PlacePicker
    label="Place"
    value={startPlace}
    text=""
    onChange={(v) => {
      f.placeId = v?.placeId;
      f.placeText = v?.placeText ?? '';
    }}
  />
  <TextField label="Description" bind:value={f.description} />
  <TextField label="Age" hint="As written on the record, for example 72 or 3 months" bind:value={f.age} />
  {#if showCause}<TextField label="Cause" bind:value={f.cause} />{/if}

  <fieldset class="parts">
    <legend>Participants</legend>
    {#if f.participants.length > 0}
      <table>
        <thead>
          <tr><th scope="col">Person</th><th scope="col">Role</th><th scope="col"><span class="sr">Actions</span></th></tr>
        </thead>
        <tbody>
          {#each f.participants as p, i (p.person.id)}
            <tr>
              <td><bdi dir="auto">{p.person.name}</bdi></td>
              <td><Select label="Role of {p.person.name}" options={ROLES} bind:value={p.role} /></td>
              <td>
                <Button variant="quiet" aria-label="Remove {p.person.name}" onclick={() => f.participants.splice(i, 1)}
                  >Remove</Button
                >
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
    {#key pickerKey}
      <PersonPicker
        label="Add participant"
        value={null}
        exclude={f.participants.map((p) => p.person.id)}
        onPick={addParticipant}
      />
    {/key}
  </fieldset>

  <Checkbox
    label="This event did not happen"
    hint="Records that the person never married, had no children, and so on"
    bind:checked={f.negative}
  />
  <Checkbox label="Private" bind:checked={f.isPrivate} />
  <Checkbox label="Preferred when there are several" bind:checked={f.preferred} />

  {#if citations}{@render citations(event)}{/if}

  {#snippet extra()}
    {#if event}<Button variant="danger" onclick={remove}>Delete event</Button>{/if}
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
