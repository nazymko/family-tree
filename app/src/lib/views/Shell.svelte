<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import IconButton from '../ui/IconButton.svelte';
  import Kbd from '../ui/Kbd.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import Button from '../ui/Button.svelte';
  import Ribbon from '../ui/Ribbon.svelte';
  import SearchPalette from '../ui/SearchPalette.svelte';
  import ToastHost from '../ui/ToastHost.svelte';
  import ShortcutSheet from './ShortcutSheet.svelte';
  import NewPersonDialog from './NewPersonDialog.svelte';
  import RoutePlaceholder from './RoutePlaceholder.svelte';
  import People from './People.svelte';
  import FamilyView from './FamilyView.svelte';
  import type { IconName } from '../ui/icons';
  import { getApi } from '../api/context';
  import type { Id, SearchHitDto } from '../api/types';
  import { nav, type Route } from '../state/nav.svelte';
  import { project } from '../state/project.svelte';
  import { settings } from '../state/settings.svelte';
  import { toasts } from '../state/toast.svelte';
  import { ui } from '../state/ui.svelte';
  import { undoState } from '../state/undo.svelte';
  import { currentPlatform, formatKeys, registerShortcuts, type Shortcut } from '../state/shortcuts';

  const api = getApi();

  type NavName = 'family' | 'people' | 'pedigree' | 'descendants' | 'sources' | 'media' | 'settings';
  const ITEMS: { name: NavName; label: string; icon: IconName }[] = [
    { name: 'family', label: 'Family', icon: 'family' },
    { name: 'people', label: 'People', icon: 'people' },
    { name: 'pedigree', label: 'Pedigree', icon: 'pedigree' },
    { name: 'descendants', label: 'Descendants', icon: 'descendants' },
    { name: 'sources', label: 'Sources', icon: 'sources' },
    { name: 'media', label: 'Media', icon: 'media' },
  ];

  let searchOpen = $state(false);
  let sheetOpen = $state(false);
  let exportOpen = $state(false);

  const homeId = $derived(project.info?.homePerson ?? settings.value.homePerson ?? null);
  const modLabel = $derived(formatKeys('Mod+K', currentPlatform()));

  function go(name: NavName) {
    if (name === 'family' && !nav.focusPerson && homeId) nav.focus(homeId);
    nav.go({ name } as Route);
  }

  async function undo() {
    if (undoState.canUndo) await api.undo();
  }
  async function redo() {
    if (undoState.canRedo) await api.redo();
  }
  function filterInput(): HTMLElement | null {
    return document.querySelector<HTMLElement>('[data-filter-input]');
  }

  const shortcuts: Shortcut[] = [
    { id: 'search', keys: 'Mod+K', label: 'Search the tree', run: () => (searchOpen = !searchOpen), inInput: true },
    { id: 'filter', keys: '/', label: 'Focus the list filter', run: () => filterInput()?.focus(), when: () => !!filterInput() },
    { id: 'undo', keys: 'Mod+Z', label: 'Undo', run: () => void undo() },
    { id: 'redo', keys: 'Mod+Shift+Z', label: 'Redo', run: () => void redo() },
    { id: 'redo-y', keys: 'Mod+Y', label: 'Redo', run: () => void redo() },
    { id: 'new', keys: 'Mod+N', label: 'New person', run: () => ui.requestAdd() },
    {
      id: 'edit',
      keys: 'Mod+E',
      label: 'Edit the focus person',
      run: () => nav.focusPerson && nav.go({ name: 'person', id: nav.focusPerson }),
      when: () => !!nav.focusPerson,
    },
    {
      id: 'home',
      keys: 'Mod+Home',
      label: 'Go to the home person',
      run: () => {
        if (homeId) nav.focus(homeId);
        nav.go({ name: 'family' });
      },
    },
    { id: 'back', keys: 'Alt+ArrowLeft', label: 'Back', run: () => nav.back() },
    { id: 'forward', keys: 'Alt+ArrowRight', label: 'Forward', run: () => nav.forward() },
    { id: 'n1', keys: 'Mod+1', label: 'Family', run: () => go('family') },
    { id: 'n2', keys: 'Mod+2', label: 'People', run: () => go('people') },
    { id: 'n3', keys: 'Mod+3', label: 'Pedigree', run: () => go('pedigree') },
    { id: 'n4', keys: 'Mod+4', label: 'Descendants', run: () => go('descendants') },
    { id: 'n5', keys: 'Mod+5', label: 'Sources', run: () => go('sources') },
    { id: 'settings', keys: 'Mod+,', label: 'Settings', run: () => go('settings') },
    { id: 'save', keys: 'Mod+S', label: 'Save', run: () => toasts.push({ text: 'All changes are saved' }) },
    { id: 'help', keys: '?', label: 'Keyboard shortcuts', run: () => (sheetOpen = true) },
  ];

  onMount(() => {
    void undoState.refresh(api);
    if (!nav.focusPerson && homeId) nav.focus(homeId);
    return registerShortcuts(shortcuts);
  });

  function picked(hit: SearchHitDto) {
    searchOpen = false;
    if (hit.kind === 'source') nav.go({ name: 'sources' });
    else if (hit.person) {
      nav.focus(hit.person.id);
      nav.go({ name: 'family' });
    }
  }

  function added(id: Id | null) {
    ui.closeAdd();
    if (id) {
      nav.focus(id);
      nav.go({ name: 'family' });
    }
  }

  const current = $derived(nav.route.name);
  const saveText = $derived(project.save === 'saving' ? 'Saving…' : project.save === 'failed' ? 'Not saved' : 'Saved');
