<script lang="ts">
  // Test host: two lists of notes, each with the toggle beside it, as on the person page and the family page.
  import NotesViewToggle from './NotesViewToggle.svelte';
  import NoteShortened from './NoteShortened.svelte';
  import { shownNote } from './notesView.svelte';

  let { notes }: { notes: { id: string; text: string; story: string | null }[] } = $props();
</script>

{#each [1, 2] as k (k)}
  <section data-testid="notes-list">
    <NotesViewToggle />
    {#each notes as n (n.id)}
      <p class="note-text">{shownNote(n).text}</p>
      <NoteShortened note={n} />
    {/each}
  </section>
{/each}
