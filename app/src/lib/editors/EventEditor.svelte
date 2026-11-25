<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { EditResult, EventDto, EventInput, GodparentInput, Id, PersonRef, PlaceOption } from '../api/types';
  import { getApi } from '../api/context';
  import { settings } from '../state/settings.svelte';
  import Select from '../ui/Select.svelte';
  import TextField from '../ui/TextField.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Button from '../ui/Button.svelte';
  import DateField from './DateField.svelte';
  import DatePartsField from './DatePartsField.svelte';
  import PlacePicker from './PlacePicker.svelte';
  import PersonPicker from './PersonPicker.svelte';
  import EditorDialog from './EditorDialog.svelte';
  import { errMessage, opt } from './errors';
  import { t, type MessageKey } from '../i18n';
  import { eventKindLabel } from '../i18n/backend';
  import { shortGodparentName, takesGodparents } from '../ui/godparents';

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
  const LIFE = ['birth', 'christening', 'baptism', 'death', 'missing', 'burial', 'cremation'];
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
    godparents: (event?.godparents ?? []).map((g) => ({
      key: g.id,
      id: g.id as Id | undefined,
      person: g.person ?? null,
      name: g.name ?? '',
      role: g.role,
      note: g.note ?? '',
    })),
  };
  let f = $state({
    ...init,
    participants: init.participants.map((p) => ({ ...p })),
    godparents: init.godparents.map((g) => ({ ...g })),
  });
  // svelte-ignore state_referenced_locally
  const startPlace: PlaceOption | null = event?.place ?? null;
  let saving = $state(false);
  let error = $state('');
  let pickerKey = $state(0);

  const dirty = $derived(JSON.stringify(f) !== JSON.stringify(init));
  const showCause = $derived(f.kind === 'death' || f.kind === 'burial');
  const kindValue = $derived(f.kind === 'custom' ? `custom:${f.custom.trim()}` : f.kind);
  const canSave = $derived(f.kind !== 'custom' || f.custom.trim().length > 0);

  const showGodparents = $derived(takesGodparents(f.kind) || f.godparents.length > 0);
  const GOD_ROLES = $derived(
    (['godfather', 'godmother', 'godparent'] as const).map((r) => ({ value: r, label: t(`godparentRole.${r}Title` as MessageKey) })),
  );
  let godPickerKey = $state(0);
  let newGodKey = 0;
  /** A new godparent's role: by the person's sex, else the one the list still lacks. */
  function nextRole(p?: PersonRef | null): string {
    if (p?.sex === 'male') return 'godfather';
    if (p?.sex === 'female') return 'godmother';
    return f.godparents.some((g) => g.role === 'godfather') ? 'godmother' : 'godfather';
  }
  function addGodPerson(p: PersonRef | null) {
    if (!p) return;
    if (!f.godparents.some((g) => g.person?.id === p.id))
      f.godparents.push({ key: `new-${++newGodKey}`, id: undefined, person: p, name: '', role: nextRole(p), note: '' });
    godPickerKey += 1;
  }
  function addGodName() {
    f.godparents.push({ key: `new-${++newGodKey}`, id: undefined, person: null, name: '', role: nextRole(), note: '' });
  }
  const godLabel = (g: { person: PersonRef | null; name: string }, i: number) =>
    g.person?.name ?? (shortGodparentName(g.name) || t('editor.godparentNameOf', { n: i + 1 }));

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
    if (f.godparents.length > 0 || (event?.godparents?.length ?? 0) > 0) {
      input.godparents = f.godparents
        .filter((g) => g.person || g.name.trim())
        .map((g) => {
          const out: GodparentInput = { role: g.role };
          if (g.id) out.id = g.id;
          if (g.person) out.person = g.person.id;
          else out.name = g.name.trim();
          if (g.note.trim()) out.note = g.note.trim();
          return out;
        });
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
  watch={[event?.id, owner.id]}
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
  {#if f.kind === 'birth' || takesGodparents(f.kind)}
    <DatePartsField label={t('editor.date')} value={f.date} onChange={(raw) => (f.date = raw)} order={settings.value.dayOrder} />
  {:else}
    <DateField label={t('editor.date')} value={f.date} onChange={(raw) => (f.date = raw)} order={settings.value.dayOrder} />
  {/if}
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

  {#if showGodparents}
    <fieldset class="parts godparents" data-godparents>
      <legend>{t('editor.godparents')}</legend>
      <p class="hint">{t('editor.godparentsHint')}</p>
      {#each f.godparents as g, i (g.key)}
        <div class="godrow" data-godparent>
          {#if g.person}
            <span class="who"><bdi dir="auto">{g.person.name}</bdi></span>
          {:else}
            <TextField label={t('editor.godparentNameOf', { n: i + 1 })} bind:value={g.name} />
          {/if}
          <Select label={t('editor.godparentRoleOf', { name: godLabel(g, i) })} options={GOD_ROLES} bind:value={g.role} />
          <TextField label={t('editor.godparentNoteOf', { name: godLabel(g, i) })} bind:value={g.note} />
          <Button variant="quiet" aria-label={t('editor.removeNamed', { name: godLabel(g, i) })} onclick={() => f.godparents.splice(i, 1)}
            >{t('common.remove')}</Button
          >
        </div>
      {/each}
      <div class="godadd">
        {#key godPickerKey}
          <PersonPicker
            label={t('editor.addGodparentPerson')}
            value={null}
            exclude={f.godparents.flatMap((g) => (g.person ? [g.person.id] : [])).concat(owner.kind === 'person' ? [owner.id] : [])}
            onPick={addGodPerson}
          />
        {/key}
        <Button onclick={addGodName}>{t('editor.addGodparentName')}</Button>
      </div>
    </fieldset>
  {/if}

  <Checkbox
    label={t('editor.negative')}
    hint={t('editor.negativeHint')}
    bind:checked={f.negative}
  />
  <Checkbox label={t('editor.private')} bind:checked={f.isPrivate} />
  <Checkbox label={t('editor.preferred')} bind:checked={f.preferred} />

  {#if citations}{@render citations(event)}{/if}

  {#snippet extra(stale: boolean)}
    {#if event}<Button variant="danger" disabled={stale} onclick={remove}>{t('editor.deleteEvent')}</Button>{/if}
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
  .hint {
    margin: 0;
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  .godrow {
    display: grid;
    grid-template-columns: minmax(10rem, 1.4fr) minmax(8rem, 1fr) minmax(8rem, 1fr) auto;
    gap: var(--s-3);
    align-items: end;
    padding-top: var(--s-2);
    border-top: 1px solid var(--rule);
  }
  .godrow .who {
    align-self: center;
    font-weight: 500;
  }
  .godadd {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-3);
    align-items: end;
  }
  @media (max-width: 640px) {
    .godrow {
      grid-template-columns: 1fr;
    }
  }
  .sr {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
</style>
