<script lang="ts">
  import { onDestroy } from 'svelte';
  import Button from '../ui/Button.svelte';
  import Select from '../ui/Select.svelte';
  import TextArea from '../ui/TextArea.svelte';
  import TextField from '../ui/TextField.svelte';
  import { getApi } from '../api/context';
  import type { PersonRef, SexKind } from '../api/types';
  import type { LinkKind, TraceCard } from './model';

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
  const SEX: { value: SexKind; label: string }[] = [
    { value: 'unknown', label: 'Not known' },
    { value: 'male', label: 'Male' },
    { value: 'female', label: 'Female' },
    { value: 'intersex', label: 'Intersex' },
    { value: 'other', label: 'Other' },
  ];

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

  function sameAs(p: PersonRef) {
    card.existing = { id: p.id, name: p.name };
    matches = [];
  }

  const title = $derived(card.existing ? 'Person already in the tree' : card.suggested ? 'Suggested person' : 'Person on the picture');
</script>

<div class="form" role="group" aria-label={title}>
  <h3>{title}</h3>

  {#if card.suggested}
    <p class="note">The AI reader suggested this card. It is not added until you accept it.</p>
    <div class="row">
      <Button variant="primary" onclick={onaccept}>Accept this person</Button>
      <Button onclick={onreject}>Remove suggestion</Button>
    </div>
  {/if}

  {#if card.existing}
    <p class="same">Same as <strong>{card.existing.name}</strong>. Nothing new is created for this card.</p>
    <Button onclick={() => (card.existing = null)}>Not the same person</Button>
  {:else}
    <TextField label="Given name" bind:value={card.given} autocomplete="off" />
    <TextField label="Surname" bind:value={card.surname} autocomplete="off" />
    <TextField label="Patronymic" bind:value={card.patronymic} autocomplete="off" />

    {#if matches.length > 0}
      <ul class="matches" aria-label="People already in the tree with a similar name">
        {#each matches as m (m.id)}
          <li>
            <Button onclick={() => sameAs(m)} aria-label="Same as {m.name}{m.lifespan ? `, ${m.lifespan}` : ''}">
              Same as {m.name}{m.lifespan ? ` (${m.lifespan})` : ''}
            </Button>
          </li>
        {/each}
      </ul>
    {/if}

    <Select label="Sex" options={SEX} value={card.sex} onchange={(v) => (card.sex = v as SexKind)} />
    <div class="two">
      <TextField label="Birth year" bind:value={card.birth} inputmode="text" autocomplete="off" />
      <TextField label="Death year" bind:value={card.death} inputmode="text" autocomplete="off" />
    </div>
    <TextArea label="Notes" bind:value={card.notes} rows={2} />
    {#if !card.given.trim() && !card.surname.trim()}
      <p class="need" role="status">Enter a given name or a surname.</p>
    {/if}
  {/if}

  <div class="links" role="group" aria-label="Link this person to another card">
    <Button onclick={() => onlink('parent')}>Parent of…</Button>
    <Button onclick={() => onlink('child')}>Child of…</Button>
    <Button onclick={() => onlink('partner')}>Partner of…</Button>
  </div>
  <div class="row">
    <Button onclick={onclose}>Done</Button>
    <Button variant="danger" onclick={ondelete}>Delete card</Button>
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
