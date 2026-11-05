<script lang="ts">
  import { tick, untrack } from 'svelte';
  import type { Id, LogRowDto } from '../api/types';
  import { getApi } from '../api/context';
  import { errMessage } from '../editors/errors';
  import { agentChanges } from '../state/agentChanges.svelte';
  import { project } from '../state/project.svelte';
  import { toasts } from '../state/toast.svelte';
  import Button from '../ui/Button.svelte';
  import { currentLocale, t } from '../i18n';
  import { editLabel } from '../i18n/backend';
  import LogChanges from './LogChanges.svelte';

  let { id }: { id: Id } = $props();

  const api = getApi();
  const uid = $props.id();
  let rows = $state<LogRowDto[]>([]);
  let heading = $state<HTMLElement>();
  let seq = 0;

  async function load(person: Id) {
    const mine = ++seq;
    try {
      const r = await api.personAgentChanges(person);
      if (mine === seq) rows = r;
    } catch {
      // The person page works without this list; the change log still has every entry.
      if (mine === seq) rows = [];
    }
  }

  $effect(() => {
    const person = id;
    void project.rev;
    void agentChanges.unreviewed;
    untrack(() => void load(person));
  });

  async function review() {
    try {
      await api.markReviewed({ kind: 'person', id });
      await agentChanges.reload();
      // The button that had focus is gone now.
      await tick();
      heading?.focus();
    } catch (e) {
      toasts.push({ text: errMessage(e), kind: 'error' });
    }
  }

  const who = (r: LogRowDto) => (r.actor.kind === 'agent' ? r.actor.name : '');
</script>

{#if rows.length > 0}
  <section class="panel" aria-labelledby="{uid}-h">
    <div class="head">
      <h2 id="{uid}-h" tabindex="-1" bind:this={heading}>{t('changeLog.panelTitle')}</h2>
      {#if agentChanges.unreviewed.has(id)}
        <Button onclick={review}>{t('changeLog.reviewed')}</Button>
      {/if}
    </div>
    <ul>
      {#each rows as r (r.group)}
        <li>
          <p class="meta">
            <span>{new Date(r.ts).toLocaleString(currentLocale())}</span>
            <span>{who(r)}</span>
            <span>{editLabel(r.label)}</span>
          </p>
          <LogChanges changes={r.changes} />
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .panel {
    display: grid;
    gap: var(--s-3);
    padding: var(--s-4);
    border: 1px solid var(--rule);
    border-radius: 4px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-4);
  }
  h2 {
    margin: 0;
    font-size: var(--t-md, 1rem);
    outline: none;
  }
  ul {
    display: grid;
    gap: var(--s-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-1) var(--s-3);
    margin: 0;
    color: var(--ink-1);
    font-size: var(--t-sm);
  }
</style>
