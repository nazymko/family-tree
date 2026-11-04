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
  import { t } from '../i18n';

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
  const SEXES = $derived(
    (['male', 'female', 'intersex', 'unknown', 'other'] as const).map((value) => ({ value, label: t(`sexOption.${value}`) })),
  );
  const ADD_KEY = {
    father: 'edit.addFather',
    mother: 'edit.addMother',
    partner: 'edit.addPartner',
    child: 'edit.addChild',
    sibling: 'edit.addSibling',
  } as const;

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
  title={t(`editor.quickTitle.${rel}`, { name: anchor.name })}
  saveLabel={t(ADD_KEY[rel])}
  {dirty}
  {saving}
  {error}
  canSave={!needsChoice || !!chosenFamily}
  onsave={save}
  oncancel={onCancel}
>
  {#if needsChoice}
    <Select
      label={t('editor.withWhichPartner')}
      options={[
        { value: '', label: t('editor.choosePartner'), disabled: true },
        ...choices.map((p) => ({ value: p.familyId, label: p.partner?.name ?? t('editor.unknownPartner') })),
      ]}
      bind:value={chosenFamily}
    />
  {/if}
  <TextField label={t('editor.givenName')} bind:value={given} />
  <TextField
    label={t('editor.surnamesShort')}
    hint={t('editor.surnamesHint')}
    bind:value={surnames}
    oninput={() => (touchedSurname = true)}
  />
  <Select label={t('editor.sex')} options={SEXES} bind:value={sex} />
  <DateField label={t('editor.birthDate')} value={birthDate} onChange={(r) => (birthDate = r)} order={settings.value.dayOrder} />
  <PlacePicker label={t('editor.birthPlace')} value={null} text="" onChange={(v) => (birthPlace = v?.placeText ?? '')} onOption={(p) => (birthPlace = p.full)} />
  <Checkbox label={t('editor.openAfter')} bind:checked={openAfter} />
  {#if unnamed}<Banner kind="warning">{t('editor.unnamed')}</Banner>{/if}
</EditorDialog>
