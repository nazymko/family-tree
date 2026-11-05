<script module lang="ts">
  /** A button that opens the editor of one record of a change; `what` says which record, for a screen reader. */
  export interface ChangeAction {
    key: string;
    what: string;
    run: () => void;
  }
</script>

<script lang="ts">
  import type { LogChangeDto } from '../api/types';
  import { t, type MessageKey } from '../i18n';
  import Button from '../ui/Button.svelte';
  import FieldChanges from '../ui/FieldChanges.svelte';
  import { factText, recordKindLabel } from '../i18n/backend';

  let { changes, actions }: { changes: LogChangeDto[]; actions?: (c: LogChangeDto) => ChangeAction[] } = $props();
</script>

{#each changes as c, i (i)}
  {@const text = factText(c.fact)}
  {@const acts = actions?.(c) ?? []}
  <div class="change">
    <p class="fact">
      <span class="what">{t(`changeLog.op.${c.op}` as MessageKey)} · {recordKindLabel(c.kind)}</span>
      {#if text}<bdi dir="auto">{text}</bdi>{/if}
      {#if c.sources.length > 0}<span class="src">{t('changeLog.sources', { list: c.sources.join('; ') })}</span>{/if}
    </p>
    <FieldChanges fields={c.fields} />
    {#if acts.length > 0}
      <div class="acts">
        {#each acts as a (a.key)}
          <Button aria-label={t('changeLog.editWhat', { what: a.what })} onclick={a.run}>{t('changeLog.edit')}</Button>
        {/each}
      </div>
    {/if}
  </div>
{/each}

<style>
  .change {
    display: grid;
    gap: var(--s-1);
  }
  .fact {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-1) var(--s-3);
    margin: 0;
  }
  .what,
  .src {
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
</style>
