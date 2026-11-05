<script lang="ts">
  import type { EditResult, Id, NameDto, NameInput } from '../api/types';
  import { getApi } from '../api/context';
  import TextField from '../ui/TextField.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Banner from '../ui/Banner.svelte';
  import Button from '../ui/Button.svelte';
  import EditorDialog from './EditorDialog.svelte';
  import { errMessage, opt, splitList } from './errors';
  import { t } from '../i18n';

  /**
   * The surname at birth and after marriage, side by side. It edits the person's birth name and their (first) married
   * name; a new married name copies the given names from the birth name. Emptying the married surname removes that name.
   */
  let {
    person,
    names,
    onSaved,
    onCancel,
  }: { person: Id; names: NameDto[]; onSaved: (r: EditResult) => void; onCancel: () => void } = $props();

  const api = getApi();
  // The dialog edits a snapshot: it is opened fresh each time.
  // svelte-ignore state_referenced_locally
  const birth = names.find((n) => n.kind === 'birth') ?? names.find((n) => n.preferred) ?? names[0];
  // svelte-ignore state_referenced_locally
  const married = names.find((n) => n.kind === 'married' && n.id !== birth?.id);

  const init = {
    birthSurname: (birth?.surnames ?? []).join(', '),
    marriedSurname: (married?.surnames ?? []).join(', '),
    useMarried: married?.preferred ?? false,
  };
  let f = $state({ ...init });
  /** Set when the user moved a patronymic out of the "after marriage" field (old imports put it there). */
  let movedPatronymic = $state('');
  /** Female patronymic endings (never surnames); the male -ович is also a surname, so it is not offered. */
  const FEMALE_PATRONYMIC = /^[^\s]+(івна|ївна|овна|евна|ична)$/iu;
  const offerMove = $derived(!movedPatronymic && !birth?.patronymic && FEMALE_PATRONYMIC.test(f.marriedSurname.trim()));
  function movePatronymic() {
    movedPatronymic = f.marriedSurname.trim();
    f.marriedSurname = '';
    f.useMarried = false;
  }
  let saving = $state(false);
  let error = $state('');
  const dirty = $derived(JSON.stringify(f) !== JSON.stringify(init) || movedPatronymic !== '');

  /** Every field of an existing name, so saving one field keeps the rest (saveName replaces the whole name). */
  function inputOf(n: NameDto): NameInput {
    const input: NameInput = { id: n.id, kind: n.kind, preferred: n.preferred, private: n.private, surnames: n.surnames };
    const extras: Partial<NameInput> = {
      kindText: opt(n.kindText),
      prefix: opt(n.prefix),
      given: opt(n.given),
      call: opt(n.call),
      surnamePrefix: opt(n.surnamePrefix),
      patronymic: opt(n.patronymic),
      suffix: opt(n.suffix),
      nickname: opt(n.nickname),
      lang: opt(n.lang),
      dateRaw: opt(n.date?.raw),
    };
    for (const [k, v] of Object.entries(extras)) if (v !== undefined) (input as unknown as Record<string, unknown>)[k] = v;
    return input;
  }

  async function save() {
    saving = true;
    error = '';
    const birthSurnames = splitList(f.birthSurname);
    const marriedSurnames = splitList(f.marriedSurname);
    let last: EditResult | null = null;
    try {
      // 1. The birth name: new surnames, and it becomes the main name again when the married one stops being it.
      const marriedMain = f.useMarried && marriedSurnames.length > 0;
      // Saving the married name as preferred takes the main-name mark over by itself.
      const birthPreferred = marriedMain ? !!birth?.preferred : !!birth?.preferred || !!married?.preferred;
      if (birth) {
        const changed =
          birthSurnames.join('|') !== birth.surnames.join('|') || birthPreferred !== birth.preferred || movedPatronymic !== '';
        if (changed && (birthSurnames.length > 0 || birth.given)) {
          const input = { ...inputOf(birth), surnames: birthSurnames, preferred: birthPreferred };
          if (movedPatronymic) input.patronymic = movedPatronymic;
          last = await api.apply({ type: 'saveName', person, name: input });
        }
      } else if (birthSurnames.length > 0) {
        last = await api.apply({ type: 'saveName', person, name: { kind: 'birth', preferred: true, private: false, surnames: birthSurnames } });
      }
      // 2. The married name.
      if (marriedSurnames.length === 0) {
        if (married) last = await api.apply({ type: 'deleteName', person, name: married.id });
      } else if (married) {
        if (marriedSurnames.join('|') !== married.surnames.join('|') || marriedMain !== married.preferred) {
          last = await api.apply({ type: 'saveName', person, name: { ...inputOf(married), surnames: marriedSurnames, preferred: marriedMain } });
        }
      } else {
        const base = birth ? inputOf(birth) : null;
        const copy: NameInput = { kind: 'married', preferred: marriedMain, private: base?.private ?? false, surnames: marriedSurnames };
        for (const k of ['prefix', 'given', 'call', 'patronymic', 'suffix', 'lang'] as const) if (base?.[k]) copy[k] = base[k];
        last = await api.apply({ type: 'saveName', person, name: copy });
      }
      if (last) onSaved(last);
      else onCancel();
    } catch (e) {
      error = errMessage(e);
    } finally {
      saving = false;
    }
  }
</script>

<EditorDialog title={t('surnames.title')} saveLabel={t('surnames.save')} watch={[person]} {dirty} {saving} {error} onsave={save} oncancel={onCancel}>
  <TextField label={t('surnames.atBirth')} hint={t('surnames.atBirthHint')} bind:value={f.birthSurname} />
  <TextField label={t('surnames.afterMarriage')} hint={t('surnames.afterMarriageHint')} bind:value={f.marriedSurname} />
  {#if offerMove}
    <Banner kind="warning">
      {t('surnames.looksPatronymic', { value: f.marriedSurname.trim() })}
      <Button onclick={movePatronymic}>{t('surnames.moveToPatronymic')}</Button>
    </Banner>
  {:else if movedPatronymic}
    <p class="moved">{t('surnames.movedToPatronymic', { value: movedPatronymic })}</p>
  {/if}
  <Checkbox label={t('surnames.useMarried')} disabled={f.marriedSurname.trim() === ''} bind:checked={f.useMarried} />
</EditorDialog>

<style>
  .moved {
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
</style>
