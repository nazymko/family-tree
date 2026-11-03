<script lang="ts" module>
  export interface SourceValues {
    title: string;
    author: string;
    publication: string;
    abbreviation: string;
    text: string;
  }
</script>

<script lang="ts">
  import TextField from '../ui/TextField.svelte';
  import TextArea from '../ui/TextArea.svelte';
  import Button from '../ui/Button.svelte';
  import type { SourceDto } from '../api/api';

  let {
    source,
    submitLabel,
    onSubmit,
  }: { source?: SourceDto; submitLabel: string; onSubmit: (v: SourceValues) => void } = $props();
  /* svelte-ignore state_referenced_locally */
  let title = $state(source?.title ?? '');
  /* svelte-ignore state_referenced_locally */
  let author = $state(source?.author ?? '');
  /* svelte-ignore state_referenced_locally */
  let publication = $state(source?.publication ?? '');
  /* svelte-ignore state_referenced_locally */
  let abbreviation = $state(source?.abbreviation ?? '');
  /* svelte-ignore state_referenced_locally */
  let text = $state(source?.text ?? '');
</script>

<form
  onsubmit={(e) => {
    e.preventDefault();
    onSubmit({ title, author, publication, abbreviation, text });
  }}
>
  <TextField label="Title" bind:value={title} />
  <TextField label="Author" bind:value={author} />
  <TextField label="Publication" bind:value={publication} />
  <TextField label="Short title" bind:value={abbreviation} />
  <TextArea label="Text" bind:value={text} rows={3} />
  <div><Button type="submit" variant="primary">{submitLabel}</Button></div>
</form>

<style>
  form {
    display: grid;
    gap: var(--s-4);
    max-width: var(--measure);
  }
</style>
