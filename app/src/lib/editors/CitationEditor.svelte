<script lang="ts">
  import Dialog from '../ui/Dialog.svelte';
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import TextField from '../ui/TextField.svelte';
  import TextArea from '../ui/TextArea.svelte';
  import Select from '../ui/Select.svelte';
  import SourcePicker, { type SourceChoice } from './SourcePicker.svelte';
  import DatePreviewField from './DatePreviewField.svelte';
  import UseLastCitation from './UseLastCitation.svelte';
  import { getApi } from '../api/context';
  import { notices } from '../ui/notices.svelte';
  import { undoNotice } from '../ui/undoable';
  import type { CitationDto, CitationInput, EditResult, OwnerRef } from '../api/api';

  let {
    target,
    citation,
    focusPage = false,
    onDone,
    onCancel,
  }: {
    target: OwnerRef;
    citation?: CitationDto;
    /** Focus the page field on open (after a clone). */
    focusPage?: boolean;
    onDone: (r: EditResult) => void;
    onCancel: () => void;
  } = $props();

  const api = getApi();
  const tri = (b: boolean | null | undefined) => (b == null ? '' : b ? 'yes' : 'no');
  // Initial values only: the editor is remounted for each citation.
  /* svelte-ignore state_referenced_locally */
  let source = $state<SourceChoice | null>(citation?.source ?? null);
  /* svelte-ignore state_referenced_locally */
  let page = $state(citation?.page ?? '');
  /* svelte-ignore state_referenced_locally */
  let dateRaw = $state(citation?.date?.raw ?? '');
  /* svelte-ignore state_referenced_locally */
  let text = $state(citation?.text ?? '');
  /* svelte-ignore state_referenced_locally */
  let quality = $state(citation?.quality == null ? '' : String(citation.quality));
  /* svelte-ignore state_referenced_locally */
  let original = $state(tri(citation?.evidence.original));
  /* svelte-ignore state_referenced_locally */
  let primaryInfo = $state(tri(citation?.evidence.primaryInfo));
  /* svelte-ignore state_referenced_locally */
  let direct = $state(tri(citation?.evidence.direct));
  let error = $state('');
  let saving = $state(false);
  let root = $state<HTMLElement>();

  const QUALITY = [
    { value: '', label: 'Not assessed' },
    { value: '3', label: 'Direct and primary evidence' },
    { value: '2', label: 'Secondary evidence' },
    { value: '1', label: 'Questionable' },
    { value: '0', label: 'Unreliable' },
  ];
  const triOptions = (yes: string, no: string) => [
    { value: '', label: 'Not stated' },
    { value: 'yes', label: yes },
    { value: 'no', label: no },
  ];

  $effect(() => {
    if (!focusPage) return;
    const t = setTimeout(() => root?.querySelector<HTMLInputElement>('input[name="page"]')?.focus(), 0);
    return () => clearTimeout(t);
  });

  async function save() {
    error = '';
    if (!source) {
      error = 'Choose a source, or create one by typing its title.';
      return;
    }
    const evidence: NonNullable<CitationInput['evidence']> = {};
    if (original) evidence.original = original === 'yes';
    if (primaryInfo) evidence.primaryInfo = primaryInfo === 'yes';
    if (direct) evidence.direct = direct === 'yes';
    const body = {
      page: page.trim() || undefined,
      dateRaw: dateRaw.trim() || undefined,
      text: text.trim() || undefined,
      quality: quality === '' ? undefined : (Number(quality) as 0 | 1 | 2 | 3),
      evidence: Object.keys(evidence).length ? evidence : undefined,
    };
    saving = true;
    try {
      let r: EditResult;
      if (citation) {
        r = await api.apply({
          type: 'updateCitation',
          id: citation.id,
          citation: { ...body, source: 'id' in source ? source.id : undefined },
        });
        notices.show(undoNotice(api, 'Citation saved', r.undo));
      } else {
        r = await api.apply({
          type: 'addCitation',
          target,
          citation: { ...body, source: 'id' in source ? source.id : { title: source.title } },
        });
        notices.show(undoNotice(api, 'Citation added', r.undo));
      }
      onDone(r);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
  }
</script>

<Dialog open title={citation ? 'Edit citation' : 'Add citation'} onclose={onCancel}>
  <div class="form" bind:this={root}>
    {#if error}<Banner kind="error">{error}</Banner>{/if}
    {#if !citation}
      <UseLastCitation {target} {onDone} onError={(m) => (error = m)} />
    {/if}
    <SourcePicker bind:value={source} allowCreate={!citation} />
    <TextField label="Page / entry" name="page" bind:value={page} autocomplete="off" />
    <DatePreviewField bind:value={dateRaw} />
    <TextArea label="Transcription" bind:value={text} rows={4} />
    <Select label="Quality" options={QUALITY} bind:value={quality} />
    <details>
      <summary>Evidence analysis</summary>
      <div class="grid">
        <Select label="Original source or derivative" options={triOptions('Original', 'Derivative')} bind:value={original} />
        <Select label="Primary or secondary information" options={triOptions('Primary', 'Secondary')} bind:value={primaryInfo} />
        <Select label="Direct or indirect evidence" options={triOptions('Direct', 'Indirect')} bind:value={direct} />
      </div>
    </details>
  </div>
  {#snippet footer()}
    <Button onclick={onCancel}>Cancel</Button>
    <Button variant="primary" disabled={saving} onclick={save}>{citation ? 'Save citation' : 'Add citation'}</Button>
  {/snippet}
</Dialog>

<style>
  .form {
    display: grid;
    gap: var(--s-5);
  }
  summary {
    cursor: pointer;
    font-weight: 500;
    padding: var(--s-2) 0;
  }
  .grid {
    display: grid;
    gap: var(--s-4);
    padding-top: var(--s-4);
  }
</style>
