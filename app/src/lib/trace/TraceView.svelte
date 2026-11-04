<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import Banner from '../ui/Banner.svelte';
  import Button from '../ui/Button.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import { notices } from '../ui/notices.svelte';
  import { undoNotice } from '../ui/undoable';
  import { getApi, getPlatform } from '../api/context';
  import type { AiReadResult, TraceImageInfo } from '../api/types';
  import { nav } from '../state/nav.svelte';
  import { plural, t, type MessageKey } from '../i18n';
  import AiReadDialog from './AiReadDialog.svelte';
  import { MEDIA_EXTENSIONS } from '../media/Dropzone.svelte';
  import TraceCanvas from './TraceCanvas.svelte';
  import TraceCardForm from './TraceCardForm.svelte';
  import {
    acceptAll,
    addSuggestions,
    acceptCard,
    addCard,
    addLink,
    buildFamilies,
    cardName,
    counts,
    newDraft,
    parseDraft,
    rejectAll,
    removeCard,
    removeLink,
    toCommand,
    type LinkKind,
    type TraceBox,
    type TraceDraft,
  } from './model';

  const api = getApi();
  const platform = getPlatform();

  const PICTURE_EXTENSIONS = MEDIA_EXTENSIONS.filter((e) => e !== 'pdf');

  let draft = $state<TraceDraft>(newDraft());
  let image = $state<TraceImageInfo | null>(null);
  let imageVersion = $state(1);
  let loaded = $state(false);
  let selectedId = $state<string | null>(null);
  let linking = $state<{ kind: LinkKind; from: string } | null>(null);
  let error = $state('');
  let busy = $state(false);
  let discardOpen = $state(false);
  let aiOpen = $state(false);
  let side = $state<HTMLElement>();
  const LINK_BANNER: Record<LinkKind, MessageKey> = { parent: 'trace.linkChild', child: 'trace.linkParent', partner: 'trace.linkPartner' };

  function aiDone(r: AiReadResult) {
    aiOpen = false;
    // Suggestions from an earlier read that were not accepted are replaced, not doubled.
    rejectAll(draft);
    const res = addSuggestions(draft, r);
    if (res.cards === 0) {
      error = t('trace.aiNone');
      return;
    }
    error = '';
    notices.show({
      message: [
        plural('trace.aiSuggested', res.cards),
        res.skipped ? plural('trace.aiSkipped', res.skipped) : '',
        r.truncated ? t('trace.aiCut') : '',
        t('trace.aiCheck'),
      ]
        .filter(Boolean)
        .join(' '),
    });
  }
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let pending: string | null = null;

  onMount(() => {
    void api
      .loadTraceDraft()
      .then((st) => {
        draft = parseDraft(st.json) ?? newDraft();
        image = st.image;
      })
      .catch((e) => (error = message(e)))
      .finally(() => (loaded = true));
  });

  const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

  // The draft is kept inside the tree a moment after every change.
  $effect(() => {
    if (!loaded) return;
    const json = JSON.stringify($state.snapshot(draft));
    pending = json;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void flush(), 400);
  });

  async function flush() {
    clearTimeout(saveTimer);
    const json = pending;
    pending = null;
    if (json === null) return;
    try {
      await api.saveTraceDraft(json);
    } catch (e) {
      error = t('trace.saveFailed', { reason: message(e) });
    }
  }
  onDestroy(() => void flush());

  const total = $derived(counts(draft));
  const built = $derived(buildFamilies(draft));
  const selIndex = $derived(draft.cards.findIndex((c) => c.id === selectedId));
  const selected = $derived(selIndex >= 0 ? draft.cards[selIndex] : null);
  const canAdd = $derived(!busy && (total.newPeople > 0 || total.families > 0) && built.problems.length === 0);
  const nameOf = (id: string) => {
    const c = draft.cards.find((x) => x.id === id);
    return (c && cardName(c)) || t('trace.unnamedCardLower');
  };
  const summary = $derived(
    [
      plural('trace.sumNew', total.newPeople),
      total.matched > 0 ? plural('trace.sumMatched', total.matched) : '',
      plural('trace.sumFamilies', total.families),
      plural('trace.sumLinks', total.links),
    ]
      .filter(Boolean)
      .join(', ') + '.',
  );

  async function choosePicture() {
    error = '';
    const path = await platform.pickFile({ title: t('trace.pickTitle'), extensions: PICTURE_EXTENSIONS });
    if (!path) return;
    try {
      image = await api.stageTraceImage(path);
      imageVersion++;
    } catch (e) {
      error = message(e);
    }
  }

  function mark(box: TraceBox) {
    error = '';
    if (linking) return;
    const card = addCard(draft, box);
    if (!card) {
      error = t('trace.tooMany');
      return;
    }
    selectedId = card.id;
  }

  function startLink(kind: LinkKind) {
    if (!selected) return;
    linking = { kind, from: selected.id };
  }

  function pick(id: string) {
    const l = linking;
    if (!l) return;
    const r = addLink(draft, l.kind, l.from, id);
    if (r.ok) {
      linking = null;
        selectedId = l.from;
    } else error = r.error;
  }

  function deleteCard(id: string) {
    removeCard(draft, id);
    if (selectedId === id) selectedId = null;
    if (linking?.from === id) linking = null;
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === 'Escape' && linking) {
      e.preventDefault();
      linking = null;
    }
  }

  async function addToTree() {
    if (!canAdd) return;
    busy = true;
    error = '';
    try {
      const r = await api.apply(toCommand(draft), { silent: true });
      const people = total.newPeople;
      const families = total.families;
      notices.show(
        undoNotice(
          api,
          t(families > 0 ? 'trace.addedWithFamilies' : 'trace.addedDone', {
            head: people > 0 ? plural('trace.addedPeople', people) : t('trace.linkedPeople'),
            families: families === 1 ? t('trace.oneFamily') : plural('trace.familiesAdded', families),
          }),
          r.undo,
        ),
      );
      await api.discardTraceDraft();
      draft = newDraft();
      image = null;
      selectedId = null;
      pending = null;
      if (r.focus) nav.focus(r.focus);
      nav.go({ name: r.focus ? 'family' : 'people' });
    } catch (e) {
      error = message(e);
    } finally {
      busy = false;
    }
  }

  async function discard() {
    discardOpen = false;
    try {
      await api.discardTraceDraft();
      draft = newDraft();
      image = null;
      selectedId = null;
      linking = null;
      pending = null;
    } catch (e) {
      error = message(e);
    }
  }
