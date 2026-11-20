<script lang="ts">
  // The quiet line under a note in the story view that says its text is the research note, shortened, and no written story.
  import { t } from '../i18n';
  import { shownNote } from './notesView.svelte';

  let { note }: { note: { text: string; story?: string | null } } = $props();
</script>

{#if shownNote(note).cleaned}<p class="shortened" data-testid="note-shortened">{t('notesView.shortened')}</p>{/if}

<style>
  .shortened {
    margin: 0;
    color: var(--ink-2);
    font-size: 0.85em;
    font-style: italic;
  }
</style>
