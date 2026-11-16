<script lang="ts">
  import Button from '../ui/Button.svelte';
  import Icon from '../ui/Icon.svelte';
  import ClipPlayer from './ClipPlayer.svelte';
  import type { OwnerRef, PersonDetail } from '../api/api';
  import { eventKindLabel } from '../i18n/backend';
  import { personRecordings } from './clips';
  import { clockMs } from './clips';
  import { plural, t } from '../i18n';
  import { recordingView } from '../state/recording.svelte';

  /** A person's page: each recording that tells of them, its clips heard on this page, and the whole of it. */
  let { detail }: { detail: PersonDetail } = $props();

  const recordings = $derived(personRecordings(detail));

  /** Which of the person's records a clip is on: "Birth", "Note: How the family came…", "Citation: Interview, 1987". */
  function where(o: OwnerRef): string {
    const first = (s: string) => {
      const line = s.split('\n')[0].trim();
      return line.length > 50 ? `${line.slice(0, 49)}…` : line;
    };
    const events = detail.events;
    switch (o.kind) {
      case 'event': {
        const e = events.find((x) => x.id === o.id);
        return e ? eventKindLabel(e.kind, e.label) : t('rec.target.event');
      }
      case 'note': {
        const n = [...detail.notes, ...events.flatMap((e) => e.notes)].find((x) => x.id === o.id);
        return n ? `${t('rec.target.note')}: ${first(n.text)}` : t('rec.target.note');
      }
      case 'citation': {
        const c = [...detail.citations, ...events.flatMap((e) => e.citations), ...detail.names.flatMap((n) => n.citations)].find((x) => x.id === o.id);
        return c?.source?.title ? `${t('rec.target.citation')}: ${c.source.title}` : t('rec.target.citation');
      }
      default:
        return t(`rec.target.${o.kind as 'person'}`);
    }
  }
</script>

{#if recordings.length === 0}
  <p class="empty">{t('rec.recordingsNone')}</p>
{:else}
  <ul class="recordings">
    {#each recordings as r (r.media.id)}
      {@const name = r.media.title || r.media.originalName || t('voice.untitled')}
      <li class="recording" data-testid="person-recording" data-media-id={r.media.id}>
        <div class="head">
          <Icon name="audio" />
          <div class="text">
            <span class="name"><bdi dir="auto">{name}</bdi></span>
            <span class="meta">
              {#if r.media.durationMs != null}<span dir="ltr">{clockMs(r.media.durationMs)}</span> ·
              {/if}
              {#if r.clips.length}{plural('rec.clipCount', r.clips.length)}{:else if r.cited}{t('rec.cited')}{/if}
            </span>
          </div>
          <Button aria-label={t('rec.openPlayerNamed', { label: name })} onclick={() => recordingView.show(r.media.id, r.clips[0]?.link.clip.id)}
            >{t('rec.openPlayer')}</Button
          >
        </div>
        {#if r.clips.length}
          <ul class="clips">
            {#each r.clips as c (c.link.clip.id)}
              <ClipPlayer link={c.link} where={where(c.owner)} />
            {/each}
          </ul>
        {/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .recordings {
    display: grid;
    gap: var(--s-4);
    margin: 0;
    padding: 0;
  }
  .recording {
    list-style: none;
    display: grid;
    gap: var(--s-3);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--s-3);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .name {
    color: var(--ink-0);
    font-weight: 600;
  }
  .meta {
    font-size: var(--t-sm);
    color: var(--ink-1);
  }
  .clips {
    display: grid;
    gap: var(--s-2);
    margin: 0;
    padding: 0;
    padding-inline-start: var(--s-6);
  }
  .empty {
    margin: 0;
    color: var(--ink-1);
  }
</style>