</script>

<svelte:window onkeydown={onkey} />

<div class="trace">
  <header class="head">
    <h1>{t('trace.title')}</h1>
    <p class="lede">{t('trace.lede')}</p>
  </header>

  {#if error}<Banner kind="error">{error}</Banner>{/if}
  {#if linking}
    <Banner kind="info">
      {t(LINK_BANNER[linking.kind], { who: nameOf(linking.from) })}
    </Banner>
  {/if}

  {#if !loaded}
    <p class="lede">{t('trace.loading')}</p>
  {:else if !image}
    <EmptyState title={t('trace.chooseTitle')}>
      {t('trace.chooseBody')}
      {#snippet action()}
        <Button variant="primary" onclick={() => void choosePicture()}>{t('trace.choosePicture')}</Button>
      {/snippet}
    </EmptyState>
  {:else}
    <div class="work">
      <TraceCanvas
        imageUrl={api.traceImageUrl(imageVersion)}
        imgW={image.width ?? 1000}
        imgH={image.height ?? 700}
        bind:draft
        {selectedId}
        linking={!!linking}
        onmark={mark}
        onselect={(id) => {
          if (!linking) selectedId = id;
        }}
        onpick={pick}
        ondelete={deleteCard}
        onskip={() => side?.querySelector<HTMLElement>('button, input')?.focus()}
      >
        {#snippet popover()}
          {#if selected}
            <TraceCardForm
              bind:card={draft.cards[selIndex]}
              onlink={startLink}
              ondelete={() => deleteCard(selected.id)}
              onclose={() => (selectedId = null)}
              onaccept={() => acceptCard(draft, selected.id)}
              onreject={() => deleteCard(selected.id)}
            />
          {/if}
        {/snippet}
      </TraceCanvas>

      <aside class="side" aria-label={t('trace.review')} bind:this={side}>
        <section>
          <h2>{t('trace.picture')}</h2>
          <p class="file" title={image.name}>{image.name}</p>
          <div class="row">
            <Button onclick={() => void choosePicture()}>{t('trace.chooseAnother')}</Button>
            <Button onclick={() => (aiOpen = true)}>{t('trace.readAi')}</Button>
          </div>
        </section>

        <section aria-labelledby="sum-h">
          <h2 id="sum-h">{t('trace.toAdd')}</h2>
          <p class="sum" role="status">{summary}</p>
          {#if built.problems.length > 0}
            <ul class="problems">
              {#each built.problems as p (p)}<li>{p}</li>{/each}
            </ul>
          {/if}
        </section>

        {#if total.suggestions > 0}
          <section aria-labelledby="sug-h">
            <h2 id="sug-h">{t('trace.suggestions')}</h2>
            <p class="sum">{plural('trace.suggestionsWaiting', total.suggestions)}</p>
            <div class="row">
              <Button onclick={() => acceptAll(draft)}>{t('trace.acceptAll')}</Button>
              <Button onclick={() => rejectAll(draft)}>{t('trace.removeAll')}</Button>
            </div>
          </section>
        {/if}

        <section aria-labelledby="people-h" class="grow">
          <h2 id="people-h">{t('trace.peopleHeading', { n: draft.cards.length })}</h2>
          {#if draft.cards.length === 0}
            <p class="sum">{t('trace.noCards')}</p>
          {:else}
            <ul class="list">
              {#each draft.cards as c (c.id)}
                <li>
                  <button type="button" class="item" class:on={c.id === selectedId} onclick={() => (selectedId = c.id)}>
                    {cardName(c) || t('trace.unnamedPerson')}{c.suggested ? t('trace.suggestionSuffix') : ''}{c.existing ? t('trace.inTreeSuffix') : ''}
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
          {#if draft.links.length > 0}
            <h3>{t('trace.linksHeading', { n: draft.links.length })}</h3>
            <ul class="list">
              {#each draft.links as l (l.id)}
                <li class="linkrow">
                  <span>
                    {t(l.kind === 'parent' ? 'trace.linkRowParent' : 'trace.linkRowPartner', { a: nameOf(l.from), b: nameOf(l.to) })}{l.suggested
                      ? t('trace.suggestionSuffix')
                      : ''}
                  </span>
                  <button
                    type="button"
                    class="rm"
                    aria-label={t(l.kind === 'parent' ? 'trace.removeLinkParent' : 'trace.removeLinkPartner', { a: nameOf(l.from), b: nameOf(l.to) })}
                    onclick={() => removeLink(draft, l.id)}>{t('common.remove')}</button
                  >
                </li>
              {/each}
            </ul>
          {/if}
        </section>

        <section class="foot">
          <Checkbox
            label={t('trace.keepPicture')}
            bind:checked={draft.keepPicture}
            hint={t('trace.keepHint')}
          />
          <div class="row">
            <Button variant="primary" disabled={!canAdd} onclick={() => void addToTree()}>{t('trace.addToTree')}</Button>
            <Button onclick={() => (discardOpen = true)}>{t('trace.discard')}</Button>
          </div>
        </section>
      </aside>
    </div>
  {/if}
</div>

<AiReadDialog open={aiOpen} onclose={() => (aiOpen = false)} onresult={aiDone} />

<Dialog open={discardOpen} title={t('trace.discardTitle')} onclose={() => (discardOpen = false)}>
  <p>{t('trace.discardBody')}</p>
  {#snippet footer()}
    <Button onclick={() => (discardOpen = false)}>{t('trace.keepWorking')}</Button>
    <Button variant="danger" onclick={() => void discard()}>{t('trace.discard')}</Button>
  {/snippet}
</Dialog>

<style>
  .trace {
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr);
    gap: var(--s-4);
    height: 100%;
    min-height: 0;
  }
  .head h1 {
    font-size: var(--t-xl, var(--t-2xl));
  }
  .lede {
    color: var(--ink-1);
  }
  .work {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 19rem;
    gap: var(--s-5);
    min-height: 0;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: var(--s-5);
    min-height: 0;
    overflow: auto;
    padding-inline-end: var(--s-2);
  }
  .side h2 {
    font-size: var(--t-md);
    margin-block-end: var(--s-2);
  }
  .side h3 {
    font-size: var(--t-base);
    margin-block: var(--s-4) var(--s-2);
  }
  .grow {
    flex: 1;
    min-height: 6rem;
  }
  .file {
    overflow-wrap: anywhere;
    color: var(--ink-1);
    margin-block-end: var(--s-3);
  }
  .sum {
    color: var(--ink-1);
  }
  .problems {
    margin: var(--s-3) 0 0;
    padding-inline-start: var(--s-5);
    color: var(--danger);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-3);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--s-1);
  }
  .item {
    width: 100%;
    text-align: start;
    padding: var(--s-2) var(--s-3);
    border: 1px solid transparent;
    border-radius: var(--r-1);
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .item:hover,
  .item.on {
    border-color: var(--rule);
    background: var(--paper-1);
  }
  .item:focus-visible,
  .rm:focus-visible {
    outline: 2px solid var(--focus, var(--ribbon));
    outline-offset: 1px;
  }
  .linkrow {
    display: flex;
    justify-content: space-between;
    gap: var(--s-3);
    align-items: baseline;
    font-size: var(--t-sm);
  }
  .rm {
    flex: none;
    border: none;
    background: none;
    color: var(--ribbon);
    text-decoration: underline;
    cursor: pointer;
    font: inherit;
  }
  .foot {
    position: sticky;
    inset-block-end: 0;
    z-index: 1;
    background: var(--paper-0);
    display: grid;
    gap: var(--s-4);
    padding-block-start: var(--s-4);
    border-block-start: 1px solid var(--rule);
  }
</style>
