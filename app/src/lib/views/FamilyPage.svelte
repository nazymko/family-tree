<script lang="ts">
  import { untrack } from 'svelte';
  import { getApi } from '../api/context';
  import type { FamilyDto, Id, PersonRef } from '../api/types';
  import EventSummary from '../editors/EventSummary.svelte';
  import FamilyEditor from '../editors/FamilyEditor.svelte';
  import { errMessage } from '../editors/errors';
  import { t } from '../i18n';
  import { eventKindLabel } from '../i18n/backend';
  import VoiceSamples from '../media/VoiceSamples.svelte';
  import { nav } from '../state/nav.svelte';
  import { project } from '../state/project.svelte';
  import { toasts } from '../state/toast.svelte';
  import Banner from '../ui/Banner.svelte';
  import Button from '../ui/Button.svelte';

  /** One family to read: the partners, their events, the children and the notes they share. Nothing is edited here. */
  let { id }: { id: Id } = $props();

  const api = getApi();
  let family = $state<FamilyDto | null>(null);
  let error = $state('');
  let editing = $state(false);
  let token = 0;

  $effect(() => {
    void project.rev;
    const wanted = id;
    untrack(() => void load(wanted));
  });

  async function load(wanted: Id) {
    const mine = ++token;
    try {
      const f = await api.getFamily(wanted);
      if (mine !== token) return;
      family = f;
      error = '';
    } catch (e) {
      if (mine !== token) return;
      // The family was deleted or undone while the page was open: leave it, no error screen.
      if ((e as { code?: string } | null)?.code === 'not_found') {
        toasts.push({ text: t('familyPage.gone') });
        if (nav.canBack) nav.back();
        else nav.replace({ name: 'family' });
      } else error = errMessage(e);
    }
  }

  const partners = $derived((family?.partners ?? [null, null]).filter((p): p is PersonRef => !!p));
  const title = $derived(
    partners.length === 2
      ? t('familyPage.title', { a: partners[0].name, b: partners[1].name })
      : partners.length === 1
        ? t('familyPage.titleOne', { a: partners[0].name })
        : t('familyPage.titleNone'),
  );

  function openPerson(person: Id) {
    nav.focus(person);
    nav.go({ name: 'family' });
  }
</script>

<article class="page-family" aria-labelledby="family-page-title">
  <header>
    {#if nav.canBack}
      <Button onclick={() => nav.back()}>{t('chart.back')}</Button>
    {/if}
    <h1 id="family-page-title"><bdi dir="auto">{title}</bdi></h1>
    {#if family && partners.length > 0}
      <Button variant="quiet" onclick={() => (editing = true)}>{t('family.editFamily')}</Button>
    {/if}
  </header>

  {#if error}
    <Banner kind="error">{error}</Banner>
  {:else if family}
    <section aria-labelledby="family-page-partners">
      <h2 id="family-page-partners">{t('family.partners')}</h2>
      {#if partners.length === 0}
        <p class="none">{t('family.noPartners')}</p>
      {:else}
        <ul class="people">
          {#each partners as p (p.id)}
            <li>
              <button type="button" class="person" onclick={() => openPerson(p.id)}><bdi dir="auto">{p.name}</bdi></button>
              {#if p.lifespan}<span class="life">{p.lifespan}</span>{/if}
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section aria-labelledby="family-page-events">
      <h2 id="family-page-events">{t('familyPage.events')}</h2>
      {#if family.events.length === 0}
        <p class="none">{t('familyPage.noEvents')}</p>
      {:else}
        <ul class="events">
          {#each family.events as e (e.id)}
            <li>
              <span class="kind">{eventKindLabel(e.kind, e.label)}</span>
              <EventSummary event={e} />
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section aria-labelledby="family-page-children">
      <h2 id="family-page-children">{t('family.children')}</h2>
      {#if family.children.length === 0}
        <p class="none">{t('family.noChildren')}</p>
      {:else}
        <ul class="people">
          {#each family.children as c (c.person.id)}
            <li>
              <button type="button" class="person" onclick={() => openPerson(c.person.id)}><bdi dir="auto">{c.person.name}</bdi></button>
              {#if c.person.lifespan}<span class="life">{c.person.lifespan}</span>{/if}
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section aria-labelledby="family-page-notes">
      <h2 id="family-page-notes">{t('person.familyNotes')}</h2>
      {#if family.notes.length === 0}
        <p class="none">{t('familyPage.noNotes')}</p>
      {:else}
        {#each family.notes as n (n.id)}
          <div class="note">
            <p class="note-kind">{t(`noteKind.${n.kind}`)}</p>
            <p class="note-text">{n.text}</p>
            <VoiceSamples items={n.media} />
          </div>
        {/each}
      {/if}
    </section>
  {/if}
</article>

{#if editing && family && partners[0]}
  <FamilyEditor family={family.id} anchor={partners[0].id} onDone={() => (editing = false)} onCancel={() => (editing = false)} />
{/if}

<style>
  .page-family {
    display: grid;
    gap: var(--s-5);
    max-width: 76ch;
    min-width: 0;
  }
  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-3);
  }
  h1 {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  section {
    display: grid;
    gap: var(--s-3);
    min-width: 0;
  }
  h2 {
    font-size: var(--t-md);
    line-height: var(--t-md-lh);
    color: var(--ink-1);
    font-weight: 600;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--s-2);
  }
  .people li {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--s-3);
  }
  .person {
    all: unset;
    cursor: pointer;
    text-decoration: underline;
    overflow-wrap: anywhere;
  }
  .person:focus-visible {
    outline: 2px solid currentColor;
    outline-offset: 2px;
  }
  .life,
  .none,
  .kind,
  .note-kind {
    color: var(--ink-1);
  }
  .life,
  .note-kind {
    font-size: var(--t-sm);
  }
  .events li {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--s-3);
  }
  .kind {
    min-width: 10ch;
  }
  .note {
    display: grid;
    gap: var(--s-2);
  }
  .note + .note {
    margin-top: var(--s-4);
  }
  .note-text {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font-family: var(--font-serif);
    font-size: var(--t-md);
    line-height: var(--rule-step);
  }
</style>
