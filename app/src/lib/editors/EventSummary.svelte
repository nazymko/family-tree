<script lang="ts">
  import { t } from '../i18n';
  import type { EventDto } from '../api/generated/EventDto';
  import VoiceSamples from '../media/VoiceSamples.svelte';
  import { nav } from '../state/nav.svelte';
  import { godparentFull, godparentRoleText } from '../ui/godparents';

  /** One event in a list: its date and place, then what its description says, the godparents, then the clips told of it. */
  let { event }: { event: EventDto } = $props();

  const line = $derived([event.date?.display, event.place?.full].filter(Boolean).join(', '));
  const description = $derived(event.description?.trim() ?? '');
  const godparents = $derived(event.godparents ?? []);
</script>

<div class="summary">
  {#if line}<span class="line"><bdi dir="auto">{line}</bdi></span>{/if}
  {#if description}<span class="description"><bdi dir="auto">{description}</bdi></span>{/if}
  {#if !line && !description}<span class="line">{t('person.noDateOrPlace')}</span>{/if}
  {#if godparents.length > 0}
    <span class="godparents" data-event-godparents
      >{t('person.godparents')}:
      {#each godparents as g, i (g.id)}{#if i > 0};
        {/if}<span title={godparentFull(g)}
          >{#if g.person}<button type="button" class="godlink" onclick={() => nav.go({ name: 'person', id: g.person!.id })}
              ><bdi dir="auto">{g.person.name}</bdi></button
            >{:else}<bdi dir="auto">{g.name}</bdi>{/if}
          <span class="role">({godparentRoleText(g.role, g.person?.sex)}{#if g.note}, <bdi dir="auto">{g.note}</bdi>{/if})</span></span
        >{/each}</span
    >
  {/if}
  {#if event.media?.some((l) => l.clip)}<VoiceSamples items={event.media} />{/if}
</div>

<style>
  .summary {
    display: block;
    flex: 1;
    min-width: 0;
    padding-block: var(--s-2);
  }
  .line,
  .description,
  .godparents {
    display: block;
    overflow-wrap: anywhere;
  }
  .description {
    white-space: pre-line;
  }
  .line + .description,
  .godparents {
    margin-top: var(--s-1);
    color: var(--ink-1);
  }
  .role {
    color: var(--ink-2);
  }
  .godlink {
    background: none;
    border: 0;
    padding: 0;
    font: inherit;
    color: var(--verdigris);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
</style>
