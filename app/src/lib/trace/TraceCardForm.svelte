<script lang="ts">
  import { onDestroy } from 'svelte';
  import Button from '../ui/Button.svelte';
  import Select from '../ui/Select.svelte';
  import TextArea from '../ui/TextArea.svelte';
  import TextField from '../ui/TextField.svelte';
  import { getApi } from '../api/context';
  import type { PersonRef, SexKind } from '../api/types';
  import type { LinkKind, TraceCard } from './model';
  import { t } from '../i18n';
  import { withSexIcon } from '../ui/sex';

  interface Props {
    card: TraceCard;
    onlink: (kind: LinkKind) => void;
    ondelete: () => void;
    onclose: () => void;
    onaccept: () => void;
    onreject: () => void;
  }
  let { card = $bindable(), onlink, ondelete, onclose, onaccept, onreject }: Props = $props();

  const api = getApi();
  const SEX = $derived<{ value: SexKind; label: string }[]>([
    { value: 'unknown', label: withSexIcon('unknown', t('trace.sexUnknown')) },
    { value: 'male', label: withSexIcon('male', t('trace.sexMale')) },
    { value: 'female', label: withSexIcon('female', t('trace.sexFemale')) },
    { value: 'intersex', label: withSexIcon('intersex', t('trace.sexIntersex')) },
    { value: 'other', label: withSexIcon('other', t('trace.sexOther')) },
  ]);

  let matches = $state<PersonRef[]>([]);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let token = 0;

  // When the typed name looks like someone already in the tree, offer them.
  $effect(() => {
    const q = `${card.given} ${card.surname}`.trim();
    const linked = !!card.existing;
    clearTimeout(timer);
    if (linked || q.length < 2) {
      matches = [];
      return;
    }
    const mine = ++token;
    timer = setTimeout(() => {
      api
        .search(q, 6)
        .then((hits) => {
          if (mine === token) matches = hits.flatMap((h) => (h.kind === 'person' && h.person ? [h.person] : [])).slice(0, 4);
        })
        .catch(() => {
          if (mine === token) matches = [];
        });
    }, 250);
  });
  onDestroy(() => clearTimeout(timer));

  // A person from the tree may be in several families; find out so the form can ask which one new
  // children belong to.
  $effect(() => {
    const ex = card.existing;
    if (!ex || ex.families !== undefined) return;
    const id = ex.id;
    void api
      .getPerson(id)
      .then((d) => {
        if (card.existing?.id !== id) return;
        card.existing.families = d.partnerships.map((x) => ({
          id: x.familyId,
          label: x.partner ? t('trace.withPartner', { name: x.partner.name }) : t('trace.noPartner'),
        }));
      })
      .catch(() => {
        if (card.existing?.id === id) card.existing.families = [];
      });
  });

  function sameAs(p: PersonRef) {
    card.existing = { id: p.id, name: p.name };
    matches = [];
  }

  const title = $derived(card.existing ? t('trace.formExisting') : card.suggested ? t('trace.formSuggested') : t('trace.formPlain'));
</script>

<div class="form" role="group" aria-label={title}>
  <h3>{title}</h3>

  {#if card.suggested}
    <p class="note">{t('trace.suggestedNote')}</p>
    {#if card.unsure}<p class="need" role="status">{t('trace.unsureNote')}</p>{/if}
    <div class="row">
      <Button variant="primary" onclick={onaccept}>{t('trace.acceptOne')}</Button>
      <Button onclick={onreject}>{t('trace.removeSuggestion')}</Button>
    </div>
  {/if}

  {#if card.existing}
    <p class="same">{t('trace.sameAsBefore')}<strong>{card.existing.name}</strong>{t('trace.sameAsAfter')}</p>
    {#if (card.existing.families?.length ?? 0) > 1}
      {@const ex = card.existing}
      <Select
        label={t('trace.familyChildren')}
        options={[{ value: '', label: t('trace.chooseFamily') }, ...(ex.families ?? []).map((f) => ({ value: f.id, label: f.label }))]}
        value={ex.familyId ?? ''}
        onchange={(v) => (ex.familyId = v || undefined)}
      />
      <p class="note">{t('trace.severalFamilies')}</p>
    {/if}
    <Button onclick={() => (card.existing = null)}>{t('trace.notSame')}</Button>
  {:else}
    <TextField label={t('trace.given')} bind:value={card.given} autocomplete="off" />
    <TextField label={t('trace.surname')} bind:value={card.surname} autocomplete="off" />
    <TextField label={t('trace.patronymic')} bind:value={card.patronymic} autocomplete="off" />

    {#if matches.length > 0}
      <ul class="matches" aria-label={t('trace.similar')}>
        {#each matches as m (m.id)}
          <li>
            <Button onclick={() => sameAs(m)} aria-label={m.lifespan ? t('trace.sameAsLabel', { name: m.name, life: m.lifespan }) : t('trace.sameAs', { name: m.name })}>
              {m.lifespan ? t('trace.sameAsLife', { name: m.name, life: m.lifespan }) : t('trace.sameAs', { name: m.name })}
            </Button>
          </li>
        {/each}
      </ul>
    {/if}

    <Select label={t('trace.sex')} options={SEX} value={card.sex} onchange={(v) => (card.sex = v as SexKind)} />
    <div class="two">
      <TextField label={t('trace.birthYear')} bind:value={card.birth} inputmode="text" autocomplete="off" />
      <TextField label={t('trace.deathYear')} bind:value={card.death} inputmode="text" autocomplete="off" />
    </div>
    <TextArea label={t('trace.notes')} bind:value={card.notes} rows={2} />
    {#if !card.given.trim() && !card.surname.trim()}
      <p class="need" role="status">{t('trace.needName')}</p>
    {/if}
  {/if}

  <div class="links" role="group" aria-label={t('trace.linkGroup')}>
    <Button onclick={() => onlink('parent')}>{t('trace.parentOf')}</Button>
    <Button onclick={() => onlink('child')}>{t('trace.childOf')}</Button>
    <Button onclick={() => onlink('partner')}>{t('trace.partnerOf')}</Button>
  </div>
  <div class="row">
    <Button onclick={onclose}>{t('trace.done')}</Button>
    <Button variant="danger" onclick={ondelete}>{t('trace.deleteCard')}</Button>
  </div>
</div>

<style>
  .form {
    display: grid;
    gap: var(--s-4);
  }
  h3 {
    font-size: var(--t-md);
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--s-3);
  }
  .row,
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-3);
  }
  .matches {
    display: grid;
    gap: var(--s-2);
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .note,
  .same,
  .need {
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  .need {
    color: var(--danger);
  }
</style>
