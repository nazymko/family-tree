<script lang="ts" module>
  export interface SourceValues {
    title: string;
    author: string;
    publication: string;
    abbreviation: string;
    text: string;
    /** The stored kind id (`metric_book`, `other:<label>`), empty when none. */
    kind: string;
    issuer: string;
    docNumber: string;
    docDateRaw: string;
    archive: string;
    callNumber: string;
  }
</script>

<script lang="ts">
  import TextField from '../ui/TextField.svelte';
  import TextArea from '../ui/TextArea.svelte';
  import Button from '../ui/Button.svelte';
  import Select from '../ui/Select.svelte';
  import DatePreviewField from '../editors/DatePreviewField.svelte';
  import { SOURCE_KIND_IDS, joinKind, splitKind } from '../sources/sourceKind';
  import StaleNotice from '../editors/StaleNotice.svelte';
  import { staleGuard } from '../state/staleGuard.svelte';
  import type { SourceDto } from '../api/api';
  import { t, type MessageKey } from '../i18n';

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
  /* svelte-ignore state_referenced_locally */
  let kindPick = $state(splitKind(source?.kind).pick);
  /* svelte-ignore state_referenced_locally */
  let kindLabel = $state(splitKind(source?.kind).label);
  /* svelte-ignore state_referenced_locally */
  let issuer = $state(source?.issuer ?? '');
  /* svelte-ignore state_referenced_locally */
  let docNumber = $state(source?.docNumber ?? '');
  /* svelte-ignore state_referenced_locally */
  let docDate = $state(source?.docDate?.raw ?? '');
  /* svelte-ignore state_referenced_locally */
  let archive = $state(source?.archive ?? '');
  /* svelte-ignore state_referenced_locally */
  let callNumber = $state(source?.callNumber ?? '');

  const kindOptions = $derived([
    { value: '', label: t('sources.kindNone') },
    ...SOURCE_KIND_IDS.map((id) => ({ value: id, label: t(`sourceKind.${id}` as MessageKey) })),
  ]);

  const guard = staleGuard(() => [source?.id], () => loadedAt ?? 0);
  const values = (s: SourceDto | undefined) => JSON.stringify([s?.title ?? '', s?.author ?? '', s?.publication ?? '', s?.abbreviation ?? '', s?.text ?? '', s?.kind ?? '', s?.issuer ?? '', s?.docNumber ?? '', s?.docDate?.raw ?? '', s?.archive ?? '', s?.callNumber ?? '']);
  /** What the form started from; follows the owner's own saves, but not what an agent wrote since. */
  /* svelte-ignore state_referenced_locally */
  let baseline = $state(values(source));
  $effect(() => {
    if (!guard.stale) baseline = values(source);
  });
  const dirty = $derived(JSON.stringify([title, author, publication, abbreviation, text, joinKind(kindPick, kindLabel), issuer, docNumber, docDate, archive, callNumber]) !== baseline);
</script>

<form
  onsubmit={(e) => {
    e.preventDefault();
    if (!guard.stale) onSubmit({ title, author, publication, abbreviation, text, kind: joinKind(kindPick, kindLabel), issuer, docNumber, docDateRaw: docDate, archive, callNumber });
  }}
>
  {#if guard.stale && onreload}<StaleNotice {dirty} {onreload} />{/if}
  <TextField label={t('sources.formTitle')} bind:value={title} />
  <TextField label={t('sources.formAuthor')} bind:value={author} />
  <TextField label={t('sources.formPublication')} bind:value={publication} />
  <TextField label={t('sources.formShortTitle')} bind:value={abbreviation} />
  <Select label={t('sources.formKind')} options={kindOptions} bind:value={kindPick} />
  {#if kindPick === 'other'}
    <TextField label={t('sources.kindOtherLabel')} bind:value={kindLabel} />
  {/if}
  <TextField label={t('sources.formIssuer')} hint={t('sources.formIssuerHint')} bind:value={issuer} />
  <TextField label={t('sources.formDocNumber')} bind:value={docNumber} />
  <DatePreviewField label={t('sources.formDocDate')} bind:value={docDate} />
  <TextField label={t('sources.formArchive')} bind:value={archive} />
  <TextField label={t('sources.formCallNumber')} bind:value={callNumber} />
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
