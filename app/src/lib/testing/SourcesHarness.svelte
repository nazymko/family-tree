<script lang="ts">
  // Dev-only: ?view=sources | media | viewer | export | settings | citation, optional &theme=dark.
  import { onMount } from 'svelte';
  import { setApi } from '../api/context';
  import { PrivacyMockApi } from '../api/mock/mockPrivacy';
  import { MockPlatform } from '../api/mock/MockPlatform';
  import { fixtureTree } from '../api/mock/fixtures';
  import { settings } from '../state/settings.svelte';
  import Sources from '../views/Sources.svelte';
  import MediaView from '../views/MediaView.svelte';
  import ExportDialog from '../views/ExportDialog.svelte';
  import Settings from '../views/Settings.svelte';
  import MediaViewer from '../media/MediaViewer.svelte';
  import CitationEditor from '../editors/CitationEditor.svelte';
  import ToastSlot from '../ui/ToastSlot.svelte';
  import { notices } from '../ui/notices.svelte';
  import type { Id, MediaLinkDto, OwnerRef } from '../api/api';

  const params = new URLSearchParams(location.search);
  const view = params.get('view') ?? 'sources';
  if (params.get('theme')) document.documentElement.setAttribute('data-theme', params.get('theme')!);

  const api = new PrivacyMockApi(fixtureTree(300), { defaultVersion: '5.5.1' });
  const platform = new MockPlatform();
  setApi(api, platform);

  let ready = $state(false);
  let home = $state<Id>('');
  let items = $state<MediaLinkDto[]>([]);
  const target = $derived<OwnerRef>({ kind: 'person', id: home });

  onMount(async () => {
    notices.forward = false;
    await settings.load(api);
    home = settings.value.homePerson!;
    const t: OwnerRef = { kind: 'person', id: home };
    await api.attachFiles(t, [
      'C:/Photos/Wedding 1901.jpg',
      'C:/Photos/Family portrait.png',
      'C:/Docs/Birth certificate.pdf',
      'C:/Photos/Garden party.jpg',
      'C:/Photos/Harbour.jpg',
    ]);
    items = (await api.getPerson(home)).media;
    // a kept note that names a living (undated) person, for the export preview
    const dead = (await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'deceased', noSource: false, noBirthDate: false, privateOnly: false }, 0, 3)).items[1];
    await api.apply({ type: 'saveNote', target: { kind: 'person', id: dead.id }, note: { kind: 'general', text: 'Lived with the family until 1951, later looked after by Uma Undated in Hamburg.' } });
    const src = (await api.suggestSources('', 1))[0];
    await api.apply({ type: 'addCitation', target: t, citation: { source: src.id, page: 'p. 7' } });
    if (view === 'export') notices.show({ message: 'Exported 41 KB to Family tree.ged', actionLabel: 'Reveal' }, 0);
    ready = true;
  });
</script>

{#if ready}
  <main>
    {#if view === 'sources'}
      <Sources selectedId={null} />
    {:else if view === 'media'}
      <MediaView personId={home} />
    {:else if view === 'viewer'}
      <MediaView personId={home} />
      <MediaViewer {items} index={0} {target} onClose={() => {}} />
    {:else if view === 'export'}
      <Sources />
      <ExportDialog onClose={() => {}} />
    {:else if view === 'settings'}
      <Settings />
    {:else if view === 'citation'}
      <Sources />
      <CitationEditor {target} onDone={() => {}} onCancel={() => {}} />
    {/if}
  </main>
  <ToastSlot />
{/if}

<style>
  main {
    padding: var(--s-7);
  }
</style>
