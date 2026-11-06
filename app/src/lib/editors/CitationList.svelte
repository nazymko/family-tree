<script lang="ts">
  import { errMessage } from './errors';
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import CitationEditor from './CitationEditor.svelte';
  import { getApi } from '../api/context';
  import type { CitationDto, EditCommand, EditResult, OwnerRef } from '../api/api';
  import { t } from '../i18n';

  let { target, citations, onChanged }: { target: OwnerRef; citations: CitationDto[]; onChanged: (r: EditResult) => void } = $props();
  const api = getApi();
  let editing = $state<{ citation?: CitationDto; focusPage: boolean } | null>(null);
  let error = $state('');

  const label = (c: CitationDto) => `${c.source?.title ?? t('citation.untitled')}${c.page ? `, ${c.page}` : ''}`;

  async function run(cmd: EditCommand) {
    error = '';
    try {
      return await api.apply(cmd);
    } catch (e) {
      error = errMessage(e, t('edit.wentWrong'));
      return null;
    }
  }

  async function clone(c: CitationDto) {
    const r = await run({ type: 'addCitation', target, citation: { cloneOf: c.id } });
    if (!r) return;
    onChanged(r);
    const id = r.touched[r.touched.length - 1] ?? c.id;
    editing = { citation: { ...c, id }, focusPage: true };
  }

  async function remove(c: CitationDto) {
    const r = await run({ type: 'removeCitation', target, citation: c.id });
    if (r) onChanged(r);
  }
</script>

<div class="list">
  {#if error}<Banner kind="error">{error}</Banner>{/if}
  {#if citations.length === 0}
    <p class="none">{t('citation.none')}</p>
  {:else}
    <ul>
      {#each citations as c (c.id)}
        <li>
          <span class="t">{label(c)}</span>
          <Button aria-label={t('citation.editNamed', { label: label(c) })} onclick={() => (editing = { citation: c, focusPage: false })}>{t('common.edit')}</Button>
          <Button aria-label={t('citation.cloneNamed', { label: label(c) })} onclick={() => clone(c)}>{t('citation.clone')}</Button>
          <Button variant="danger" aria-label={t('citation.removeNamed', { label: label(c) })} onclick={() => remove(c)}>{t('common.remove')}</Button>
        </li>
      {/each}
    </ul>
  {/if}
  <div><Button onclick={() => (editing = { focusPage: false })}>{t('citation.add')}</Button></div>
</div>

{#if editing}
  <CitationEditor
    {target}
    citation={editing.citation}
    focusPage={editing.focusPage}
    onCancel={() => (editing = null)}
    onDone={(r) => {
      editing = null;
      onChanged(r);
    }}
  />
{/if}

<style>
  .list {
    display: grid;
    gap: var(--s-3);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    min-height: var(--row-h);
    border-bottom: 1px solid var(--rule);
  }
  .t {
    flex: 1;
  }
  .none {
    color: var(--ink-1);
  }
</style>
