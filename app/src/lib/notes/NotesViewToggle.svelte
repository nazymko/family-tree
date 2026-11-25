<script lang="ts">
  // The one switch for every note in the app: research (as written, with the references) or story (readable).
  // The choice is shared and remembered on this computer, so it is not asked again on each note.
  import { t } from '../i18n';
  import { notesView, type NotesView } from './notesView.svelte';

  const options: NotesView[] = ['research', 'story'];
</script>

<div class="nv" role="group" aria-label={t('notesView.group')} data-testid="notes-view-toggle">
  {#each options as v (v)}
    <button
      type="button"
      class="opt"
      class:on={notesView.value === v}
      aria-pressed={notesView.value === v}
      title={t(v === 'research' ? 'notesView.researchTip' : 'notesView.storyTip')}
      data-view={v}
      onclick={() => notesView.set(v)}>{t(`notesView.${v}`)}</button
    >
  {/each}
</div>

<style>
  .nv {
    display: inline-flex;
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    overflow: hidden;
  }
  .opt {
    min-height: calc(2.25rem - 2px);
    padding: 0 var(--s-4);
    border: 0;
    background: var(--paper-1);
    color: var(--ink-0);
    font: inherit;
    line-height: var(--t-base-lh);
    cursor: pointer;
  }
  .opt + .opt {
    border-inline-start: 1px solid var(--rule);
  }
  .opt:hover:not(.on) {
    background: var(--paper-2);
  }
  .opt.on {
    background: var(--ribbon-wash, var(--paper-2));
    font-weight: 600;
    box-shadow: inset 0 -2px 0 var(--ribbon, var(--ink-2));
  }
  .opt:focus-visible {
    outline: 2px solid var(--ribbon, var(--ink-1));
    outline-offset: -2px;
  }
  @media (pointer: coarse) {
    .opt {
      min-height: 2.75rem;
    }
  }
</style>
