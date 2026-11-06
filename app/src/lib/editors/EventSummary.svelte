<script lang="ts">
  import { t } from '../i18n';
  import type { EventDto } from '../api/generated/EventDto';

  /** One event in a list: its date and place, then what its description says. */
  let { event }: { event: EventDto } = $props();

  const line = $derived([event.date?.display, event.place?.full].filter(Boolean).join(', '));
  const description = $derived(event.description?.trim() ?? '');
</script>

<span class="summary">
  {#if line}<span class="line"><bdi dir="auto">{line}</bdi></span>{/if}
  {#if description}<span class="description"><bdi dir="auto">{description}</bdi></span>{/if}
  {#if !line && !description}<span class="line">{t('person.noDateOrPlace')}</span>{/if}
</span>

<style>
  .summary {
    display: block;
    flex: 1;
    min-width: 0;
    padding-block: var(--s-2);
  }
  .line,
  .description {
    display: block;
    overflow-wrap: anywhere;
  }
  .description {
    white-space: pre-line;
  }
  .line + .description {
    margin-top: var(--s-1);
    color: var(--ink-1);
  }
</style>
