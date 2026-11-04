<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import IconButton from '../ui/IconButton.svelte';
  import Kbd from '../ui/Kbd.svelte';
  import Button from '../ui/Button.svelte';
  import Ribbon from '../ui/Ribbon.svelte';
  import SearchPalette from '../ui/SearchPalette.svelte';
  import ToastHost from '../ui/ToastHost.svelte';
  import ShortcutSheet from './ShortcutSheet.svelte';
  import NewPersonDialog from './NewPersonDialog.svelte';
  import RoutePlaceholder from './RoutePlaceholder.svelte';
  import PersonEditor from './PersonEditor.svelte';
  import Sources from './Sources.svelte';
  import MediaView from './MediaView.svelte';
  import Settings from './Settings.svelte';
  import Pedigree from './Pedigree.svelte';
  import WholeTree from './WholeTree.svelte';
  import Descendants from './Descendants.svelte';
  import Relatives from './Relatives.svelte';
  import ExportDialog from './ExportDialog.svelte';
  import TraceView from '../trace/TraceView.svelte';
  import QuickAdd from '../editors/QuickAdd.svelte';
  import CitationList from '../editors/CitationList.svelte';
  import MediaGrid from '../media/MediaGrid.svelte';
  import People from './People.svelte';
  import FamilyView from './FamilyView.svelte';
  import type { IconName } from '../ui/icons';
  import { getApi } from '../api/context';
  import type { Id, PersonDetail, PersonRef, SearchHitDto } from '../api/types';
  import { nav, type Route } from '../state/nav.svelte';
  import { project } from '../state/project.svelte';
  import { settings } from '../state/settings.svelte';
  import { toasts } from '../state/toast.svelte';
  import { ui } from '../state/ui.svelte';
  import { undoState } from '../state/undo.svelte';
  import { closeTree } from '../state/openTree';
  import { currentPlatform, formatKeys, registerShortcuts, type Shortcut } from '../state/shortcuts';
  import { t, plural, type MessageKey } from '../i18n';
  import { editLabel } from '../i18n/backend';

  const api = getApi();

  type NavName = 'family' | 'people' | 'whole' | 'pedigree' | 'descendants' | 'relatives' | 'sources' | 'media' | 'settings';
  const ITEMS: { name: NavName; label: MessageKey; icon: IconName }[] = [
    { name: 'family', label: 'nav.family', icon: 'family' },
    { name: 'people', label: 'nav.people', icon: 'people' },
    { name: 'whole', label: 'nav.whole', icon: 'whole' },
    { name: 'pedigree', label: 'nav.pedigree', icon: 'pedigree' },
    { name: 'descendants', label: 'nav.descendants', icon: 'descendants' },
    { name: 'relatives', label: 'nav.relatives', icon: 'relatives' },
    { name: 'sources', label: 'nav.sources', icon: 'sources' },
    { name: 'media', label: 'nav.media', icon: 'media' },
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

  async function closeThisTree() {
    try {
      await closeTree(api);
    } catch (e) {
      toasts.push({ text: e instanceof Error ? e.message : String(e) });
    }
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
    { id: 'search', keys: 'Mod+K', get label() {
      return t('shortcuts.search');
    }, run: () => (searchOpen = !searchOpen), inInput: true },
    { id: 'filter', keys: '/', get label() {
      return t('shortcuts.filter');
    }, run: () => filterInput()?.focus(), when: () => !!filterInput() },
    { id: 'undo', keys: 'Mod+Z', get label() {
      return t('shortcuts.undo');
    }, run: () => void undo() },
    { id: 'redo', keys: 'Mod+Shift+Z', get label() {
      return t('shortcuts.redo');
    }, run: () => void redo() },
    { id: 'redo-y', keys: 'Mod+Y', get label() {
      return t('shortcuts.redo');
    }, run: () => void redo() },
    { id: 'new', keys: 'Mod+N', get label() {
      return t('shortcuts.newPerson');
    }, run: () => ui.requestAdd() },
    {
      id: 'edit',
      keys: 'Mod+E',
      get label() {
      return t('shortcuts.editFocus');
    },
      run: () => nav.focusPerson && nav.go({ name: 'person', id: nav.focusPerson }),
      when: () => !!nav.focusPerson,
    },
    {
      id: 'home',
      keys: 'Mod+Home',
      get label() {
      return t('shortcuts.home');
    },
      run: () => {
        if (homeId) nav.focus(homeId);
        nav.go({ name: 'family' });
      },
    },
    { id: 'back', keys: 'Alt+ArrowLeft', get label() {
      return t('shortcuts.back');
    }, run: () => nav.back() },
    { id: 'forward', keys: 'Alt+ArrowRight', get label() {
      return t('shortcuts.forward');
    }, run: () => nav.forward() },
    { id: 'n1', keys: 'Mod+1', get label() {
      return t('nav.family');
    }, run: () => go('family') },
    { id: 'n2', keys: 'Mod+2', get label() {
      return t('nav.people');
    }, run: () => go('people') },
    { id: 'n3', keys: 'Mod+3', get label() {
      return t('nav.pedigree');
    }, run: () => go('pedigree') },
    { id: 'n4', keys: 'Mod+4', get label() {
      return t('nav.descendants');
    }, run: () => go('descendants') },
    { id: 'n5', keys: 'Mod+5', get label() {
      return t('nav.sources');
    }, run: () => go('sources') },
    { id: 'n6', keys: 'Mod+6', get label() {
      return t('nav.media');
    }, run: () => go('media') },
    { id: 'n7', keys: 'Mod+7', get label() {
      return t('nav.relatives');
    }, run: () => go('relatives') },
    { id: 'n8', keys: 'Mod+8', get label() {
      return t('nav.whole');
    }, run: () => go('whole') },
    { id: 'export', keys: 'Mod+Shift+E', get label() {
      return t('shortcuts.exportGedcom');
    }, run: () => (exportOpen = true) },
    { id: 'settings', keys: 'Mod+,', get label() {
      return t('nav.settings');
    }, run: () => go('settings') },
    { id: 'save', keys: 'Mod+S', get label() {
      return t('shortcuts.save');
    }, run: () => {
        if (project.info && project.save === 'saved') toasts.push({ text: t('shell.allSaved') });
      }, },
    { id: 'help', keys: '?', get label() {
      return t('shortcuts.title');
    }, run: () => (sheetOpen = true) },
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

  // Quick add with an anchor person (the "+" menu on the family view) opens the editors lane's QuickAdd.
  let quickAnchor = $state<PersonRef | null>(null);
  const quickWanted = $derived(ui.add.open && ui.add.relation && ui.add.anchor ? ui.add : null);
  $effect(() => {
    const want = quickWanted;
    if (!want?.anchor) {
      quickAnchor = null;
      return;
    }
    let live = true;
    api
      .getPerson(want.anchor)
      .then((d) => {
        if (live) quickAnchor = d.ref;
      })
      .catch(() => {
        if (live) ui.closeAdd();
      });
    return () => {
      live = false;
    };
  });

  function openPerson(id: Id) {
    nav.focus(id);
    nav.go({ name: 'person', id });
  }

  function openMedia(id: Id) {
    nav.focus(id);
    nav.go({ name: 'media' });
  }

  function added(id: Id | null) {
    ui.closeAdd();
    if (id) {
      nav.focus(id);
      nav.go({ name: 'family' });
    }
  }

  const current = $derived(nav.route.name);
  const saveText = $derived(
    project.save === 'saving' ? t('shell.saving') : project.save === 'failed' ? t('shell.notSaved') : t('shell.saved'),
  );
</script>

{#snippet mediaSection(d: PersonDetail)}
  {#if d.media.length > 0}
    <MediaGrid items={d.media} target={{ kind: 'person', id: d.ref.id }} onOpen={() => openMedia(d.ref.id)} />
  {:else}
    <p class="sec-empty">{t('shell.nothingAttached')}</p>
  {/if}
  <p><Button onclick={() => openMedia(d.ref.id)}>{t('shell.openMedia')}</Button></p>
{/snippet}

{#snippet sourcesSection(d: PersonDetail)}
  <CitationList target={{ kind: 'person', id: d.ref.id }} citations={d.citations} onChanged={() => {}} />
{/snippet}

<div class="shell">
  <aside class="side">
    <div class="brand">
      <Ribbon title="Gene Tree" size="1.25rem" />
      <span class="brand-name">Gene Tree</span>
    </div>
    <nav aria-label={t('shell.main')}>
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
              <span class="lbl">{t(it.label)}</span>
            </a>
          </li>
        {/each}
        <li class="gap">
          <button type="button" class="item" onclick={() => (exportOpen = true)}>
            <Icon name="download" />
            <span class="lbl">{t('shell.export')}</span>
          </button>
        </li>
        <li>
          <a
            href="#/trace"
            class="item"
            aria-current={current === 'trace' ? 'page' : undefined}
            onclick={(e) => {
              e.preventDefault();
              nav.go({ name: 'trace' });
            }}
          >
            <Icon name="upload" />
            <span class="lbl">{t('shell.tracePicture')}</span>
          </a>
        </li>
        <li>
          <button type="button" class="item" onclick={() => void closeThisTree()}>
            <Icon name="x" />
            <span class="lbl">{t('shell.closeTree')}</span>
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
            <span class="lbl">{t('nav.settings')}</span>
          </a>
        </li>
      </ul>
    </nav>
    <footer class="foot">
      <div class="pname" title={project.info?.dir}>{project.info?.name}</div>
      <div class="pcount tnum">{plural('common.people', project.info?.personCount ?? 0)}</div>
      <div class="saved" role="status" aria-label={t('shell.saveStatus')} class:bad={project.save === 'failed'}>
        <Icon name="check" />
        <span class="lbl">{saveText}</span>
      </div>
    </footer>
  </aside>

  <div class="col">
    <header class="top">
      <div class="hist">
        <IconButton label={t('common.back')} icon="chevron-left" disabled={!nav.canBack} onclick={() => nav.back()} />
        <IconButton label={t('common.forward')} icon="chevron-right" disabled={!nav.canForward} onclick={() => nav.forward()} />
      </div>
      <button type="button" class="find" onclick={() => (searchOpen = true)}>
        <Icon name="search" />
        <span>{t('ui.searchTheTree')}</span>
        <Kbd>{modLabel}</Kbd>
      </button>
      <div class="hist">
        <IconButton
          label={undoState.undoLabel ? editLabel(undoState.undoLabel) : t('shell.undoLast')}
          icon="undo"
          disabled={!undoState.canUndo}
          onclick={() => void undo()}
        />
        <IconButton
          label={undoState.redoLabel ? editLabel(undoState.redoLabel) : t('shell.redoLast')}
          icon="redo"
          disabled={!undoState.canRedo}
          onclick={() => void redo()}
        />
      </div>
    </header>
    <main id="content" class="main">
      <div class="page" class:chart={current === 'pedigree' || current === 'descendants' || current === 'relatives' || current === 'whole' || current === 'trace'}>
        {#if nav.route.name === 'family'}
          <FamilyView />
        {:else if nav.route.name === 'people'}
          <People />
        {:else if nav.route.name === 'whole'}
          <WholeTree onOpenPerson={openPerson} />
        {:else if nav.route.name === 'pedigree'}
          <Pedigree onOpenPerson={openPerson} />
        {:else if nav.route.name === 'descendants'}
          <Descendants onOpenPerson={openPerson} />
        {:else if nav.route.name === 'relatives'}
          <Relatives onOpenPerson={openPerson} />
        {:else if nav.route.name === 'person'}
          {#key nav.route.id}
            <PersonEditor id={nav.route.id} {mediaSection} {sourcesSection} />
          {/key}
        {:else if nav.route.name === 'sources'}
          <Sources />
        {:else if nav.route.name === 'media'}
          {#if nav.focusPerson ?? homeId}
            <MediaView personId={(nav.focusPerson ?? homeId)!} />
          {:else}
            <RoutePlaceholder title={t('nav.media')} note={t('shell.chooseFirst')} />
          {/if}
        {:else if nav.route.name === 'trace'}
          <TraceView />
        {:else if nav.route.name === 'settings'}
          <Settings />
        {/if}
      </div>
    </main>
  </div>
</div>

<SearchPalette open={searchOpen} onclose={() => (searchOpen = false)} onpick={picked} />
<ShortcutSheet open={sheetOpen} onclose={() => (sheetOpen = false)} {shortcuts} />
{#if quickWanted && quickAnchor && quickWanted.relation}
  <QuickAdd
    anchor={quickAnchor}
    relation={quickWanted.relation}
    newParentSet={quickWanted.newParentSet}
    onDone={() => ui.closeAdd()}
    onCancel={() => ui.closeAdd()}
  />
{/if}
<NewPersonDialog
  open={ui.add.open && !quickWanted}
  relation={ui.add.relation}
  anchor={ui.add.anchor}
  newParentSet={ui.add.newParentSet}
  onclose={() => ui.closeAdd()}
  ondone={added}
/>
{#if exportOpen}
  <ExportDialog onClose={() => (exportOpen = false)} />
{/if}
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
  .sec-empty {
    color: var(--ink-1);
    margin: 0 0 var(--s-3);
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
  /* Charts fill the pane: a fixed-height flex parent. */
  .main:has(.page.chart) {
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .page.chart {
    flex: 1;
    min-height: 0;
    max-width: none;
    padding-bottom: var(--s-5);
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
