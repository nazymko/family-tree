<script lang="ts">
  import { onMount } from 'svelte';
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import Ribbon from '../ui/Ribbon.svelte';
  import ImportWizard from './ImportWizard.svelte';
  import { getApi, getPlatform } from '../api/context';
  import type { RecentProject } from '../api/types';
  import { enterProject } from '../state/openTree';
  import { openReceivedCopy } from '../state/openReceived';
  import { t, currentLocale } from '../i18n';

  const api = getApi();
  const platform = getPlatform();

  let recents = $state<RecentProject[] | null>(null);
  let mode = $state<'home' | 'import' | 'create' | 'trace'>('home');
  let error = $state('');

  onMount(() => {
    void api
      .recentProjects()
      .then((r) => (recents = r))
      .catch(() => (recents = []));
  });

  async function open(dir: string) {
    error = '';
    try {
      await enterProject(api, await api.openProject(dir));
    } catch (e) {
      error = (e as Error).message;
    }
  }

  async function openFolder() {
    const dir = await platform.pickFolder(t('welcome.openTree'));
    if (dir) await open(dir);
  }

  async function openReceived() {
    error = (await openReceivedCopy(api, platform)) ?? '';
  }

  // The Api has no "forget this tree" call yet, so removal only lasts for this session.
  function remove(dir: string) {
    recents = (recents ?? []).filter((r) => r.dir !== dir);
  }

  // Hidden trees stay on this computer only (per-viewer preference), listed under a collapsed "Hidden trees".
  const HIDDEN_KEY = 'gt.hiddenRecents';
  function readHidden(): string[] {
    try {
      const v = JSON.parse(localStorage.getItem(HIDDEN_KEY) ?? '[]');
      return Array.isArray(v) ? v.filter((x) => typeof x === 'string') : [];
    } catch {
      return [];
    }
  }
  let hidden = $state<string[]>(readHidden());
  function setHidden(dir: string, hide: boolean) {
    hidden = hide ? [...new Set([...hidden, dir])] : hidden.filter((d) => d !== dir);
    try {
      localStorage.setItem(HIDDEN_KEY, JSON.stringify(hidden));
    } catch {
      /* storage unavailable: the choice lasts for this session */
    }
  }
  const shown = $derived((recents ?? []).filter((r) => !hidden.includes(r.dir)));
  const hiddenRecents = $derived((recents ?? []).filter((r) => hidden.includes(r.dir)));

  /** "Opened <date>" is shown only for a tree opened in the last week; an older date says nothing useful. */
  const WEEK_MS = 7 * 86_400_000;
  const recent = (ms: number) => Date.now() - ms <= WEEK_MS;

  function when(ms: number): string {
    return new Date(ms).toLocaleDateString(currentLocale(), { dateStyle: 'medium' });
  }
</script>

