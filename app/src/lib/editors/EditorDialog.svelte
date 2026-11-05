<script lang="ts">
  import { getContext, type Snippet } from 'svelte';
  import Dialog from '../ui/Dialog.svelte';
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import StaleNotice from './StaleNotice.svelte';
  import { EDITOR_HOST, type EditorHost } from '../state/agentChanges.svelte';
  import { staleGuard } from '../state/staleGuard.svelte';
  import { t } from '../i18n';

  let {
    title,
    saveLabel,
    dirty = false,
    canSave = true,
    saving = false,
    error = '',
    watch = [],
    aboutPerson = false,
    onsave,
    oncancel,
    children,
    extra,
  }: {
    title: string;
    saveLabel: string;
    dirty?: boolean;
    canSave?: boolean;
    saving?: boolean;
    error?: string;
    /** Records this editor shows. When an agent changes one, what is on screen is out of date and Save is blocked. */
    watch?: (string | null | undefined)[];
    /** The editor is about a person as a whole, so the stale banner can say "this person". */
    aboutPerson?: boolean;
    onsave: () => void;
    oncancel: () => void;
    children: Snippet;
    /** Footer actions of the editor's own; the argument says the editor is out of date, so they must not write. */
    extra?: Snippet<[boolean]>;
  } = $props();

  let confirming = $state(false);

  const guard = staleGuard(() => watch);
  const stale = $derived(guard.stale);
  const host = getContext<EditorHost | undefined>(EDITOR_HOST);
  /** The page that opened this dialog reloads and reopens it; if there is none, closing is the way to fresh data. */
  async function reload() {
    if (!(await host?.reopen())) oncancel();
  }

  function requestCancel() {
    if (confirming) confirming = false;
    else if (dirty) confirming = true;
    else oncancel();
  }

  function submit() {
    if (canSave && !saving && !stale) onsave();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key !== 'Enter' || e.defaultPrevented || e.isComposing) return;
    const t = e.target as HTMLElement;
    if (t.tagName === 'INPUT' && (t as HTMLInputElement).type !== 'checkbox') {
      e.preventDefault();
      submit();
    }
  }
</script>

<Dialog open={true} {title} onclose={requestCancel}>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <form class="form" onsubmit={(e) => e.preventDefault()} {onkeydown}>
    {#if stale}<StaleNotice {aboutPerson} {dirty} onreload={reload} />{/if}
    {#if error}<Banner kind="error">{error}</Banner>{/if}
    {@render children()}
  </form>
  {#snippet footer()}
    {#if extra && !confirming}<div class="extra">{@render extra(stale)}</div>{/if}
    {#if confirming}
      <span class="ask">{t('editor.discardAsk')}</span>
      <Button onclick={() => (confirming = false)}>{t('editor.keepEditing')}</Button>
      <Button variant="danger" onclick={oncancel}>{t('editor.discard')}</Button>
    {:else}
      <Button onclick={requestCancel}>{t('common.cancel')}</Button>
      <Button variant="primary" disabled={!canSave || saving || stale} onclick={submit}>{saveLabel}</Button>
    {/if}
  {/snippet}
</Dialog>

<style>
  .form {
    display: grid;
    gap: var(--s-5);
  }
  .extra {
    margin-inline-end: auto;
  }
  .ask {
    align-self: center;
    font-weight: 500;
  }
</style>
