<script lang="ts">
  import { defaultApi, setApi } from './lib/api/context';
  import { settings } from './lib/state/settings.svelte';
  import { nav } from './lib/state/nav.svelte';
  import { project } from './lib/state/project.svelte';
  import { observeApi } from './lib/state/observe';
  import Shell from './lib/views/Shell.svelte';

  document.documentElement.lang = 'en';

  // Plain browser (npm run dev): MockApi over a seeded 500-person tree. Inside Tauri: the real adapter.
  const bundle = defaultApi();
  const api = observeApi(bundle.api);
  setApi(api, bundle.platform);
  void settings.load(api);

  // Development only: `?route=family|people|...` opens the mock tree straight onto a view (used for screenshots).
  if (import.meta.env.DEV && typeof location !== 'undefined') {
    const wanted = new URLSearchParams(location.search).get('route');
    if (wanted && wanted !== 'welcome') {
      void (async () => {
        const info = await api.openProject('C:/Genealogy/Schmidt family');
        project.open(info);
        nav.focus(info.homePerson);
        nav.go({ name: wanted } as never);
      })();
    }
  }
</script>

{#if nav.route.name === 'welcome' || !project.info}
  <main>
    <h1>Gene Tree</h1>
  </main>
{:else}
  <Shell />
{/if}