<main class="welcome">
  <div class="col">
    <header class="hero">
      <Ribbon title="Gene Tree" size="2rem" />
      <h1>Gene Tree</h1>
      <p class="lede">{t('welcome.lede')}</p>
    </header>

    {#if error}<Banner kind="error">{error}</Banner>{/if}

    {#if mode === 'home'}
      <div class="actions">
        <Button variant="primary" onclick={() => (mode = 'import')}>{t('welcome.importFile')}</Button>
        <Button onclick={() => (mode = 'create')}>{t('welcome.createTree')}</Button>
        <Button onclick={() => (mode = 'trace')}>{t('welcome.traceTree')}</Button>
        <Button onclick={openFolder}>{t('welcome.openTree')}</Button>
        <Button onclick={openReceived}>{t('merge.openReceived')}</Button>
      </div>

      <section aria-labelledby="recent-h">
        <div class="ledger">
          <h2 id="recent-h" class="rh">{t('welcome.recent')}</h2>
          {#if recents === null}
            <p class="note">&nbsp;</p>
          {:else if recents.length === 0}
            <p class="note">{t('welcome.noneYet')}</p>
          {:else}
            {#if shown.length === 0}
              <p class="note">{t('welcome.allHidden')}</p>
            {:else}
              <ul class="recent">
                {#each shown as r (r.dir)}
                  <li>
                    {#if r.missing}
                      <div class="gone">
                        <span class="rname">{r.name}</span>
                        <Banner kind="warning">{t('welcome.folderGone')}</Banner>
                        <Button variant="quiet" onclick={() => remove(r.dir)} aria-label={t('welcome.removeNamed', { name: r.name })}>
                          {t('common.remove')}
                        </Button>
                      </div>
                    {:else}
                      <div class="item">
                        <a
                          href="#/open"
                          class="row"
                          onclick={(e) => {
                            e.preventDefault();
                            void open(r.dir);
                          }}
                        >
                          <span class="rname">{r.name}</span>
                          <bdi dir="auto" class="rdir">{r.dir}</bdi>
                          <span class="rwhen tnum">{#if recent(r.openedAt)}{t('welcome.opened', { date: when(r.openedAt) })}{/if}</span>
                        </a>
                        <Button variant="quiet" onclick={() => setHidden(r.dir, true)} aria-label={t('welcome.hideNamed', { name: r.name })}>
                          {t('welcome.hide')}
                        </Button>
                      </div>
                    {/if}
                  </li>
                {/each}
              </ul>
            {/if}
            {#if hiddenRecents.length > 0}
              <details class="hidden-trees">
                <summary>{t('welcome.hiddenTrees', { n: hiddenRecents.length })}</summary>
                <ul class="recent">
                  {#each hiddenRecents as r (r.dir)}
                    <li>
                      <div class="item">
                        {#if r.missing}
                          <span class="row">
                            <span class="rname">{r.name}</span>
                            <span class="rdir">{t('welcome.folderGone')}</span>
                            <span></span>
                          </span>
                        {:else}
                          <a
                            href="#/open"
                            class="row"
                            onclick={(e) => {
                              e.preventDefault();
                              void open(r.dir);
                            }}
                          >
                            <span class="rname">{r.name}</span>
                            <bdi dir="auto" class="rdir">{r.dir}</bdi>
                            <span class="rwhen tnum">{#if recent(r.openedAt)}{t('welcome.opened', { date: when(r.openedAt) })}{/if}</span>
                          </a>
                        {/if}
                        <Button variant="quiet" onclick={() => setHidden(r.dir, false)} aria-label={t('welcome.showNamed', { name: r.name })}>
                          {t('welcome.show')}
                        </Button>
                      </div>
                    </li>
                  {/each}
                </ul>
              </details>
            {/if}
          {/if}
        </div>
      </section>
    {:else}
      <ImportWizard mode={mode === 'import' ? 'import' : 'create'} thenTrace={mode === 'trace'} oncancel={() => (mode = 'home')} />
    {/if}
  </div>
</main>

<style>
  .welcome {
    min-height: 100vh;
    padding: var(--s-9) var(--s-7);
    background: var(--paper-0);
  }
  .col {
    display: grid;
    gap: var(--s-7);
    max-width: 52rem;
    margin-inline: auto;
  }
  .hero {
    display: grid;
    gap: var(--s-3);
    justify-items: start;
    color: var(--ribbon);
  }
  h1 {
    font-size: var(--t-2xl);
    line-height: var(--t-2xl-lh);
  }
  .lede {
    font-family: var(--font-serif);
    font-size: var(--t-md);
    color: var(--ink-1);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-4);
  }
  .actions :global(.btn) {
    min-height: 2.75rem;
    padding-inline: var(--s-6);
  }
  .ledger {
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background: var(--paper-1);
    padding: 0 var(--s-6);
  }
  .rh {
    padding: var(--s-5) 0 var(--s-3);
    border-block-end: 1px solid var(--ink-2);
  }
  .note {
    color: var(--ink-1);
    padding: var(--s-5) 0;
  }
  .recent li + li {
    border-block-start: 1px solid var(--rule);
  }
  .recent {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .actions :global(.btn.quiet) {
    border-color: var(--ink-2);
  }
  .row {
    display: grid;
    grid-template-columns: minmax(10rem, 1fr) 2fr auto;
    align-items: center;
    gap: var(--s-5);
    min-height: 56px;
    color: inherit;
    text-decoration: none;
  }
  .row:hover .rname {
    text-decoration: underline;
    text-decoration-color: var(--ribbon);
  }
  .rname {
    font-family: var(--font-serif);
    font-weight: 600;
    font-size: var(--t-md);
  }
  .rdir,
  .rwhen {
    color: var(--ink-1);
    font-size: var(--t-sm);
    overflow-wrap: anywhere;
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--s-4);
  }
  .item .row {
    flex: 1;
    min-width: 0;
  }
  .hidden-trees {
    border-block-start: 1px solid var(--ink-2);
    padding-block: var(--s-3);
  }
  .hidden-trees summary {
    list-style: none;
    cursor: pointer;
    color: var(--ink-1);
    min-height: 44px;
    display: flex;
    align-items: center;
    gap: var(--s-2);
  }
  .hidden-trees summary::before {
    content: '▸';
    transition: transform 0.15s;
  }
  .hidden-trees[open] summary::before {
    transform: rotate(90deg);
  }
  .hidden-trees summary::-webkit-details-marker {
    display: none;
  }
  .hidden-trees summary:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .hidden-trees .rname {
    color: var(--ink-1);
  }
  @media (prefers-reduced-motion: reduce) {
    .hidden-trees summary::before {
      transition: none;
    }
  }
  .gone {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-4);
    min-height: 56px;
    padding: var(--s-3) 0;
  }
</style>
