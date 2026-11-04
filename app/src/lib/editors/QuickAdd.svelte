<script lang="ts">
  import type { EditCommand, EditResult, Id, PersonDetail, PersonRef, SexKind } from '../api/types';
  import { getApi } from '../api/context';
  import { nav } from '../state/nav.svelte';
  import TextField from '../ui/TextField.svelte';
  import Select from '../ui/Select.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Banner from '../ui/Banner.svelte';
  import RadioGroup from '../ui/RadioGroup.svelte';
  import Button from '../ui/Button.svelte';
  import PersonPicker from './PersonPicker.svelte';
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
  /** 'new' creates a person; 'existing' links someone already in the tree. */
  let mode = $state<'new' | 'existing'>('new');
  let picked = $state<PersonRef | null>(null);
  /** People likely meant: for a parent, the partners of the parent already recorded (the father's wife, ...). */
  let suggested = $state<PersonRef[]>([]);

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

  $effect(() => {
    if (rel !== 'father' && rel !== 'mother') return;
    const d = detail;
    if (!d || newParentSet) return;
    const set = d.parentSets.find((s) => s.preferred) ?? d.parentSets[0];
    const known = set?.parents.filter((x): x is PersonRef => !!x) ?? [];
    if (known.length !== 1) return;
    const want = rel === 'father' ? 'male' : 'female';
    void api
      .getFamilyView(known[0].id)
      .then((v) => {
        suggested = v.partnerships
          .map((u) => u.partner)
          .filter((x): x is PersonRef => !!x && x.id !== anchor.id && x.sex !== (want === 'male' ? 'female' : 'male'));
      })
      .catch(() => {});
  });

  const choices = $derived(rel === 'child' && !family && detail && detail.partnerships.length > 1 ? detail.partnerships : []);
  const needsChoice = $derived(choices.length > 0);
  const unnamed = $derived(mode === 'new' && !given.trim() && !surnames.trim());
  const dirty = $derived(mode === 'existing' ? !!picked : !!(given || birthDate || birthPlace) || (touchedSurname && !!surnames));

  async function save() {
    saving = true;
    error = '';
    const fam = family ?? (needsChoice ? chosenFamily : undefined);
    const cmd: EditCommand =
      mode === 'existing' && picked
        ? {
            type: 'linkExisting',
            anchor: anchor.id,
            relation: rel,
            person: picked.id,
            ...(fam ? { family: fam } : {}),
            ...(newParentSet ? { newParentSet: true } : {}),
          }
        : {
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
  canSave={(!needsChoice || !!chosenFamily) && (mode === 'new' || !!picked)}
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
  <RadioGroup
    legend={t('editor.who')}
    options={[
      { value: 'new', label: t('editor.whoNew') },
      { value: 'existing', label: t('editor.whoExisting') },
    ]}
    bind:value={mode}
  />
  {#if mode === 'existing'}
    {#if suggested.length > 0}
      <div class="sugg" role="group" aria-label={t('editor.suggested')}>
        <span class="sugg-cap">{t('editor.suggested')}</span>
        {#each suggested as s (s.id)}
          <Button onclick={() => (picked = s)} aria-pressed={picked?.id === s.id}>{s.name}{s.lifespan ? ` (${s.lifespan})` : ''}</Button>
        {/each}
      </div>
    {/if}
    <PersonPicker label={t('editor.findPerson')} value={picked} exclude={[anchor.id]} onPick={(p) => (picked = p)} hint={t('editor.findPersonHint')} />
  {:else}
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
  {/if}
  {#if unnamed}<Banner kind="warning">{t('editor.unnamed')}</Banner>{/if}
</EditorDialog>

<style>
  .sugg {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
    align-items: center;
  }
  .sugg-cap {
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
</style>
