<script lang="ts">
  import Dialog from '../ui/Dialog.svelte';
  import Button from '../ui/Button.svelte';
  import TextField from '../ui/TextField.svelte';
  import Select from '../ui/Select.svelte';
  import { getApi } from '../api/context';
  import type { Id, SexKind } from '../api/types';
  import type { AddRelation as Relation } from '../state/ui.svelte';
  import { t, type MessageKey } from '../i18n';

  let {
    open,
    onclose,
    ondone,
    relation,
    anchor,
    newParentSet,
  }: {
    open: boolean;
    onclose: () => void;
    /** Called with the new person's id once Rust has confirmed the edit. */
    ondone: (id: Id | null) => void;
    relation?: Relation;
    anchor?: Id;
    newParentSet?: boolean;
  } = $props();

  const api = getApi();
  const RELATION_KEY: Record<Relation, MessageKey> = {
    father: 'edit.addFather',
    mother: 'edit.addMother',
    partner: 'edit.addPartner',
    child: 'edit.addChild',
    sibling: 'edit.addSibling',
  };
  let given = $state('');
  let surname = $state('');
  let patronymic = $state('');
  let sex = $state<string>('unknown');
  let birth = $state('');
  let error = $state('');
  let busy = $state(false);

  const title = $derived(t(relation ? RELATION_KEY[relation] : 'edit.addPerson'));

  $effect(() => {
    if (open) {
      given = '';
      surname = '';
      patronymic = '';
      sex = relation === 'father' ? 'male' : relation === 'mother' ? 'female' : 'unknown';
      birth = '';
      error = '';
    }
  });

  async function save(e: SubmitEvent) {
    e.preventDefault();
    if (!given.trim() && !surname.trim()) {
      error = t('newPerson.enterName');
      return;
    }
    busy = true;
    try {
      const draft = {
        given: given.trim() || undefined,
        surnames: surname.trim() ? [surname.trim()] : [],
        patronymic: patronymic.trim() || undefined,
        sex: sex as SexKind,
        birthDateRaw: birth.trim() || undefined,
      };
      const r =
        relation && anchor
          ? await api.apply({ type: 'quickAdd', anchor, relation, draft, newParentSet })
          : await api.apply({ type: 'createPerson', draft });
      ondone(r.focus);
    } catch (err) {
      error = (err as Error).message;
    } finally {
      busy = false;
    }
  }
</script>

<Dialog {open} {title} {onclose}>
  <form id="new-person" onsubmit={save}>
    <TextField label={t('newPerson.givenNames')} bind:value={given} autocomplete="off" />
    <TextField label={t('newPerson.surname')} bind:value={surname} autocomplete="off" error={error || undefined} />
    <TextField label={t('editor.patronymic')} bind:value={patronymic} autocomplete="off" />
    <Select
      label={t('newPerson.sex')}
      bind:value={sex}
      options={[
        { value: 'unknown', label: t('sexOption.unknown') },
        { value: 'female', label: t('sexOption.female') },
        { value: 'male', label: t('sexOption.male') },
        { value: 'intersex', label: t('sexOption.intersex') },
        { value: 'other', label: t('sexOption.other') },
      ]}
    />
    <TextField label={t('newPerson.birthDate')} bind:value={birth} hint={t('newPerson.birthHint')} />
  </form>
  {#snippet footer()}
    <Button onclick={onclose}>{t('common.cancel')}</Button>
    <Button variant="primary" type="submit" form="new-person" disabled={busy}>{title}</Button>
  {/snippet}
</Dialog>

<style>
  form {
    display: grid;
    gap: var(--s-4);
    min-width: min(26rem, 80vw);
  }
</style>