</script>

<div class="shell">
  <aside class="side">
    <div class="brand">
      <Ribbon title="Gene Tree" size="1.25rem" />
      <span class="brand-name">Gene Tree</span>
    </div>
    <nav aria-label="Main">
      <ul>
        {#each ITEMS as it (it.name)}
          <li>
            <a
              href="#/{it.name}"
              class="item"
              aria-current={current === it.name ? 'page' : undefined}
              onclick={(e) => {
                e.preventDefault();
                go(it.name);
              }}
            >
              <Icon name={it.icon} />
              <span class="lbl">{it.label}</span>
            </a>
          </li>
        {/each}
        <li class="gap">
          <button type="button" class="item" onclick={() => (exportOpen = true)}>
            <Icon name="download" />
            <span class="lbl">Export</span>
          </button>
        </li>
        <li>
          <a
            href="#/settings"
            class="item"
            aria-current={current === 'settings' ? 'page' : undefined}
            onclick={(e) => {
              e.preventDefault();
              go('settings');
            }}
          >
            <Icon name="settings" />
            <span class="lbl">Settings</span>
          </a>
        </li>
      </ul>
    </nav>
    <footer class="foot">
      <div class="pname" title={project.info?.dir}>{project.info?.name}</div>
      <div class="pcount tnum">{project.info?.personCount ?? 0} people</div>
      <div class="saved" role="status" aria-label="Save status" class:bad={project.save === 'failed'}>
        <Icon name="check" />
        <span class="lbl">{saveText}</span>
      </div>
    </footer>
  </aside>

  <div class="col">
    <header class="top">
      <div class="hist">
        <IconButton label="Back" icon="chevron-left" disabled={!nav.canBack} onclick={() => nav.back()} />
        <IconButton label="Forward" icon="chevron-right" disabled={!nav.canForward} onclick={() => nav.forward()} />
      </div>
      <button type="button" class="find" onclick={() => (searchOpen = true)}>
        <Icon name="search" />
        <span>Search the tree</span>
        <Kbd>{modLabel}</Kbd>
      </button>
      <div class="hist">
        <IconButton
          label={undoState.undoLabel ?? 'Undo last change'}
          icon="undo"
          disabled={!undoState.canUndo}
          onclick={() => void undo()}
        />
        <IconButton
          label={undoState.redoLabel ?? 'Redo last change'}
          icon="redo"
          disabled={!undoState.canRedo}
          onclick={() => void redo()}
        />
      </div>
    </header>
    <main id="content" class="main">
      <div class="page">
        {#if nav.route.name === 'family'}
          <FamilyView />
        {:else if nav.route.name === 'people'}
          <People />
        {:else if nav.route.name === 'pedigree'}
          <RoutePlaceholder title="Pedigree" note="The pedigree chart is wired in when its task merges." />
        {:else if nav.route.name === 'descendants'}
          <RoutePlaceholder title="Descendants" note="The descendant chart is wired in when its task merges." />
        {:else if nav.route.name === 'person'}
          <RoutePlaceholder title="Edit person" note="The person editor is wired in when its task merges." />
        {:else if nav.route.name === 'sources'}
          <RoutePlaceholder title="Sources" note="Sources are wired in when their task merges." />
        {:else if nav.route.name === 'media'}
          <RoutePlaceholder title="Media" note="The media library is wired in when its task merges." />
        {:else if nav.route.name === 'settings'}
          <RoutePlaceholder title="Settings" note="Settings are wired in when their task merges." />
        {/if}
      </div>
    </main>
  </div>
</div>

<SearchPalette open={searchOpen} onclose={() => (searchOpen = false)} onpick={picked} />
<ShortcutSheet open={sheetOpen} onclose={() => (sheetOpen = false)} {shortcuts} />
<NewPersonDialog
  open={ui.add.open}
  relation={ui.add.relation}
  anchor={ui.add.anchor}
  newParentSet={ui.add.newParentSet}
  onclose={() => ui.closeAdd()}
  ondone={added}
/>
<Dialog open={exportOpen} title="Export GEDCOM" onclose={() => (exportOpen = false)}>
  <p>The export dialog is wired in when its task merges.</p>
  {#snippet footer()}
    <Button onclick={() => (exportOpen = false)}>Close</Button>
  {/snippet}
</Dialog>
<ToastHost />

<style>
  .shell {
    display: grid;
    grid-template-columns: var(--sidebar-w) minmax(0, 1fr);
    height: 100vh;
    background: var(--paper-0);
  }
  .side {
    display: flex;
    flex-direction: column;
    background: var(--paper-2);
    border-inline-end: 1px solid var(--rule);
    min-height: 0;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    padding: var(--s-5) var(--s-5) var(--s-4);
    color: var(--ribbon);
  }
  .brand-name {
    font-family: var(--font-serif);
    font-weight: 600;
    font-size: var(--t-lg);
    color: var(--ink-0);
  }
  nav {
    flex: 1;
    padding: var(--s-3);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--s-1);
  }
  .gap {
    margin-top: var(--s-5);
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--s-4);
    width: 100%;
    min-height: 2.25rem;
    padding: 0 var(--s-4);
    border: 0;
    border-inline-start: 3px solid transparent;
    border-radius: var(--r-1);
    background: transparent;
    color: var(--ink-1);
    text-decoration: none;
    cursor: pointer;
    text-align: start;
    transition: background-color var(--dur-fast) var(--ease);
  }
  .item:hover {
    background: var(--rule);
    color: var(--ink-0);
  }
  .item[aria-current='page'] {
    background: var(--ribbon-wash);
    color: var(--ink-0);
    font-weight: 700;
    border-inline-start-color: var(--ribbon);
  }
  .foot {
    padding: var(--s-4) var(--s-5);
    border-block-start: 1px solid var(--rule);
    display: grid;
    gap: var(--s-1);
    font-size: var(--t-sm);
    line-height: var(--t-sm-lh);
  }
  .pname {
    font-family: var(--font-serif);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pcount {
    color: var(--ink-1);
  }
  .saved {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    color: var(--verdigris);
    font-weight: 500;
  }
  .saved.bad {
    color: var(--danger);
  }
  .col {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .top {
    display: flex;
    align-items: center;
    gap: var(--s-4);
    padding: var(--s-3) var(--s-6);
    border-block-end: 1px solid var(--rule);
    background: var(--paper-0);
  }
  .hist {
    display: flex;
    gap: var(--s-1);
  }
  .find {
    flex: 1;
    max-width: 30rem;
    display: flex;
    align-items: center;
    gap: var(--s-3);
    min-height: 2.25rem;
    padding: 0 var(--s-4);
    border: 1px solid var(--ink-2);
    border-radius: var(--r-1);
    background: var(--paper-1);
    color: var(--ink-1);
    cursor: pointer;
    text-align: start;
  }
  .find span {
    flex: 1;
  }
  .find:hover {
    border-color: var(--ink-0);
  }
  .main {
    flex: 1;
    overflow: auto;
    min-height: 0;
  }
  .page {
    max-width: var(--content-max);
    padding: var(--s-6) var(--s-7) var(--s-8);
  }
  @media (max-width: 1100px) {
    .shell {
      grid-template-columns: var(--sidebar-w-collapsed) minmax(0, 1fr);
    }
    .lbl,
    .brand-name,
    .pname,
    .pcount {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
    }
    .brand {
      justify-content: center;
      padding-inline: 0;
    }
    .item {
      justify-content: center;
      padding: 0;
    }
    .saved {
      justify-content: center;
    }
    .foot {
      padding-inline: var(--s-2);
    }
    .page {
      padding-inline: var(--s-5);
    }
  }
</style>
