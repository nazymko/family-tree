<script lang="ts">
  import type { EditCommand, EditResult, Id, PersonDetail, PersonRef, SexKind } from '../api/types';
  import { getApi } from '../api/context';
  import { nav } from '../state/nav.svelte';
  import TextField from '../ui/TextField.svelte';
  import Select from '../ui/Select.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Banner from '../ui/Banner.svelte';
  import DateField from './DateField.svelte';
  import PlacePicker from './PlacePicker.svelte';
  import { settings } from '../state/settings.svelte';
  import EditorDialog from './EditorDialog.svelte';
  import { errMessage, opt, splitList } from './errors';

  type Relation = 'father' | 'mother' | 'partner' | 'child' | 'sibling';
  let {
    anchor,
    relation,
    family,
    newParentSet,
    onDone,
    onCancel,
  }: {
    anchor: PersonRef;
    relation: Relation;
    family?: Id;
    /** Father/mother: start a new set of parents instead of filling the existing one. */
    newParentSet?: boolean;
    onDone: (r: EditResult) => void;
    onCancel: () => void;
  } = $props();

  const api = getApi();
  // svelte-ignore state_referenced_locally
  const rel = relation;
  const SEXES = [
    ['male', 'Male'],
    ['female', 'Female'],
    ['intersex', 'Intersex'],
    ['unknown', 'Unknown'],
    ['other', 'Other'],
  ].map(([value, label]) => ({ value, label }));

  let given = $state('');
  let surnames = $state('');
  let sex = $state<string>(rel === 'father' ? 'male' : rel === 'mother' ? 'female' : 'unknown');
  let birthDate = $state('');
  let birthPlace = $state('');
  let openAfter = $state(false);
  let chosenFamily = $state('');
  let detail = $state<PersonDetail | null>(null);
  let touchedSurname = $state(false);
  let saving = $state(false);
  let error = $state('');

  const prefillSurname = rel === 'child' || rel === 'sibling' || rel === 'father';
  $effect(() => {
    void api
      .getPerson(anchor.id)
      .then((d) => {
        detail = d;
        if (prefillSurname && !touchedSurname && !surnames) {
          const n = d.names.find((x) => x.preferred) ?? d.names[0];
          surnames = (n?.surnames ?? []).join(', ');
        }
      })
      .catch(() => {
        // The prefill is a convenience: without it the user types the surname.
      });
  });

  const choices = $derived(rel === 'child' && !family && detail && detail.partnerships.length > 1 ? detail.partnerships : []);
  const needsChoice = $derived(choices.length > 0);
  const unnamed = $derived(!given.trim() && !surnames.trim());
  const dirty = $derived(!!(given || birthDate || birthPlace) || (touchedSurname && !!surnames));

  async function save() {
    saving = true;
    error = '';
    const fam = family ?? (needsChoice ? chosenFamily : undefined);
    const cmd: EditCommand = {
      type: 'quickAdd',
      anchor: anchor.id,
      relation: rel,
      draft: {
        given: opt(given),
        surnames: splitList(surnames),
        sex: sex as SexKind,
        birthDateRaw: opt(birthDate),
        birthPlaceText: opt(birthPlace),
      },
      ...(fam ? { family: fam } : {}),
      ...(newParentSet ? { newParentSet: true } : {}),
    };
    try {
      const r = await api.apply(cmd);
      if (openAfter && r.focus) nav.focus(r.focus);
      onDone(r);
    } catch (e) {
      error = errMessage(e);
    } finally {
      saving = false;
    }
  }
</script>

<EditorDialog
  title="Add {rel} of {anchor.name}"
  saveLabel="Add {rel}"
  {dirty}
  {saving}
  {error}
  canSave={!needsChoice || !!chosenFamily}
  onsave={save}
  oncancel={onCancel}
>
  {#if needsChoice}
    <Select
      label="With which partner?"
      options={[
        { value: '', label: 'Choose a partner', disabled: true },
        ...choices.map((p) => ({ value: p.familyId, label: p.partner?.name ?? 'Unknown partner' })),
      ]}
      bind:value={chosenFamily}
    />
  {/if}
  <TextField label="Given name" bind:value={given} />
  <TextField
    label="Surname(s)"
    hint="Separate several surnames with a comma"
    bind:value={surnames}
    oninput={() => (touchedSurname = true)}
  />
  <Select label="Sex" options={SEXES} bind:value={sex} />
  <DateField label="Birth date" value={birthDate} onChange={(r) => (birthDate = r)} order={settings.value.dayOrder} />
  <PlacePicker label="Birth place" value={null} text="" onChange={(v) => (birthPlace = v?.placeText ?? '')} onOption={(p) => (birthPlace = p.full)} />
  <Checkbox label="Open new person afterwards" bind:checked={openAfter} />
  {#if unnamed}<Banner kind="warning">No name entered: saved as Unknown</Banner>{/if}
</EditorDialog>
