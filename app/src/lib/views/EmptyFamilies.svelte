<script lang="ts">
  import { onMount } from 'svelte';
  import Banner from '../ui/Banner.svelte';
  import Button from '../ui/Button.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import { notices } from '../ui/notices.svelte';
  import { undoNotice } from '../ui/undoable';
  import { getApi } from '../api/context';
  import { nav } from '../state/nav.svelte';
  import { errMessage } from '../editors/errors';
  import { t } from '../i18n';
  import type { EmptyFamilyDto, EmptyFamilyKind } from '../api/types';

  const api = getApi();

  let rows = $state<EmptyFamilyDto[] | null>(null);
  let error = $state('');
  let asking = $state<EmptyFamilyDto | null>(null);
  let busy = $state(false);

  async function load() {
    try {
      rows = await api.listEmptyFamilies();
      error = '';
    } catch (e) {
      error = errMessage(e);
      rows ??= [];
    }
  }
  onMount(() => void load());

  const BY = { owner: 'by_owner', agent: 'by_agent', copy: 'by_copy' } as const;
  const KIND: Record<EmptyFamilyKind, 'kindParentless' | 'kindSingle' | 'kindEmpty'> = {
    parentless: 'kindParentless',
    singlePartner: 'kindSingle',
    empty: 'kindEmpty',
  };

  const names = (list: { name: string }[]) => list.map((p) => p.name || '?').join(', ');

  /** Who takes over the family's notes, citations and pictures when it is removed. */
  function receiver(f: EmptyFamilyDto): string | null {
    if (!f.needsNotesTo || !f.suggestedNotesTo) return null;
    const all = [...f.partners, ...f.children.map((c) => c.person)];
    return all.find((p) => p.id === f.suggestedNotesTo)?.name ?? null;
  }

  async function remove() {
    const f = asking;
    if (!f || busy) return;
    busy = true;
    try {
      const r = await api.removeEmptyFamily(f.id, f.needsNotesTo ? f.suggestedNotesTo : null);
      asking = null;
      notices.show(undoNotice(api, t('emptyFamilies.done'), r.undo, () => void load()));
      await load();
    } catch (e) {
      error = t('emptyFamilies.failed', { e: errMessage(e) });
      asking = null;
    } finally {
      busy = false;
    }
  }
</script>

<section class="empty-families" aria-labelledby="empty-families-h">
  <h2 id="empty-families-h">{t('emptyFamilies.title')}</h2>
  <p class="hint">{t('emptyFamilies.hint')}</p>
  {#if error}<Banner kind="error">{error}</Banner>{/if}
  {#if rows && rows.length === 0 && !error}
    <p class="hint" data-testid="empty-families-none">{t('emptyFamilies.none')}</p>
  {/if}
  {#if rows && rows.length > 0}
    <ul class="rows" data-testid="empty-families-list">
      {#each rows as f (f.id)}
        <li class="row" data-testid="empty-family-row">
          <div class="what">
            <span class="kind">{t(`emptyFamilies.${KIND[f.kind]}`)}</span>
            {#if f.partners.length > 0}
              <span>{t('emptyFamilies.partner', { names: names(f.partners) })}</span>
            {/if}
            {#if f.children.length > 0}
              <span>{t('emptyFamilies.children', { names: names(f.children.map((c) => c.person)) })}</span>
              {#if f.children.every((c) => c.hasOtherFamily)}
                <span class="hint">{t('emptyFamilies.allKeepParents')}</span>
              {/if}
            {/if}
            {#if f.notes + f.citations + f.media > 0}
              <span class="hint">{t('emptyFamilies.holds', { notes: f.notes, citations: f.citations, media: f.media })}</span>
            {/if}
            {#if f.origin}
              <span class="hint">{t(`emptyFamilies.${BY[f.origin.by as keyof typeof BY] ?? 'by_owner'}`, { name: f.origin.name ?? '' })}</span>
            {/if}
            {#if !f.removable}
              <span class="why" data-testid="empty-family-blocked">{t('emptyFamilies.blocked', { why: f.blockers.join(' ') })}</span>
            {/if}
          </div>
          <div class="actions">
            <Button variant="secondary" onclick={() => nav.go({ name: 'familyPage', id: f.id })}>{t('emptyFamilies.open')}</Button>
            <Button variant="danger" disabled={!f.removable} onclick={() => (asking = f)}>{t('emptyFamilies.delete')}</Button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

{#if asking}
  <Dialog open title={t('emptyFamilies.confirmTitle')} onclose={() => (asking = null)} width="30rem">
    <p>{t('emptyFamilies.confirmBody')}</p>
    {#if receiver(asking)}<p>{t('emptyFamilies.confirmMoves', { name: receiver(asking) ?? '' })}</p>{/if}
    {#snippet footer()}
      <Button variant="quiet" onclick={() => (asking = null)}>{t('common.cancel')}</Button>
      <Button variant="danger" disabled={busy} onclick={() => void remove()}>{t('emptyFamilies.delete')}</Button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .empty-families {
    display: grid;
    gap: var(--s-4);
  }
  h2 {
    margin: 0;
  }
  .hint {
    margin: 0;
    color: var(--ink-2);
  }
  .rows {
    display: grid;
    gap: var(--s-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-4);
    padding: var(--s-4);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
  }
  .what {
    display: grid;
    gap: var(--s-1);
    min-inline-size: 0;
    flex: 1 1 18rem;
  }
  .kind {
    font-weight: 600;
  }
  .why {
    color: var(--ink-2);
    font-style: italic;
  }
  .actions {
    display: flex;
    gap: var(--s-3);
  }
</style>
