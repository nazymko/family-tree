<script lang="ts">
  // The citations under a note: the ones its text refers to (by shelf number or source title), else all of the person's, compact.
  import { t } from '../i18n';
  import type { CitationDto } from '../api/api';
  import { citationsReferredTo } from './noteCitations';

  let { text, citations, onOpen }: { text: string; citations: CitationDto[]; onOpen: (c: CitationDto) => void } = $props();

  const matched = $derived(citationsReferredTo(text, citations));
  const shown = $derived(matched.length > 0 ? matched : citations);
  const label = (c: CitationDto) => `${c.source?.title ?? t('citation.untitled')}${c.page ? `, ${c.page}` : ''}`;
</script>

{#if shown.length > 0}
  <div class="nc" data-testid="note-citations">
    <p class="head">{matched.length > 0 ? t('noteSources.referred') : t('noteSources.all', { n: citations.length })}</p>
    <ul>
      {#each shown as c (c.id)}
        <li><button type="button" class="link" onclick={() => onOpen(c)}>{label(c)}</button></li>
      {/each}
    </ul>
  </div>
{/if}

<style>
  .nc {
    font-size: 0.9em;
  }
  .head {
    margin: 0;
    color: var(--ink-2);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: inherit;
    text-decoration: underline;
    cursor: pointer;
    font: inherit;
    text-align: left;
  }
</style>
