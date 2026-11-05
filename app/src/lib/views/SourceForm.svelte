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
  import StaleNotice from '../editors/StaleNotice.svelte';
  import { staleGuard } from '../state/staleGuard.svelte';
  import type { SourceDto } from '../api/api';
  import { t } from '../i18n';

  let {
    source,
    submitLabel,
    onSubmit,
    loadedAt,
    onreload,
  }: {
    source?: SourceDto;
    submitLabel: string;
    onSubmit: (v: SourceValues) => void;
    /** The agent event count when `source` was read. */
    loadedAt?: number;
    /** Reads the source again and remounts this form; if the read fails, the form stays as it is. */
    onreload?: () => void | Promise<void>;
  } = $props();
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

  const guard = staleGuard(() => [source?.id], () => loadedAt ?? 0);
  const values = (s: SourceDto | undefined) => JSON.stringify([s?.title ?? '', s?.author ?? '', s?.publication ?? '', s?.abbreviation ?? '', s?.text ?? '']);
  /** What the form started from; follows the owner's own saves, but not what an agent wrote since. */
  /* svelte-ignore state_referenced_locally */
  let baseline = $state(values(source));
  $effect(() => {
    if (!guard.stale) baseline = values(source);
  });
  const dirty = $derived(JSON.stringify([title, author, publication, abbreviation, text]) !== baseline);
</script>

<form
  onsubmit={(e) => {
    e.preventDefault();
    if (!guard.stale) onSubmit({ title, author, publication, abbreviation, text });
  }}
>
  {#if guard.stale && onreload}<StaleNotice {dirty} {onreload} />{/if}
  <TextField label={t('sources.formTitle')} bind:value={title} />
  <TextField label={t('sources.formAuthor')} bind:value={author} />
  <TextField label={t('sources.formPublication')} bind:value={publication} />
  <TextField label={t('sources.formShortTitle')} bind:value={abbreviation} />
  <TextArea label={t('sources.formText')} bind:value={text} rows={3} />
  <div><Button type="submit" variant="primary" disabled={guard.stale}>{submitLabel}</Button></div>
</form>

<style>
  form {
    display: grid;
    gap: var(--s-4);
    max-width: var(--measure);
  }
</style>
