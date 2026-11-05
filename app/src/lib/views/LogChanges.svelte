<script lang="ts">
  import type { LogChangeDto } from '../api/types';
  import { t, type MessageKey } from '../i18n';
  import FieldChanges from '../ui/FieldChanges.svelte';
  import { factText, recordKindLabel } from '../i18n/backend';

  let { changes }: { changes: LogChangeDto[] } = $props();
</script>

{#each changes as c, i (i)}
  {@const text = factText(c.fact)}
  <p class="fact">
    <span class="what">{t(`changeLog.op.${c.op}` as MessageKey)} · {recordKindLabel(c.kind)}</span>
    {#if text}<bdi dir="auto">{text}</bdi>{/if}
    {#if c.sources.length > 0}<span class="src">{t('changeLog.sources', { list: c.sources.join('; ') })}</span>{/if}
  </p>
  <FieldChanges fields={c.fields} />
{/each}

<style>
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
</style>
