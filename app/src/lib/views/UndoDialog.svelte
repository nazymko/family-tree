<script module lang="ts">
  import type { Id } from '../api/types';

  /** What to undo: one change group or a whole batch. */
  export type Pending = { kind: 'group'; id: Id } | { kind: 'batch'; id: Id; title: string };
</script>

<script lang="ts">
  import { tick } from 'svelte';
  import type { LogPersonDto, UndoReport } from '../api/types';
  import { getApi } from '../api/context';
  import { errMessage } from '../editors/errors';
  import { agentChanges } from '../state/agentChanges.svelte';
  import Banner from '../ui/Banner.svelte';
  import Button from '../ui/Button.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import { plural, t } from '../i18n';
  import { factText, recordKindLabel } from '../i18n/backend';

  let {
    pending,
    onclose,
    onundone,
  }: {
    pending: Pending | null;
    /** The dialog has closed (after the answer or on "Keep"); `undone` is how many changes the undo took back. */
    onclose: (undone: number) => void;
    /** The undo ran, the answer is still on screen. */
    onundone?: (r: UndoReport) => void;
  } = $props();

  const api = getApi();
  let report = $state<UndoReport | null>(null);
  let busy = $state(false);
  let undoError = $state('');
  let summary = $state<HTMLElement>();

  async function confirmUndo() {
    if (busy || !pending) return;
    busy = true;
    undoError = '';
    try {
      report = pending.kind === 'group' ? await api.undoChangeGroup(pending.id) : await api.undoChangeBatch(pending.id);
      void agentChanges.reload();
      onundone?.(report);
      // The button just pressed has gone with the question.
      await tick();
      summary?.focus();
    } catch (e) {
      undoError = errMessage(e);
    } finally {
      busy = false;
    }
  }

  function close() {
    // What the undo did comes back to this dialog; closing now would lose it.
    if (busy) return;
    const undone = report?.undone ?? 0;
    report = null;
    undoError = '';
    onclose(undone);
  }

  const title = $derived(
    report ? t('changeLog.resultTitle') : pending?.kind === 'batch' ? t('changeLog.confirmBatchTitle') : t('changeLog.confirmChangeTitle'),
  );

  /** The backend's word for a person with no name, in the active language. */
  const personName = (p: Pick<LogPersonDto, 'name'>) => factText({ kind: 'text', text: p.name });
</script>

<Dialog open={pending !== null} {title} onclose={close}>
  {#if report}
    <p tabindex="-1" bind:this={summary} class="summary">
      {#if report.undone > 0 || report.skipped.length > 0}{plural('changeLog.resultUndone', report.undone)}{:else}{t('changeLog.resultNothing')}{/if}
    </p>
    {#if report.skipped.length > 0}
      <p>{t('changeLog.skippedHeading')}</p>
      <ul class="skipped">
        {#each report.skipped as s, i (i)}
          <li>
            {recordKindLabel(s.kind)}{#if s.person}, <bdi dir="auto">{personName(s.person)}</bdi>{/if}: <bdi dir="auto">{factText(s.fact)}</bdi>
          </li>
        {/each}
      </ul>
    {/if}
  {:else if pending}
    {#if undoError}<Banner kind="error">{undoError}</Banner>{/if}
    <p>{pending.kind === 'batch' ? t('changeLog.confirmBatchBody', { title: pending.title }) : t('changeLog.confirmChangeBody')}</p>
  {/if}
  {#snippet footer()}
    {#if report}
      <Button variant="primary" onclick={close}>{t('changeLog.done')}</Button>
    {:else}
      <Button disabled={busy} onclick={close}>{t('changeLog.keep')}</Button>
      <Button variant="danger" onclick={confirmUndo}>{t('changeLog.undo')}</Button>
    {/if}
  {/snippet}
</Dialog>

<style>
  .summary {
    outline: none;
    font-weight: 500;
  }
  .skipped {
    display: grid;
    gap: var(--s-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
</style>
